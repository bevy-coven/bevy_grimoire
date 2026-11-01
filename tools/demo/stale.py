#!/usr/bin/env python3
"""Which of the README's standing demos no longer show the editor as it is (owner, 2026-11-01).

The eight GIFs in `docs/media/` and `docs/media/editor.png` are what the README and `docs/guide/` show a new
reader. Each row of `docs/media/README.md` says which commit it was *Captured* at. A **visible change** is a
`CHANGELOG.md` line that has a clip; a demo is **stale** when a visible change landed after its commit, and the
lines that did are printed with it. Exits 1 when any demo is stale, so a pass can use it as a check.

    python3 tools/demo/stale.py [--repo DIR]

A row whose *Captured* cell has a date and no commit (`editor.png`) is the last commit made on or before the end
of that day, in UTC.
"""
import argparse
import re
import subprocess
import sys
from pathlib import Path

ROW = re.compile(r"^\|\s*`(?P<name>[^`|]+\.(?:gif|png))`\s*\|(?P<command>.*)\|\s*(?P<captured>[^|]*?)\s*\|(?P<shows>.*)\|\s*$")
LINE = re.compile(r"^- \*\*(?P<id>[^*]+)\*\*\s+`(?P<sha>[0-9a-f]{7,40})`")


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)


def demos(readme):
    """`(name, date, sha or None)` for each row of the media README's table."""
    out = []
    for line in readme.splitlines():
        m = ROW.match(line)
        if not m:
            continue
        captured = m["captured"].split()
        date = captured[0] if captured else ""
        sha = captured[1] if len(captured) > 1 and re.fullmatch(r"[0-9a-f]{7,40}", captured[1]) else None
        out.append((m["name"], date, sha))
    return out


def visible_changes(changelog):
    """`(id, sha)` of each changelog line that has a clip, newest first as the file lists them."""
    return [(m["id"], m["sha"]) for line in changelog.splitlines()
            if "<a href=" in line and ".mp4" in line and (m := LINE.match(line))]


def captured_commit(repo, date, sha):
    if sha:
        return sha
    r = git(repo, "rev-list", "-1", f"--before={date} 23:59:59 +0000", "HEAD")
    return r.stdout.strip() or None


def stale_lines(repo, base, changes):
    """The changes whose commit is a descendant of `base`, and not `base` itself."""
    full = git(repo, "rev-parse", "--verify", "--quiet", f"{base}^{{commit}}").stdout.strip()
    if not full:
        return None
    later = []
    for cid, sha in changes:
        commit = git(repo, "rev-parse", "--verify", "--quiet", f"{sha}^{{commit}}").stdout.strip()
        if commit and commit != full and git(repo, "merge-base", "--is-ancestor", full, commit).returncode == 0:
            later.append((cid, sha))
    return later


def report(repo):
    """`(text, any_stale)`."""
    repo = Path(repo)
    readme = (repo / "docs" / "media" / "README.md").read_text(encoding="utf-8")
    changes = visible_changes((repo / "CHANGELOG.md").read_text(encoding="utf-8"))
    lines, any_stale = [], False
    for name, date, sha in demos(readme):
        base = captured_commit(repo, date, sha)
        later = stale_lines(repo, base, changes) if base else None
        if later is None:
            captured = f"{date} {sha or ''}".strip()
            lines.append(f"{name}: stale (its Captured cell `{captured}` names no commit in this repository)")
            any_stale = True
        elif later:
            lines.append(f"{name}: stale ({len(later)} visible change(s) since {base[:8]})")
            lines.extend(f"    {cid} {sha[:8]}" for cid, sha in later)
            any_stale = True
        else:
            lines.append(f"{name}: fresh (no visible change since {base[:8]})")
    return "\n".join(lines), any_stale


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--repo", default=str(Path(__file__).resolve().parents[2]))
    args = ap.parse_args()
    text, any_stale = report(args.repo)
    print(text)
    sys.exit(1 if any_stale else 0)


if __name__ == "__main__":
    main()
