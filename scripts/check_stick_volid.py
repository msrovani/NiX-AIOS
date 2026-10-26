#!/usr/bin/env python3
"""Compare ISO9660 volume ID: Downloads s515 ISO vs PhysicalDrive1 (admin)."""
from __future__ import annotations

import ctypes
import sys
from ctypes import wintypes

ISO = r"C:\Users\msrov\Downloads\nixaios-minimal-s515-webcam-cinema.iso"
OUT = r"C:\DEV\nix-aios\stick-volid-check.txt"
DISK = r"\\.\PhysicalDrive1"

GENERIC_READ = 0x80000000
FILE_SHARE_READ = 0x00000001
FILE_SHARE_WRITE = 0x00000002
OPEN_EXISTING = 3
FILE_ATTRIBUTE_NORMAL = 0x80
INVALID_HANDLE_VALUE = wintypes.HANDLE(-1).value


def read_pvd_file(path: str) -> bytes:
    with open(path, "rb") as f:
        f.seek(16 * 2048)
        return f.read(2048)


def read_pvd_phys(path: str) -> bytes:
    k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    CreateFileW = k32.CreateFileW
    CreateFileW.argtypes = [
        wintypes.LPCWSTR,
        wintypes.DWORD,
        wintypes.DWORD,
        wintypes.LPVOID,
        wintypes.DWORD,
        wintypes.DWORD,
        wintypes.HANDLE,
    ]
    CreateFileW.restype = wintypes.HANDLE
    ReadFile = k32.ReadFile
    ReadFile.argtypes = [
        wintypes.HANDLE,
        wintypes.LPVOID,
        wintypes.DWORD,
        ctypes.POINTER(wintypes.DWORD),
        wintypes.LPVOID,
    ]
    ReadFile.restype = wintypes.BOOL
    SetFilePointerEx = k32.SetFilePointerEx
    SetFilePointerEx.argtypes = [
        wintypes.HANDLE,
        ctypes.c_int64,
        ctypes.POINTER(ctypes.c_int64),
        wintypes.DWORD,
    ]
    SetFilePointerEx.restype = wintypes.BOOL
    CloseHandle = k32.CloseHandle

    h = CreateFileW(
        path,
        GENERIC_READ,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        None,
    )
    if h == INVALID_HANDLE_VALUE or int(h) == -1:
        raise OSError(ctypes.get_last_error(), f"CreateFile failed for {path}")

    try:
        # sector-aligned read from LBA 0 then slice PVD (sector 16 @ 2048-byte sectors)
        # Physical disks often require 512-byte alignment; read 16*2048+2048 from start.
        total = 16 * 2048 + 2048
        # round up to 4096
        total = (total + 4095) // 4096 * 4096
        buf = (ctypes.c_char * total)()
        new_pos = ctypes.c_int64(0)
        if not SetFilePointerEx(h, 0, ctypes.byref(new_pos), 0):
            raise OSError(ctypes.get_last_error(), "Seek failed")
        read = wintypes.DWORD(0)
        if not ReadFile(h, buf, total, ctypes.byref(read), None):
            raise OSError(ctypes.get_last_error(), "ReadFile failed")
        raw = bytes(buf)[: read.value]
        off = 16 * 2048
        if len(raw) < off + 2048:
            raise RuntimeError(f"short read got={len(raw)} need>={off+2048}")
        return raw[off : off + 2048]
    finally:
        CloseHandle(h)


def volid(pvd: bytes) -> str:
    return pvd[40:72].decode("ascii", "replace").strip(" \x00")


def main() -> int:
    lines: list[str] = []
    iso_pvd = read_pvd_file(ISO)
    iso_vol = volid(iso_pvd)
    lines.append(f"ISO_VOL={iso_vol}")
    try:
        usb_pvd = read_pvd_phys(DISK)
        usb_vol = volid(usb_pvd)
        lines.append(f"USB_VOL={usb_vol}")
        lines.append(f"PVD_MATCH={iso_pvd == usb_pvd}")
        if usb_vol == "NIXAIOS515":
            lines.append("VERDICT=STICK_IS_S515")
        elif usb_vol.startswith("NIXAIOS"):
            lines.append(f"VERDICT=STICK_OTHER_NIXAIOS_{usb_vol}")
        elif usb_vol.startswith("NIXOS"):
            lines.append("VERDICT=STICK_STOCK_OR_OLD_NIXOS")
        else:
            lines.append(f"VERDICT=UNEXPECTED_VOL[{usb_vol}]")
            lines.append(f"USB_PVD_HEAD={usb_pvd[:64].hex()}")
    except Exception as e:
        lines.append(f"USB_ERR={type(e).__name__}: {e}")

    text = "\n".join(lines) + "\n"
    with open(OUT, "w", encoding="utf-8") as f:
        f.write(text)
    print(text, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
