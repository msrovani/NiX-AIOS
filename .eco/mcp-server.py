#!/usr/bin/env python3
"""
Eco-Tokens v3 — Context Engineering & Token Economy.
MCP stdio zero-dependency (stdlib only).
Tools: get_current_task | update_task_status | run_verification | search_ast_symbol
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

VALID_STATUS = {"TODO", "WORKING", "BLOCKED", "READY_FOR_REVIEW"}
# CSI / OSC-style ANSI (ECMA-48)
ANSI_RE = re.compile(r"\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])")
MAX_SYMBOL_LINES = 25
MAX_ERR_CHARS = 1000  # ~250 tokens

_HERE = Path(__file__).resolve().parent
if _HERE.name == ".eco":
    ROOT = _HERE.parent
    ECO_DIR = _HERE
elif _HERE.name == "templates":
    ROOT = _HERE.parent
    ECO_DIR = ROOT / ".eco"
else:
    ROOT = _HERE.parent
    ECO_DIR = ROOT / ".eco"

STATE_PATH = ECO_DIR / "state.json"
STATE_TMP = ECO_DIR / "state.json.tmp"
VERIFY_SH = ROOT / "scripts" / "verify.sh"


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def send(msg: Dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(msg, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def ok(req_id: Any, result: Any) -> None:
    send({"jsonrpc": "2.0", "id": req_id, "result": result})


def fail(req_id: Any, code: int, message: str, data: Any = None) -> None:
    err: Dict[str, Any] = {"code": code, "message": message}
    if data is not None:
        err["data"] = data
    send({"jsonrpc": "2.0", "id": req_id, "error": err})


def default_state() -> Dict[str, Any]:
    return {
        "schema": 3,
        "active_task": {
            "id": 1,
            "domain": "core",
            "status": "TODO",
            "objective": "Setup inicial",
            "attempts": 0,
            "reason": "",
        },
        "last_hash": None,
        "last_failed_hash": None,
        "last_diff_hash": None,
        "last_ok_hash": None,
        "last_verify_failed": False,
        "verify_failures": 0,
        "updated_at": _now(),
    }


def ensure_eco() -> None:
    ECO_DIR.mkdir(parents=True, exist_ok=True)
    if not STATE_PATH.exists():
        atomic_write_json(default_state())


def atomic_write_json(doc: Dict[str, Any]) -> None:
    """Grava via .eco/state.json.tmp → os.replace (atômico)."""
    ECO_DIR.mkdir(parents=True, exist_ok=True)
    backup = STATE_PATH.with_suffix(".json.bak")
    if STATE_PATH.exists():
        try:
            shutil.copy2(STATE_PATH, backup)
        except OSError:
            pass
    payload = json.dumps(doc, ensure_ascii=False, indent=2) + "\n"
    with STATE_TMP.open("w", encoding="utf-8") as fh:
        fh.write(payload)
        fh.flush()
        os.fsync(fh.fileno())
    os.replace(STATE_TMP, STATE_PATH)


def read_state() -> Dict[str, Any]:
    ensure_eco()
    try:
        with STATE_PATH.open("r", encoding="utf-8") as fh:
            doc = json.load(fh)
        if not isinstance(doc, dict) or "active_task" not in doc:
            raise ValueError("invalid state")
        # migrate v2 keys
        if "last_hash" not in doc:
            doc["last_hash"] = doc.get("last_diff_hash")
        if "last_verify_failed" not in doc:
            doc["last_verify_failed"] = bool(doc.get("last_failed_hash"))
        return doc
    except (OSError, ValueError, json.JSONDecodeError):
        bak = STATE_PATH.with_suffix(".json.bak")
        if bak.exists():
            try:
                with bak.open("r", encoding="utf-8") as fh:
                    return json.load(fh)
            except (OSError, ValueError, json.JSONDecodeError):
                pass
        doc = default_state()
        atomic_write_json(doc)
        return doc


def write_state(doc: Dict[str, Any]) -> None:
    doc["schema"] = 3
    doc["updated_at"] = _now()
    atomic_write_json(doc)


def strip_ansi(text: str) -> str:
    return ANSI_RE.sub("", text)


def smart_trim(text: str, failed: bool) -> str:
    """Head/Tail: se >20 linhas e falha → 5 primeiras + 15 últimas; strip ANSI."""
    clean = strip_ansi(text).replace("\r\n", "\n").replace("\r", "\n")
    lines = [ln for ln in clean.split("\n") if ln.strip()]
    lines = [
        ln
        for ln in lines
        if not re.fullmatch(r"[-=_*~.]{4,}", ln.strip())
    ]
    if not lines:
        return "(empty)"
    if not failed or len(lines) <= 20:
        body = lines[-20:] if not failed else lines
    else:
        body = lines[:5] + ["…"] + lines[-15:]
    out = "\n".join(body)
    if len(out) > MAX_ERR_CHARS:
        out = out[:500] + "\n…\n" + out[-480:]
    return out


def git_diff_hash() -> str:
    try:
        r = subprocess.run(
            ["git", "diff", "HEAD"],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
        payload = r.stdout or ""
        if r.returncode != 0 and not payload.strip():
            r2 = subprocess.run(
                ["git", "diff"],
                cwd=str(ROOT),
                capture_output=True,
                text=True,
                timeout=30,
                check=False,
            )
            payload = r2.stdout or ""
    except (OSError, subprocess.TimeoutExpired):
        payload = ""
    return hashlib.sha256(payload.encode("utf-8", errors="replace")).hexdigest()


# NOTA NixAIOS (AGENTS.md, lição 34): o kit upstream faz `git checkout .` ao
# trip do circuit breaker. Neste repositório isso apaga trabalho de outra sessão
# por causa de um teste flaky. O breaker RECUSA (BLOQUEIA) e nunca reverte.
# Não reintroduzir `git checkout`/`git restore`/`git stash` aqui.
REVERT_POLICY = "refuse-no-revert"


def find_bash() -> Optional[str]:
    # Ordem importa no Windows: `bash` no PATH é o C:\windows\system32\bash.exe
    # (WSL) e responde 127 a um caminho Windows. Git Bash vem primeiro.
    if os.name == "nt":
        order = (
            os.environ.get("ECO_BASH"),
            r"C:\Program Files\Git\bin\bash.exe",
            r"C:\Program Files\Git\usr\bin\bash.exe",
            r"C:\Program Files (x86)\Git\bin\bash.exe",
            "bash",
        )
    else:
        order = (
            os.environ.get("ECO_BASH"),
            "bash",
            "/bin/bash",
            "/usr/bin/bash",
        )
    for c in order:
        if not c:
            continue
        if c.lower().endswith(r"system32\bash.exe"):
            continue
        try:
            r = subprocess.run(
                [c, "-lc", "echo ok"],
                capture_output=True,
                text=True,
                timeout=5,
                check=False,
            )
            if r.returncode == 0:
                return c
        except (OSError, subprocess.TimeoutExpired):
            continue
    return None


def update_task_status(status: str, reason: str = "") -> Dict[str, Any]:
    if status not in VALID_STATUS:
        return {
            "ok": False,
            "error": f"invalid status; use {'|'.join(sorted(VALID_STATUS))}",
        }
    doc = read_state()
    task = doc.setdefault("active_task", {})
    prev = task.get("status", "TODO")
    task["status"] = status
    task["reason"] = reason or ""
    task["updated_at"] = _now()
    if status == "WORKING":
        task["attempts"] = int(task.get("attempts") or 0) + 1
    elif status == "TODO":
        task["attempts"] = 0
        doc["verify_failures"] = 0
        doc["last_failed_hash"] = None
        doc["last_hash"] = None
        doc["last_verify_failed"] = False
    elif status == "READY_FOR_REVIEW":
        doc["verify_failures"] = 0
        doc["last_verify_failed"] = False
    doc["active_task"] = task
    doc["last_transition"] = {"from": prev, "to": status, "reason": reason or ""}
    write_state(doc)
    return {"ok": True, "active_task": task}


def _trip_circuit_breaker(
    doc: Dict[str, Any], current_hash: str, reason: str
) -> Dict[str, Any]:
    task = doc.get("active_task") or {}
    reverted = False
    task["status"] = "BLOCKED"
    task["reason"] = reason
    task["updated_at"] = _now()
    doc["active_task"] = task
    doc["last_hash"] = current_hash
    doc["last_diff_hash"] = current_hash
    doc["last_failed_hash"] = current_hash
    doc["last_verify_failed"] = True
    doc["verify_failures"] = int(doc.get("verify_failures") or 0) + 1
    write_state(doc)
    return {
        "ok": False,
        "circuit_breaker": True,
        "exit_code": 2,
        "diff_hash": current_hash,
        "reverted": reverted,
        "revert_policy": REVERT_POLICY,
        "output": (
            "LOOP DETECTADO: Nenhuma alteração real foi produzida no código "
            "desde a última falha.\n"
            "Status=BLOCKED. Sem revert automático (política NixAIOS: recusa, "
            "nunca `git checkout .`). O humano decide o reset.\n"
            f"hash={current_hash[:16]}…"
        ),
    }


def run_verification(dry_run: bool = False) -> Dict[str, Any]:
    doc = read_state()
    current_hash = git_diff_hash()
    last_hash = doc.get("last_hash") or doc.get("last_diff_hash")
    prev_failed = bool(
        doc.get("last_verify_failed") or doc.get("last_failed_hash")
    )

    # Anti-loop: mesmo fingerprint + falha anterior → BLOCKED SEM build
    if (
        prev_failed
        and last_hash
        and current_hash == last_hash
        and not dry_run
    ):
        return _trip_circuit_breaker(
            doc,
            current_hash,
            "LOOP DETECTADO: fingerprint SHA256(git diff) idêntico após falha",
        )

    doc["last_hash"] = current_hash
    doc["last_diff_hash"] = current_hash

    if dry_run:
        write_state(doc)
        last_ok = doc.get("last_ok_hash")
        unchanged = last_ok is not None and current_hash == last_ok
        return {
            "ok": True,
            "dry_run": True,
            "diff_hash": current_hash,
            "unchanged_since_last_ok": unchanged,
            "last_verify_failed": prev_failed,
            "output": (
                f"DRY_RUN — sem build. unchanged_since_last_ok={unchanged} "
                f"prev_failed={prev_failed}"
            ),
        }

    if not VERIFY_SH.exists():
        write_state(doc)
        return {
            "ok": False,
            "exit_code": 1,
            "diff_hash": current_hash,
            "output": "VERIFY_FAIL reason=scripts/verify.sh missing",
        }

    bash = find_bash()
    if not bash:
        write_state(doc)
        return {
            "ok": False,
            "exit_code": 1,
            "diff_hash": current_hash,
            "output": "VERIFY_FAIL reason=bash_not_found",
        }

    try:
        r = subprocess.run(
            [bash, str(VERIFY_SH)],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
            timeout=int(os.environ.get("ECO_VERIFY_TIMEOUT", "600")),
            check=False,
        )
    except subprocess.TimeoutExpired:
        doc["last_verify_failed"] = True
        doc["last_failed_hash"] = current_hash
        write_state(doc)
        return {
            "ok": False,
            "exit_code": 124,
            "diff_hash": current_hash,
            "output": "VERIFY_FAIL reason=timeout",
        }

    combined = (r.stdout or "") + (r.stderr or "")
    failed = r.returncode != 0
    slim = smart_trim(combined, failed=failed)
    exit_code = 1 if r.returncode is None else r.returncode

    if failed:
        doc["last_verify_failed"] = True
        doc["last_failed_hash"] = current_hash
        doc["last_hash"] = current_hash
        doc["verify_failures"] = int(doc.get("verify_failures") or 0) + 1
        write_state(doc)
        return {
            "ok": False,
            "exit_code": exit_code,
            "diff_hash": current_hash,
            "output": slim,
        }

    doc["last_verify_failed"] = False
    doc["last_failed_hash"] = None
    doc["last_ok_hash"] = current_hash
    doc["last_hash"] = current_hash
    doc["verify_failures"] = 0
    write_state(doc)
    return {
        "ok": True,
        "exit_code": 0,
        "diff_hash": current_hash,
        "output": slim if slim != "(empty)" else "VERIFY_OK",
    }


def _python_scan(symbol: str) -> str:
    """Fallback stdlib para máquinas sem rg/grep/ast-grep no PATH (Windows lab)."""
    skip_dirs = {
        ".git",
        "target",
        "node_modules",
        "__pycache__",
        ".direnv",
        ".eco",
        "result",
    }
    hits: List[str] = []
    for dirpath, dirnames, filenames in os.walk(str(ROOT)):
        dirnames[:] = sorted(
            d
            for d in dirnames
            if d not in skip_dirs and not d.startswith("result-")
        )
        for name in sorted(filenames):
            path = Path(dirpath) / name
            try:
                if path.stat().st_size > 2_000_000:
                    continue
                with path.open("r", encoding="utf-8", errors="ignore") as fh:
                    for lineno, line in enumerate(fh, 1):
                        if symbol in line:
                            rel = path.relative_to(ROOT).as_posix()
                            hits.append(f"{rel}:{lineno}:{line.rstrip()[:300]}")
                            if len(hits) >= MAX_SYMBOL_LINES:
                                return "\n".join(hits)
            except OSError:
                continue
    return "\n".join(hits)


def search_ast_symbol(symbol: str) -> Dict[str, Any]:
    if not symbol or not isinstance(symbol, str):
        return {"ok": False, "error": "symbol required"}

    def run(cmd: List[str]) -> Tuple[int, str]:
        try:
            r = subprocess.run(
                cmd,
                cwd=str(ROOT),
                capture_output=True,
                text=True,
                timeout=60,
                check=False,
            )
            return r.returncode, strip_ansi((r.stdout or "") + (r.stderr or ""))
        except (OSError, subprocess.TimeoutExpired) as exc:
            return 1, str(exc)

    tool = "none"
    text = ""
    if shutil.which("ast-grep") or shutil.which("sg"):
        bin_name = "ast-grep" if shutil.which("ast-grep") else "sg"
        _, text = run([bin_name, "run", "--pattern", symbol, "-A", "2", "-B", "1"])
        tool = "ast-grep"
    if not text.strip():
        rg = os.environ.get("ECO_RG", "rg")
        if shutil.which(rg):
            _, text = run(
                [
                    rg,
                    "-n",
                    "--no-heading",
                    "--color",
                    "never",
                    "-m",
                    str(MAX_SYMBOL_LINES),
                    "-g",
                    "!target/**",
                    "-g",
                    "!**/node_modules/**",
                    "-g",
                    "!.git/**",
                    "-g",
                    "!.eco/**",
                    symbol,
                    ".",
                ]
            )
            tool = "ripgrep"
    if not text.strip():
        grep = shutil.which("grep")
        if grep:
            _, text = run(
                [
                    grep,
                    "-RIn",
                    "--exclude-dir=.git",
                    "--exclude-dir=target",
                    "--exclude-dir=node_modules",
                    "--exclude-dir=.eco",
                    "-m",
                    str(MAX_SYMBOL_LINES),
                    symbol,
                    ".",
                ]
            )
            tool = "grep"

    if not text.strip():
        text = _python_scan(symbol)
        tool = "python"

    lines = [ln for ln in text.splitlines() if ln.strip()][:MAX_SYMBOL_LINES]
    snippet = "\n".join(lines) if lines else "(no matches)"
    return {
        "ok": bool(lines),
        "tool": tool,
        "symbol": symbol,
        "snippet": snippet,
        "lines": len(lines),
    }


TOOLS = [
    {
        "name": "get_current_task",
        "description": "Retorna apenas a tarefa ativa (.eco/state.json).",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": False,
        },
    },
    {
        "name": "update_task_status",
        "description": "Atualiza status atomicamente (TODO|WORKING|BLOCKED|READY_FOR_REVIEW).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["TODO", "WORKING", "BLOCKED", "READY_FOR_REVIEW"],
                },
                "reason": {"type": "string"},
            },
            "required": ["status"],
            "additionalProperties": False,
        },
    },
    {
        "name": "run_verification",
        "description": (
            "SHA256(git diff) anti-loop; smart_trim; dry_run=true sem build. "
            "Loop idêntico pós-falha → BLOCKED sem build."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {"dry_run": {"type": "boolean"}},
            "additionalProperties": False,
        },
    },
    {
        "name": "search_ast_symbol",
        "description": "Busca cirúrgica ast-grep → rg → grep → python stdlib (máx. 25 linhas).",
        "inputSchema": {
            "type": "object",
            "properties": {"symbol": {"type": "string"}},
            "required": ["symbol"],
            "additionalProperties": False,
        },
    },
]


def call_tool(name: str, args: Dict[str, Any]) -> Dict[str, Any]:
    if name == "get_current_task":
        doc = read_state()
        return {
            "content": [
                {
                    "type": "text",
                    "text": json.dumps(
                        doc.get("active_task"), ensure_ascii=False, indent=2
                    ),
                }
            ]
        }
    if name == "update_task_status":
        result = update_task_status(
            args.get("status", ""), args.get("reason", "") or ""
        )
        return {
            "content": [
                {
                    "type": "text",
                    "text": json.dumps(result, ensure_ascii=False, indent=2),
                }
            ]
        }
    if name == "run_verification":
        result = run_verification(dry_run=bool(args.get("dry_run", False)))
        return {
            "content": [
                {
                    "type": "text",
                    "text": json.dumps(result, ensure_ascii=False, indent=2),
                }
            ]
        }
    if name == "search_ast_symbol":
        result = search_ast_symbol(args.get("symbol", ""))
        return {
            "content": [
                {
                    "type": "text",
                    "text": json.dumps(result, ensure_ascii=False, indent=2),
                }
            ]
        }
    raise ValueError(f"Unknown tool: {name}")


def handle(msg: Dict[str, Any]) -> None:
    req_id = msg.get("id")
    method = msg.get("method")
    params = msg.get("params") or {}

    if method == "initialize":
        ok(
            req_id,
            {
                "protocolVersion": params.get("protocolVersion") or "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "eco-tokens", "version": "0.3.0"},
            },
        )
        return
    if method in ("notifications/initialized", "initialized"):
        return
    if method == "tools/list":
        ok(req_id, {"tools": TOOLS})
        return
    if method == "tools/call":
        name = params.get("name")
        args = params.get("arguments") or {}
        try:
            ok(req_id, call_tool(name, args))
        except Exception as exc:  # noqa: BLE001
            fail(req_id, -32000, str(exc), {"tool": name})
        return
    if method == "ping":
        ok(req_id, {})
        return
    if req_id is not None:
        fail(req_id, -32601, f"Method not found: {method}")


def main() -> None:
    # Windows: stdout/stdin herdados podem ser cp1252 e rebentar em "→"/"…"
    # (UnicodeEncodeError a meio do tools/list). Forçar UTF-8 nos dois.
    for stream in (sys.stdin, sys.stdout):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass
    ensure_eco()
    for line in sys.stdin:
        trimmed = line.strip()
        if not trimmed:
            continue
        try:
            handle(json.loads(trimmed))
        except json.JSONDecodeError as exc:
            send(
                {
                    "jsonrpc": "2.0",
                    "id": None,
                    "error": {
                        "code": -32700,
                        "message": "Parse error",
                        "data": str(exc),
                    },
                }
            )


if __name__ == "__main__":
    main()
