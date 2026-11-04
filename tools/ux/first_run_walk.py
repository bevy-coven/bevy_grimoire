#!/usr/bin/env python3
"""The first-run walk (#304): the 2026-11-04 audit, as a standing check.

Starts Cauldron under its own Xvfb with a scratch HOME and a copy of the example project, walks a
newcomer's first minutes, screenshots every step and asserts by BRP text and state, never by pixels:

  1. the first-run home (a welcome with Open the example)
  2. Open the example: the editor on its map, in the same process
  3. place one piece: at most two clicks after the map opens
  4. back to the project page
  5. a new map from the page
  6. back to the Projects home
  7. New project, Starter: place a piece
  8. New project, Empty: Add meshes with a fixture .glb (dropped on the window)

Every step checks that the process answering BRP is the one that started.

Usage (from the repository root; needs Xvfb and a built binary):

  python3 tools/ux/first_run_walk.py --bin <path to bevy_cauldron> [--screen 1280x720]

The scratch folder (screenshots, the editor's log) is kept and printed: view every screenshot.

It is never run by the push gate (it needs a display). Run it by hand at the end of a phase; see
tools/ux/README.md.
"""

import argparse
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools" / "demo"))
import record  # noqa: E402  (the storyboard recorder's Injector and widget_centre)

# How long the editor may take to answer BRP after launch, and a state change to land, in seconds.
BOOT_S = 120
SETTLE_S = 30
POLL_S = 0.5
# The audit's promise: a first placement takes at most this many clicks once the map is open.
FIRST_PLACE_CLICKS = 2
# Offsets from the viewport's centre, in window px, tried in order when looking for empty floor.
EMPTY_PROBE_STEPS = (0, 60, -60, 120, -120)
# The inspector's under-cursor row carries this chord exactly while a piece is under the cursor.
PIECE_UNDER_CURSOR = "Ctrl+Del"
# The mesh dropped into the Empty project: one of the example's self-contained pieces.
FIXTURE_GLB = REPO / "assets" / "low_poly_furniture"


class WalkFailed(Exception):
    pass


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def free_display():
    for n in range(140, 200):
        if not Path(f"/tmp/.X11-unix/X{n}").exists() and not Path(f"/tmp/.X{n}-lock").exists():
            return n
    raise WalkFailed("no free X display between :140 and :199")


def brp(port, method, params=None):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(f"http://127.0.0.1:{port}/", data=body, headers={"Content-Type": "application/json"})
    reply = json.loads(urllib.request.urlopen(req, timeout=30).read())
    if "error" in reply:
        raise WalkFailed(f"{method}: {reply['error']}")
    return reply.get("result", reply)


def texts(port):
    rows = brp(port, "world.query", {"data": {"components": ["bevy_ui::widget::text::Text", "bevy_ui::ui_node::ComputedNode"]}})
    out = []
    for r in rows:
        c = r.get("components", {})
        t, n = c.get("bevy_ui::widget::text::Text"), c.get("bevy_ui::ui_node::ComputedNode")
        if isinstance(t, str) and isinstance(n, dict) and all(v > 0 for v in n.get("size", [0, 0])):
            out.append(t.strip())
    return out


def state(port):
    return brp(port, "grimoire/state", {})


def screen(port):
    return state(port).get("screen")


def wait_for(what, check, limit=SETTLE_S):
    t0 = time.time()
    last = None
    while time.time() - t0 < limit:
        try:
            last = check()
            if last:
                return last
        except (WalkFailed, OSError, ValueError) as e:
            last = e
        time.sleep(POLL_S)
    raise WalkFailed(f"never got there: {what} (last: {last!r})")


def placements(port):
    m = state(port).get("map") or {}
    n = m.get("placements")
    return len(n) if isinstance(n, list) else n


class Walk:
    def __init__(self, args):
        self.args = args
        self.tmp = Path(tempfile.mkdtemp(prefix="cauldron-walk-"))
        self.shots = self.tmp / "shots"
        self.shots.mkdir()
        self.port = free_port()
        self.display = free_display()
        self.procs = []
        self.pid = None
        self.inj = record.Injector(self.port)
        self.step = 0
        self.results = []

    # ── process ───────────────────────────────────────────────────────────────────────────────
    def start(self):
        w, h = self.args.screen.split("x")
        xvfb = subprocess.Popen(["Xvfb", f":{self.display}", "-screen", "0", f"{w}x{h}x24"],
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.procs.append(xvfb)
        home = self.tmp / "home"
        example = self.tmp / "example"
        (home / ".local" / "share").mkdir(parents=True)
        (home / ".config").mkdir(parents=True)
        (home / "Documents").mkdir(parents=True)
        example.mkdir()
        shutil.copytree(REPO / "assets", example / "assets", symlinks=False)
        env = dict(os.environ, DISPLAY=f":{self.display}", HOME=str(home),
                   XDG_DATA_HOME=str(home / ".local" / "share"), XDG_CONFIG_HOME=str(home / ".config"),
                   XDG_DOCUMENTS_DIR=str(home / "Documents"),
                   GRIMOIRE_EXAMPLE=str(example), BEVY_BRP_PORT=str(self.port))
        env.pop("GRIMOIRE_FULLSCREEN", None)
        log = open(self.tmp / "editor.log", "w")
        app = subprocess.Popen([self.args.bin], cwd=self.tmp, env=env, stdout=log, stderr=subprocess.STDOUT)
        self.procs.append(app)
        self.pid = app.pid
        wait_for("the editor answers BRP", lambda: brp(self.port, "rpc.discover") is not None, BOOT_S)

    def stop(self):
        for p in reversed(self.procs):
            if p.poll() is None:
                p.send_signal(signal.SIGTERM)
                try:
                    p.wait(10)
                except subprocess.TimeoutExpired:
                    p.kill()

    def same_process(self):
        app = self.procs[-1]
        if app.poll() is not None:
            raise WalkFailed(f"the editor exited (code {app.returncode}); log: {self.tmp / 'editor.log'}")
        return True

    # ── input ─────────────────────────────────────────────────────────────────────────────────
    def click_label(self, label, prefix=False, within=None):
        x, y = wait_for(f"a visible {label!r}", lambda: record.widget_centre(self.port, label, within, prefix))
        self.inj.cursor(x, y)
        self.inj.send(kind="Mouse", button="Left", action="Tap")
        time.sleep(0.4)

    def click_at(self, x, y):
        self.inj.cursor(x, y)
        self.inj.send(kind="Mouse", button="Left", action="Tap")
        time.sleep(0.4)

    def type_text(self, text):
        self.inj.send(kind="Keyboard", text=text)
        time.sleep(0.3)

    def key(self, key):
        self.inj.tap(key)
        time.sleep(0.3)

    def drop(self, path):
        rows = brp(self.port, "world.query", {"data": {"components": []}, "filter": {"with": ["bevy_window::window::PrimaryWindow"]}})
        if not rows:
            raise WalkFailed("no primary window to drop a file on")
        brp(self.port, "world.write_message", {"message": "bevy_window::event::FileDragAndDrop",
                                               "value": {"DroppedFile": {"window": rows[0]["entity"], "path_buf": str(path)}}})
        time.sleep(1.0)

    def shot(self, name):
        out = self.shots / f"{self.step:02d}-{name}"
        out.mkdir()
        brp(self.port, "grimoire/capture_sequence_start", {"label": f"walk_{self.step:02d}", "out_dir": str(out), "seconds": 0.5, "fps": 2})
        frames = wait_for(f"a screenshot of {name}", lambda: sorted(out.glob("*.png")) or None, 20)
        time.sleep(1.0)
        frames = sorted(out.glob("*.png")) or frames
        final = self.shots / f"{self.step:02d}-{name}.png"
        shutil.copy(frames[-1], final)
        shutil.rmtree(out)
        return final

    def viewport_centre(self):
        """The centre of the map's viewport node, in window pixels: the space `bevy_fairy/input` reads.

        The map is drawn through one UI node (`chrome::ViewportSlot`); its `UiGlobalTransform` is
        in the same space `record.widget_centre` clicks in.
        """
        rows = brp(self.port, "world.query", {"data": {"components": ["bevy_ui::ui_node::ComputedNode", "bevy_ui::ui_transform::UiGlobalTransform"]},
                                              "filter": {"with": ["bevy_cauldron::chrome::ViewportSlot"]}})
        for r in rows:
            c = r.get("components", {})
            at, node = c.get("bevy_ui::ui_transform::UiGlobalTransform"), c.get("bevy_ui::ui_node::ComputedNode")
            if isinstance(at, list) and isinstance(node, dict) and node["size"][0] > 0 and node["size"][1] > 0:
                return (at[4], at[5])
        raise WalkFailed("no map viewport on screen: is the editor on a map?")

    def empty_floor(self):
        """A point on the map with no piece under it, found by moving the cursor, never by clicking.

        A click on a piece with a brush armed replaces that piece, so the viewport's centre (the
        bedroom's bed) is not where a person would put a new one. The inspector names what is under the
        cursor with its delete chord; a point where no text carries that chord is empty.
        """
        cx, cy = self.viewport_centre()
        for dy in EMPTY_PROBE_STEPS:
            for dx in EMPTY_PROBE_STEPS:
                x, y = cx + dx, cy + dy
                self.inj.cursor(x, y)
                time.sleep(POLL_S)
                if not any(PIECE_UNDER_CURSOR in t for t in texts(self.port)):
                    return (x, y)
        raise WalkFailed("no empty floor near the middle of the map")

    # ── the walk ──────────────────────────────────────────────────────────────────────────────
    def check(self, name, fn):
        self.step += 1
        try:
            detail = fn()
            self.same_process()
            path = self.shot(name)
            self.results.append((self.step, name, "ok", detail, path))
            print(f"[{self.step:02d}] ok    {name}: {detail}  ({path})")
        except Exception as e:  # a failed step is reported, then the walk stops
            path = None
            try:
                path = self.shot(name + "-failed")
            except Exception:
                pass
            self.results.append((self.step, name, "FAIL", str(e), path))
            print(f"[{self.step:02d}] FAIL  {name}: {e}  ({path})")
            raise

    def place_a_piece(self):
        before = placements(self.port) or 0
        clicks = 0
        # One click on the palette's first row (its words are whatever the project names first).
        first = wait_for("a palette row", lambda: self.first_palette_row())
        self.click_at(*first)
        clicks += 1
        self.click_at(*self.empty_floor())
        clicks += 1
        wait_for("one more placement", lambda: (placements(self.port) or 0) > before)
        if clicks > FIRST_PLACE_CLICKS:
            raise WalkFailed(f"the first placement took {clicks} clicks")
        return f"{clicks} clicks, {before} -> {placements(self.port)} placements"

    def first_palette_row(self):
        rows = brp(self.port, "world.query", {"data": {"components": ["bevy_ui::ui_node::ComputedNode", "bevy_ui::ui_transform::UiGlobalTransform"]},
                                              "filter": {"with": ["bevy_cauldron::editor::PaletteRow"]}})
        best = None
        for r in rows:
            c = r["components"]
            at, node = c["bevy_ui::ui_transform::UiGlobalTransform"], c["bevy_ui::ui_node::ComputedNode"]
            if not node["size"][0] or not node["size"][1]:
                continue
            x, y = at[4], at[5]
            # A row scrolled above the list's top reports a y above the window; it is not clickable.
            if y < 0:
                continue
            if best is None or y < best[1]:
                best = (x, y)
        return best

    def run(self):
        self.start()
        self.check("first-run home", lambda: wait_for(
            "the welcome", lambda: "Open the example" in texts(self.port) and screen(self.port) == "launcher"))
        def open_example():
            self.click_label("Open the example")
            wait_for("the editor", lambda: screen(self.port) == "editor", BOOT_S)
            return state(self.port).get("map", {}).get("name")
        self.check("open the example", open_example)
        self.check("place one piece", self.place_a_piece)
        def back_to_page():
            self.click_label("‹", prefix=True)
            return wait_for("the project page", lambda: screen(self.port) == "project")
        self.check("back to the project page", back_to_page)
        def new_map():
            self.click_label("+ New map")
            self.type_text("walk map")
            self.click_label("Create")
            return wait_for("the new map in the editor", lambda: screen(self.port) == "editor" and state(self.port).get("map", {}).get("name"), BOOT_S)
        self.check("new map", new_map)
        def home():
            self.click_label("‹", prefix=True)
            wait_for("the project page", lambda: screen(self.port) == "project")
            self.click_label("Projects")
            return wait_for("the Projects home", lambda: screen(self.port) == "launcher" and "New project" in texts(self.port))
        self.check("Projects home", home)
        def new_project(template, name):
            self.click_label("New project")
            # The name field, by its hint: a person clicks where they will type.
            self.click_label("project name")
            self.type_text(name)
            if template != "Starter":
                # The template is a choice: focus it, then step to the next option.
                self.click_label("Starter")
                self.key("ArrowRight")
                wait_for(f"the {template} template", lambda: template in texts(self.port))
            self.click_label("Create")
            return wait_for(f"the {template} project's first map", lambda: screen(self.port) == "editor", BOOT_S)
        self.check("new Starter project", lambda: new_project("Starter", "walk starter"))
        self.check("place in the Starter project", self.place_a_piece)
        def back_home():
            self.click_label("‹", prefix=True)
            wait_for("the project page", lambda: screen(self.port) == "project")
            self.click_label("Projects")
            return wait_for("the Projects home", lambda: screen(self.port) == "launcher")
        self.check("back to the Projects home", back_home)
        self.check("new Empty project", lambda: new_project("Empty", "walk empty"))
        def add_meshes():
            if "Add meshes…" not in texts(self.port):
                raise WalkFailed("an empty project offers no Add meshes…")
            glb = next(FIXTURE_GLB.rglob("*.glb"))
            self.drop(glb)
            wait_for("one palette row", lambda: self.first_palette_row())
            return f"dropped {glb.name}"
        self.check("Add meshes in the Empty project", add_meshes)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", required=True, help="the bevy_cauldron binary to walk")
    ap.add_argument("--screen", default="1280x720", help="the Xvfb screen, WxH (the small case is 1024x640)")
    args = ap.parse_args()
    walk = Walk(args)
    ok = False
    try:
        walk.run()
        ok = True
    except Exception:
        pass
    finally:
        walk.stop()
    print(f"screenshots: {walk.shots}  (scratch: {walk.tmp}; delete it once reviewed)")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
