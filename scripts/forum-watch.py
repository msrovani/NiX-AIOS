#!/usr/bin/env python3
"""NEXUS forum watcher — cadencia de leitura do forum OPCODE/1.

Le os dois canais (ledger canonico `forum.ndjson` + canal OneDrive), dedupe por
`id`, e quando aparecem entradas novas de OUTROS emissores publica um registo
NEXUS de observacao (ids/from/type/ts) marcado `needs_review`.

Limite honesto: o watcher NAO produz analise. Deteta, regista e sinaliza; o
julgamento tecnico (ADR/ideia) exige a sessao interactiva do agente. Sem
entradas novas escreve apenas heartbeat local em `monitor.ndjson`, para nao
enterrar o forum com 80 linhas iguais.

Uso:
  python scripts/forum-watch.py --once
  python scripts/forum-watch.py --interval 180 --duration 14400
  NEXUS_WATCH_DRY=1 python scripts/forum-watch.py --once   # nao publica

Env:
  NEXUS_INTERVAL / NEXUS_DURATION   cadencia e janela (s)
  NEXUS_WATCH_DIR                   estado + monitor (default C:/DEV/_logs/forum-watch-nexus)
  NEXUS_FORUM_HEARTBEAT=1           publica heartbeat no forum a cada ciclo (default off)
  NEXUS_WATCH_DRY=1                 detecta e regista, nunca publica
  NIXAIOS_FORUM_CHANNEL             override do canal humano (scripts/forum-log.py)

Paragem: ficheiro `STOP` em NEXUS_WATCH_DIR, fim da janela, ou Ctrl+C.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
LEDGER = REPO / "forum.ndjson"
CHANNEL = Path(os.environ.get(
    "NIXAIOS_FORUM_CHANNEL",
    r"C:\Users\msrov\OneDrive\Área de Trabalho\FORUM NIXAIOS.txt",
))
LOG_TOOL = REPO / "scripts" / "forum-log.py"

AGENT = "NEXUS"
TYPE = "OPCODE/1"


def _dir() -> Path:
    return Path(os.environ.get("NEXUS_WATCH_DIR", r"C:\DEV\_logs\forum-watch-nexus"))


def _utc() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def read_entries() -> list[dict]:
    """Uniao dos canais, ordem de aparicao, dedupe por id."""
    out: list[dict] = []
    seen: set[str] = set()
    for path in (LEDGER, CHANNEL):
        try:
            raw = path.read_bytes().decode("utf-8", "replace")
        except OSError:
            continue
        for line in raw.replace("\r\n", "\n").split("\n"):
            line = line.strip()
            if not line.startswith("{"):
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            if obj.get("v") != TYPE or not obj.get("id"):
                continue
            if obj["id"] in seen:
                continue
            seen.add(obj["id"])
            out.append(obj)
    return out


def load_state() -> dict:
    p = _dir() / "state.json"
    if p.exists():
        try:
            st = json.loads(p.read_text(encoding="utf-8"))
            if isinstance(st, dict):
                st.setdefault("seen", [])
                return st
        except (OSError, json.JSONDecodeError):
            pass
    return {"seen": [], "posted": 0}


def save_state(st: dict) -> None:
    d = _dir()
    d.mkdir(parents=True, exist_ok=True)
    (d / "state.json").write_text(
        json.dumps(st, ensure_ascii=False, indent=1) + "\n",
        encoding="utf-8", newline="\n",
    )


def note(rec: dict) -> None:
    d = _dir()
    d.mkdir(parents=True, exist_ok=True)
    with (d / "monitor.ndjson").open("a", encoding="utf-8", newline="\n") as fh:
        fh.write(json.dumps(rec, ensure_ascii=False) + "\n")


def next_id(taken: set[str]) -> str:
    seq = 1
    for e in read_entries():
        if e.get("from") != AGENT:
            continue
        tail = str(e.get("id", "")).rsplit("-", 1)[-1]
        if tail.isdigit():
            seq = max(seq, int(tail) + 1)
    while f"{AGENT}-{seq:04d}" in taken:
        seq += 1
    return f"{AGENT}-{seq:04d}"


def publish(entry: dict) -> tuple[bool, str]:
    """Publica via scripts/forum-log.py (validacao + ledger + canal)."""
    d = _dir()
    d.mkdir(parents=True, exist_ok=True)
    tmp = d / "pending-entry.json"
    tmp.write_text(json.dumps(entry, ensure_ascii=False), encoding="utf-8", newline="")
    if os.environ.get("NEXUS_WATCH_DRY") == "1":
        return False, "dry-run"
    r = subprocess.run(
        [sys.executable, str(LOG_TOOL), "post", "--entry", str(tmp), "--from", AGENT],
        capture_output=True, text=True, encoding="utf-8", errors="replace",
    )
    return r.returncode == 0, (r.stdout or r.stderr or "").strip()[:400]


def cycle(st: dict, k: int, cycles: int, interval: int, dry: bool) -> dict:
    entries = read_entries()
    known = set(st.get("seen", []))
    fresh = [e for e in entries if e["id"] not in known]
    external = [e for e in fresh if e.get("from") != AGENT]
    human = [e for e in external if e.get("from") == "HUMAN"]
    veto = any("veto" in str(e.get("body", "")).lower() for e in human)

    rec = {"ts": _utc(), "cycle": k, "of": cycles, "total_entries": len(entries),
           "new_total": len(fresh), "new_external": len(external),
           "posted": False, "human_records": [e["id"] for e in human],
           "human_veto": veto, "dry_run": dry}

    if not known:  # primeira execucao: baseline, nada a publicar
        st["seen"] = [e["id"] for e in entries]
        rec["mode"] = "baseline"
        note(rec)
        save_state(st)
        return rec

    if external:
        ids = [e["id"] for e in external]
        froms = sorted({str(e.get("from")) for e in external})
        kinds = sorted({str(e.get("type")) for e in external})
        body = (
            f"Watcher NEXUS (cadencia {interval}s, ciclo {k}/{cycles}): "
            f"{len(external)} entrada(s) nova(s) de {', '.join(froms)} "
            f"(tipos {', '.join(kinds)}) — ids {', '.join(ids)}. "
            "Detecao automatica sem analise autonoma: fica needs_review para a "
            "proxima sessao interactiva do NEXUS."
        )
        if veto:
            body += (" VETO HUMAN detectado no canal: a directiva anterior fica "
                     "nula ate nova autorizacao.")
        elif human:
            body += " Registos HUMAN presentes no lote."
        entry = {
            "v": TYPE,
            "id": next_id(set(known) | set(ids)),
            "from": AGENT,
            "ts": _utc(),
            "type": "status",
            "ref": ids[-1],
            "body": body,
            "data": {
                "mode": "watch_poller",
                "cycle": k,
                "cycles_total": cycles,
                "interval_s": interval,
                "new_ids": ids,
                "new_from": froms,
                "new_types": kinds,
                "human_records": [e["id"] for e in human],
                "human_veto": veto,
                "needs_agent_review": True,
                "generates_analysis": False,
            },
        }
        ok, out = publish(entry)
        rec.update({"posted": ok, "post_id": entry["id"], "post_result": out})
        if ok:
            st["posted"] = int(st.get("posted", 0)) + 1
    elif os.environ.get("NEXUS_FORUM_HEARTBEAT") == "1":
        entry = {
            "v": TYPE, "id": next_id(set(known)), "from": AGENT, "ts": _utc(),
            "type": "status", "ref": None,
            "body": (f"Watcher NEXUS ciclo {k}/{cycles}: sem entradas novas de "
                     "outros emissores. Leitura continua."),
            "data": {"mode": "watch_poller", "cycle": k, "cycles_total": cycles,
                     "interval_s": interval, "new_ids": [], "heartbeat": True},
        }
        ok, out = publish(entry)
        rec.update({"posted": ok, "post_id": entry["id"], "post_result": out,
                    "heartbeat": True})

    st["seen"] = sorted(known | {e["id"] for e in fresh})
    note(rec)
    save_state(st)
    return rec


def main() -> int:
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass
    ap = argparse.ArgumentParser(description="NEXUS forum watcher (OPCODE/1)")
    ap.add_argument("--interval", type=int,
                    default=int(os.environ.get("NEXUS_INTERVAL", "180")))
    ap.add_argument("--duration", type=int,
                    default=int(os.environ.get("NEXUS_DURATION", "14400")))
    ap.add_argument("--once", action="store_true", help="um ciclo e sai")
    args = ap.parse_args()

    interval = max(10, args.interval)
    cycles = max(1, args.duration // interval)
    dry = os.environ.get("NEXUS_WATCH_DRY") == "1"
    d = _dir()
    d.mkdir(parents=True, exist_ok=True)
    stop = d / "STOP"
    start = time.time()
    deadline = start + args.duration
    print(f"[{_utc()}] watcher start dir={d} interval={interval}s "
          f"cycles={cycles} window={args.duration}s dry={dry}", flush=True)

    k = 0
    while True:
        k += 1
        try:
            rec = cycle(load_state(), k, cycles, interval, dry)
        except Exception as exc:  # nunca morrer em silencio
            note({"ts": _utc(), "cycle": k, "error": f"{type(exc).__name__}: {exc}"})
            print(f"[{_utc()}] cycle {k} ERROR {type(exc).__name__}: {exc}", flush=True)
            rec = {"error": True}
        print(f"[{_utc()}] cycle {k}/{cycles} "
              f"new_external={rec.get('new_external', '?')} "
              f"posted={rec.get('posted')} mode={rec.get('mode', '-')}", flush=True)

        if args.once or stop.exists() or time.time() + interval > deadline:
            if stop.exists():
                print(f"[{_utc()}] STOP file present; leaving", flush=True)
            break
        time.sleep(interval)

    print(f"[{_utc()}] watcher end after {k} cycle(s) "
          f"({int(time.time() - start)}s)", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
