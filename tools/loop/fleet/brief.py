#!/usr/bin/env python3
"""Render a fleet worker brief from brief.md and a cluster of GitHub issues.

brief.py <issue>... --cluster <slug> --branch <b> --worktree <path> --host <label> [--extra <file>] --out <file>

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


def fetch(number: str) -> dict | None:
    proc = subprocess.run(
        ["gh", "issue", "view", number, "-R", REPO, "--json", "number,title,body"],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        print(f"gh issue view {number} failed: {proc.stderr.strip()}", file=sys.stderr)
        return None
    return json.loads(proc.stdout)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("issues", nargs="+")
    ap.add_argument("--cluster", required=True)
    ap.add_argument("--branch", required=True)
    ap.add_argument("--worktree", required=True)
    ap.add_argument("--host", required=True)
    ap.add_argument("--extra")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    issues = []
    for number in a.issues:
        issue = fetch(number)
        if issue is None:
            return 1
        issues.append(issue)
    extra = Path(a.extra).read_text() if a.extra else ""
    values = {
        "ISSUES": ", ".join(f"#{i['number']}" for i in issues),
        "ISSUE_BODIES": "".join(f"## Issue #{i['number']}: {i['title']}\n\n{i['body'] or ''}\n\n" for i in issues),
        "CLUSTER": a.cluster,
        "BRANCH": a.branch,
        "WORKTREE": a.worktree,
        "HOST": a.host,
        "EXTRA": extra,
    }
    template = TEMPLATE.read_text()
    # An unknown placeholder survives the pass as-is; check the template's own, not the substituted bodies.
    unknown = [m for m in re.findall(r"\{\{([A-Z_]+)\}\}", template) if m not in values]
    if unknown:
        print(f"unknown placeholders in brief.md: {sorted(set(unknown))}", file=sys.stderr)
        return 1
    # One pass over the template: substituted text (an issue body may quote a placeholder) is never rescanned.
    text = re.sub(r"\{\{([A-Z_]+)\}\}", lambda m: values[m.group(1)], template)
    Path(a.out).parent.mkdir(parents=True, exist_ok=True)
    Path(a.out).write_text(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
