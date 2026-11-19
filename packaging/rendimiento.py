#!/usr/bin/env python3
"""Rendimiento de la versión optimizada (solo Linux): abrir, pasar página y memoria al leer un cómic entero.

Uso (ver docs/rendimiento.md):
  cargo build --release -p imgxplorer --features tauri/custom-protocol   # tras «pnpm build»
  python3 packaging/rendimiento.py <cómic.cbz> [segundos entre páginas = 0.4] [espera final = 0]

Usa una copia aislada de la app (otra carpeta de ajustes, datos y socket, en un directorio
temporal) y la maneja por MCP, como haría una IA. No toca tus ajustes ni tus recientes.
"""
import json, os, statistics, subprocess, sys, tempfile, time
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.environ.get("IMGX_BIN", os.path.join(ROOT, "target", "release", "imgxplorer"))
if len(sys.argv) < 2: sys.exit(__doc__)
COMIC = os.path.abspath(sys.argv[1])
PACE = float(sys.argv[2]) if len(sys.argv) > 2 else 0.4
P = tempfile.mkdtemp(prefix="imgx-rendimiento-")
env = dict(os.environ)
for k, d in [("XDG_RUNTIME_DIR", "run"), ("XDG_CONFIG_HOME", "cfg"), ("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache")]:
    p = os.path.join(P, d); os.makedirs(p, mode=0o700, exist_ok=True); env[k] = p
cfg = os.path.join(P, "cfg", "com.imgxplorer.app"); os.makedirs(cfg, exist_ok=True)
json.dump({"aiControl": True, "tourDone": True, "resume": False}, open(os.path.join(cfg, "settings.json"), "w"))
proc = subprocess.Popen([BIN, "--mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, env=env, text=True, bufsize=1)
nid = 0
def call(name, args=None):
    global nid; nid += 1
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": nid, "method": "tools/call", "params": {"name": name, "arguments": args or {}}}) + "\n"); proc.stdin.flush()
    r = json.loads(proc.stdout.readline())
    c = (r.get("result") or {}).get("content") or [{"text": json.dumps(r)}]
    return json.loads(c[0]["text"]) if c[0]["text"].startswith("{") else c[0]["text"]
def init():
    global nid; nid += 1
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": nid, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "perf", "version": "1"}}}) + "\n"); proc.stdin.flush()
    proc.stdout.readline()

def app_pids():
    """La app de esta prueba (por su XDG_CONFIG_HOME) y sus procesos de WebKit."""
    mine = []
    for pid in os.listdir("/proc"):
        if not pid.isdigit(): continue
        try:
            cmd = open(f"/proc/{pid}/cmdline", "rb").read().split(b"\0")[0].decode()
            if os.path.realpath(cmd) != os.path.realpath(BIN): continue
            if f"XDG_CONFIG_HOME={env['XDG_CONFIG_HOME']}".encode() in open(f"/proc/{pid}/environ", "rb").read():
                args = open(f"/proc/{pid}/cmdline", "rb").read().split(b"\0")
                if b"--mcp" not in args: mine.append(int(pid))
        except OSError: pass
    out = list(mine)
    for pid in os.listdir("/proc"):
        if not pid.isdigit(): continue
        try:
            st = open(f"/proc/{pid}/stat").read()
            ppid = int(st.rsplit(")", 1)[1].split()[1])
            if ppid in mine: out.append(int(pid))
        except OSError: pass
    return out
def rss_mb():
    total = 0
    for pid in app_pids():
        try:
            for l in open(f"/proc/{pid}/status"):
                if l.startswith("VmRSS:"): total += int(l.split()[1])
        except OSError: pass
    return round(total / 1024)

def wait_loaded(i, limit=10.0):
    t0 = time.time()
    while time.time() - t0 < limit:
        s = call("app_status")
        if s.get("current", {}).get("image") == i and "loadMs" in s: return s["loadMs"], time.time() - t0
        time.sleep(0.02)
    return None, limit

init()
t0 = time.time(); call("app_status"); print(f"arranque de la app (puente + app + interfaz): {time.time() - t0:.2f} s")
time.sleep(1.5)
print(f"memoria sin nada abierto: {rss_mb()} MB")
t0 = time.time()
r = call("open_source", {"path": COMIC})
t_open = time.time() - t0
ms, waited = wait_loaded(0)
print(f"abrir {os.path.basename(COMIC)}: open_source {t_open*1000:.0f} ms; primera página vista en {ms} ms (desde que la pide el visor); total ≈ {(t_open + waited)*1000:.0f} ms")
n = r.get("source", {}).get("images", 0) if isinstance(r, dict) else 0
loads, mem = [], [(0, rss_mb())]
for i in range(1, n):
    call("go_to", {"image": i})
    ms, _ = wait_loaded(i)
    loads.append(ms if ms is not None else 99999)
    if i % 25 == 0: mem.append((i, rss_mb()))
    time.sleep(PACE)
mem.append((n - 1, rss_mb()))
IDLE = float(sys.argv[3]) if len(sys.argv) > 3 else 0
if IDLE:
    for k in range(4):
        time.sleep(IDLE / 4); mem.append((f"+{(k+1)*IDLE/4:.0f}s", rss_mb()))
q = sorted(loads)
print(f"pasar página ({len(loads)} veces, ritmo {PACE} s): mediana {statistics.median(q)} ms · p90 {q[int(len(q)*0.9)]} ms · p99 {q[int(len(q)*0.99)]} ms · máx {q[-1]} ms · >50 ms: {sum(1 for x in q if x > 50)}")
print("memoria (MB) por página:", mem)
# Vuelta atrás rápida (lo ya leído puede haber salido de la caché).
back = []
for i in range(n - 2, n - 12, -1):
    call("go_to", {"image": i}); ms, _ = wait_loaded(i); back.append(ms)
print("hacia atrás (10 páginas):", back)
proc.stdin.close(); proc.wait(timeout=10)
for pid in app_pids():
    try: os.kill(pid, 15)
    except OSError: pass
