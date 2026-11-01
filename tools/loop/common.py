#!/usr/bin/env python3
"""What every script under tools/loop/ shares: the repo, the binaries, a scratch project, an editor.

Python 3 standard library only. `McpClient` and `brp` come from tools/demo/record.py rather than
being written a second time — that file is import-safe (`main()` is behind `if __name__`), and a
second MCP client would be a second answer to "how does a script talk to the editor".
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import time
import urllib.error
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]

sys.path.insert(0, str(REPO / "tools/demo"))
from record import McpClient, brp  # noqa: E402  (path set above; see the module doc)


def binary(name):
    """`target/debug/<name>`, from the workspace's own target directory.

    Read from cargo rather than assumed, because this workspace shares one target directory with
    other checkouts (`~/.cargo-target-shared/global`) and `target/debug` under the repo is then the
    wrong place to look.
    """
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=REPO, capture_output=True, text=True, check=True,
    )
    target = Path(json.loads(out.stdout)["target_directory"])
    path = target / "debug" / name
    if not path.is_file():
        sys.exit(f"{path} is not built: run `cargo build -p bevy_cauldron -p bevy_cauldron_mcp` first")
    return path


def free_port():
    """A port nothing is listening on, chosen by the kernel."""
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


# **Directories a live check never reads.** Reference art and DCC sources: no editor path opens
# them, and on a real game project they are the bulk of the tree.
SKIP_DIRS = ("third_party", "sources", ".git")


def make_scratch(source, dest):
    """A throwaway copy of a project: `dest/assets` holds what `source` holds, minus `SKIP_DIRS`.

    A project root is a directory holding `assets/grimoire`, so copying `assets` is what makes one.
    The source is only ever read — a live check that wrote to a person's project would be the one
    thing this loop must never do.
    """
    dest = Path(dest)
    if dest.exists():
        sys.exit(f"{dest} already exists; refusing to write into a directory this run did not make")
    src = Path(source)
    assets = dest / "assets"
    # **What a run reads, and not the whole tree.** Sanctuary's `assets` is 1.5 GB and the acceptance
    # touches `grimoire/` (the kit, the maps, the vocabulary — under a megabyte) and `models/` (the
    # meshes the palette draws). `third_party/` and `sources/` are 1.4 GB of reference art and
    # `.blend` files no editor path reads, and copying them filled the disk twice — `errno=28`
    # mid-run, which is a full disk and not a failure of the thing being measured.
    assets.mkdir(parents=True)
    for entry in sorted(src.iterdir()):
        if entry.name in SKIP_DIRS:
            continue
        target = assets / entry.name
        if entry.is_dir():
            shutil.copytree(entry, target, symlinks=True)
        else:
            shutil.copy2(entry, target)
    return dest


class Editor:
    """One editor process on a scratch project, closed by its own pid.

    **Closed by pid, never by name.** The person may have an editor open on their own project, and
    `pkill -f bevy_cauldron` would take theirs down with ours.
    """

    def __init__(self, project, *args, capture_log=True):
        self.project = Path(project)
        self.port = free_port()
        self.mapper = binary("bevy_cauldron")
        log = self.project / "target" / "loop" / "editor.log"
        log.parent.mkdir(parents=True, exist_ok=True)
        self.log_path = log
        self._log = open(log, "w") if capture_log else subprocess.DEVNULL
        self.proc = subprocess.Popen(
            [str(self.mapper), str(self.project), *args],
            cwd=self.project,
            env={**os.environ, "BEVY_BRP_PORT": str(self.port)},
            stdout=self._log, stderr=subprocess.STDOUT,
        )

    def alive(self):
        return self.proc.poll() is None

    def wait_ready(self, pred=lambda state: True, limit=120):
        """Block until the editor answers `grimoire/state` and `pred(state)` holds.

        The port is open before the world is: BRP answers as soon as the plugin is added, so a
        predicate on the state is what "ready" means. Anything else races the first frame.
        """
        deadline = time.time() + limit
        last = None
        while time.time() < deadline:
            if not self.alive():
                tail = ""
                try:
                    tail = self.log_path.read_text()[-2000:]
                except OSError:
                    pass
                raise RuntimeError(f"the editor exited before it was ready (see {self.log_path})\n{tail}")
            try:
                last = brp(self.port, "grimoire/state", {})["result"]
            # **A refused connection and a method that is not registered yet are both "not ready".**
            # `brp` raises `RuntimeError` for a JSON-RPC error, which at this point means the bridge is
            # up and its handlers are not — and treating that as fatal made the script give up while
            # the editor was still starting, under load, intermittently.
            except (urllib.error.URLError, ConnectionError, OSError, RuntimeError):
                time.sleep(0.25)
                continue
            if pred(last):
                return last
            time.sleep(0.25)
        raise TimeoutError(f"the editor did not reach the expected state in {limit}s; last {json.dumps(last)[:600]}")

    def close(self):
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(20)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(20)
        if self._log is not subprocess.DEVNULL:
            self._log.close()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
        return False


class Client(McpClient):
    """The demo recorder's client, plus the one call it does not make: `tools/list`."""

    def list_tools(self):
        """The names the server publishes, read the way a fresh agent reads them."""
        want = self._next()
        self._send({"jsonrpc": "2.0", "id": want, "method": "tools/list", "params": {}})
        reply = self._read(want)
        if "error" in reply:
            raise RuntimeError(f"tools/list failed: {reply['error']}")
        return [t["name"] for t in reply["result"]["tools"]]

    def text(self, tool, args=None):
        """One call, its answer as text, without parsing it as JSON.

        `grimoire_capture_still` answers with a path and a fingerprint; a tool that answers prose
        would break `json_call`, and a caller that only wants the text should not have to care.
        """
        want = self._next()
        self._send({"jsonrpc": "2.0", "id": want, "method": "tools/call",
                    "params": {"name": tool, "arguments": args or {}}})
        reply = self._read(want)
        if "error" in reply:
            return {"refused": str(reply["error"])}
        result = reply["result"]
        body = next((c["text"] for c in result["content"] if c["type"] == "text"), "")
        if result.get("isError"):
            return {"refused": body}
        try:
            return json.loads(body)
        except json.JSONDecodeError:
            return {"text": body}
