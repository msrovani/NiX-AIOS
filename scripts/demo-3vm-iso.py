#!/usr/bin/env python3
"""Demo 3-VM da ISO `iso-demo` (NUNCA release): mesh pre-configurada + video.

Cada VM tem duas NICs:
  - ctl  (user net, hostfwd 2201-2203 -> 22): SSH para assercoes;
  - mesh (socket,udp via relay userspace): 192.168.50.1/2/3 derivado do MAC.
A ISO demo ja traz identidade (REQUIRE_SIG), job bind, firewall aberto e
broadcast de sub-rede; nao ha configuracao por serial.

Uso: python3 scripts/demo-3vm-iso.py <iso-demo>
"""
import os
import shutil
import socket
import subprocess
import sys
import threading
import time

ROOT = "/mnt/c/DEV/nix-aios"
ART = f"{ROOT}/docs/evidence/artifacts"
RUN = "/tmp/meshdemo"
ISO = sys.argv[1] if len(sys.argv) > 1 else "/nix/store/g4g6g8gzjs7hlgffbmn4qb33dqvadvaq-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso"
IPS = {1: "192.168.50.1", 2: "192.168.50.2", 3: "192.168.50.3"}
SSH_PORT = {1: 2201, 2: 2202, 3: 2203}


def log(msg):
    print(f"[demo] {msg}", flush=True)


class Hub(threading.Thread):
    def __init__(self):
        super().__init__(daemon=True)
        self.s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.s.bind(("127.0.0.1", 9000))
        self.s.settimeout(0.5)
        self.stop_flag = False
        self.recv_n = 0
        self.fwd_n = 0

    def run(self):
        while not self.stop_flag:
            try:
                d, a = self.s.recvfrom(65535)
            except socket.timeout:
                continue
            except OSError:
                break
            self.recv_n += 1
            for p in (10001, 10002, 10003):
                if p != a[1]:
                    try:
                        self.s.sendto(d, ("127.0.0.1", p))
                        self.fwd_n += 1
                    except OSError:
                        pass

    def stop(self):
        self.stop_flag = True


def start_vms():
    shutil.rmtree(RUN, ignore_errors=True)
    os.makedirs(RUN, exist_ok=True)
    os.makedirs(ART, exist_ok=True)
    procs = {}
    for i in (1, 2, 3):
        args = [
            "qemu-system-x86_64", "-m", "1280", "-smp", "2",
            "-cdrom", ISO, "-boot", "d",
            "-device", "virtio-gpu-pci", "-vga", "none",
            "-vnc", f"127.0.0.1:{i}",
            "-monitor", f"unix:{RUN}/mon{i}.sock,server=on,wait=off",
            "-serial", f"file:{ART}/demo-3vm-serial{i}.log",
            "-netdev", f"user,id=ctl,hostfwd=tcp:127.0.0.1:{SSH_PORT[i]}-:22",
            "-device", f"virtio-net-pci,netdev=ctl,mac=52:54:00:aa:00:{i:02x}",
            "-netdev", f"socket,id=mesh,udp=127.0.0.1:9000,localaddr=127.0.0.1:{10000 + i}",
            "-device", f"virtio-net-pci,netdev=mesh,mac=52:54:00:12:34:{i:02x}",
        ]
        p = subprocess.Popen(args, stdout=open(f"{RUN}/qemu{i}.log", "wb"),
                             stderr=subprocess.STDOUT, start_new_session=True)
        procs[i] = p
        log(f"node {i} pid={p.pid} vnc=127.0.0.1:590{i} ssh=127.0.0.1:{SSH_PORT[i]}")
        time.sleep(1.5)
    return procs


def askpass():
    p = f"{RUN}/askpass.sh"
    with open(p, "w") as f:
        f.write("#!/bin/sh\necho nixaios\n")
    os.chmod(p, 0o755)
    env = dict(os.environ)
    env.update({"SSH_ASKPASS": p, "SSH_ASKPASS_REQUIRE": "force", "DISPLAY": ":0"})
    return env


SSH_ENV = None


def ssh(i, cmd, timeout=30):
    args = ["ssh", "-o", "StrictHostKeyChecking=no", "-o", "UserKnownHostsFile=/dev/null",
            "-o", "ConnectTimeout=5", "-o", "LogLevel=ERROR",
            "-p", str(SSH_PORT[i]), "root@127.0.0.1", cmd]
    r = subprocess.run(args, stdin=subprocess.DEVNULL, capture_output=True,
                       text=True, env=SSH_ENV, timeout=timeout)
    return r.returncode, (r.stdout or "") + (r.stderr or "")


def wait_ssh(i, timeout=600):
    end = time.time() + timeout
    while time.time() < end:
        try:
            socket.create_connection(("127.0.0.1", SSH_PORT[i]), timeout=1).close()
        except OSError:
            time.sleep(2)
            continue
        rc, out = ssh(i, "true", timeout=15)
        if rc == 0:
            return True
        time.sleep(2)
    return False


def screenshot(i):
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(5)
    s.connect(f"{RUN}/mon{i}.sock")
    try:
        s.recv(4096)
    except socket.timeout:
        pass
    ppm = f"{RUN}/shot{i}.ppm"
    s.sendall(f"screendump {ppm}\n".encode())
    time.sleep(1.5)
    s.close()
    return ppm


def ppm_to_png(src, dst):
    import struct
    import zlib
    data = open(src, "rb").read()
    if not data.startswith(b"P6"):
        raise RuntimeError("ppm invalido")
    pos, vals = 2, []
    while len(vals) < 3:
        while pos < len(data) and data[pos:pos + 1].isspace():
            pos += 1
        if data[pos:pos + 1] == b"#":
            while pos < len(data) and data[pos] != 0x0A:
                pos += 1
            continue
        st = pos
        while pos < len(data) and not data[pos:pos + 1].isspace():
            pos += 1
        vals.append(int(data[st:pos]))
    pos += 1
    w, h, _ = vals
    px = data[pos:pos + w * h * 3]
    raw = b"".join(b"\x00" + px[y * w * 3:(y + 1) * w * 3] for y in range(h))

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(raw, 6))
           + chunk(b"IEND", b""))
    open(dst, "wb").write(png)


def main():
    if not os.path.exists(ISO):
        sys.exit(f"ISO demo em falta: {ISO}")
    sha = subprocess.run(["sha256sum", ISO], capture_output=True, text=True).stdout.split()[0]
    log(f"ISO={ISO}\n[demo] sha256={sha}")
    hub = Hub()
    hub.start()
    procs = start_vms()
    global SSH_ENV
    SSH_ENV = askpass()
    results = []
    failure = None
    try:
        for i in (1, 2, 3):
            if not wait_ssh(i):
                raise RuntimeError(f"node {i}: SSH nao abriu")
            log(f"node {i}: ssh ok")
        # Mesh: identidade + TOFU + firewall aberto na ISO demo.
        for i in (1, 2, 3):
            end = time.time() + 240
            health = ""
            while time.time() < end:
                rc, health = ssh(i, "/run/current-system/sw/bin/nosctl health")
                if '"mesh_pinned": 2' in health:
                    break
                time.sleep(3)
            ok = '"mesh_pinned": 2' in health
            log(f"node {i}: pinned2={ok}")
            results.append(f"## node {i} health pinned2={ok}\n{health.strip()[-1600:]}")
            if not ok:
                raise RuntimeError(f"node {i}: mesh_pinned != 2")
        n1, n2, n3 = IPS[1], IPS[2], IPS[3]
        rc, out = ssh(1, "/run/current-system/sw/bin/nosctl crdt-put k1 v1")
        results.append(f"## crdt-put node1 rc={rc}\n{out.strip()[-500:]}")
        rc, out = ssh(1, f"/run/current-system/sw/bin/nosctl crdt-sync {n2}:19001", timeout=90)
        results.append(f"## sync 1->2 rc={rc}\n{out.strip()[-500:]}")
        rc, out = ssh(2, "/run/current-system/sw/bin/nosctl crdt-get k1")
        results.append(f"## get k1 node2 rc={rc}\n{out.strip()[-500:]}")
        rc, out = ssh(2, f"/run/current-system/sw/bin/nosctl crdt-sync {n3}:19001", timeout=90)
        results.append(f"## sync 2->3 rc={rc}\n{out.strip()[-500:]}")
        rc, out = ssh(3, "/run/current-system/sw/bin/nosctl crdt-get k1")
        results.append(f"## get k1 node3 rc={rc}\n{out.strip()[-500:]}")
        rc, out = ssh(1, f"/run/current-system/sw/bin/nosctl job-submit {n3}:19001 'oi demo 3vm' --deadline-ms 10000", timeout=90)
        results.append(f"## job 1->3 rc={rc}\n{out.strip()[-800:]}")
        # o firstboot sobe o cage/HUD depois das assercoes: esperar antes do
        # screendump, senao a captura e o console preto.
        for i in (1, 2, 3):
            end = time.time() + 360
            hud = ""
            while time.time() < end:
                rc2, hud = ssh(i, "systemctl is-active jarbas-cage")
                if hud.strip().endswith("active"):
                    break
                time.sleep(5)
            log(f"node {i}: jarbas-cage={hud.strip()}")
            results.append(f"## node {i} jarbas-cage: {hud.strip()}")
    except Exception as e:
        failure = e
        log(f"FALHA: {e}")
    finally:
        shots = []
        for i in (1, 2, 3):
            try:
                if not os.path.exists(f"{RUN}/mon{i}.sock"):
                    continue
                ppm = screenshot(i)
                if os.path.exists(ppm) and os.path.getsize(ppm) > 1000:
                    png = f"{ART}/demo-3vm-node{i}.png"
                    ppm_to_png(ppm, png)
                    shots.append(png)
                    log(f"node {i}: screenshot {png}")
            except Exception as e:
                log(f"node {i}: screenshot falhou: {e}")
        summary = f"{ART}/demo-3vm-iso.txt"
        with open(summary, "w", encoding="utf-8") as f:
            f.write("# Demo 3-VM da ISO demo (mesh pre-configurada)\n")
            f.write(f"# data: {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}\n")
            f.write(f"# iso: {ISO}\n# sha256: {sha}\n")
            f.write(f"# hub: recv={hub.recv_n} fwd={hub.fwd_n}\n")
            f.write(f"# screenshots: {shots}\n")
            f.write(f"# resultado: {'FALHA: ' + str(failure) if failure else 'OK'}\n\n")
            for r in results:
                f.write(r + "\n\n")
        log(f"resumo: {summary}")
        hub.stop()
        for p in procs.values():
            p.terminate()
        if failure:
            raise failure


if __name__ == "__main__":
    main()
