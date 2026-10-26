#!/usr/bin/env python3
"""Convert CRLF->LF for all nix/sh/rs under nixaios (safe: never delete bare 'r')."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXTS = {".nix", ".sh", ".rs", ".py", ".toml", ".md"}

n_files = 0
n_bytes = 0
for p in ROOT.rglob("*"):
    if not p.is_file():
        continue
    if p.suffix.lower() not in EXTS:
        continue
    if "target" in p.parts or ".git" in p.parts:
        continue
    b = p.read_bytes()
    if b"\r\n" not in b and b"\r" not in b:
        continue
    fixed = b.replace(b"\r\n", b"\n").replace(b"\r", b"\n")
    p.write_bytes(fixed)
    n_files += 1
    n_bytes += b.count(b"\r")
    print(f"fixed {p.relative_to(ROOT)}")

print(f"done: {n_files} files, {n_bytes} CR removed")
