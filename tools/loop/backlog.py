#!/usr/bin/env python3
"""Keep docs/loop/backlog.json and its archive honest, and hand out the next id.

usage:
  backlog.py check
  backlog.py next-id B|E|S
  backlog.py archive

`check` exits 1 on any violation and names it, so the brief can gate on it. It reads both files,
because the ids, the kinds and the statuses are properties of the whole set — an id duplicated
across the working list and the archive is the same defect as one duplicated inside either.
"""
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
LIVE = REPO / "docs/loop/backlog.json"
ARCHIVE = REPO / "docs/loop/backlog-archive.json"

KINDS = {"bug", "enhancement", "series"}
STATUSES = {"open", "fixed", "blocked", "unconfirmed", "deferred", "rejected", "shipped"}
ARCHIVED = {"fixed", "shipped", "rejected"}


def load(path):
    with open(path) as f:
        return json.load(f)


def save(path, data):
    with open(path, "w") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)
        f.write("\n")


def both():
    return [(LIVE, load(LIVE)), (ARCHIVE, load(ARCHIVE))]


def check():
    """Every violation, as a list of sentences. Empty is the passing answer."""
    problems = []
    seen = {}
    for path, data in both():
        if not isinstance(data.get("items"), list):
            problems.append(f"{path.name}: no items list")
            continue
        for item in data["items"]:
            where = f"{path.name}:{item.get('id', '<no id>')}"
            item_id = item.get("id")
            if not item_id:
                problems.append(f"{where}: has no id")
                continue
            if item_id in seen:
                problems.append(f"{where}: id is already used in {seen[item_id].name}")
            else:
                seen[item_id] = path
            if item.get("kind") not in KINDS:
                problems.append(f"{where}: kind {item.get('kind')!r} is not one of {sorted(KINDS)}")
            if item.get("status") not in STATUSES:
                problems.append(f"{where}: status {item.get('status')!r} is not one of {sorted(STATUSES)}")
            if item.get("kind") == "series":
                for field in ("goal", "approved_by", "acceptance", "steps"):
                    if not item.get(field):
                        problems.append(f"{where}: a series needs `{field}`")
                for step in item.get("steps") or []:
                    if not step.get("id") or not step.get("title"):
                        problems.append(f"{where}: a step needs an id and a title")
                    if step.get("status") not in {"open", "fixed", "blocked"}:
                        problems.append(
                            f"{where}:{step.get('id')}: step status {step.get('status')!r} is not "
                            "one of ['blocked', 'fixed', 'open']")
    return problems


def next_id(prefix):
    """One past the highest number for `prefix`, across both files."""
    highest = 0
    for _, data in both():
        for item in data["items"]:
            item_id = item.get("id", "")
            head, _, tail = item_id.partition("-")
            if head == prefix and tail.isdigit():
                highest = max(highest, int(tail))
    return f"{prefix}-{highest + 1:03d}"


def archive():
    """Move every settled item to the end of the archive, in order, and report what moved."""
    live = load(LIVE)
    old = load(ARCHIVE)
    moving = [i for i in live["items"] if i.get("status") in ARCHIVED]
    keeping = [i for i in live["items"] if i.get("status") not in ARCHIVED]
    old["items"].extend(moving)
    save(ARCHIVE, old)
    live["items"] = keeping
    save(LIVE, live)
    return [i["id"] for i in moving]


def main(argv):
    if len(argv) < 2:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    command = argv[1]
    if command == "check":
        problems = check()
        for p in problems:
            print(p)
        if problems:
            print(f"{len(problems)} problem(s)", file=sys.stderr)
            return 1
        print("the backlog is consistent")
        return 0
    if command == "next-id":
        if len(argv) < 3 or argv[2] not in {"B", "E", "S"}:
            print("next-id needs one of B, E, S", file=sys.stderr)
            return 2
        print(next_id(argv[2]))
        return 0
    if command == "archive":
        moved = archive()
        print(f"archived {len(moved)}" + (f": {', '.join(moved)}" if moved else ""))
        return 0
    print(f"unknown command {command!r}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
