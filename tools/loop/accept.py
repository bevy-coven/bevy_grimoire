#!/usr/bin/env python3
"""Take one sentence from an agent to a presented, saved result, and say how far it got.

usage: accept.py --source <project assets dir> --out <json> [--glb <mesh to import>]

The loop's headline number. Every iteration runs this before and after its change, and `reached` is
how many of the ten steps below an outside agent completes with no project knowledge beyond the
tools' own descriptions. A count lower than the previous iteration's is a must-fix bug.

The sentence is the brief's own: *"a mossy ruined courtyard with two exits"*. The steps are the four
beats the brief names — describe, set up, inspect, present — cut where an agent makes a decision:

  1 tools    the tools an agent needs are published
  2 make_map it can make the map the sentence asks for
  3 open_map  it can start working in the map it made
  4 place     it can put pieces in it
  5 propose   it can ask the generator for a layout
  6 still     it can look at the result
  7 accept    it can keep the layout
  8 save      it can save
  9 present   it can hand the result back for review
 10 verify    the result is on disk, clean, and waiting on the person

A step whose dependency failed is `skip`, not `fail` — a step that could not be attempted has not
been shown to be broken, and counting it as broken is how a report starts blaming the wrong thing.
"""
import argparse
import json
import os
import shutil
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import Client, Editor, binary, brp, make_scratch  # noqa: E402

REQUEST = "a mossy ruined courtyard with two exits"
MAP = "courtyard"
NEEDED_TOOLS = ("grimoire_apply_operation", "grimoire_state", "grimoire_capture_still", "grimoire_present")
SOURCES = ("learned", "composed", "declared")


def summarize(steps):
    """`reached`, `of` and `first_failure` from the step records.

    Pure, and separately tested: these three numbers are what the loop grades itself on, and
    arithmetic buried in a `main()` is arithmetic nobody can check.
    """
    reached = sum(1 for s in steps if s["status"] == "pass")
    first = next((s["id"] for s in steps if s["status"] == "fail"), None)
    return {"reached": reached, "of": len(steps), "first_failure": first}


def _record(steps, step_id, status, detail, ms=None, **extra):
    row = {"id": step_id, "status": status, "ms": ms, "detail": detail}
    row.update(extra)
    steps.append(row)
    return row


class Run:
    def __init__(self, client, editor, scratch):
        self.client = client
        self.editor = editor
        self.scratch = scratch
        self.steps = []
        # **Every editor this run has started**, so `run()`'s `finally` can reap them all. Nothing
        # relaunches any more (E-078 removed the one that did), but a list survives the next step that
        # needs a second process, and an editor left behind holds this machine at load 33.
        self.editors = [editor]

    def op(self, name, params):
        """Apply one registered operation, and return its answer."""
        return self.client.json_call("grimoire_apply_operation",
                                     {"operation": name, "parameters": params, "dry_run": False})

    def ok(self, answer):
        """Whether an answer is an applied change rather than a refusal."""
        return not answer.get("refused") and answer.get("outcome") == "applied"

    def why(self, answer):
        return answer.get("refused") or answer.get("summary") or json.dumps(answer)[:300]

    # -- the steps ---------------------------------------------------------------------------

    def step_tools(self):
        t0 = time.time()
        try:
            listed = self.client.list_tools()
        except Exception as e:  # noqa: BLE001 — a broken list is the step's own failure
            return _record(self.steps, "tools", "fail", f"tools/list failed: {e}",
                           (time.time() - t0) * 1000)
        missing = [t for t in NEEDED_TOOLS if t not in listed]
        ms = (time.time() - t0) * 1000
        if missing:
            return _record(self.steps, "tools", "fail",
                           f"not published: {', '.join(missing)} (the server lists {len(listed)})", ms)
        return _record(self.steps, "tools", "pass", f"all {len(NEEDED_TOOLS)} are published", ms)

    def step_make_map(self):
        t0 = time.time()
        answer = self.op("map_create", {"name": MAP, "bounds": [12, 4, 12], "note": REQUEST})
        ms = (time.time() - t0) * 1000
        if not self.ok(answer):
            return _record(self.steps, "make_map", "fail", self.why(answer), ms)
        return _record(self.steps, "make_map", "pass", f"made `{MAP}` with the request as its note", ms)

    def step_open_map(self):
        """Can an agent start working in the map it just made — on the editor it already has?

        This step found E-067 (`door_open` takes a door and a tab, and nothing took a map's name) and
        then E-078 (`map_open` exists and applies from the menu, and the window stayed on the menu).
        Until E-078 it **relaunched the editor on the map** and passed on that, recording the relaunch
        as a `workaround` — which meant the loop's headline number measured the script rather than the
        product. The relaunch is gone: `map_open` applied on the menu now enters the editor on that
        map, and the map arriving is what the step waits for on the **same** editor.
        """
        t0 = time.time()
        answer = self.op("map_open", {"name": MAP})
        if not self.ok(answer):
            return _record(self.steps, "open_map", "fail", self.why(answer), (time.time() - t0) * 1000)
        # **The crossing is the screen's, so the screen is what is waited for.**
        #
        # A predicate on the map's name alone would pass on a build with the defect this step is
        # here to catch: `map_open` on the menu inserted `Project` and `OpenMap` and left the screen
        # at `Menu`, so `grimoire/state` reported `map.name == courtyard` with the MAPS column in
        # front of the person — measured 2026-10-25, and found by the review rather than by the
        # script. `screen` is `grimoire/state`'s own answer for exactly this, and the map's name is
        # kept beside it because entering the *wrong* map is the other way to be wrong.
        try:
            self.editor.wait_ready(
                lambda s: s.get("screen") == "editor"
                and ((s.get("map") or {}).get("name")) == MAP,
                limit=60,
            )
        except TimeoutError:
            return _record(
                self.steps, "open_map", "fail",
                "map_open applied but the editor stayed on the menu",
                (time.time() - t0) * 1000,
            )
        return _record(
            self.steps, "open_map", "pass",
            "an operation opens the map it made, and the editor entered it",
            (time.time() - t0) * 1000,
        )

    def step_place(self):
        t0 = time.time()
        listed = self.client.json_call("grimoire_list_pieces", {})
        pieces = (listed.get("pieces") or []) if isinstance(listed, dict) else []
        if len(pieces) < 2:
            return _record(self.steps, "place", "fail",
                           f"fewer than two pieces in the project ({len(pieces)})",
                           (time.time() - t0) * 1000)
        a, b = pieces[0]["id"], pieces[1]["id"]
        wanted = [(a, [0.5, 0.5]), (b, [1.5, 0.5]), (b, [0.5, 1.5]), (a, [1.5, 1.5])]
        refusals = []
        for piece, at in wanted:
            answer = self.op("placement_add", {"piece": piece, "at": at})
            if not self.ok(answer):
                refusals.append(f"{piece} at {at}: {self.why(answer)}")
        ms = (time.time() - t0) * 1000
        if refusals:
            return _record(self.steps, "place", "fail", "; ".join(refusals), ms)
        return _record(self.steps, "place", "pass", f"placed {a} and {b} in a 2 by 2 square", ms)

    def step_propose(self):
        t0 = time.time()
        tried = []
        for source in SOURCES:
            answer = self.op("layout_propose", {"source": source})
            if self.ok(answer):
                return _record(self.steps, "propose", "pass",
                               f"`{source}` proposed a layout", (time.time() - t0) * 1000,
                               source=source)
            tried.append(f"{source}: {self.why(answer)}")
        return _record(self.steps, "propose", "fail", " | ".join(tried), (time.time() - t0) * 1000)

    def step_still(self):
        t0 = time.time()
        answer = self.client.text("grimoire_capture_still", {"label": "accept-proposal"})
        ms = (time.time() - t0) * 1000
        path = answer.get("path") if isinstance(answer, dict) else None
        if answer.get("refused"):
            return _record(self.steps, "still", "fail", answer["refused"], ms)
        if not path:
            return _record(self.steps, "still", "fail",
                           f"the still answered without a path: {json.dumps(answer)[:300]}", ms)
        if not Path(path).is_file() or Path(path).stat().st_size == 0:
            return _record(self.steps, "still", "fail", f"{path} is missing or empty", ms)
        return _record(self.steps, "still", "pass", f"{Path(path).name}, "
                       f"{Path(path).stat().st_size} bytes", ms, path=str(path))

    def step_accept(self):
        t0 = time.time()
        answer = self.op("layout_accept", {})
        ms = (time.time() - t0) * 1000
        if not self.ok(answer):
            return _record(self.steps, "accept", "fail", self.why(answer), ms)
        return _record(self.steps, "accept", "pass", answer.get("summary", "kept"), ms,
                       summary=answer.get("summary", ""))

    def step_save(self):
        t0 = time.time()
        answer = self.op("map_save", {})
        ms = (time.time() - t0) * 1000
        if not self.ok(answer):
            return _record(self.steps, "save", "fail", self.why(answer), ms)
        return _record(self.steps, "save", "pass", answer.get("summary", "saved"), ms)

    def step_present(self, accepted):
        t0 = time.time()
        applied = [s.get("operation_id") for s in self.steps if s.get("operation_id")]
        summary = (accepted or {}).get("detail") or "a 12 x 4 x 12 m courtyard map"
        answer = self.client.text("grimoire_present", {
            "title": MAP,
            "summary": summary,
            "specified": ["a courtyard", "two exits"],
            "inferred": ["12 x 4 x 12 m"],
            "check": [],
            "operation_ids": [i for i in applied if i],
        })
        ms = (time.time() - t0) * 1000
        if isinstance(answer, dict) and answer.get("refused"):
            return _record(self.steps, "present", "fail", answer["refused"], ms)
        return _record(self.steps, "present", "pass",
                       f"asked the person about {answer.get('presentation')}", ms)

    def step_verify(self):
        t0 = time.time()
        state = self.client.json_call("grimoire_state", {})
        ms = (time.time() - t0) * 1000
        presentation = state.get("presentation")
        if not presentation:
            return _record(self.steps, "verify", "fail", "grimoire_state carries no presentation", ms)
        if presentation.get("answer") is not None:
            return _record(self.steps, "verify", "fail",
                           f"the question was already answered: {presentation.get('answer')}", ms)
        if state.get("map", {}).get("dirty"):
            return _record(self.steps, "verify", "fail", "the map is unsaved after a save", ms)
        placed = state.get("map", {}).get("placements")
        stamps = state.get("map", {}).get("stamps")
        return _record(self.steps, "verify", "pass",
                       f"{presentation.get('id')} waiting on the person; "
                       f"{placed} placements, {stamps} stamps, clean", ms)

    def step_import(self, glb):
        """**A mesh becomes a placed piece** — the bridge from golem to the palette (S2.3).

        Runs only when `--glb` is given, so a run without a mesh reports `of: 10` and a run with one
        reports `of: 11`. The mesh is copied INTO the project first, because `piece_import` confines
        its path to the project's assets folder — which is the brief's rule for a path parameter, and
        the reason this step exists rather than an operation that takes any path on the disk.
        """
        t0 = time.time()
        stem = Path(glb).stem
        inbox = Path(self.scratch) / "assets" / "inbox"
        inbox.mkdir(parents=True, exist_ok=True)
        shutil.copy2(glb, inbox / f"{stem}.glb")
        answer = self.op("piece_import", {"glb": f"assets/inbox/{stem}.glb"})
        if not self.ok(answer):
            return _record(self.steps, "import", "fail", self.why(answer), (time.time() - t0) * 1000)
        place = self.op("placement_add", {"piece": answer.get("id") or stem, "at": [3.5, 3.5]})
        ms = (time.time() - t0) * 1000
        if not self.ok(place):
            return _record(self.steps, "import", "fail",
                           f"imported, but placing it: {self.why(place)}", ms)
        return _record(self.steps, "import", "pass", f"imported and placed `{stem}`", ms)


def run(source, out, glb=None):
    """Every step, in order, with dependencies expressed as data."""
    stamp = int(time.time())
    scratch = make_scratch(Path(source), Path(f"/tmp/grimoire_accept/run-{stamp}"))
    steps = []
    report = {"source": str(source), "scratch": str(scratch), "glb": glb,
              "load_avg_1m": os.getloadavg()[0], "request": REQUEST}

    editor = Editor(scratch)  # the menu: no door, because the map does not exist yet
    # **Every editor this run starts, so a failure closes all of them.** `Run` holds its list, and a
    # step that ever starts a second process is reaped by the same loop.
    run_ = None
    client = None
    try:
        # **Ready means the operation surface answers, not just the port.** `grimoire/project_root`
        # is registered by the bridge and answers as soon as the plugin is added, while
        # `grimoire/operation_apply` needs the world's resources — so waiting only for the former
        # raced the first frame and the first step reported a BRP timeout under load. The
        # predicate is the call the next step actually makes.
        # `grimoire/project_root` answers from the moment the bridge's plugin is added, and that is
        # this editor's own definition of "the menu is up" — the same predicate the project-root guard
        # uses. Waiting on `grimoire/operation_list` instead made the script give up after 120 s while
        # the editor was still starting: a method that is not registered yet raises on every poll, and
        # the loop treated "not yet" as "never".
        editor.wait_ready(lambda s: True)
        report["editor_port"] = editor.port
        report["ready_at_s"] = round(time.time() - stamp, 1)
        client = Client(str(binary("bevy_cauldron_mcp")), str(scratch), editor.port, "accept")
        run_ = Run(client, editor, scratch)

        run_.step_tools()
        # **`make_map` depends on nothing but the server answering.** A listing check that finds one
        # tool missing does not stop `map_create` from working, and wiring it that way was a bug in
        # this script's first version: `grimoire_present` was absent and the report said `reached 0/10`
        # when a map could be made, which is the opposite of the honest measurement this exists for.
        run_.step_make_map()
        if steps_ok(run_.steps, "make_map"):
            run_.step_open_map()
        else:
            _skip(run_.steps, "open_map", "make_map")
        for step_id, fn, dep in (("place", run_.step_place, "open_map"),
                                 ("propose", run_.step_propose, "open_map"),
                                 ("still", run_.step_still, "open_map"),
                                 ("accept", run_.step_accept, "propose"),
                                 ("save", run_.step_save, "open_map")):
            if steps_ok(run_.steps, dep):
                fn()
            else:
                _skip(run_.steps, step_id, dep)
        if steps_ok(run_.steps, "make_map"):
            accepted = next((s for s in run_.steps if s["id"] == "accept"), None)
            run_.step_present(accepted)
        else:
            _skip(run_.steps, "present", "make_map")
        if steps_ok(run_.steps, "present"):
            run_.step_verify()
        else:
            _skip(run_.steps, "verify", "present")
        if glb:
            if steps_ok(run_.steps, "open_map"):
                run_.step_import(glb)
            else:
                _skip(run_.steps, "import", "open_map")
    finally:
        if client is not None:
            client.close()
        # **Every editor this run started**, not just the one this function holds. Closing only that
        # left live processes behind — measured: nine leaked editors from failed runs, which is what
        # held the machine at load 33.
        for e in (run_.editors if run_ is not None else [editor]):
            try:
                e.close()
            except Exception:  # noqa: BLE001 — a close that fails must not hide the run's result
                pass
        if run_ is not None:
            steps = run_.steps

    summary = summarize(steps)
    report.update(summary)
    report["steps"] = steps
    out = Path(out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2))
    landed = Path(scratch) / "target" / "loop" / "accept" / "report.json"
    landed.parent.mkdir(parents=True, exist_ok=True)
    landed.write_text(json.dumps(report, indent=2))

    for step in steps:
        detail = step["detail"]
        print(f"{step['status'].upper():4} {step['id']} — {detail}")
    print(f"reached {summary['reached']}/{summary['of']}")
    return 0 if summary["reached"] == summary["of"] else 1


def steps_ok(steps, step_id):
    return any(s["id"] == step_id and s["status"] == "pass" for s in steps)


def _skip(steps, step_id, dep):
    steps.append({"id": step_id, "status": "skip", "ms": None,
                  "detail": f"not attempted: `{dep}` did not pass"})


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--source", required=True, help="the project's assets directory (read only)")
    ap.add_argument("--out", required=True)
    ap.add_argument("--glb", help="a mesh to import as a piece; adds an 11th step")
    args = ap.parse_args()
    return run(args.source, args.out, args.glb)


if __name__ == "__main__":
    sys.exit(main())
