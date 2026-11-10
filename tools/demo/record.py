#!/usr/bin/env python3
"""Run a demo storyboard against a live bevy_grimoire editor and record it.

usage: record.py <storyboard.json> --port <brp-port> --project <project-root> [--server <bevy_cauldron_mcp>]

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
    "setup": [{"key": "KeyJ", "mods": ["ControlLeft"]},   a chord before the capture (here: collapse the terminal)
              {"stage": "settings"},                    put the shell on one panel before the capture starts,
                                                        through grimoire/stage_panel (the editor's own catalog)
              {"mcp": "grimoire_frame_view", "args": {"target": {"rect": [-150, -110, 150, 110]}}}],
    "chunks": [ {"seconds": 6}, {"seconds": 4, "operation": {"name": "...", "parameters": {...}}},
                {"seconds": 0.0667, "stage": "place"} ],   one chunk PER staged panel, staged before it starts
    "steps": [
      {"at": 0.5, "stage": "place"},            stage a panel from inside the take (a chunk boundary, so the
                                                frame after it shows the new panel)
      {"at": 0.5, "cursor": [x, y]},            logical px
      {"at": 0.8, "click": [x, y]},             "mods": ["ShiftLeft"] holds keys through it; "button": "Right" is a right-click
      {"at": 0.9, "click_label": "sofa"},       a widget found by the words on it: the recorder asks
                                                BRP's `world.query` where its `Text` is drawn and clicks
                                                that centre. Use it instead of a guessed pixel — a
                                                coordinate drifts the moment a bar above it is added.
                                                "within": [x0, y0, x1, y1] keeps only widgets inside that
                                                rectangle: a palette row and a map piece can share a word
                                                (`sofa` is both), and the rectangle says which one.
                                                "prefix": true matches a widget whose words MOVE — the
                                                status band's job item counts up (`Baking previews 3 of
                                                40`), so no storyboard can write the exact words.
      {"at": 1.0, "key": "KeyZ", "mods": ["SuperLeft"]},
      {"at": 2.0, "hold": "KeyD", "secs": 1.5},
      {"at": 2.5, "drag": [x0, y0, x1, y1], "secs": 1.6},   press, drag, release (snip box)
      {"at": 3.0, "wheel": {"y": 1, "n": 3, "at": [x, y]}},
      {"at": 3.2, "scroll_to": "6:00", "over": [x, y], "region": [x0, y0, x1, y1]},
                                                wheel over (x, y) until the editor says those words are
                                                inside the region (the panel's rect), then let go
      {"at": 3.5, "drag_divider": -90, "secs": 1.6},   drag the shell's split divider by dx px, found by BRP
      {"at": 4.0, "type": "44"},
      {"at": 4.5, "drop_file": "/tmp/crate.glb"},   a file dropped on the window, as the window system reports it
                                                (a `FileDragAndDrop::DroppedFile` over BRP: no real drag can be made
                                                under Xvfb)
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
    """One bevy_cauldron_mcp process, spoken to over its stdio transport."""

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


# **The editor's own capture rate floor and ceiling**, frames a second. `sequencer::MIN_FPS`/`MAX_FPS` refuse a rate
# outside this range, so a plan is refused here rather than by a capture that fails halfway through a take.
MIN_FPS = 2
MAX_FPS = 15


def brp(port, method, params):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(f"http://127.0.0.1:{port}/", data=body, method="POST",
                                 headers={"Content-Type": "application/json"})
    reply = json.loads(urllib.request.urlopen(req, timeout=20).read())
    if "error" in reply:
        raise RuntimeError(reply["error"])
    return reply


def stage_a_panel(port, panel):
    """**Put the shell on one panel, through the editor's own catalog** (UI-92, #258).

    The answer names the screen it staged and, for a workbench panel, the panel; a name the editor does not
    have is refused there, with the list it does have, so a storyboard's typo is a refusal rather than a take
    that quietly films the panel before it."""
    reply = brp(port, "grimoire/stage_panel", {"panel": panel})
    answer = reply.get("result", reply)
    if "refused" in answer or answer.get("error"):
        raise RuntimeError(f"stage_panel refused `{panel}`: {answer}")
    return answer


def staged_panels(port):
    """Every panel a capture can be staged on, in the editor's own order, and the screen it is on now."""
    reply = brp(port, "grimoire/stage_panels", {})
    return reply.get("result", reply)


def plan_staged_panels(name, fps, panels):
    """**One chunk and one frame per staged panel** (UI-92, #258).

    A clip of every panel is a capture per panel, because a capture starts from the state the editor was in
    when the sequence began: staging two panels inside one sequence would film the second from whatever frame
    the first had already passed. So the plan is one chunk of a single frame per panel, each chunk staging its
    panel before its capture starts. The frame count is therefore the panel count, which the run checks rather
    than assumes.

    **`fps` is the editor's range, not any number.** `grimoire_capture_sequence` refuses a rate outside
    `MIN_FPS..=MAX_FPS`, so a plan below or above it is refused here rather than by a capture that fails halfway
    through a take."""
    if not MIN_FPS <= fps <= MAX_FPS:
        raise ValueError(f"a capture runs between {MIN_FPS} and {MAX_FPS} frames a second; {fps} is outside that")
    if not panels:
        raise ValueError("the editor named no panel to stage")
    return {
        "name": name,
        "fps": fps,
        "setup": [],
        "chunks": [{"seconds": 1.0 / fps, "stage": panel} for panel in panels],
        "steps": [],
    }


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
            if "stage" in chunk:
                # **Before the sequence starts**, so frame 0 is the staged panel and not the one before it, and stamped on
                # this chunk's own clock — `wall` is this chunk's start, which is only known below.
                staged = stage_a_panel(port, chunk["stage"])
                timeline.add(t=timeline.now(), type="stage", panel=chunk["stage"], screen=staged.get("screen"))
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
            if not answer["frames"]:
                # **A take of nothing is not a take.** The plan asked for a frame per panel, and the editor that answered
                # with none leaves a clip of the panel before it or of nothing at all.
                raise RuntimeError(f"chunk {i} planned {max(int(chunk['seconds'] * storyboard['fps']), 1)} frame(s) and captured none")
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


def widget_centre(port, label, within=None, prefix=False, bounded=True):
    """Where the interface draws the visible `Text` node whose words are `label`, in logical pixels.

    **Asks the running editor, never a guess.** `world.query` returns every `Text` with its
    `ComputedNode` (size, physical pixels) and `UiGlobalTransform` (its node's centre, same space); the
    click target is the middle of the text's own rect, which is inside the row, tab or button the words
    sit on — a click there lands on whatever `Pickable` ancestor the words belong to, exactly as a
    person's would.
    """
    want = label.strip()
    reply = brp(port, "world.query", {
        "data": {"components": ["bevy_ui::widget::text::Text",
                                "bevy_ui::ui_node::ComputedNode",
                                "bevy_ui::ui_transform::UiGlobalTransform"]},
    })
    rows = reply.get("result", reply)
    for row in rows:
        comps = row.get("components", {})
        text = comps.get("bevy_ui::widget::text::Text")
        node = comps.get("bevy_ui::ui_node::ComputedNode")
        at = comps.get("bevy_ui::ui_transform::UiGlobalTransform")
        if not text or not node or not at:
            continue
        # **`Text` reflects as its bare string** (`'Turn left'`), not as an object with a field.
        #
        # **`prefix` is for a widget whose words move as its work does.** The status band's job item
        # reads `Baking previews 3 of 40` and the number climbs every second, so no storyboard can
        # write the exact words down; a prefix names the widget and lets the number be whatever it is.
        # It is opt-in, because exact matching is what stops a storyboard aiming at `floor` when it
        # said `floor_light` — the `test_the_exact_words_are_matched` case.
        if not isinstance(text, str):
            continue
        words = text.strip()
        if not (words.startswith(want) if prefix else words == want):
            continue
        # **`UiGlobalTransform` reflects as an affine array** `[a, b, c, d, tx, ty]`, not as an object
        # with a `translation`: reading it as an object skipped every row and the step reported "no
        # visible widget" for a palette of 394 texts.
        if not isinstance(at, list) or len(at) < 6:
            continue
        if not isinstance(node, dict):
            continue
        size = node.get("size")
        if not isinstance(size, list) or len(size) < 2 or size[0] <= 0 or size[1] <= 0:
            continue
        # **The translation is ALREADY in the space `bevy_fairy/input` reads — do not scale it.**
        #
        # This multiplied by `inverse_scale_factor` (0.833 at the editor's 1.2 UI scale) on the theory
        # that `UiGlobalTransform` reports physical pixels. It does not: measured on a live 1280 x 720
        # window, the status band's `labeler: not configured` text reports raw y = 705.5, which is
        # inside that window and would be y = 587.5 if it were physical. So the multiplication moved
        # every click 17% up and left — off the window for the palette, whose rows sit at x ~ 1122 —
        # and it is why `click_label` never landed a click anywhere.
        #
        # Proved on the running editor: clicking the raw centre of five palette rows armed exactly
        # those five pieces (`pipe`, `pipe_corner`, `toilet_roll_holder`, `towel_holder`, `bath`).
        x, y = at[4], at[5]
        # **A node dragged out of its panel is not a target.** The palette scrolls, so most of its rows
        # sit at y = 1900 in a 720 px window; clicking there lands off the window and the take fails
        # with a click that changed nothing. A word whose centre is outside the window is not the
        # widget the storyboard means, and saying so is better than aiming at nothing.
        if bounded and not (0.0 <= x <= 1280.0 and 0.0 <= y <= 720.0):
            continue
        # **`within` says which widget of that name.** A palette row and a placed piece can carry the
        # same word — `sofa` is both — and the rectangle is the storyboard's statement of which one it
        # means, rather than an accident of entity order.
        if within:
            x0, y0, x1, y1 = within
            if not (x0 <= x <= x1 and y0 <= y <= y1):
                continue
        return (x, y)
    raise RuntimeError(f"click_label: no visible widget says {want!r}")


def scroll_to(port, inj, label, at, region=None, prefix=False, max_notches=40, pause=0.15):
    """Wheel over `at` until the words `label` are drawn inside `region`, then return their centre.

    **A storyboard names a widget and the panel to find it in, never a notch count** (#282). The
    inspector's content is taller than the window and the wheel scrolls it by whole lines, so "12
    notches" was right for one window height and put the buttons in the top bar (y = 6) at the next:
    the following `click_label` clicked the bar's `kits & maps` and the editor left the project. The
    editor reports where the words are even when they are off the window (`bounded=False`), so this
    reads that after every notch and stops the moment they are inside `region` — `[x0, y0, x1, y1]`,
    the window by default; give the panel's own rect, because a widget scrolled under a bar is
    "inside the window" and still cannot be clicked.

    The wheel is over `at` (a point on the panel's *content*, not its scrollbar lane: measured on big
    2026-11-03, a wheel over the lane scrolled the other dock). A widget below the region is reached
    by notches of `y = -1`, one above it by `y = +1`.
    """
    x0, y0, x1, y1 = region or [0.0, 0.0, 1280.0, 720.0]
    inj.cursor(*at)
    try:
        for _ in range(max_notches + 1):
            x, y = widget_centre(port, label, None, prefix, bounded=False)
            if x0 <= x <= x1 and y0 <= y <= y1:
                return (x, y)
            inj.send(kind="Scroll", y=-1.0 if y > y1 else 1.0)
            time.sleep(pause)
    finally:
        inj.clear_cursor()
    raise RuntimeError(f"scroll_to: {label!r} never came inside {region or 'the window'} in {max_notches} notches")


def divider_centre(port):
    """Where the shell's split divider is drawn, in the input space, from BRP's `world.query`."""
    rows = brp(port, "world.query", {
        "data": {"components": ["bevy_ui::ui_transform::UiGlobalTransform",
                                "bevy_gooey::split::SplitDivider"]},
    }).get("result", [])
    for row in rows:
        at = row.get("components", {}).get("bevy_ui::ui_transform::UiGlobalTransform")
        if isinstance(at, list) and len(at) >= 6:
            return (at[4], at[5])
    raise RuntimeError("drag_divider: the editor reports no split divider")


def do_step(step, timeline, inj, agent, named=None):
    named = {} if named is None else named
    kind = step.get("_kind")
    if kind == "drag_divider":
        # **The shell's divider is found by asking the editor, never by a pixel** (#224): its position
        # depends on the width the inspector's panel asks for, which a layout change moves. The drag
        # then runs as any other `drag` does, from the divider's centre by `drag_divider` px along x.
        x, y = divider_centre(inj.port)
        step = {**step, "drag": [x, y, x + step["drag_divider"], y]}
        kind = "drag"
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
    elif kind == "click_label":
        # **A widget found by its words, then clicked where it is drawn.** The editor reports every
        # `Text` node's rect through BRP's `world.query`, so the click lands on the row a storyboard
        # means however far up or down the layout has moved: two passes lost takes to a bar inserted
        # above the palette, which shifted every guessed pixel by its height.
        x, y = widget_centre(inj.port, step["click_label"], step.get("within"), step.get("prefix", False))
        inj.cursor(x, y)
        timeline.add(t=timeline.now(), type="cursor", x=x, y=y)
        timeline.add(t=timeline.now(), type="click", x=x, y=y)
        inj.send(kind="Mouse", button=step.get("button", "Left"), action="Tap")
    elif kind == "scroll_to":
        x, y = scroll_to(inj.port, inj, step["scroll_to"], step["over"], step.get("region"), step.get("prefix", False))
        timeline.add(t=timeline.now(), type="cursor", x=x, y=y)
        timeline.add(t=timeline.now(), type="clear_cursor")
    elif kind == "drop_file":
        # **A file dropped on the window.** The window system sends `FileDragAndDrop::DroppedFile`; under Xvfb
        # no drag can be made, so the same message is written through BRP, naming the primary window.
        window = brp(inj.port, "world.query", {"data": {"components": []}, "filter": {"with": ["bevy_window::window::PrimaryWindow"]}})
        rows = window.get("result", window)
        if not rows:
            raise RuntimeError("drop_file: the editor has no primary window")
        brp(inj.port, "world.write_message", {"message": "bevy_window::event::FileDragAndDrop",
                                              "value": {"DroppedFile": {"window": rows[0]["entity"], "path_buf": step["drop_file"]}}})
        timeline.add(t=timeline.now(), type="drop", path=step["drop_file"])
    elif kind == "stage":
        # **The editor's own catalog, never a guessed pixel at a tab.** `grimoire/stage_panel` selects the
        # panel's tab (or moves to its screen), so a clip of a panel starts on the panel itself however the
        # shell's layout has been rearranged.
        answer = stage_a_panel(inj.port, step["stage"])
        timeline.add(t=timeline.now(), type="stage", panel=step["stage"], screen=answer.get("screen"))
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
        # **The first move lands within 100 ms of the press, the rest spread over `secs`.**
        #
        # `check.py` tells a drag's press from a click by looking for a cursor move strictly later than
        # the press and no more than `0.1 s` after it. A move 150 ms out (the old `secs / 8` on a 1.2 s
        # box) is therefore *not* a drag: the press was classified as a `place` change moment and the
        # check measured the cursor ring, not the box — hero failed at 1.63 %, 3.23 %, 4.17 % and 4.98 %
        # against 5 % across four takes whose boxes were drawn correctly. A 30 ms first gap is inside
        # the window a real drag has, and the remaining moves keep the drag's shape.
        time.sleep(0.03)
        for i in range(1, 9):
            t = i / 8
            x, y = round(x0 + (x1 - x0) * t), round(y0 + (y1 - y0) * t)
            if i > 1:
                time.sleep(step["secs"] / 8)
            inj.cursor(x, y)
            timeline.add(t=timeline.now(), type="cursor", x=x, y=y)
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
    elif kind == "settle":
        # **A blended change has landed.** Some editor changes are not a step: `bevy_elements` fades
        # the sky to a new clock over about a third of a second, so the picture at the operation's own
        # timestamp is still the old one. `check.py` scores a change against the frames a fifth of a
        # second after its moment, which is right for a keypress and too early for a fade. A storyboard
        # that knows a fade is coming says when it has arrived, and that is a change moment of its own.
        timeline.add(t=timeline.now(), type="settle", what=step["settle"], span=step.get("span", 0.0))
    elif kind == "caption":
        timeline.add(t=timeline.now(), type="caption", text=step["caption"])
    else:
        raise ValueError(f"unknown step: {step}")
    if "caption" in step and kind != "caption":
        timeline.add(t=timeline.now(), type="caption", text=step["caption"])


def kind_of(step):
    # **`click_label` first, and it is a separate key rather than a spelling of `click`.** The list
    # did not carry it, so a storyboard's `click_label` step matched nothing and fell through to
    # `caption`: the words were printed on the take and no click was ever sent, which reads from
    # `events.json` as a storyboard that only had captions. It went unnoticed because no storyboard
    # used it until the status band's job item — a widget whose words move, so a guessed pixel cannot
    # reach it.
    for k in ("scroll_to", "drag_divider", "click_label", "stage", "cursor", "clear_cursor", "click", "drag", "key", "hold", "type", "drop_file", "wheel", "mcp", "write", "settle", "caption"):
        if k in step:
            return k
    raise ValueError(f"step has no action: {step}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("storyboard")
    ap.add_argument("--port", type=int, required=True)
    ap.add_argument("--project", required=True)
    ap.add_argument("--server", default=os.environ.get("BEVY_GRIMOIRE_MCP") or shutil.which("bevy_cauldron_mcp"))
    args = ap.parse_args()
    if not args.server:
        sys.exit("bevy_cauldron_mcp not found: pass --server or set BEVY_GRIMOIRE_MCP")

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

    inj = Injector(args.port)
    agent = McpClient(args.server, args.project, args.port, f"{sb['name']}_agent")
    errors = []
    for call in sb.get("setup", []):
        # **A chord before the capture starts**, so a demo can begin from a state the person reaches with a
        # key (the terminal collapsed with Ctrl+J, say) without a frame of the change in the clip.
        if "key" in call:
            mods = call.get("mods", [])
            for mod in mods:
                inj.press(mod)
            inj.tap(call["key"])
            for mod in reversed(mods):
                inj.release(mod)
            time.sleep(call.get("settle", 1.0))
            continue
        if "stage" in call:
            stage_a_panel(args.port, call["stage"])
            time.sleep(call.get("settle", 1.0))
            continue
        agent.json_call(call["mcp"], call.get("args", {}))
    # **The clock starts after the setup**, so a setup that takes time (a chord and its settle, an MCP call)
    # is not a frozen first frame at the head of the clip: every frame and step is timed from here.
    timeline = Timeline(len(sb["chunks"]))
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
