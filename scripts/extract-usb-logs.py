#!/usr/bin/env python3
"""Extrai logs/evidencia do pendrive `NIXAIOSLOG` para `logs/` (read-only).

Porque existe: a FAT32 do pendrive híbrido aparece sem letra no Windows
(lição 45) e `mount`-offsets mentem (lição 50). Este script lê o dispositivo
**cru**: BPB -> FAT -> cadeia de clusters, sem montar e sem escrever nada no
pendrive. No Windows precisa de admin para abrir `\\\\.\\PhysicalDriveN`, por
isso re-executa-se elevado sozinho.

Uso:
    python scripts/extract-usb-logs.py                # auto-detecta o USB
    python scripts/extract-usb-logs.py --list         # só lista a raiz
    python scripts/extract-usb-logs.py --disk 1 --offset 8323596288
    python scripts/extract-usb-logs.py --out logs
    python scripts/extract-usb-logs.py --with-mic     # inclui *.wav do mic
    python scripts/extract-usb-logs.py --mounted /media/$USER/NIXAIOSLOG

Nunca copia `wifi.conf` / `firstboot.conf` (PSK) nem `*.wav` (voz) sem
`--with-mic`. Escreve `MANIFEST.txt` com tamanho+sha256 de cada ficheiro.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import struct
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

SECRET_NAMES = {"wifi.conf", "firstboot.conf"}
PROJECT_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUT = PROJECT_ROOT / "logs"


# ---------------------------------------------------------------- Windows ---
def is_admin() -> bool:
    if os.name != "nt":
        return os.geteuid() == 0
    try:
        import ctypes

        return bool(ctypes.windll.shell32.IsUserAnAdmin())
    except Exception:
        return False


def relaunch_elevated() -> None:
    """Pede UAC e re-executa o script com os mesmos argumentos."""
    import ctypes

    params = " ".join(f'"{a}"' for a in [os.path.abspath(__file__), *sys.argv[1:]])
    rc = ctypes.windll.shell32.ShellExecuteW(None, "runas", sys.executable, params, None, 1)
    if rc <= 32:
        sys.exit(f"elevacao recusada/falhou (ShellExecuteW rc={rc})")
    sys.exit(0)


def detect_usb_partition() -> dict | None:
    """Primeiro disco USB com uma particao FAT32. Devolve Number/Offset/Size."""
    ps = (
        "$d = Get-Disk | Where-Object { $_.BusType -eq 'USB' } | Select-Object -First 1;"
        "if ($d) {"
        "  $p = Get-Partition -DiskNumber $d.Number |"
        "       Where-Object { \"$($_.Type)\" -match 'FAT32' } | Select-Object -First 1;"
        "  if ($p) {"
        "    [pscustomobject]@{disk=$d.Number; name=$d.FriendlyName;"
        "      offset=[int64]$p.Offset; size=[int64]$p.Size} | ConvertTo-Json -Compress"
        "  }"
        "}"
    )
    try:
        out = subprocess.run(
            ["powershell", "-NoProfile", "-Command", ps],
            capture_output=True,
            text=True,
            timeout=60,
        ).stdout.strip()
    except Exception as e:  # noqa: BLE001
        print(f"aviso: deteccao PowerShell falhou ({e})", file=sys.stderr)
        return None
    if not out:
        return None
    try:
        return json.loads(out)
    except json.JSONDecodeError:
        return None


# -------------------------------------------------------------------- FAT ---
class Fat32:
    """Leitor FAT32 read-only sobre um dispositivo/caminho já aberto."""

    def __init__(self, path: Path, offset: int):
        self.f = open(path, "rb", buffering=0)
        self.off = offset
        boot = self.rd(0, 512)
        if boot[510:512] != b"\x55\xaa":
            print("aviso: sem assinatura 55AA no sector de boot", file=sys.stderr)
        self.bps = struct.unpack_from("<H", boot, 11)[0]
        self.spc = boot[13]
        self.reserved = struct.unpack_from("<H", boot, 14)[0]
        self.nfats = boot[16]
        self.root_ents = struct.unpack_from("<H", boot, 17)[0]
        fatsz16 = struct.unpack_from("<H", boot, 22)[0]
        fatsz32 = struct.unpack_from("<I", boot, 36)[0]
        self.fatsz = fatsz16 or fatsz32
        self.fat32 = fatsz16 == 0
        self.root_clus = struct.unpack_from("<I", boot, 44)[0] if self.fat32 else 0
        self.label = (
            boot[71:82].decode("ascii", "replace").strip()
            if self.fat32
            else boot[43:54].decode("ascii", "replace").strip()
        )
        if self.bps not in (512, 1024, 2048, 4096) or self.fatsz == 0:
            raise ValueError(f"BPB invalido (bps={self.bps}, fatsz={self.fatsz}) — offset errado?")
        self.fat_start = self.reserved * self.bps
        self.data_start = self.fat_start + self.nfats * self.fatsz * self.bps
        self.fat = self.rd(self.fat_start, self.fatsz * self.bps)

    def rd(self, rel: int, n: int) -> bytes:
        self.f.seek(self.off + rel)
        return self.f.read(n)

    def next_clus(self, clus: int) -> int:
        if self.fat32:
            i = clus * 4
            if i + 4 > len(self.fat):
                return 0x0FFFFFFF
            return struct.unpack_from("<I", self.fat, i)[0] & 0x0FFFFFFF
        i = clus * 2
        if i + 2 > len(self.fat):
            return 0xFFFF
        return struct.unpack_from("<H", self.fat, i)[0]

    def chain(self, clus: int, limit: int = 8192) -> list[int]:
        out, seen = [], set()
        while 2 <= clus < 0x0FFFFFF8 and clus not in seen and len(out) < limit:
            out.append(clus)
            seen.add(clus)
            clus = self.next_clus(clus)
        return out

    def read_clus(self, clus: int, size: int | None = None) -> bytes:
        blob = b"".join(
            self.rd(self.data_start + (c - 2) * self.spc * self.bps, self.spc * self.bps)
            for c in self.chain(clus)
        )
        return blob[:size] if size is not None else blob

    def entries(self, clus: int) -> list[dict]:
        if clus == 0 and self.root_ents:
            data = self.rd(self.fat_start + self.nfats * self.fatsz * self.bps, self.root_ents * 32)
        elif clus == 0:
            data = b""
        else:
            data = self.read_clus(clus)

        out: list[dict] = []
        lfn: list[str] = []
        for i in range(0, len(data) - 31, 32):
            ent = data[i : i + 32]
            if ent[0] == 0x00:
                break
            if ent[0] == 0xE5:
                lfn.clear()
                continue
            attr = ent[11]
            if attr == 0x0F:
                chunk = ""
                for off in (1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30):
                    ch = struct.unpack_from("<H", ent, off)[0]
                    if ch in (0x0000, 0xFFFF):
                        break
                    chunk += chr(ch)
                lfn.append(chunk)
                continue
            if attr & 0x08:
                lfn.clear()
                continue
            name83 = ent[0:11]
            clus_e = struct.unpack_from("<H", ent, 26)[0] | (struct.unpack_from("<H", ent, 20)[0] << 16)
            size = struct.unpack_from("<I", ent, 28)[0]
            date = struct.unpack_from("<H", ent, 24)[0]
            tm = struct.unpack_from("<H", ent, 22)[0]
            if lfn:
                name = "".join(reversed(lfn))
                lfn.clear()
            else:
                base = name83[0:8].decode("ascii", "replace").strip()
                ext = name83[8:11].decode("ascii", "replace").strip()
                name = f"{base}.{ext}" if ext else base
            out.append(
                {
                    "name": name,
                    "clus": clus_e,
                    "size": size,
                    "dir": bool(attr & 0x10),
                    "mtime": f"{(date >> 9) + 1980:04d}-{(date >> 5) & 0xF:02d}-{date & 0x1F:02d} "
                    f"{(tm >> 11):02d}:{(tm >> 5) & 0x3F:02d}",
                }
            )
        return out

    def close(self) -> None:
        self.f.close()


# ----------------------------------------------------------------- export ---
def skip(name: str, with_mic: bool) -> bool:
    low = name.lower()
    if low in SECRET_NAMES:
        return True
    return low.endswith(".wav") and not with_mic


def export_tree(fs: Fat32, dest: Path, with_mic: bool) -> list[tuple[str, int, str]]:
    written: list[tuple[str, int, str]] = []

    def dump_file(ent: dict, ddir: Path) -> None:
        if ent["dir"] or ent["size"] <= 0 or skip(ent["name"], with_mic):
            if ent["size"] > 0 and skip(ent["name"], with_mic):
                print(f"  skip (privacidade): {ent['name']}")
            return
        data = fs.read_clus(ent["clus"], ent["size"])
        ddir.mkdir(parents=True, exist_ok=True)
        path = ddir / ent["name"]
        path.write_bytes(data)
        written.append((str(path.relative_to(dest)), len(data), hashlib.sha256(data).hexdigest()))

    def walk(clus: int, ddir: Path, depth: int = 0) -> None:
        for ent in fs.entries(clus):
            if ent["name"] in (".", ".."):
                continue
            if ent["dir"]:
                if depth < 3:
                    walk(ent["clus"], ddir / ent["name"], depth + 1)
            else:
                dump_file(ent, ddir)

    root = fs.entries(fs.root_clus)
    for ent in root:
        if ent["name"] in (".", ".."):
            continue
        if ent["dir"] and ent["name"].lower() == "boots":
            for bdir in [e for e in fs.entries(ent["clus"]) if e["dir"] and not e["name"].startswith(".")]:
                walk(bdir["clus"], dest / "boots" / bdir["name"])
        elif ent["dir"]:
            continue  # outras pastas: só boots/ (journal/firstboot/cage/harvest)
        else:
            dump_file(ent, dest)
    return written


def list_root(fs: Fat32) -> None:
    print(f"label={fs.label!r} FAT{'32' if fs.fat32 else '16'} spc={fs.spc} root_clus={fs.root_clus}")
    for ent in fs.entries(fs.root_clus):
        if ent["name"] in (".", ".."):
            continue
        kind = "DIR " if ent["dir"] else f"{ent['size']:>9}B"
        print(f"  {kind} {ent['mtime']}  {ent['name']}")


def copy_mounted(src: Path, dest: Path, with_mic: bool) -> list[tuple[str, int, str]]:
    written: list[tuple[str, int, str]] = []
    for path in sorted(src.rglob("*")):
        if not path.is_file() or skip(path.name, with_mic):
            continue
        rel = path.relative_to(src)
        target = dest / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        data = target.read_bytes()
        written.append((str(rel), len(data), hashlib.sha256(data).hexdigest()))
    return written


def main() -> int:
    ap = argparse.ArgumentParser(description="Extrai logs do pendrive NIXAIOSLOG (read-only)")
    ap.add_argument("--disk", help="numero do disco fisico (Windows)")
    ap.add_argument("--offset", type=int, help="offset em bytes da particao FAT32")
    ap.add_argument("--out", default=str(DEFAULT_OUT), help="directorio destino (default: logs/)")
    ap.add_argument("--mounted", help="caminho ja montado (Linux/WSL) em vez do dispositivo cru")
    ap.add_argument("--list", action="store_true", help="só listar a raiz, sem copiar")
    ap.add_argument("--with-mic", action="store_true", help="incluir *.wav (voz do operador)")
    args = ap.parse_args()

    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")

    if args.mounted:
        src = Path(args.mounted)
        if not src.is_dir():
            return print_error(f"--mounted nao e um directorio: {src}")
        dest = Path(args.out) / f"{stamp}_mounted"
        dest.mkdir(parents=True, exist_ok=True)
        written = copy_mounted(src, dest, args.with_mic)
        return finish(written, dest)

    disk, offset = args.disk, args.offset
    if disk is None or offset is None:
        found = detect_usb_partition()
        if not found:
            return print_error(
                "sem disco USB com particao FAT32. Ligue o pendrive ou passe --disk N --offset BYTES "
                "(Get-Partition -DiskNumber N)."
            )
        disk = disk if disk is not None else str(found["disk"])
        offset = offset if offset is not None else int(found["offset"])
        print(f"pendrive: disco {disk} ({found.get('name', '?')}) FAT32 @ offset {offset}")

    if os.name == "nt" and not is_admin():
        print("precisa de admin para ler o dispositivo cru; a pedir elevacao…", file=sys.stderr)
        relaunch_elevated()
        return 0

    dev = Path(f"\\\\.\\PhysicalDrive{disk}")
    try:
        fs = Fat32(dev, int(offset))
    except PermissionError:
        return print_error("acesso negado ao dispositivo — corra num shell elevado")
    except FileNotFoundError:
        return print_error(f"dispositivo inexistente: {dev}")

    try:
        if args.list:
            list_root(fs)
            return 0
        dest = Path(args.out) / f"{stamp}_disk{disk}"
        dest.mkdir(parents=True, exist_ok=True)
        written = export_tree(fs, dest, args.with_mic)
    finally:
        fs.close()
    return finish(written, dest)


def print_error(msg: str) -> int:
    print(f"erro: {msg}", file=sys.stderr)
    return 2


def finish(written: list[tuple[str, int, str]], dest: Path) -> int:
    manifest = dest / "MANIFEST.txt"
    with manifest.open("w", encoding="utf-8", newline="\n") as fh:
        fh.write(f"# extract-usb-logs {datetime.now(timezone.utc).isoformat()}\n")
        fh.write("# caminho\ttamanho\tsha256\n")
        for rel, size, digest in written:
            fh.write(f"{rel}\t{size}\t{digest}\n")
    total = sum(s for _, s, _ in written)
    print(f"\n{dest}: {len(written)} ficheiros, {total} bytes")
    print(f"manifest: {manifest}")
    if not written:
        print("aviso: nada copiado (raiz vazia ou tudo filtrado)", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
