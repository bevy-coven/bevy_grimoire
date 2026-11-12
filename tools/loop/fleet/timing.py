#!/usr/bin/env python3
"""Where a fleet run's time went, read from the omp sessions of its workers, subagents and manager.

`steps.py` turns ONE session into rows (model, tool, bg, wait); this reads many sessions and answers the
fleet questions: how much of a worker's time is the model, how much is waiting on cargo, how much is a
session that was simply parked, and what the push gate cost.

    timing.py --workers DIR [DIR...] --manager SESSION.jsonl [--subagents DIR]

`--workers` takes directories holding worker session JSONL (copy `~/.omp/agent/sessions/*fleet*` from every
host first; the files are found recursively). The manager session's own directory holds the subagents.

How to read it: a `model` row longer than PARK_S is **parked time**, not thinking. omp disposes a
finished session and revives it on the next message, and `steps.py` charges the whole gap to the model
(measured: a reviewer's 14.1 h "model" row was `session_exit` followed 14 h later by an IRC message). It
is a stall only when the row ends in a 429 (the worker's `errorMessage`), which the report counts apart.
Stdlib only.
"""

import argparse
import collections
import glob
import json
import os
import re
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)) + "/..")  # tools/loop
import steps as S  # noqa: E402  (tools/loop/steps.py)

PARK_S = 600
# What a worker's own test command looks like when it names a test, a binary or a filter.
FILTERED = re.compile(r"-E |--test |--lib|test\(|\s--\s")
GATE_WALL = re.compile(r"Wall time: ([\d.]+) seconds")


def sessions(dirs):
    return [p for d in dirs for p in glob.glob(f"{d}/**/*.jsonl", recursive=True)]


def classify(rows, buckets):
    """Add one session's rows to `buckets` (category -> [seconds]); return (active, parked)."""
    active = parked = 0.0
    for r in rows:
        if r["kind"] == "bg":
            continue  # the job's run time is also in the wait/tool rows that sat on it
        if r["kind"] == "model" and r["seconds"] > PARK_S:
            buckets["PARKED (session disposed, revived by a later message)"].append(r["seconds"])
            parked += r["seconds"]
            continue
        name = {"model": "model thinking/writing", "wait": "wait for background job"}.get(r["kind"], r["category"])
        buckets[name].append(r["seconds"])
        active += r["seconds"]
    return active, parked


def table(title, buckets, top=12, out=sys.stdout):
    total = sum(sum(v) for v in buckets.values()) or 1.0
    out.write(f"\n### {title}: {total / 3600:.1f} h\n\n| h | share | n | median s | what |\n|---|---|---|---|---|\n")
    for name, v in sorted(buckets.items(), key=lambda kv: -sum(kv[1]))[:top]:
        out.write(f"| {sum(v) / 3600:.2f} | {100 * sum(v) / total:.0f}% | {len(v)} | {statistics.median(v):.1f} | {name} |\n")


def test_runs(paths):
    """How many `nextest`/`cargo test` commands a worker typed, filtered or not."""
    out = collections.Counter()
    for p in paths:
        for o in S.read_session(p):
            m = o.get("message") or {}
            if o.get("type") == "message" and m.get("role") == "assistant" and isinstance(m.get("content"), list):
                for c in m["content"]:
                    cmd = (c.get("arguments") or {}).get("command", "") if c.get("type") == "toolCall" else ""
                    if re.search(r"nextest run|cargo test", cmd):
                        out["filtered" if FILTERED.search(cmd) else "unfiltered"] += 1
                    elif re.search(r"cargo clippy", cmd):
                        out["clippy"] += 1
    return out


def gate_runs(manager):
    """Every push-gate result in the manager's session: {wall seconds: (outcome, [failing tests])}."""
    seen = {}

    def strings(o):
        if isinstance(o, str):
            yield o
        elif isinstance(o, dict):
            for v in o.values():
                yield from strings(v)
        elif isinstance(o, list):
            for v in o:
                yield from strings(v)

    for o in S.read_session(manager):
        for t in strings(o):
            w = GATE_WALL.search(t)
            if w and "pre-push gate" in t:
                outcome = "passed" if "pre-push gate: passed" in t else ("refused" if "REFUSED" in t else "other")
                seen[float(w.group(1))] = (outcome, sorted(set(re.findall(r"FAIL \[[^\]]*\] \(\d+/\d+\) \S+ (\S+)", t))))
    return seen


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--workers", nargs="+", required=True)
    ap.add_argument("--manager", required=True)
    a = ap.parse_args(argv)
    manager_dir = a.manager[:-len(".jsonl")]
    groups = {"workers": sessions(a.workers), "subagents (reviews, fixes)": sessions([manager_dir]), "manager": [a.manager]}
    w = sys.stdout
    w.write(f"# Fleet timing\n\nPARK_S = {PARK_S}: a model row longer than that is parked time, not work.\n")
    for name, paths in groups.items():
        b, parked, active = collections.defaultdict(list), 0.0, 0.0
        for p in paths:
            ac, pk = classify(S.steps(S.read_session(p)), b)
            active, parked = active + ac, parked + pk
        w.write(f"\n## {name}: {len(paths)} sessions, {active / 3600:.1f} h active, {parked / 3600:.1f} h parked\n")
        table(name, b, out=w)
    runs = test_runs(groups["workers"])
    w.write(f"\n## Worker commands\n\n{dict(runs)}\n")
    gates = gate_runs(a.manager)
    by = collections.defaultdict(list)
    for sec, (outcome, _) in gates.items():
        by[outcome].append(sec)
    w.write("\n## Push gate\n\n| outcome | runs | total h | mean min |\n|---|---|---|---|\n")
    for outcome, v in by.items():
        w.write(f"| {outcome} | {len(v)} | {sum(v) / 3600:.2f} | {sum(v) / len(v) / 60:.1f} |\n")
    causes = collections.Counter(t for _, (o, f) in gates.items() if o == "refused" for t in f)
    w.write("\nTests that refused a push: " + ", ".join(f"{t} x{n}" for t, n in causes.most_common(8)) + "\n")


if __name__ == "__main__":
    main()
