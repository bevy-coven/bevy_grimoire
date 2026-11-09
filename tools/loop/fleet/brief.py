#!/usr/bin/env python3
"""Render a fleet worker brief from brief.md and a GitHub issue.

brief.py <issue> --branch <b> --worktree <path> --host <label> [--extra <file>] --out <file>

Exits 1 with the reason on stderr when `gh` fails or any {{PLACEHOLDER}} survives; never writes a partial brief.
"""
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

REPO = "dark-incorporated/bevy_dark_factory"
TEMPLATE = Path(__file__).with_name("brief.md")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("issue")
    ap.add_argument("--branch", required=True)
    ap.add_argument("--worktree", required=True)
    ap.add_argument("--host", required=True)
    ap.add_argument("--extra")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    proc = subprocess.run(
        ["gh", "issue", "view", a.issue, "-R", REPO, "--json", "number,title,body"],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        print(f"gh issue view {a.issue} failed: {proc.stderr.strip()}", file=sys.stderr)
        return 1
    issue = json.loads(proc.stdout)
    extra = Path(a.extra).read_text() if a.extra else ""
    values = {
        "ISSUE": str(issue["number"]),
        "TITLE": issue["title"],
        "BODY": issue["body"] or "",
        "BRANCH": a.branch,
        "WORKTREE": a.worktree,
        "HOST": a.host,
        "EXTRA": extra,
    }
    text = TEMPLATE.read_text()
    # One pass over the template: substituted text (an issue body may quote a placeholder) is never rescanned.
    text = re.sub(r"\{\{([A-Z]+)\}\}", lambda m: values.get(m.group(1), m.group(0)), text)
    # An unknown placeholder survives the pass as-is; check the template's own, not the substituted bodies.
    unknown = [m for m in re.findall(r"\{\{([A-Z]+)\}\}", TEMPLATE.read_text()) if m not in values]
    if unknown:
        print(f"unknown placeholders in brief.md: {sorted(set(unknown))}", file=sys.stderr)
        return 1
    Path(a.out).parent.mkdir(parents=True, exist_ok=True)
    Path(a.out).write_text(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
