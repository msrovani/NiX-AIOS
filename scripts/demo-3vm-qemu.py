#!/usr/bin/env python3
"""Demo manual: 3 QEMU (ISO NixAIOS) com video GTK + rede mesh partilhada.

NAO e CI. Corre no WSL com WSLg (DISPLAY=:0). Cada VM:
  - -display gtk (janela no desktop via WSLg) + virtio-gpu
  - -netdev socket,mcast=... = hub L2 partilhado por N VMs
  - serial unix socket (comandos) + monitor unix socket (screendump)

O script:
  1. espera pelo login, confirma sudo e o fim do firstboot;
  2. descobre a NIC (nomes previsiveis, nao assumir eth0), poe IP estatico;
  3. drop-in: NOSD_MESH_REQUIRE_SIG=1 + NOSD_JOB_BIND=0.0.0.0:19001;
  4. restart do nosd (gera chave -> TOFU) e espera `mesh_pinned: 2`;
  5. crdt.put/sync nos 3 e job 1->3;
  6. screenshot de cada janela; deixa as VMs a correr.

Os marcadores de shell sao SEMPRE avaliados (`$?`), nunca literais: a consola
faz eco da linha do comando e um marcador literal faria match antes da saida.
"""
import os
import re
import select
import shutil
import socket
import struct
import subprocess
import sys
import threading
import time
import zlib

ROOT = "/mnt/c/DEV/nix-aios"
ART = f"{ROOT}/docs/evidence/artifacts"
RUN = "/tmp/mesh3"
ISO = sys.argv[1] if len(sys.argv) > 1 else "/mnt/c/Users/msrov/Downloads/nixaios-minimal-051.iso"
MEM = os.environ.get("MEM", "1280")
SMP = os.environ.get("SMP", "2")
HUB = 12345
IPS = {1: "192.168.50.1", 2: "192.168.50.2", 3: "192.168.50.3"}
NOSCTL = "sudo /run/current-system/sw/bin/nosctl"
RC = re.compile(r"__RC(?:1|2|3)__=(\d+)")


def log(msg):
    print(f"[demo] {msg}", flush=True)


class UdpHub(threading.Thread):
    """Hub L2 em userspace: 1 datagrama UDP = 1 frame Ethernet.

    QEMU nao tem hub para N processos e sem root nao ha bridge/tap. Cada VM
    liga-se por `-netdev socket,udp=127.0.0.1:9000,localaddr=...:1000i`; este
    relay recebe de todas e reenvia para as outras (sem reenviar a origem).
    """

    def __init__(self, port=9000, peers=(10001, 10002, 10003)):
        super().__init__(daemon=True)
        self.port = port
        self.peers = peers
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind(("127.0.0.1", port))
        self.sock.settimeout(0.5)
        self.stop_flag = False
        self.recv_n = 0
        self.fwd_n = 0
        self.srcs = set()
        self.sample = b""

    def run(self):
        while not self.stop_flag:
            try:
                data, addr = self.sock.recvfrom(65535)
            except socket.timeout:
                continue
            except OSError:
                break
            self.recv_n += 1
            self.srcs.add(addr[1])
            if not self.sample and len(data) > 100:
                self.sample = data
            for p in self.peers:
                if p != addr[1]:
                    try:
                        self.sock.sendto(data, ("127.0.0.1", p))
                        self.fwd_n += 1
                    except OSError:
                        pass

    def stop(self):
        self.stop_flag = True


class Node:
    def __init__(self, idx):
        self.i = idx
        self.ip = IPS[idx]
        self.ser_path = f"{RUN}/ser{idx}.sock"
        self.mon_path = f"{RUN}/mon{idx}.sock"
        self.sock = None
        self.proc = None
        self.buf = b""
        self.raw = open(f"{ART}/mesh-3qemu-serial{idx}.log", "wb")
        self.iface = None

    def connect_serial(self, timeout=120):
        end = time.time() + timeout
        while time.time() < end:
            if self.proc and self.proc.poll() is not None:
                raise RuntimeError(f"qemu {self.i} saiu cedo (rc={self.proc.returncode})")
            if os.path.exists(self.ser_path):
                try:
                    s = socket.socket(socket.AF_UNIX)
                    s.connect(self.ser_path)
                    s.setblocking(False)
                    self.sock = s
                    return
                except OSError:
                    pass
            time.sleep(0.2)
        raise RuntimeError(f"serial {self.i} nao abriu")

    def pump(self, timeout=0.5):
        if not self.sock:
            return
        r, _, _ = select.select([self.sock], [], [], timeout)
        if r:
            try:
                d = self.sock.recv(65536)
            except (BlockingIOError, OSError):
                d = b""
            if d:
                self.buf += d
                self.raw.write(d)
                self.raw.flush()

    def wait_for(self, pat, timeout=120):
        end = time.time() + timeout
        needle = pat.encode()
        while time.time() < end:
            if self.proc and self.proc.poll() is not None:
                raise RuntimeError(f"qemu {self.i} morreu (rc={self.proc.returncode})")
            self.pump(0.5)
            if needle in self.buf:
                return True
        return False

    def send(self, line):
        self.sock.sendall((line + "\n").encode())

    def sh(self, cmd, timeout=60):
        """Envia cmd e espera pelo `__RCn__=<digitos>` (avaliado, nao ecoado).

        Devolve (rc, texto_da_saida) onde a saida e o que veio antes do fim.
        """
        mark = f"__RC{self.i}__"
        start = len(self.buf)
        self.send(f"{cmd}; echo {mark}=$?")
        end = time.time() + timeout
        while time.time() < end:
            if self.proc and self.proc.poll() is not None:
                raise RuntimeError(f"qemu {self.i} morreu (rc={self.proc.returncode})")
            self.pump(0.5)
            text = self.buf.decode("utf-8", "replace")
            m = RC.search(text[start:])
            if m:
                cut = text.rfind(f"{mark}=")
                return int(m.group(1)), text[start:cut]
        return -1, self.buf.decode("utf-8", "replace")[start:]


def kill_stale():
    """QEMU antigos da mesma ISO seguram 5901-5903 e o mcast: mata-os."""
    subprocess.run(["pkill", "-f", f"qemu-system.*{os.path.basename(ISO)}"],
                   capture_output=True)
    time.sleep(2)
    for i in (1, 2, 3):
        if os.path.exists(f"{RUN}/mon{i}.sock"):
            try:
                socket.socket(socket.AF_UNIX).connect(f"{RUN}/mon{i}.sock")
                raise RuntimeError(f"porta/monitor {i} ainda ocupado por QEMU antigo")
            except (ConnectionRefusedError, FileNotFoundError):
                pass


def start_nodes():
    kill_stale()
    shutil.rmtree(RUN, ignore_errors=True)
    os.makedirs(RUN, exist_ok=True)
    procs = {}
    for i in (1, 2, 3):
        args = [
            "qemu-system-x86_64",
            "-m", MEM, "-smp", SMP,
            "-cdrom", ISO, "-boot", "d",
            "-device", "virtio-gpu-pci",
            "-vga", "none",
            # VNC em vez de GTK: uma janela fechada matava o QEMU (rc=0) e
            # abortava o teste. VNC mantem a VM viva; screenshots via monitor.
            "-vnc", f"127.0.0.1:{i}",  # 5901/5902/5903
            "-monitor", f"unix:{RUN}/mon{i}.sock,server=on,wait=off",
            "-serial", f"unix:{RUN}/ser{i}.sock,server=on,wait=off",
            # UDP hub em userspace (mcast nao passa no WSL; tap exige root).
            "-netdev", f"socket,id=n0,udp=127.0.0.1:9000,localaddr=127.0.0.1:{10000 + i}",
            "-device", f"virtio-net-pci,netdev=n0,mac=52:54:00:12:34:{i:02x}",
        ]
        out = open(f"{RUN}/qemu{i}.log", "wb")
        p = subprocess.Popen(args, stdout=out, stderr=subprocess.STDOUT,
                             start_new_session=True)
        procs[i] = p
        log(f"node {i} pid={p.pid}")
        time.sleep(1.5)
    return procs


def screenshot(n):
    s = socket.socket(socket.AF_UNIX)
    s.connect(n.mon_path)
    s.settimeout(2)
    try:
        s.recv(4096)
    except socket.timeout:
        pass
    ppm = f"{RUN}/shot{n.i}.ppm"
    s.sendall(f"screendump {ppm}\n".encode())
    time.sleep(1.2)
    s.close()
    return ppm


def ppm_to_png(src, dst):
    data = open(src, "rb").read()
    if not data.startswith(b"P6"):
        raise RuntimeError("ppm invalido")
    pos = 2
    vals = []
    while len(vals) < 3:
        while pos < len(data) and data[pos:pos + 1].isspace():
            pos += 1
        if data[pos:pos + 1] == b"#":
            while pos < len(data) and data[pos] != 0x0A:
                pos += 1
            continue
        start = pos
        while pos < len(data) and not data[pos:pos + 1].isspace():
            pos += 1
        vals.append(int(data[start:pos]))
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


def wait_prompt(n, timeout=900):
    """O firstboot do ISO corre em foreground no ttyS0 e nao devolve o
    prompt. Ctrl+C interrompe o script (nosd/cage ficam em processos
    separados) e devolve a shell."""
    end = time.time() + timeout
    last_ctrlc = 0.0
    while time.time() < end:
        if n.proc and n.proc.poll() is not None:
            raise RuntimeError(f"qemu {n.i} morreu (rc={n.proc.returncode})")
        n.pump(0.5)
        if b"nixos@nixaios" in n.buf:
            return True
        if time.time() - last_ctrlc > 15:
            try:
                n.sock.sendall(b"\x03\n")
            except OSError:
                pass
            last_ctrlc = time.time()
    return False


def setup_node(n):
    ok = wait_prompt(n, timeout=900)
    log(f"node {n.i}: prompt={ok}")
    if not ok:
        raise RuntimeError(f"node {n.i}: sem prompt (ctrl-c)")
    n.send("")

    rc, out = n.sh("sudo -n true", 30)
    log(f"node {n.i}: sudo_rc={rc}")
    if rc != 0:
        raise RuntimeError(f"node {n.i}: sudo sem password")

    # firstboot: o servico corre em background; o DONE esta no log.
    end = time.time() + 700
    fb = False
    while time.time() < end:
        rc, _ = n.sh("sudo grep -q 'DONE (GUI up)' /tmp/nos-firstboot.log", 20)
        if rc == 0:
            fb = True
            break
        time.sleep(5)
    log(f"node {n.i}: firstboot_done={fb}")

    rc, out = n.sh("ls -d /nix/store/*-nosd-0.6.0-alpha.11", 30)
    log(f"node {n.i}: store_alpha11={rc == 0}")

    rc, out = n.sh("ip -br link | awk '$1!=\"lo\"{print $1; exit}'", 30)
    names = re.findall(r"\b(?:eth|ens|enp|eno)[0-9a-z]*\b", out)
    n.iface = names[-1] if names else None
    log(f"node {n.i}: iface={n.iface}")
    if not n.iface:
        raise RuntimeError(f"node {n.i}: sem NIC")

    # A ISO usa o firewall NixOS default (o nixosTest desliga-o). Sem isto o
    # HB UDP 19000 entra no NIC e e descartado; o job TCP 19001 idem.
    n.sh("sudo systemctl stop NetworkManager dhcpcd systemd-networkd firewall 2>/dev/null; true", 30)
    n.sh(f"sudo ip addr flush dev {n.iface}; sudo ip addr add {n.ip}/24 dev {n.iface}; "
         f"sudo ip link set {n.iface} up", 30)
    time.sleep(4)
    rc, out = n.sh(f"ip -4 addr show {n.iface}", 30)
    if f"inet {n.ip}/24" not in out:
        n.sh(f"sudo ip addr add {n.ip}/24 dev {n.iface}; sudo ip link set {n.iface} up", 30)
    # 255.255.255.255 (limited broadcast) precisa de rota; sem default o
    # kernel devolve ENETUNREACH e o nosd nao manda HB nenhum.
    n.sh(f"sudo ip route replace 255.255.255.255 dev {n.iface}", 30)
    n.sh(f"sudo ip route replace default dev {n.iface}", 30)
    rc, out = n.sh("ip route", 30)
    log(f"node {n.i}: routes={out.strip().splitlines()[-3:] if out.strip() else '?'}")
    n.sh("sudo mkdir -p /run/systemd/system/nosd.service.d", 30)
    n.sh("printf '[Service]\\nEnvironment=NOSD_MESH_REQUIRE_SIG=1\\n"
         "Environment=NOSD_JOB_BIND=0.0.0.0:19001\\n' | "
         "sudo tee /run/systemd/system/nosd.service.d/demo.conf >/dev/null", 30)
    n.sh("sudo systemctl daemon-reload", 30)
    n.sh("sudo systemctl restart nosd", 90)
    rc, out = n.sh("ip -4 addr show " + n.iface + " | grep -o 'inet [0-9.]*'", 30)
    log(f"node {n.i}: inet={out.strip().splitlines()[-1][:40] if out.strip() else '?'}")


def wait_pinned(n, timeout=240):
    end = time.time() + timeout
    while time.time() < end:
        rc, out = n.sh(f"{NOSCTL} health", 30)
        if '"mesh_pinned": 2' in out:
            return True, out
        time.sleep(5)
    return False, out


def main():
    if not os.path.exists(ISO):
        sys.exit(f"ISO em falta: {ISO}")
    sha = subprocess.run(["sha256sum", ISO], capture_output=True, text=True).stdout.split()[0]
    log(f"ISO={ISO} sha256={sha} mem={MEM}M smp={SMP}")
    os.makedirs(ART, exist_ok=True)
    procs = start_nodes()
    nodes = {i: Node(i) for i in (1, 2, 3)}
    for i, n in nodes.items():
        n.proc = procs[i]
    results = []
    hub = UdpHub()
    hub.start()
    error = None
    try:
        for n in nodes.values():
            n.connect_serial()
            log(f"node {n.i}: serial ligada")
        for n in nodes.values():
            setup_node(n)
        for n in nodes.values():
            ok, health = wait_pinned(n)
            rc, j = n.sh("sudo journalctl -u nosd -o cat | grep -iE 'mesh (udp|HB)|rejeitado' | tail -12", 30)
            log(f"node {n.i}: pinned2={ok} journal_mesh={len(j)}b")
            results.append(f"## node {n.i} journal mesh\n{j.strip()[-1200:]}")
            results.append(f"## node {n.i} health (pinned2={ok})\n{health.strip()[-1200:]}")
            if not ok:
                raise RuntimeError(f"node {n.i}: peers nao fixados (ver serial)")
        n1, n2, n3 = nodes[1], nodes[2], nodes[3]
        rc, out = n1.sh(f"{NOSCTL} crdt-put k1 v1", 30)
        results.append(f"## crdt-put node1 rc={rc}\n{out.strip()[-600:]}")
        rc, out = n1.sh(f"{NOSCTL} crdt-sync {n2.ip}:19001", 90)
        results.append(f"## sync 1->2 rc={rc}\n{out.strip()[-600:]}")
        rc, out = n2.sh(f"{NOSCTL} crdt-get k1", 30)
        results.append(f"## get k1 node2 rc={rc}\n{out.strip()[-600:]}")
        rc, out = n2.sh(f"{NOSCTL} crdt-sync {n3.ip}:19001", 90)
        results.append(f"## sync 2->3 rc={rc}\n{out.strip()[-600:]}")
        rc, out = n3.sh(f"{NOSCTL} crdt-get k1", 30)
        results.append(f"## get k1 node3 rc={rc}\n{out.strip()[-600:]}")
        rc, out = n1.sh(f"{NOSCTL} job-submit {n3.ip}:19001 'oi demo 3vm' --deadline-ms 10000", 90)
        results.append(f"## job 1->3 rc={rc}\n{out.strip()[-800:]}")
    except Exception as e:
        error = e
    finally:
        shots = []
        for n in nodes.values():
            try:
                if not os.path.exists(n.mon_path):
                    log(f"node {n.i}: sem monitor (qemu morto)")
                    continue
                ppm = screenshot(n)
                if os.path.exists(ppm) and os.path.getsize(ppm) > 1000:
                    png = f"{ART}/mesh-3qemu-node{n.i}-0.6.0-alpha.11.png"
                    ppm_to_png(ppm, png)
                    shots.append(png)
                    log(f"node {n.i}: screenshot {png}")
            except Exception as e:
                log(f"node {n.i}: screenshot falhou: {e}")
        summary = f"{ART}/mesh-3qemu-0.6.0-alpha.11.txt"
        with open(summary, "w", encoding="utf-8") as f:
            f.write("# Demo 3 x QEMU (ISO alpha.11) - mesh\n")
            f.write(f"# data: {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}\n")
            f.write(f"# iso: {ISO}\n# sha256: {sha}\n# mem={MEM}M smp={SMP}\n")
            f.write(f"# pids: {[p.pid for p in procs.values()]}\n")
            f.write(f"# ifaces: {{{', '.join(f'{i}: {n.iface}' for i, n in nodes.items())}}}\n")
            f.write(f"# screenshots: {shots}\n")
            f.write(f"# hub: recv={hub.recv_n} fwd={hub.fwd_n} srcs={sorted(hub.srcs)} "
                    f"sample={hub.sample[:24].hex()}\n\n")
            for r in results:
                f.write(r + "\n\n")
        if error and os.environ.get("KEEP") != "1":
            for p in procs.values():
                p.terminate()
            time.sleep(1)
            for p in procs.values():
                if p.poll() is None:
                    p.kill()
        hub.stop()
        log(f"resumo: {summary}")
        print("\n===== RESULTADOS =====")
        for r in results:
            for line in r.splitlines():
                if any(k in line for k in ('mesh_pinned', 'mesh_peers', 'entries', 'value',
                                           '"ok"', 'verified', 'received', 'sent', 'job_id')):
                    print(line.strip()[:300])
        print("======================")
    if error:
        raise error
    log(f"VMs a correr: {[p.pid for p in procs.values()]}")
    log(f"parar: kill {' '.join(str(p.pid) for p in procs.values())}")


if __name__ == "__main__":
    main()
