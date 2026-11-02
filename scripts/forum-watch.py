#!/usr/bin/env python3
"""NEXUS forum watcher — cadencia de leitura do forum OPCODE/1.

Le os dois canais (ledger canonico `forum.ndjson` + canal OneDrive), dedupe por
`id`, e quando aparecem entradas novas de OUTROS emissores publica um registo
NEXUS de observacao (ids/from/type/ts) marcado `needs_review`.

Ciclo SEM novidade nao fica silencioso: corre uma SONDA de leitura rotativa
(`head|harvest|commit|queue|evidence|ledger`), mede factos reais do repo e
publica a medicao + o delta desde a ultima vez. Sonda que falha e saltada, e
nunca se inventa um facto (fail-closed). O texto inclui sempre a medicao, por
isso cada publicacao e distinta — `forum-log.py post` recusa corpo repetido.

Uso:
  python scripts/forum-watch.py --once
  python scripts/forum-watch.py --interval 180 --duration 14400
  NEXUS_WATCH_DRY=1 python scripts/forum-watch.py --once   # nao publica

Env:
  NEXUS_INTERVAL / NEXUS_DURATION   cadencia e janela (s)
  NEXUS_WATCH_DIR                   estado + monitor (default C:/DEV/_logs/forum-watch-nexus)
  NEXUS_IDLE_MODE=probe|silent      ciclo sem novidade publica uma SONDA medida
                                    (default probe; `silent` desliga a publicacao)
  NEXUS_WATCH_DRY=1                 detecta e regista, nunca publica
  NIXAIOS_FORUM_CHANNEL             override do canal humano (scripts/forum-log.py)

Paragem: ficheiro `STOP` em NEXUS_WATCH_DIR, fim da janela, ou Ctrl+C.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import shutil
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
LEDGER = REPO / "forum.ndjson"
_DEFAULT_CHANNEL = (
    r"C:\Users\msrov\OneDrive\Área de Trabalho\FORUM NIXAIOS.txt"
    if os.name == "nt" else ""
)
_env_channel = os.environ.get("NIXAIOS_FORUM_CHANNEL")
CHANNEL = Path(_env_channel or _DEFAULT_CHANNEL) if (_env_channel or _DEFAULT_CHANNEL) else None
LOG_TOOL = REPO / "scripts" / "forum-log.py"

AGENT = "NEXUS"
TYPE = "OPCODE/1"


def _dir() -> Path:
    default = (r"C:\DEV\_logs\forum-watch-nexus" if os.name == "nt"
               else "~/.local/state/nixaios/forum-watch")
    return Path(os.environ.get("NEXUS_WATCH_DIR", default)).expanduser()


def _utc() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def read_entries() -> list[dict]:
    """Uniao dos canais, ordem de aparicao, dedupe por id."""
    out: list[dict] = []
    seen: set[str] = set()
    for path in (p for p in (LEDGER, CHANNEL) if p is not None):
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
    """Publica via scripts/forum-log.py (validacao + ledger + canal).

    W3 (licao 47): ficheiro temporario POR CHAMADA. Antes era sempre
    `pending-entry.json` no dir partilhado, portanto dois pollers no mesmo
    NEXUS_WATCH_DIR podiam publicar a entrada um do outro.
    """
    d = _dir()
    d.mkdir(parents=True, exist_ok=True)
    if os.environ.get("NEXUS_WATCH_DRY") == "1":
        return False, "dry-run"
    fd, tmp = tempfile.mkstemp(prefix="pending-entry-", suffix=".json", dir=str(d))
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="") as fh:
            json.dump(entry, fh, ensure_ascii=False)
        r = subprocess.run(
            [sys.executable, str(LOG_TOOL), "post", "--entry", tmp, "--from", AGENT],
            capture_output=True, text=True, encoding="utf-8", errors="replace",
        )
        return r.returncode == 0, (r.stdout or r.stderr or "").strip()[:400]
    finally:
        try:
            os.unlink(tmp)
        except OSError:
            pass


# --------------------------------------------------------------- sondas idle
# Ciclo sem novidade nao fica silencioso: corre UMA sonda de LEITURA (rotativa)
# e publica o que MEDIU + o delta desde a medicao anterior. E medicao, nao
# analise autonoma: nada e inventado, e sonda que falha e saltada (fail-closed).


def _utc_now() -> datetime:
    return datetime.now(timezone.utc)


def _git(*args: str) -> str:
    r = subprocess.run(
        ["git", "-C", str(REPO), *args],
        capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=25,
    )
    if r.returncode != 0:
        raise RuntimeError((r.stderr or "git rc!=0").strip().splitlines()[0][:120])
    return r.stdout


def _newest(entries: list[dict], sender: str | None = None) -> str | None:
    for e in reversed(entries):
        if e.get("from") == AGENT:
            continue
        if sender and e.get("from") != sender:
            continue
        return str(e.get("id"))
    return None


def probe_head(entries: list[dict]) -> dict:
    sha = _git("log", "-1", "--format=%h").strip()
    subject = _git("log", "-1", "--format=%s").strip()
    dirty = [ln for ln in _git("status", "--porcelain").splitlines()
             if ln.strip() and not ln.startswith("??")]
    names = [ln[3:].strip() for ln in dirty]
    return {
        "ref": _newest(entries),
        "facts": {"sha": sha, "dirty_tracked": len(dirty), "files": names[:8]},
        "text": (f"HEAD `{sha}` ({subject[:64]}); {len(dirty)} ficheiro(s) tracked "
                 f"modificado(s)" + (f": {', '.join(names[:5])}" if names else "") + "."),
        "why": ("e a prova de que o commit observado pelo ANVIL continua a ser o HEAD "
                "e de quanto trabalho ficou fora dele."),
    }


def _code_lines(src: str) -> list[str]:
    """W2: linhas sem comentario — substring num comentario nao e prova."""
    out = []
    for raw in src.splitlines():
        if raw.lstrip().startswith("#"):
            continue
        bare = raw.split("#", 1)[0]
        if bare.strip():
            out.append(bare)
    return out


def probe_harvest(entries: list[dict]) -> dict:
    """W2: codigo efectivo (linha do A4/A6/journalctl/trap) + `bash -n`."""
    p = REPO / "scripts" / "metal-m3b-harvest.sh"
    if not p.is_file():
        raise RuntimeError("metal-m3b-harvest.sh ausente")
    src = p.read_text(encoding="utf-8", errors="replace")
    code = _code_lines(src)
    mtime = datetime.fromtimestamp(p.stat().st_mtime, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    # patch3: o cabecalho `echo "--- A4 arecord 2s ---"` contem "arecord ";
    # escolher o comando real, nao a legenda.
    a4 = next((ln.strip() for ln in code
               if "arecord " in ln and not ln.lstrip().startswith("echo ")), "")
    a6 = next((ln.strip() for ln in code if "play_ok" in ln), "")
    journal = next((ln.strip() for ln in code if "journalctl" in ln), "")
    trap = next((ln.strip() for ln in code if ln.lstrip().startswith("trap ")), "")
    bash_bin = shutil.which("bash") or shutil.which("bash.exe")
    syntax_ok = None  # honestidade: nao medido != falhou
    syntax_err: list[str] = []
    if bash_bin:
        try:
            syn = subprocess.run([bash_bin, "-n", str(p)], capture_output=True, text=True,
                                 encoding="utf-8", errors="replace", timeout=30)
            syntax_ok = syn.returncode == 0
            syntax_err = (syn.stderr or "").strip().splitlines()[:1]
        except (OSError, subprocess.SubprocessError) as exc:
            syntax_err = [type(exc).__name__]
    else:
        syntax_err = ["nao medido (bash ausente no PATH do poller)"]
    facts = {
        "mtime": mtime,
        "syntax_ok": syntax_ok,
        "a4_rate_16000": ("-r" in a4 and "16000" in a4 and "-c" in a4),
        "f9_play_ok_asserted": bool(a6),
        "f9_since_in_journal": ("--since" in journal),
        "f10_trap_stmt": bool(trap),
        "a4_arecord": a4[:80],
        "syntax_error": (syntax_err[0][:120] if syntax_err else ""),
    }

    def _s(k: str) -> str:
        if facts[k] is None:
            return "nao-medido"
        return "sim" if facts[k] else "nao"

    return {
        "ref": _newest(entries, "FORGE"),
        "facts": facts,
        "text": (f"`scripts/metal-m3b-harvest.sh` mtime {mtime}, `bash -n` "
                 f"ok={_s('syntax_ok')}; rate16k no A4={_s('a4_rate_16000')}, "
                 f"F9(assert play_ok)={_s('f9_play_ok_asserted')}, "
                 f"F9(--since no journalctl)={_s('f9_since_in_journal')}, "
                 f"F10(trap stmt)={_s('f10_trap_stmt')}; A4='{facts['a4_arecord']}'"
                 + (f"; erro de sintaxe: {syntax_err[0][:80]}" if syntax_err else "") + "."),
        "why": ("mede codigo efectivo (W2 do ANVIL-0032). O rate do A4 e CONSISTENCIA "
                "com o nos-voice, nao bloqueador: whisper-cpp 1.9.5 reamostra (WAV 8 kHz "
                "-> rc=0; binario sem 'must be' e ligado a libswresample), portanto a "
                "premissa do NEXUS-0041 foi refutada por execucao."),
    }


def probe_commit(entries: list[dict]) -> dict:
    """HEAD + dirty tracked (W1: nao usar manifesto PENDING apagado pos-commit)."""
    import subprocess

    def _git(*args: str) -> str:
        try:
            r = subprocess.run(
                ["git", *args],
                cwd=REPO,
                capture_output=True,
                text=True,
                timeout=30,
                check=False,
            )
            return (r.stdout or "").strip()
        except OSError:
            return ""

    head = _git("rev-parse", "--short", "HEAD")
    dirty = [
        ln[3:]
        for ln in _git("status", "--porcelain").splitlines()
        if ln and not ln.startswith("??")
    ]
    return {
        "ref": _newest(entries, "FORGE") or _newest(entries, "ANVIL"),
        "facts": {"head": head, "dirty_tracked": len(dirty)},
        "text": (f"HEAD `{head or '?'}`; tracked dirty={len(dirty)}"
                 + (f" (ex: {', '.join(dirty[:3])})" if dirty else " (limpo).")),
        "why": ("substitui a sonda `manifest` (W1/ANVIL-0032): ausencia de "
                "PENDING_COMMIT_MANIFEST.md nao implica 'nao gastar HUMAN'."),
    }


def probe_queue(entries: list[dict]) -> dict:
    """So conta pedido DECLARADO (`data.awaiting` nao vazio) e sem `ref` posterior.

    Ack FORGE-0039: seguir a cadeia por topico nao e mecanizavel; um `ref` para
    outro id pode ser resposta ao mesmo assunto. Melhor 2 pedidos reais do que 5
    falsos positivos.
    """
    refs = {str(e.get("ref")) for e in entries if e.get("ref")}
    pending: list[str] = []
    for e in entries:
        if e.get("from") in (AGENT, "HUMAN"):
            continue
        d = e.get("data") or {}
        if str(d.get("awaiting") or "").strip() and str(e.get("id")) not in refs:
            pending.append(str(e.get("id")))
    tail = pending[-4:]
    return {
        "ref": tail[-1] if tail else _newest(entries),
        "facts": {"pending_count": len(pending), "pending": tail},
        "text": (f"{len(pending)} entrada(s) com `data.awaiting` explicito e sem "
                 f"`ref` posterior" + (f": {', '.join(tail)}" if tail else "") + "."),
        "why": ("criterio estreitado (FORGE-0039): pedido declarado pelo autor e "
                "mecanizavel; cadeia por topico nao e."),
    }


def probe_evidence(entries: list[dict]) -> dict:
    d = REPO / "docs" / "evidence"
    allmd = list(d.glob("*.md"))
    files = sorted(allmd, key=lambda x: x.stat().st_mtime, reverse=True)[:3]
    if not files:
        raise RuntimeError("docs/evidence sem .md")
    latest = files[0]
    facts = {
        "count": len(allmd),
        "newest": latest.name,
        "newest_mtime": datetime.fromtimestamp(latest.stat().st_mtime, timezone.utc)
        .strftime("%Y-%m-%dT%H:%M:%SZ"),
        "recent": [x.name for x in files],
    }
    return {
        "ref": _newest(entries, "ANVIL"),
        "facts": facts,
        "text": (f"evidence: {facts['count']} docs .md; mais recente `{facts['newest']}` "
                 f"({facts['newest_mtime']}); 3 ultimos: {', '.join(facts['recent'])}."),
        "why": "o evidence doc e o unico sitio onde 'Not-tested' encolhe com prova.",
    }


def probe_ledger(entries: list[dict]) -> dict:
    r = subprocess.run(
        [sys.executable, str(LOG_TOOL), "validate"],
        capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=60,
    )
    ok = r.returncode == 0
    info: dict = {}
    try:
        info = json.loads((r.stdout or "").strip().splitlines()[-1])
    except (json.JSONDecodeError, IndexError):
        pass
    last: dict[str, str] = {}
    for e in entries:
        last[str(e.get("from"))] = str(e.get("id"))
    facts = {
        "entries": info.get("entries", len(entries)),
        "validate_ok": ok,
        "id_duplicates": len(info.get("duplicates") or []),
        "content_duplicates": len(info.get("content_duplicates") or []),
        "last_by_agent": last,
    }
    return {
        "ref": _newest(entries),
        "facts": facts,
        "text": (f"ledger {facts['entries']} entradas, validate ok={'sim' if ok else 'nao'} "
                 f"(ids duplicados {facts['id_duplicates']}, conteudo duplicado "
                 f"{facts['content_duplicates']}); ultimo por emissor: "
                 + ", ".join(f"{a}={b}" for a, b in sorted(last.items())) + "."),
        "why": ("o ledger e o registo canonico: se a validacao cai, as provas deixam de ser "
                "citaveis."),
    }


PROBES = [
    ("head", probe_head),
    ("harvest", probe_harvest),
    ("commit", probe_commit),
    ("queue", probe_queue),
    ("evidence", probe_evidence),
    ("ledger", probe_ledger),
]


def _delta(prev: dict | None, facts: dict) -> tuple[str, bool]:
    """(frase, mudou) — comparacao explicita com a medicao anterior da sonda."""
    if prev is None:
        return "primeira medicao desta sonda (sem base de comparacao).", False
    changed = []
    for key in sorted(set(prev) | set(facts)):
        a, b = prev.get(key, "<ausente>"), facts.get(key, "<ausente>")
        if a != b:
            changed.append(f"{key}: {json.dumps(a, ensure_ascii=False)} -> "
                           f"{json.dumps(b, ensure_ascii=False)}")
    if changed:
        return "delta desde a medicao anterior: " + "; ".join(changed[:6]) + ".", True
    return "sem alteracao face a medicao anterior.", False


def idle_entry(st: dict, entries: list[dict], k: int, cycles: int, interval: int,
               new_ids: list[str] | None = None, froms: list[str] | None = None):
    """Constroi a entrada de ciclo vazio: uma sonda medida, nunca texto repetido."""
    n = len(PROBES)
    start = int(st.get("probe_idx", 0))
    new_ids = list(new_ids or [])
    froms = list(froms or [])
    last_ext = st.get("last_external") or {}
    mins = -1
    if last_ext.get("id"):
        try:
            t = datetime.strptime(str(last_ext.get("ts")), "%Y-%m-%dT%H:%M:%SZ")
            mins = int((_utc_now() - t.replace(tzinfo=timezone.utc)).total_seconds() // 60)
        except (TypeError, ValueError):
            mins = -1
    if new_ids:
        head = (f"novidade: {len(new_ids)} entrada(s) nova(s) de "
                f"{', '.join(froms) or '?'} — ids {', '.join(new_ids)}.")
    elif last_ext.get("id"):
        head = (f"sem entradas novas de outros emissores (ultima: "
                f"{last_ext['id']} ha {mins} min).")
    else:
        head = "sem entradas novas de outros emissores (sem referencia externa)."
    seen_bodies = set(st.get("bodies", []))
    for off in range(n):
        name, fn = PROBES[(start + off) % n]
        try:
            res = fn(entries)
        except Exception as exc:  # sonda falhou: saltar, nunca inventar
            note({"ts": _utc(), "probe": name,
                  "skipped": f"{type(exc).__name__}: {exc}"})
            continue
        dtext, changed = _delta(st.get("probe_last", {}).get(name), res["facts"])
        stall = 0 if changed else int(st.get("probe_stall", {}).get(name, 0)) + 1
        body = (
            f"Review continuo NEXUS (ciclo {k}/{cycles}, {_utc()}, cadencia {interval}s) — "
            f"{head} Sonda `{name}` [{off + 1}/{n} do ciclo]: {res['text']} {dtext}"
            + (f" Inalterada ha {stall} medicao(oes) desta sonda." if stall > 1 else "")
            + f" Porque conta: {res['why']} Ref {res['ref'] or '-'}."
        )
        key = hashlib.sha1(" ".join(body.split()).lower().encode("utf-8")).hexdigest()
        if key in seen_bodies:
            continue
        entry = {
            "v": TYPE,
            "id": next_id(set(st.get("seen", []))),
            "from": AGENT,
            "ts": _utc(),
            "type": "status",
            "ref": res["ref"],
            "body": body,
            "data": {
                "mode": "probe_news" if new_ids else "probe_idle",
                "cycle": k,
                "cycles_total": cycles,
                "interval_s": interval,
                "probe": name,
                "facts": res["facts"],
                "stall": stall,
                "new_ids": new_ids,
                "new_from": froms,
                "needs_agent_review": True,
                "generates_analysis": False,
            },
        }
        return entry, name, res, key, stall
    return None


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

    start_idx = int(st.get("probe_idx", 0))

    if not known:  # primeira execucao: baseline, nada a publicar
        ext = [e for e in entries if e.get("from") != AGENT]
        if ext:
            st["last_external"] = {"id": ext[-1]["id"], "ts": ext[-1].get("ts")}
        st["seen"] = [e["id"] for e in entries]
        rec["mode"] = "baseline"
        note(rec)
        save_state(st)
        return rec

    ids = [e["id"] for e in external]
    froms = sorted({str(e.get("from")) for e in external})
    if external:
        newest_ext = external[-1]
        if newest_ext.get("id"):
            st["last_external"] = {"id": newest_ext["id"], "ts": newest_ext.get("ts")}

    built = None
    if os.environ.get("NEXUS_IDLE_MODE", "probe") != "silent":
        built = idle_entry(st, entries, k, cycles, interval, ids, froms)

    if built is not None:
        entry, name, res, key, stall = built
        ok, out = publish(entry)
        rec.update({"posted": ok, "post_id": entry["id"], "post_result": out,
                    "idle_probe": name, "idle_facts": res["facts"], "idle_stall": stall})
        rec["mode"] = "probe_news" if ids else "probe_idle"
        if ok:
            st["posted"] = int(st.get("posted", 0)) + 1
            st["bodies"] = (list(st.get("bodies", [])) + [key])[-100:]
            st["probe_idx"] = (start_idx + 1) % len(PROBES)
            lastm = dict(st.get("probe_last", {}))
            lastm[name] = res["facts"]
            st["probe_last"] = lastm
            stalls = dict(st.get("probe_stall", {}))
            stalls[name] = stall
            st["probe_stall"] = stalls
    elif external:
        # fallback honesto: sem sonda, mas a novidade nao se perde
        kinds = sorted({str(e.get("type")) for e in external})
        body = (
            f"Watcher NEXUS (cadencia {interval}s, ciclo {k}/{cycles}): "
            f"{len(external)} entrada(s) nova(s) de {', '.join(froms)} "
            f"(tipos {', '.join(kinds)}) — ids {', '.join(ids)}. "
            "Sondas indisponiveis neste ciclo; nada foi inventado."
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
    else:
        rec["mode"] = "probe_skipped"

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
          f"cycles={cycles} window={args.duration}s dry={dry} "
          f"idle_mode={os.environ.get('NEXUS_IDLE_MODE', 'probe')}", flush=True)

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
