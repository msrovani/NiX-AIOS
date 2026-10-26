#!/usr/bin/env python3
from pathlib import Path

p = Path("/mnt/c/Users/msrov/Downloads/nixaios-gnome-s520-transmute-persist.img")
b = p.read_bytes()[: 2 * 1024 * 1024]
pvd = b[32768 : 32768 + 2048]
print("pvd_volid=", pvd[40:72])
print("has_NIXAIOS520=", b"NIXAIOS520" in b)
print("size_gb=", round(p.stat().st_size / (1024**3), 2))
