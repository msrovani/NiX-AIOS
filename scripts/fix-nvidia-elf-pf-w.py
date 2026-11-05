#!/usr/bin/env python3
"""OR PF_W on every PT_LOAD (patchelf + glibc 2.44).

nvidia-x11-*-bin non-PIE tools (nvidia-smi, …) can land .dynamic and other
write targets (e.g. the DT_DEBUG r_debug slot ld-linux writes at startup) in
read-only LOAD segments. ld-linux then SIGSEGV writing to them. Metal
2026-11-03: fixing only the PT_LOAD covering PT_DYNAMIC was insufficient;
segfault at 0x401488 inside ld-linux persists.

Fix: OR PF_W on ALL PT_LOAD segments so every address the dynamic linker
may write to is writable.
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

    # Sanity: confirm PT_DYNAMIC exists (it always does for dynamically linked
    # ELF, but guard anyway).
    dyn_found = False
    for i in range(e_phnum):
        off = e_phoff + i * e_phentsize
        p_type, _flags = struct.unpack_from("<II", data, off)
        if p_type == PT_DYNAMIC:
            dyn_found = True
            break
    if not dyn_found:
        return "skip-no-dynamic"

    changed = 0
    for i in range(e_phnum):
        off = e_phoff + i * e_phentsize
        p_type, p_flags, _po, p_vaddr, _pp, _pf, p_memsz, _pa = struct.unpack_from(
            "<IIQQQQQQ", data, off
        )
        if p_type != PT_LOAD:
            continue
        if p_memsz == 0:
            continue
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
        print(f"{p}: {status}", file=sys.stderr)
        if status.startswith("skip-"):
            rc = 1
    return rc


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
