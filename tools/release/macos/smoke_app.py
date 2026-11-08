#!/usr/bin/env python3
"""Drive a freshly built Cauldron.app through its own bundled MCP server and refuse a black window (#306, #307).

    python3 tools/release/macos/smoke_app.py --app dist/Cauldron.app --out dist/smoke

1. Launch the app with `open -n` and a scratch HOME, so its log, recent list and settings are its own.
2. Wait for its BRP port, then start `Contents/MacOS/bevy_cauldron_mcp` (no --project: it follows the editor) and speak
   MCP to it over stdio.
3. The log must start with this app's version; its window self-check (`startup_probe`) must report at least
   MIN_WINDOW_COLOURS colours and never "the window is black".
4. A still of the window and one of the surface, each decoded here, must both show at least MIN_WINDOW_COLOURS colours.

Any failure prints the reason, leaves the evidence in --out (reason.txt, cauldron.log, the stills, the MCP answers) and
exits 1. Standard library only, and Python 3.9 (mac_air's /usr/bin/python3).
"""

import argparse
import json
import os
import queue
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request
import zlib

SMOKE_PORT = 15791
READY_TIMEOUT_S = 120
# How long a window that has said it has not had focus is waited for before the app is launched afresh.
FOCUS_PATIENCE_S = 40
# How many launches a window that never takes focus gets, and the pause between them.
LAUNCH_ATTEMPTS = 3
RETRY_PAUSE_S = 5
MIN_WINDOW_COLOURS = 8
SAMPLE_GRID = 64
# How often the log is re-read while waiting for the window self-check.
POLL_S = 2
# How long one MCP answer may take: a capture goes through bevy_fairy's screenshot and readback.
MCP_CALL_TIMEOUT_S = 60
# How long `open -n` may take to show the new process.
PID_TIMEOUT_S = 30
# How many log lines each read asks for.
LOG_LINES = 400
LOG_REL = os.path.join("Library", "Application Support", "Grimoire", "cauldron.log")
TOOLS_NEEDED = ("grimoire_read_log", "grimoire_open_project", "grimoire_capture_still")


class SmokeFailure(Exception):
    """A reason the release must not ship, said in one sentence."""


class NeverFocused(SmokeFailure):
    """The window never took the foreground, so its pixels cannot be judged: the one failure a fresh launch can change."""


def pids_of(exe):
    out = subprocess.run(["pgrep", "-f", exe + "$"], capture_output=True, text=True).stdout
    return {int(p) for p in out.split()}


def brp_answers(port):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "rpc.discover"}).encode()
    req = urllib.request.Request("http://127.0.0.1:%d" % port, data=body, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=3) as r:
            return r.status == 200
    except (urllib.error.URLError, ConnectionError, OSError):
        return False


class Mcp:
    """Newline-delimited JSON-RPC over the server's stdio, read on a thread so a silent server is a timeout."""

    def __init__(self, exe, env, out_dir):
        self.proc = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=open(os.path.join(out_dir, "mcp-stderr.txt"), "w"), env=env)
        self.lines = queue.Queue()
        self.next_id = 0
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for line in self.proc.stdout:
            self.lines.put(line)
        self.lines.put(None)

    def send(self, body):
        self.proc.stdin.write((json.dumps(body) + "\n").encode())
        self.proc.stdin.flush()

    def request(self, method, params):
        self.next_id += 1
        want = self.next_id
        self.send({"jsonrpc": "2.0", "id": want, "method": method, "params": params})
        deadline = time.time() + MCP_CALL_TIMEOUT_S
        while True:
            left = deadline - time.time()
            if left <= 0:
                raise SmokeFailure("the bundled MCP server did not answer %s within %d s" % (method, MCP_CALL_TIMEOUT_S))
            try:
                line = self.lines.get(timeout=left)
            except queue.Empty:
                continue
            if line is None:
                raise SmokeFailure("the bundled MCP server exited while answering %s (see mcp-stderr.txt)" % method)
            msg = json.loads(line)
            if msg.get("id") == want:
                if "error" in msg:
                    raise SmokeFailure("%s was refused at the protocol level: %s" % (method, msg["error"]))
                return msg["result"]

    def call(self, name, arguments):
        return self.request("tools/call", {"name": name, "arguments": arguments})

    def close(self):
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def text_of(result):
    return "\n".join(b.get("text", "") for b in result.get("content", []) if b.get("type") == "text")


def ok(result, what):
    if result.get("isError"):
        raise SmokeFailure("%s was refused: %s" % (what, text_of(result)))
    return result


def png_pixels(path):
    """(width, height, channels, rows) of an 8-bit RGB or RGBA PNG, unfiltered. Filters 0-4, no interlace."""
    with open(path, "rb") as f:
        data = f.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SmokeFailure("%s is not a PNG" % path)
    pos, idat, header = 8, [], None
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        kind = data[pos + 4:pos + 8]
        chunk = data[pos + 8:pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", chunk)
        elif kind == b"IDAT":
            idat.append(chunk)
        elif kind == b"IEND":
            break
    if header is None:
        raise SmokeFailure("%s has no IHDR" % path)
    width, height, depth, colour, _, _, interlace = header
    channels = {2: 3, 6: 4}.get(colour)
    if depth != 8 or channels is None or interlace != 0:
        raise SmokeFailure("%s is depth %d colour type %d interlace %d; this reads 8-bit RGB(A) only" % (path, depth, colour, interlace))
    raw = zlib.decompress(b"".join(idat))
    stride = width * channels
    rows, prev = [], bytearray(stride)
    for y in range(height):
        base = y * (stride + 1)
        kind = raw[base]
        line = bytearray(raw[base + 1:base + 1 + stride])
        if kind == 1:
            for i in range(channels, stride):
                line[i] = (line[i] + line[i - channels]) & 0xFF
        elif kind == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif kind == 3:
            for i in range(stride):
                left = line[i - channels] if i >= channels else 0
                line[i] = (line[i] + ((left + prev[i]) >> 1)) & 0xFF
        elif kind == 4:
            for i in range(stride):
                a = line[i - channels] if i >= channels else 0
                b = prev[i]
                c = prev[i - channels] if i >= channels else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pred) & 0xFF
        elif kind != 0:
            raise SmokeFailure("%s has a row with filter %d" % (path, kind))
        rows.append(line)
        prev = line
    return width, height, channels, rows


def sampled_colours(path, grid):
    """Distinct RGB triples on a grid x grid lattice of cell centres, as `startup_probe::sampled_colours` samples."""
    width, height, channels, rows = png_pixels(path)
    seen = set()
    for gy in range(grid):
        y = min((gy * 2 + 1) * height // (grid * 2), height - 1)
        for gx in range(grid):
            x = min((gx * 2 + 1) * width // (grid * 2), width - 1)
            i = x * channels
            seen.add(bytes(rows[y][i:i + 3]))
    return len(seen)


def log_lines(mcp):
    result = ok(mcp.call("grimoire_read_log", {"lines": LOG_LINES}), "grimoire_read_log")
    return result.get("structuredContent", {}).get("lines", [])


def smoke(app, out_dir, home, state):
    exe = os.path.join(app, "Contents", "MacOS", "bevy_cauldron")
    mcp_exe = os.path.join(app, "Contents", "MacOS", "bevy_cauldron_mcp")
    version = subprocess.run(["plutil", "-extract", "CFBundleShortVersionString", "raw",
                              os.path.join(app, "Contents", "Info.plist")],
                             capture_output=True, text=True, check=True).stdout.strip()
    print("smoke: Cauldron %s at %s, HOME %s" % (version, app, home))

    before = pids_of(exe)
    subprocess.run(["open", "-n", "--env", "HOME=" + home, "--env", "BEVY_BRP_PORT=%d" % SMOKE_PORT, app], check=True)
    deadline = time.time() + PID_TIMEOUT_S
    while time.time() < deadline and state.get("pid") is None:
        new = pids_of(exe) - before
        if new:
            state["pid"] = max(new)
        else:
            time.sleep(0.5)
    if state.get("pid") is None:
        raise SmokeFailure("no new %s process within %d s of `open -n`" % (exe, PID_TIMEOUT_S))
    print("smoke: editor pid %d" % state["pid"])

    deadline = time.time() + READY_TIMEOUT_S
    while not brp_answers(SMOKE_PORT):
        if time.time() > deadline:
            raise SmokeFailure("BRP did not answer on 127.0.0.1:%d within %d s" % (SMOKE_PORT, READY_TIMEOUT_S))
        time.sleep(1)
    print("smoke: BRP answers on %d" % SMOKE_PORT)

    env = dict(os.environ, HOME=home, BEVY_BRP_PORT=str(SMOKE_PORT))
    mcp = Mcp(mcp_exe, env, out_dir)
    state["mcp"] = mcp
    mcp.request("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                               "clientInfo": {"name": "cauldron release smoke", "version": version}})
    mcp.send({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}})
    tools = [t["name"] for t in mcp.request("tools/list", {})["tools"]]
    missing = [t for t in TOOLS_NEEDED if t not in tools]
    if missing:
        raise SmokeFailure("the bundled MCP server lacks %s" % ", ".join(missing))

    state_answer = ok(mcp.call("grimoire_state", {}), "grimoire_state")
    with open(os.path.join(out_dir, "state.json"), "w") as f:
        f.write(text_of(state_answer))

    start = "--- Cauldron %s " % version
    if not any(l.startswith(start) for l in log_lines(mcp)):
        raise SmokeFailure("the log has no line starting %r" % start)

    deadline = time.time() + READY_TIMEOUT_S
    colours = None
    focus_deadline = time.time() + FOCUS_PATIENCE_S
    while colours is None:
        lines = log_lines(mcp)
        if any("the window is black" in l for l in lines):
            raise SmokeFailure("the editor's window self-check says the window is black")
        for l in lines:
            if "window self-check:" in l:
                m = re.search(r"(\d+) colours", l)
                if m:
                    colours = int(m.group(1))
                    print("smoke: %s" % l.strip())
        if colours is None:
            unfocused = any("window has not had focus" in l for l in lines)
            if unfocused and time.time() > focus_deadline:
                raise NeverFocused("the window never took focus within %d s: nothing can be judged; another app held the foreground" % FOCUS_PATIENCE_S)
            if time.time() > deadline:
                raise SmokeFailure("no window self-check line within %d s" % READY_TIMEOUT_S)
            time.sleep(POLL_S)
    if colours < MIN_WINDOW_COLOURS:
        raise SmokeFailure("the window self-check saw %d colours, fewer than %d" % (colours, MIN_WINDOW_COLOURS))

    for source, label in (("window", "smoke_window"), ("surface", "smoke_surface")):
        result = ok(mcp.call("grimoire_capture_still", {"label": label, "source": source}), "capture of the %s" % source)
        path = json.loads(text_of(result))["path"]
        kept = os.path.join(out_dir, "%s.png" % source)
        shutil.copyfile(path, kept)
        n = sampled_colours(kept, SAMPLE_GRID)
        print("smoke: %s still %d colours" % (source, n))
        if n < MIN_WINDOW_COLOURS:
            raise SmokeFailure("the %s still shows %d colours, fewer than %d" % (source, n, MIN_WINDOW_COLOURS))


def attempt(app, out_dir):
    """One launch of the app, driven and torn down. Returns `(failure, unfocused)`: why it must not ship
    (`None` when it passed), and whether the cause was a window that never took the foreground."""
    home = tempfile.mkdtemp(prefix="cauldron-smoke-")
    state = {}
    failure = None
    unfocused = False
    try:
        smoke(app, out_dir, home, state)
    except NeverFocused as e:
        failure, unfocused = str(e), True
    except SmokeFailure as e:
        failure = str(e)
    except Exception as e:  # anything unexpected is a failed release, with its reason kept
        failure = "the smoke itself failed: %r" % (e,)
    finally:
        log = os.path.join(home, LOG_REL)
        if os.path.exists(log):
            shutil.copyfile(log, os.path.join(out_dir, "cauldron.log"))
        elif failure is None:
            failure = "the app did not honour HOME; the smoke cannot isolate its log"
        if state.get("mcp") is not None:
            state["mcp"].close()
        if state.get("pid") is not None:
            subprocess.run(["kill", str(state["pid"])])
        shutil.rmtree(home, ignore_errors=True)
    return failure, unfocused


def run_attempts(attempt_once, attempts, pause=lambda: None, say=print):
    """Launches until one passes or fails for a reason a relaunch cannot change. **Only a window that never
    took the foreground is tried again**: macOS gives a window the foreground to an app launched by a
    background job only when no other app is in use, so a release smoke on a Mac someone is working at
    (2026-11-08, 0.4.4: run once under launchd it passed, the next launch ten seconds later never took
    focus) is flaky, and a fresh launch is the thing that has been seen to work. Any other failure is
    the app's, and is returned at once. Returns the last failure, or `None`."""
    failure = None
    for n in range(1, attempts + 1):
        failure, unfocused = attempt_once(n)
        if failure is None:
            return None
        if not unfocused or n == attempts:
            break
        say("smoke: attempt %d of %d: %s; launching a fresh app" % (n, attempts, failure))
        pause()
    return failure


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--app", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    app = os.path.abspath(args.app)
    out_dir = os.path.abspath(args.out)
    os.makedirs(out_dir, exist_ok=True)
    failure = run_attempts(lambda n: attempt(app, out_dir), LAUNCH_ATTEMPTS, pause=lambda: time.sleep(RETRY_PAUSE_S))
    if failure is not None:
        with open(os.path.join(out_dir, "reason.txt"), "w") as f:
            f.write(failure + "\n")
        print("smoke: FAILED: %s" % failure, file=sys.stderr)
        return 1
    print("smoke: passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
