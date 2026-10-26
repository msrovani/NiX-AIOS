#!/usr/bin/env python3
"""List NIXAIOSLOG FAT32 root (+ dump small logs) from hybrid USB img without mount."""
from __future__ import annotations

import struct
import sys
from pathlib import Path

IMG = Path(sys.argv[1] if len(sys.argv) > 1 else r"C:\Users\msrov\Downloads\nixaios-gnome-s520-transmute-persist.img")
# s520 pack: partition 3 start sector
START_SECT = int(sys.argv[2]) if len(sys.argv) > 2 else 6594560
OFF = START_SECT * 512
WANT = {
    b"JOURNALB",  # JOURNAL-BOOT.LOG truncated 8.3
    b"JOURNALF",
    b"TRANSMUT",
    b"WIZARD-W",
    b"HOTSWAP ",
    b"JARBAS-C",
    b"README  ",
    b"NOSD-PER",
}


def main() -> None:
    print(f"IMG={IMG} exists={IMG.exists()} size={IMG.stat().st_size if IMG.exists() else 0}")
    with IMG.open("rb") as f:
        f.seek(OFF)
        boot = f.read(512)
    if boot[510:512] != b"\x55\xaa":
        print("WARN: no AA55 at FAT boot sector; dumping first 16 bytes:", boot[:16].hex())
    bps = struct.unpack_from("<H", boot, 11)[0]
    spc = boot[13]
    reserved = struct.unpack_from("<H", boot, 14)[0]
    fats = boot[16]
    root_ents = struct.unpack_from("<H", boot, 17)[0]
    fatsz16 = struct.unpack_from("<H", boot, 22)[0]
    fatsz32 = struct.unpack_from("<I", boot, 36)[0]
    fatsz = fatsz16 or fatsz32
    root_clus = struct.unpack_from("<I", boot, 44)[0] if fatsz16 == 0 else 0
    print(f"bps={bps} spc={spc} reserved={reserved} fats={fats} fatsz={fatsz} root_ents={root_ents} root_clus={root_clus}")

    # FAT32: data starts after reserved + fats*fatsz; root is cluster root_clus
    fat_start = OFF + reserved * bps
    data_start = fat_start + fats * fatsz * bps
    print(f"fat_start={fat_start} data_start={data_start}")

    def cluster_offset(clus: int) -> int:
        return data_start + (clus - 2) * spc * bps

    entries: list[tuple[str, int, int]] = []
    with IMG.open("rb") as f:
        if root_ents:
            # FAT12/16 root
            root_off = fat_start + fats * fatsz * bps
            f.seek(root_off)
            blob = f.read(root_ents * 32)
        else:
            # FAT32 root cluster chain (first cluster only — enough for small roots)
            f.seek(cluster_offset(root_clus))
            blob = f.read(spc * bps * 8)  # up to 8 clusters

    i = 0
    long_name_parts: list[str] = []
    while i + 32 <= len(blob):
        ent = blob[i : i + 32]
        i += 32
        if ent[0] == 0x00:
            break
        if ent[0] == 0xE5:
            long_name_parts.clear()
            continue
        attr = ent[11]
        if attr == 0x0F:
            # LFN
            chars = []
            for off in (1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30):
                ch = struct.unpack_from("<H", ent, off)[0]
                if ch in (0x0000, 0xFFFF):
                    break
                chars.append(chr(ch))
                long_name_parts.insert(0, "".join(chars) if False else "")
            # rebuild properly
            chunk = ""
            for off in (1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30):
                ch = struct.unpack_from("<H", ent, off)[0]
                if ch in (0x0000, 0xFFFF):
                    break
                chunk += chr(ch)
            long_name_parts.insert(0, chunk)
            continue
        name83 = ent[0:11]
        clus = struct.unpack_from("<H", ent, 26)[0] | (struct.unpack_from("<H", ent, 20)[0] << 16)
        size = struct.unpack_from("<I", ent, 28)[0]
        if long_name_parts:
            name = "".join(long_name_parts)
            long_name_parts.clear()
        else:
            base = name83[0:8].decode("ascii", "replace").strip()
            ext = name83[8:11].decode("ascii", "replace").strip()
            name = f"{base}.{ext}" if ext else base
        is_dir = bool(attr & 0x10)
        entries.append((name, clus, size if not is_dir else -1))

    print(f"root_entries={len(entries)}")
    for name, clus, size in entries:
        tag = "DIR" if size < 0 else f"{size}B"
        print(f"  {name:40} clus={clus} {tag}")

    # Dump text logs if present
    by_name = {n.lower(): (c, s) for n, c, s in entries}
    for key in (
        "journal-boot.log",
        "journal-follow.log",
        "transmute-launch.log",
        "wizard-watch.log",
        "hotswap.log",
        "jarbas-cage.log",
        "readme.txt",
    ):
        if key not in by_name:
            print(f"MISS {key}")
            continue
        clus, size = by_name[key]
        if size <= 0:
            print(f"EMPTY {key}")
            continue
        with IMG.open("rb") as f:
            f.seek(cluster_offset(clus))
            data = f.read(min(size, 12000))
        print(f"---- FOUND {key} ({size} bytes, show {len(data)}) ----")
        sys.stdout.write(data.decode("utf-8", "replace"))
        if not data.endswith(b"\n"):
            print()
        print("---- END ----")


if __name__ == "__main__":
    main()
