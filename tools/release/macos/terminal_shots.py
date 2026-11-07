#!/usr/bin/env python3
"""Type into a Cauldron build's terminal the way a person does, and keep a still after each step, for a person to look at.

    python3 tools/release/macos/terminal_shots.py --binary /Applications/Cauldron.app/Contents/MacOS/bevy_cauldron \\
        --project <a project root> --out /tmp/terminal-shots [--prompt 'sh-5.3$ ']

Run on the Mac (Python 3.9, standard library only). The editor runs on a scratch HOME (its own recent list, settings,
log and zsh) in a 1280x703 window, with the caret's blink off so every still shows it, and on a scratch copy of
`--project`'s `assets`; the person's own editor and files are never touched. Keys go through `bevy_fairy/input`, the
editor's injection path. A Ctrl chord is Ctrl held with the letter typed, which is what a keyboard sends: a bare
`KeyC` tap carries no character, and the terminal turns the character into the control byte.

Written for the terminal screenshots of 2026-11-06, which found the caret drifting after wide characters and a cell
measured from the letters' ink; both were fixed, and these scenarios are the ones that showed them. It asserts
nothing: the stills are the output, `NN-name.png` in scenario order. Avoid `!` inside double quotes in a scenario,
because zsh reads `!"` as history and waits for the quote.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request

# The window every still is taken in, logical px: the owner's MacBook Air, windowed.
WINDOW_FRAME = "(x: 0, y: 52, w: 1280, h: 703)"
# How long the editor gets to open its project before the first still.
SETTLE_S = 8.0
# Pauses that let a keystroke's echo reach the screen before the next one, s.
KEY_GAP_S = 0.4
TEXT_GAP_S = 0.6
SHOT_SETTLE_S = 0.8
SHOT_WRITE_S = 1.2


class Editor:
    def __init__(self, port):
        self.port = port

    def brp(self, method, params=None):
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
        req = urllib.request.Request("http://127.0.0.1:%d" % self.port, data=body,
                                     headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=15) as r:
            out = json.load(r)
        if "error" in out:
            raise RuntimeError("%s: %s" % (method, out["error"]))
        return out.get("result")

    def inp(self, **params):
        return self.brp("bevy_fairy/input", params)

    def text(self, s, pause=TEXT_GAP_S):
        self.inp(kind="Keyboard", text=s)
        time.sleep(pause)

    def key(self, name, pause=KEY_GAP_S):
        self.inp(kind="Keyboard", key=name, action="Tap")
        time.sleep(pause)

    def ctrl(self, letter, pause=0.7):
        """Ctrl plus a letter, as a keyboard sends it: Ctrl held, then the letter's character."""
        self.inp(kind="Keyboard", key="ControlLeft", action="Press")
        time.sleep(0.15)
        self.inp(kind="Keyboard", text=letter)
        time.sleep(0.15)
        self.inp(kind="Keyboard", key="ControlLeft", action="Release")
        time.sleep(pause)

    def chord(self, modifier, key, pause=0.8):
        """A modifier held over a named key: Cmd+K is `chord("SuperLeft", "KeyK")`."""
        self.inp(kind="Keyboard", key=modifier, action="Press")
        time.sleep(0.1)
        self.inp(kind="Keyboard", key=key, action="Tap")
        time.sleep(0.1)
        self.inp(kind="Keyboard", key=modifier, action="Release")
        time.sleep(pause)

    def centre_of(self, name):
        """The centre of the node with this `Name`, in surface (physical) px."""
        r = self.brp("world.query", {"data": {"components": [
            "bevy_ecs::name::Name", "bevy_ui::ui_node::ComputedNode", "bevy_ui::ui_transform::UiGlobalTransform"]}})
        for e in r:
            c = e["components"]
            if c["bevy_ecs::name::Name"] == name:
                t = c["bevy_ui::ui_transform::UiGlobalTransform"]
                return t["translation"] if isinstance(t, dict) and "translation" in t else t[4:6]
        raise RuntimeError("no node named %r" % name)

    def click(self, x, y):
        """A left click at logical window px."""
        self.inp(kind="Cursor", x=float(x), y=float(y))
        time.sleep(0.2)
        self.inp(kind="Mouse", button="Left", action="Tap")
        time.sleep(KEY_GAP_S)


def scenarios(ed, shot, scale):
    shot("00-unfocused-prompt")
    x, y = ed.centre_of("terminal body")
    ed.click(x / scale, y / scale)
    shot("01-focused-prompt")

    ed.text("echo 'Hello, Cauldron'")
    shot("02-typed-command")
    for _ in range(9):
        ed.key("ArrowLeft", pause=0.1)
    shot("03-caret-left-9")
    ed.text("big ")
    shot("04-insert-mid-line")
    ed.key("End", pause=0.3)
    for _ in range(4):
        ed.key("Backspace", pause=0.1)
    shot("05-backspace-4")
    ed.key("Enter", pause=0.8)
    shot("06-enter-output")

    ed.text("printf '\\e[31mred \\e[32mgreen \\e[1;34mbold-blue \\e[0;7m inverse \\e[0m \\e[2mdim\\e[0m plain\\n'")
    ed.key("Enter", pause=0.8)
    shot("07-colours")

    ed.text("ls -G /")
    ed.key("Enter", pause=1.0)
    shot("08-ls-columns")

    ed.text("echo 'cafe: café naïve — wide: 日本語 テスト 🎉 end'")
    shot("09-wide-typed")
    ed.key("Enter", pause=0.8)
    shot("10-wide-output")

    ed.text("echo " + " ".join("word%02d" % i for i in range(45)), pause=1.0)
    shot("11-long-line-wraps")
    ed.ctrl("c")
    shot("12-ctrl-c")

    ed.text("ls /Appl", pause=0.5)
    ed.key("Tab", pause=1.2)
    shot("13-tab-completion")
    ed.ctrl("u")
    shot("14-ctrl-u-cleared")

    ed.text("for i in 1 2 3; do echo line $i; done")
    ed.key("Enter", pause=0.8)
    shot("15-loop-output")
    ed.key("ArrowUp", pause=0.6)
    shot("16-history-up")
    ed.ctrl("u")

    ed.text("seq 1 200 | less")
    ed.key("Enter", pause=1.5)
    shot("17-less-fullscreen")
    ed.text("q", pause=0.8)
    shot("18-after-less")

    ed.text("vim -u NONE -N")
    ed.key("Enter", pause=1.5)
    ed.text("ihello from vim", pause=0.6)
    shot("19-vim-insert")
    ed.key("Escape", pause=0.6)
    shot("20-vim-normal")
    ed.text(":q!", pause=0.3)
    ed.key("Enter", pause=1.0)
    shot("21-after-vim")

    ed.chord("SuperLeft", "KeyK")
    shot("22-cmd-k-cleared")
    ed.text("echo after clear")
    ed.key("Enter", pause=0.8)
    shot("23-after-clear")

    ed.click(640, 250)
    shot("24-unfocused")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--binary", required=True, help="a bevy_cauldron executable, e.g. inside Cauldron.app")
    ap.add_argument("--project", required=True, help="a project root whose assets/ is copied, never written")
    ap.add_argument("--map", default="bedroom", help="the map to open")
    ap.add_argument("--out", required=True, help="where the stills go; must not exist")
    ap.add_argument("--port", type=int, default=15815, help="the editor's BRP port")
    ap.add_argument("--prompt", help="the scratch zsh's PROMPT; zsh's default when not given")
    args = ap.parse_args()
    if os.path.exists(args.out):
        sys.exit("%s exists; refusing to write into a directory this run did not make" % args.out)
    os.makedirs(args.out)

    home = tempfile.mkdtemp(prefix="terminal-shots-")
    data = os.path.join(home, "Library", "Application Support", "Grimoire")
    os.makedirs(data)
    with open(os.path.join(data, "recent.ron"), "w") as f:
        f.write("(version: 1, projects: [], window: Some(%s))\n" % WINDOW_FRAME)
    with open(os.path.join(data, "settings.ron"), "w") as f:
        f.write("(version: 1, terminal_cursor_blink: Some(false))\n")
    with open(os.path.join(home, ".zshrc"), "w") as f:
        f.write("PROMPT=%s\n" % json.dumps(args.prompt) if args.prompt else "")
    project = os.path.join(home, "project")
    shutil.copytree(os.path.join(args.project, "assets"), os.path.join(project, "assets"),
                    ignore=shutil.ignore_patterns("third_party", "sources", ".git"))

    env = dict(os.environ, HOME=home, ZDOTDIR=home, SHELL="/bin/zsh", BEVY_BRP_PORT=str(args.port))
    proc = subprocess.Popen([args.binary, project, args.map], env=env, cwd=project,
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    ed = Editor(args.port)
    taken = []

    def shot(name):
        time.sleep(SHOT_SETTLE_S)
        ed.brp("bevy_fairy/screenshot", {"path": os.path.join(args.out, name + ".png")})
        time.sleep(SHOT_WRITE_S)
        taken.append(name)
        print(name, flush=True)

    try:
        for _ in range(240):
            try:
                ed.brp("rpc.discover")
                break
            except (OSError, RuntimeError):
                time.sleep(0.5)
        else:
            sys.exit("the editor never answered on port %d" % args.port)
        time.sleep(SETTLE_S)
        window = ed.brp("world.query", {"data": {"components": ["bevy_window::window::Window"]},
                                        "filter": {"with": ["bevy_window::window::PrimaryWindow"]}})
        scale = window[0]["components"]["bevy_window::window::Window"]["resolution"]["scale_factor"]
        scenarios(ed, shot, scale)
        print(json.dumps({"out": args.out, "stills": len(taken)}))
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            proc.kill()
        shutil.rmtree(home, ignore_errors=True)


if __name__ == "__main__":
    main()
