#!/usr/bin/env python3
"""OR PF_W on every PT_LOAD that covers PT_DYNAMIC (patchelf + glibc 2.44).

nvidia-x11-*-bin non-PIE tools (nvidia-smi, …) can land .dynamic in a
read-only LOAD. ld-linux then SIGSEGV writing DT_DEBUG. Metal 2026-10-31.
"""
from __future__ import annotations

import struct
import sys
from pathlib import Path

PF_W = 2
PT_LOAD = 1
PT_DYNAMIC = 2


def fix_elf(path: Path) -> str:
    data = bytearray(path.read_bytes())
    if data[:4] != b"\x7fELF":
        return "skip-not-elf"
    if data[4] != 2:  # ELFCLASS64
        return "skip-not-elf64"
    e_phoff = struct.unpack_from("<Q", data, 32)[0]
    e_phentsize, e_phnum = struct.unpack_from("<HH", data, 54)
    dyn_vaddr = None
    for i in range(e_phnum):
        off = e_phoff + i * e_phentsize
        p_type, _flags, _po, p_vaddr, _pp, _pf, _memsz, _pa = struct.unpack_from(
            "<IIQQQQQQ", data, off
        )
        if p_type == PT_DYNAMIC:
            dyn_vaddr = p_vaddr
            break
    if dyn_vaddr is None:
        return "skip-no-dynamic"

    covering = []
    for i in range(e_phnum):
        off = e_phoff + i * e_phentsize
        p_type, p_flags, _po, p_vaddr, _pp, _pf, p_memsz, _pa = struct.unpack_from(
            "<IIQQQQQQ", data, off
        )
        if p_type != PT_LOAD:
            continue
        if p_memsz == 0:
            continue
        if p_vaddr <= dyn_vaddr < p_vaddr + p_memsz:
            covering.append((off, p_flags))

    if not covering:
        return "skip-no-load"

    changed = 0
    for off, p_flags in covering:
        if p_flags & PF_W:
            continue
        struct.pack_into("<I", data, off + 4, p_flags | PF_W)
        changed += 1

    if changed == 0:
        return "ok-already"
    path.write_bytes(data)
    return f"fixed-{changed}"


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("usage: fix-nvidia-elf-pf-w.py FILE [FILE…]", file=sys.stderr)
        return 2
    rc = 0
    for raw in argv[1:]:
        p = Path(raw)
        if not p.is_file():
            print(f"{p}: missing", file=sys.stderr)
            rc = 1
            continue
        status = fix_elf(p)
        print(f"{p}: {status}")
        if status.startswith("skip-"):
            rc = 1
    return rc


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
