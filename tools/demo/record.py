#!/usr/bin/env python3
"""Run a demo storyboard against a live bevy_grimoire editor and record it.

usage: record.py <storyboard.json> --port <brp-port> --project <project-root> [--server <bevy_grimoire_mcp>]

The editor must already be running on the project, with BEVY_BRP_PORT set to the port. The recorder
never launches or closes the editor.

Frames come from the editor's own grimoire_capture_sequence (no screen capture). A storyboard lists
`chunks`: one capture per chunk, each at the storyboard's fps. An agent operation can only be applied
at the start of a sequence (frame 0 is the state before it), so every operation starts a new chunk.
Chunk k is due at the sum of the seconds of the chunks before it. The person's input is injected over
bevy_fairy/input on its own schedule, so input and frames share one clock (t0).

Output: <project>/target/loop/demo/<name>/events.json, the timeline render.py reads. Frame PNGs stay in
the editor's capture folder, named in events.json.

Storyboard format (JSON):
  {
    "name": "undo_place", "fps": 15,
    "setup": [{"mcp": "grimoire_frame_view", "args": {"target": {"rect": [-150, -110, 150, 110]}}}],
    "chunks": [ {"seconds": 6}, {"seconds": 4, "operation": {"name": "...", "parameters": {...}}} ],
    "steps": [
      {"at": 0.5, "cursor": [x, y]},            logical px
      {"at": 0.8, "click": [x, y]},             "mods": ["ShiftLeft"] holds keys through it; "button": "Right" is a right-click
      {"at": 1.0, "key": "KeyZ", "mods": ["SuperLeft"]},
      {"at": 2.0, "hold": "KeyD", "secs": 1.5},
      {"at": 2.5, "drag": [x0, y0, x1, y1], "secs": 1.6},   press, drag, release (snip box)
      {"at": 3.0, "wheel": {"y": 1, "n": 3, "at": [x, y]}},
      {"at": 4.0, "type": "44"},
      {"at": 5.0, "mcp": "grimoire_frame_view", "args": {...}, "caption": "agent: frames the walls"},
      {"at": 5.5, "mcp": "grimoire_apply_operation", "args": {...}, "as": "built"},
                                                saves the answer's operation_id; "$built" anywhere in a
                                                later step's args is replaced by it (note_resolve's ops)
      {"at": 6.0, "caption": "person: ..."}
      {"at": 7.0, "write": {"path": "assets/grimoire/standards.ron", "text": "..."}}
                                                a person saving a project file (path under --project),
                                                for a live reload; the editor reads it, nothing is injected.
                                                "$now_ms" in the text is the time of the write in Unix
                                                milliseconds (a game's verdict is stamped with when it
                                                decided); the path's directory is made if it is missing
    ]
  }
Step times are seconds from the start of the chunk that covers them (see chunk_of), and each step runs
when that chunk has really started, so an overrunning chunk delays what follows it, not what it contains.
Each event then moves to the first frame captured at or after its stamp (align_events). The stamp is
kept as t_stamp. A chunk's planned_s and captured_s in events.json give its overrun.
A caption stays until the next caption step.
"""
import bisect
import argparse
import json
import os
import shutil
import subprocess
import sys
import threading
import time
import urllib.request


class McpClient:
    """One bevy_grimoire_mcp process, spoken to over its stdio transport."""

    def __init__(self, server, project, port, label):
        self.proc = subprocess.Popen(
            [server, "--project", project],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            env={**os.environ, "BEVY_BRP_PORT": str(port)},
        )
        self._id = 0
        self._send({"jsonrpc": "2.0", "id": self._next(), "method": "initialize",
                    "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                               "clientInfo": {"name": label, "version": "1"}}})
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}})

    def _next(self):
        self._id += 1
        return self._id

    def _send(self, obj):
        self.proc.stdin.write((json.dumps(obj) + "\n").encode())
        self.proc.stdin.flush()

    def _read(self, want):
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError("the MCP server closed the connection")
            reply = json.loads(line)
            if reply.get("id") == want:
                return reply

    def json_call(self, tool, args=None):
        want = self._next()
        self._send({"jsonrpc": "2.0", "id": want, "method": "tools/call",
                    "params": {"name": tool, "arguments": args or {}}})
        reply = self._read(want)
        if "error" in reply:
            raise RuntimeError(f"{tool} refused: {reply['error']}")
        result = reply["result"]
        text = next((c["text"] for c in result["content"] if c["type"] == "text"), None)
        if result.get("isError"):
            return {"refused": text}
        return json.loads(text)

    def close(self):
        self.proc.kill()


def brp(port, method, params):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(f"http://127.0.0.1:{port}/", data=body, method="POST",
                                 headers={"Content-Type": "application/json"})
    reply = json.loads(urllib.request.urlopen(req, timeout=20).read())
    if "error" in reply:
        raise RuntimeError(reply["error"])
    return reply


class Injector:
    """Person input over bevy_fairy/input, the editor's sanctioned injection path."""

    def __init__(self, port):
        self.port = port

    def send(self, **params):
        return brp(self.port, "bevy_fairy/input", params)

    def cursor(self, x, y):
        self.send(kind="Cursor", x=float(x), y=float(y))

    def clear_cursor(self):
        self.send(kind="Cursor", clear=True)

    def press(self, key):
        self.send(kind="Keyboard", key=key, action="Press")

    def release(self, key):
        self.send(kind="Keyboard", key=key, action="Release")

    def tap(self, key):
        self.send(kind="Keyboard", key=key, action="Tap")


def keep_awake(port, timeline):
    """Return a stop function. An operation applied to an idle window can leave the last
    readbacks unprocessed, so the sequence never reports done. A few pixels of cursor jitter
    keeps the window redrawing. The jitter is not recorded: the cursor's place in the overlay
    is the storyboard's own."""
    stop = threading.Event()
    cursors = [e for e in timeline.events if e.get("type") == "cursor"]
    x, y = (cursors[-1]["x"], cursors[-1]["y"]) if cursors else (700, 470)
    inj = Injector(port)

    def jitter():
        k = 0
        while not stop.is_set():
            inj.cursor(x + (k % 2) * 6, y)
            k += 1
            stop.wait(0.1)

    thread = threading.Thread(target=jitter, daemon=True)
    thread.start()

    def done():
        stop.set()
        thread.join()
        inj.cursor(x, y)

    return done


def run_chunks(storyboard, server, project, port, timeline, errors):
    """The editor's own captures, one per chunk, each started at its scheduled time."""
    client = McpClient(server, project, port, f"{storyboard['name']}_capture")
    try:
        start = 0.0
        for i, chunk in enumerate(storyboard["chunks"]):
            wait_until(timeline, start)
            args = {"label": f"demo_{storyboard['name']}_{i}", "seconds": chunk["seconds"],
                    "fps": storyboard["fps"]}
            if "operation" in chunk:
                args["operation"] = chunk["operation"]
            wall = timeline.now()
            timeline.chunk_started[i] = wall
            timeline.started[i].set()
            keep = keep_awake(port, timeline) if "operation" in chunk else None
            try:
                answer = client.json_call("grimoire_capture_sequence", args)
            finally:
                if keep:
                    keep()
            if "refused" in answer:
                raise RuntimeError(f"chunk {i} refused: {answer['refused']}")
            if answer.get("status") != "done":
                raise RuntimeError(f"chunk {i} did not finish capturing: {json.dumps(answer)}")
            with timeline.lock:
                timeline.chunks.append({
                    "index": i, "label": args["label"], "start": round(wall, 3),
                    "operation_applied_t_s": answer.get("operation_applied_t_s"),
                    "operation_result": answer.get("operation_result"),
                    "planned_s": chunk["seconds"],
                    "captured_s": round(max((f["actual_t_s"] for f in answer["frames"]), default=0.0), 3),
                })
                for f in answer["frames"]:
                    timeline.frames.append({"t": round(wall + f["actual_t_s"], 3), "path": f["path"]})
            start += chunk["seconds"]
    except Exception as exc:  # reported on the main thread after the join
        errors.append(exc)
    finally:
        client.close()


class Timeline:
    def __init__(self, n_chunks=0):
        self.t0 = time.monotonic()
        self.lock = threading.Lock()
        self.events = []
        self.frames = []
        self.chunks = []
        self.chunk_started = [None] * n_chunks
        self.started = [threading.Event() for _ in range(n_chunks)]

    def now(self):
        return time.monotonic() - self.t0

    def add(self, **event):
        with self.lock:
            self.events.append(event)


def wait_until(timeline, t):
    delay = t - timeline.now()
    if delay > 0:
        time.sleep(delay)


def chunk_of(at, seconds):
    """The chunk covering storyboard time `at`, and the offset into it. A boundary belongs to the later chunk."""
    start = 0.0
    for k, s in enumerate(seconds):
        if at < start + s or k == len(seconds) - 1:
            return k, round(at - start, 6)
        start += s


def wait_for_chunk(timeline, k, capture):
    """Block until chunk k has started. False if the capture thread ended first (it failed)."""
    while not timeline.started[k].wait(0.1):
        if not capture.is_alive():
            return False
    return True


def align_events(events, frame_times):
    """Move each event to the first frame captured at or after its stamp: that is the frame
    that shows the event. An event after the last frame stays on the last frame. The stamp
    is kept as t_stamp so the shift can be measured."""
    if not frame_times:
        return
    for e in events:
        i = bisect.bisect_left(frame_times, e["t"] - 1e-6)
        e["t_stamp"] = e["t"]
        e["t"] = frame_times[min(i, len(frame_times) - 1)]



def resolve(value, named):
    """An mcp step's args with every `"$name"` replaced by the operation id a step saved `as` name.
    A `$name` no earlier step saved is an error: sent as written, the editor would refuse it as an
    unknown id far from the step that should have saved it."""
    if isinstance(value, str) and value.startswith("$") and value[1:].isidentifier():
        if value[1:] not in named:
            raise RuntimeError(f"{value} names no operation saved by an earlier step (saved: {sorted(named)})")
        return named[value[1:]]
    if isinstance(value, list):
        return [resolve(v, named) for v in value]
    if isinstance(value, dict):
        return {k: resolve(v, named) for k, v in value.items()}
    return value


def do_step(step, timeline, inj, agent, named=None):
    named = {} if named is None else named
    kind = step.get("_kind")
    if kind == "cursor":
        x, y = step["cursor"]
        inj.cursor(x, y)
        timeline.add(t=timeline.now(), type="cursor", x=x, y=y)
    elif kind == "clear_cursor":
        inj.clear_cursor()
        timeline.add(t=timeline.now(), type="clear_cursor")
    elif kind == "click":
        # `mods` are held through the click (a Shift-click picks pieces for a note).
        x, y = step["click"]
        mods = step.get("mods", [])
        inj.cursor(x, y)
        timeline.add(t=timeline.now(), type="cursor", x=x, y=y)
        for m in mods:
            inj.press(m)
        if mods:
            timeline.add(t=timeline.now(), type="key", keys=list(mods))
            time.sleep(0.05)
        timeline.add(t=timeline.now(), type="click", x=x, y=y)
        inj.send(kind="Mouse", button=step.get("button", "Left"), action="Tap")
        if mods:
            time.sleep(0.1)
        for m in reversed(mods):
            inj.release(m)
    elif kind == "key":
        mods = step.get("mods", [])
        for m in mods:
            inj.press(m)
        timeline.add(t=timeline.now(), type="key", keys=mods + [step["key"]])
        inj.tap(step["key"])
        for m in reversed(mods):
            inj.release(m)
    elif kind == "hold":
        inj.press(step["hold"])
        timeline.add(t=timeline.now(), type="key", keys=[step["hold"]])
        time.sleep(step["secs"])
        inj.release(step["hold"])
    elif kind == "type":
        inj.send(kind="Keyboard", text=step["type"])
        timeline.add(t=timeline.now(), type="text", text=step["type"])
    elif kind == "wheel":
        w = step["wheel"]
        x, y = w["at"]
        inj.cursor(x, y)
        timeline.add(t=timeline.now(), type="cursor", x=x, y=y)
        for _ in range(w["n"]):
            inj.send(kind="Scroll", y=float(w["y"]))
            time.sleep(0.1)
        inj.clear_cursor()
        timeline.add(t=timeline.now(), type="clear_cursor")
    elif kind == "drag":
        x0, y0, x1, y1 = step["drag"]
        inj.cursor(x0, y0)
        timeline.add(t=timeline.now(), type="cursor", x=x0, y=y0)
        inj.send(kind="Mouse", button="Left", action="Press")
        timeline.add(t=timeline.now(), type="click", x=x0, y=y0)
        for i in range(1, 9):
            t = i / 8
            x, y = round(x0 + (x1 - x0) * t), round(y0 + (y1 - y0) * t)
            inj.cursor(x, y)
            timeline.add(t=timeline.now(), type="cursor", x=x, y=y)
            time.sleep(step["secs"] / 8)
        inj.send(kind="Mouse", button="Left", action="Release")
    elif kind == "mcp":
        # Stamped when the call starts: the editor acts on the call as it arrives, and a stamp taken
        # after a slow answer would place the change seconds after the frames that show it.
        t = timeline.now()
        answer = agent.json_call(step["mcp"], resolve(step.get("args", {}), named))
        timeline.add(t=t, type="agent", tool=step["mcp"], result=json.dumps(answer)[:200])
        if "as" in step:
            if not answer.get("operation_id"):
                raise RuntimeError(f"{step['mcp']} at {step['at']} s, saved as {step['as']}, applied nothing: {answer}")
            named[step["as"]] = answer["operation_id"]
    elif kind == "write":
        # A person saving a file the editor watches. Stamped before the write, like an agent call:
        # the frames after it are the ones that show the editor reading it.
        t = timeline.now()
        os.makedirs(os.path.dirname(step["_path"]), exist_ok=True)
        with open(step["_path"], "w") as fh:
            fh.write(step["write"]["text"].replace("$now_ms", str(int(time.time() * 1000))))
        timeline.add(t=t, type="file", path=step["write"]["path"])
    elif kind == "caption":
        timeline.add(t=timeline.now(), type="caption", text=step["caption"])
    else:
        raise ValueError(f"unknown step: {step}")
    if "caption" in step and kind != "caption":
        timeline.add(t=timeline.now(), type="caption", text=step["caption"])


def kind_of(step):
    for k in ("cursor", "clear_cursor", "click", "drag", "key", "hold", "type", "wheel", "mcp", "write", "caption"):
        if k in step:
            return k
    raise ValueError(f"step has no action: {step}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("storyboard")
    ap.add_argument("--port", type=int, required=True)
    ap.add_argument("--project", required=True)
    ap.add_argument("--server", default=os.environ.get("BEVY_GRIMOIRE_MCP") or shutil.which("bevy_grimoire_mcp"))
    args = ap.parse_args()
    if not args.server:
        sys.exit("bevy_grimoire_mcp not found: pass --server or set BEVY_GRIMOIRE_MCP")

    with open(args.storyboard) as fh:
        sb = json.load(fh)
    total = sum(c["seconds"] for c in sb["chunks"])
    steps = sorted(sb["steps"], key=lambda s: s["at"])
    for s in steps:
        s["_kind"] = kind_of(s)
        if s["at"] > total:
            sys.exit(f"step at {s['at']} s is after the last chunk ends at {total} s: {s}")
        if s["_kind"] == "write":
            rel = s["write"]["path"]
            if os.path.isabs(rel) or ".." in rel.split("/"):
                sys.exit(f"a write step's path is relative to --project and stays inside it: {rel}")
            s["_path"] = os.path.join(args.project, rel)

    out_dir = os.path.join(args.project, "target/loop/demo", sb["name"])
    os.makedirs(out_dir, exist_ok=True)

    timeline = Timeline(len(sb["chunks"]))
    inj = Injector(args.port)
    agent = McpClient(args.server, args.project, args.port, f"{sb['name']}_agent")
    errors = []
    for call in sb.get("setup", []):
        agent.json_call(call["mcp"], call.get("args", {}))
    capture = threading.Thread(target=run_chunks,
                               args=(sb, args.server, args.project, args.port, timeline, errors))
    capture.start()
    try:
        seconds = [c["seconds"] for c in sb["chunks"]]
        named = {}
        for step in steps:
            k, offset = chunk_of(step["at"], seconds)
            if not wait_for_chunk(timeline, k, capture):
                break
            wait_until(timeline, timeline.chunk_started[k] + offset)
            do_step(step, timeline, inj, agent, named)
    finally:
        capture.join()
        agent.close()
    if errors:
        raise errors[0]

    frames = sorted(timeline.frames, key=lambda f: f["t"])
    align_events(timeline.events, [f["t"] for f in frames])
    record = {
        "name": sb["name"], "fps": sb["fps"], "scale": sb.get("scale", 2),
        "crop": sb.get("crop"), "width": sb.get("width", 1100), "title": sb.get("title"),
        "frames": frames, "chunks": timeline.chunks,
        "events": sorted(timeline.events, key=lambda e: e["t"]),
    }
    path = os.path.join(out_dir, "events.json")
    with open(path, "w") as fh:
        json.dump(record, fh, indent=1)
    print(f"{len(frames)} frames, {len(timeline.events)} events -> {path}")


if __name__ == "__main__":
    main()
