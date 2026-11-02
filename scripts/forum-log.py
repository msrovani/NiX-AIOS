#!/usr/bin/env python3
"""Forum NixAIOS — log NDJSON (OPCODE/1).

Uso:
  python scripts/forum-log.py post --entry entry.json [--from FORGE]
  python scripts/forum-log.py tail [N]
  python scripts/forum-log.py validate

O ledger canonico e versionado: forum.ndjson (raiz do repo).
O canal humano (OneDrive) recebe as mesmas linhas, uma por linha.
Tudo em UTF-8 sem BOM, LF; uma entrada = uma linha JSON valida.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
LEDGER = REPO / "forum.ndjson"
# Canal humano (OneDrive) é opcional e específico da máquina: em Linux/CI não
# existe, e um Path com "C:\..." em POSIX é um nome relativo — não criar lixo.
_DEFAULT_CHANNEL = (
    r"C:\Users\msrov\OneDrive\Área de Trabalho\FORUM NIXAIOS.txt"
    if os.name == "nt"
    else ""
)
_env_channel = os.environ.get("NIXAIOS_FORUM_CHANNEL")
CHANNEL = Path(_env_channel or _DEFAULT_CHANNEL) if (_env_channel or _DEFAULT_CHANNEL) else None

REQUIRED = ("v", "id", "from", "ts", "type", "body", "data")
TYPES = {"idea", "adr", "status", "claim", "execution"}


def _lines(path: Path) -> list[str]:
    if not path.exists():
        return []
    raw = path.read_bytes().decode("utf-8", errors="strict")
    return [ln for ln in raw.replace("\r\n", "\n").split("\n") if ln.strip()]


def _entries(path: Path) -> list[dict]:
    out = []
    for i, ln in enumerate(_lines(path), 1):
        if not ln.startswith("{"):
            continue  # cabecalho humano no canal
        try:
            out.append(json.loads(ln))
        except json.JSONDecodeError as exc:
            raise SystemExit(f"{path}:{i}: JSON invalido: {exc}")
    return out


def _append(path: Path, line: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    prefix = b""
    if path.exists():
        data = path.read_bytes()
        if data and not data.endswith(b"\n"):
            prefix = b"\n"
    with path.open("ab") as fh:
        fh.write(prefix + line.encode("utf-8") + b"\n")


def cmd_post(args: argparse.Namespace) -> int:
    entry = json.loads(Path(args.entry).read_text(encoding="utf-8")) if args.entry else {}
    now = datetime.now(timezone.utc).replace(microsecond=0)
    entry.setdefault("v", "OPCODE/1")
    entry.setdefault("ts", now.strftime("%Y-%m-%dT%H:%M:%SZ"))
    entry.setdefault("from", args.sender)
    entry.setdefault("ref", None)
    missing = [k for k in REQUIRED if k not in entry]
    if missing:
        raise SystemExit(f"faltam campos: {', '.join(missing)}")
    if entry["type"] not in TYPES:
        raise SystemExit(f"type invalido: {entry['type']} (use {sorted(TYPES)})")
    if not str(entry["body"]).strip():
        raise SystemExit("body vazio")
    ids = {e.get("id") for e in _entries(LEDGER)}
    if entry["id"] in ids:
        raise SystemExit(f"id duplicado: {entry['id']}")
    if not args.allow_duplicate:
        body = " ".join(str(entry["body"]).split()).lower()
        for prev in _entries(LEDGER):
            if (
                prev.get("from") == entry["from"]
                and prev.get("type") == entry["type"]
                and " ".join(str(prev.get("body", "")).split()).lower() == body
            ):
                raise SystemExit(
                    f"conteudo duplicado de {prev.get('id')} (use --allow-duplicate)"
                )
    line = json.dumps(entry, ensure_ascii=False, separators=(",", ":"))
    json.loads(line)  # guard
    _append(LEDGER, line)
    if not args.no_channel and CHANNEL is not None:
        _append(CHANNEL, line)
    print(line)
    return 0


def cmd_tail(args: argparse.Namespace) -> int:
    for e in _entries(LEDGER)[-args.count:]:
        print(f"[{e.get('ts')}] {e.get('from')}/{e.get('type')} {e.get('id')}: {e.get('body')}")
    return 0


def cmd_validate(_: argparse.Namespace) -> int:
    entries = _entries(LEDGER)
    ids = [e.get("id") for e in entries]
    dup = {i for i in ids if ids.count(i) > 1}
    bad = [e.get("id") for e in entries if any(k not in e for k in REQUIRED)]
    seen: dict[tuple, str] = {}
    cdup = []
    for e in entries:
        key = (
            e.get("from"),
            e.get("type"),
            " ".join(str(e.get("body", "")).split()).lower(),
        )
        if key in seen:
            cdup.append([seen[key], e.get("id")])
        else:
            seen[key] = str(e.get("id"))
    ok = not dup and not bad
    print(json.dumps({
        "ok": ok, "entries": len(entries), "duplicates": sorted(dup),
        "malformed": bad, "content_duplicates": cdup, "ledger": str(LEDGER),
    }, ensure_ascii=False))
    return 0 if ok else 1


def main() -> int:
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("post")
    p.add_argument("--entry", help="ficheiro JSON (sem v/ts/from opcionais)")
    p.add_argument("--from", dest="sender", default="FORGE")
    p.add_argument("--no-channel", action="store_true")
    p.add_argument("--allow-duplicate", action="store_true")
    p.set_defaults(func=cmd_post)
    t = sub.add_parser("tail")
    t.add_argument("count", nargs="?", type=int, default=10)
    t.set_defaults(func=cmd_tail)
    v = sub.add_parser("validate")
    v.set_defaults(func=cmd_validate)
    args = ap.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
