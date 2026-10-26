#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
files = [
    ROOT / "modules/transmute-stage.nix",
    ROOT / "modules/neural-os.nix",
    ROOT / "modules/jarbas-cage.nix",
    ROOT / "modules/log-fat32.nix",
    ROOT / "images/iso.nix",
    ROOT / "packages/nosd/crates/nosd/src/transmute.rs",
]
for p in files:
    t = p.read_text(encoding="utf-8", errors="replace")
    corrupt = ("sevices" in t) or ("/va/lib" in t) or ("witeShellScipt" in t) or ("tansmute" in t)
    print(f"{'CORRUPT' if corrupt else 'ok':8} {p.relative_to(ROOT)}")
