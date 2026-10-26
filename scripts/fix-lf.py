#!/usr/bin/env python3
"""Strip CRLF from listed files (Nix writeShellScript breaks on \\r)."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FILES = [
    ROOT / "modules/transmute-stage.nix",
    ROOT / "modules/neural-os.nix",
    ROOT / "modules/jarbas-cage.nix",
    ROOT / "modules/log-fat32.nix",
    ROOT / "images/iso.nix",
    ROOT / "packages/nosd/crates/nosd/src/transmute.rs",
]

for p in FILES:
    b = p.read_bytes()
    n = b.count(b"\r\n")
    fixed = b.replace(b"\r\n", b"\n").replace(b"\r", b"\n")
    if fixed != b:
        p.write_bytes(fixed)
        print(f"fixed {p.name}: {n} CRLF")
    else:
        print(f"ok {p.name}")
