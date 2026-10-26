#!/usr/bin/env python3
"""Fix networkmanage typos without mangling networkmanager (prefix trap)."""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1] / "modules"


def fix_text(t: str) -> str:
    # Undo accidental networkmanagerrr / networkmanagerr from bad replace
    t = t.replace("networkmanagerrr", "networkmanager")
    t = t.replace("networkmanagerr", "networkmanager")
    # Bare line identifier only (not pkgs.networkmanager)
    t = re.sub(r"(?m)^(\s+)networkmanage\s*$", r"\1networkmanager", t)
    # pkgs.networkmanage as whole attr — not prefix of networkmanager
    t = re.sub(r"\bpkgs\.networkmanage\b(?!r)", "pkgs.networkmanager", t)
    return t.replace("\r\n", "\n")


bad = []
for p in sorted(ROOT.glob("*.nix")):
    t = p.read_text(encoding="utf-8")
    t2 = fix_text(t)
    if t != t2:
        p.write_text(t2, encoding="utf-8", newline="\n")
        print(f"{p.name}: FIXED")
    else:
        print(f"{p.name}: ok")
    for i, line in enumerate(t2.splitlines(), 1):
        if re.search(r"networkmanage([^r]|$)", line) and "networkmanager" not in line:
            bad.append(f"{p.name}:{i}:{line.strip()}")
        if "networkmanagerr" in line:
            bad.append(f"{p.name}:{i}: still mangled: {line.strip()}")

if bad:
    print("STILL BAD:")
    print("\n".join(bad))
    raise SystemExit(1)
print("TYPO_GONE")
