#!/usr/bin/env python3
"""The editor's version policy, written down and checked: `version.py next` / `version.py check`.

# Why this exists

The version is the one number in this repository that a person reads off a download page, and it is
the one number an automated pass can quietly turn into a build counter. `0.2.5 -> 0.2.6` in a day is a
release; `0.2.278` is a counter, and `0.2.278.2884.12` is what a counter becomes when nobody stopped
it. The owner's rule is that the digits stay readable, so this file states the ceiling and `check`
refuses a manifest or a tag that breaks it.

# The policy

Pre-1.0 SemVer, one bump per **release**, and digits with a ceiling:

- **PATCH counts non-breaking releases** — a feature, a fix, a record-format change that the same app
  reads. `0.2.5 -> 0.2.6`.
- **When PATCH would reach 10 the bump rolls MINOR instead**: `0.2.9 -> 0.3.0`. That is the whole
  reason PATCH cannot run away, and it is why `patch` is capped at 9 by `check`.
- **MINOR is capped at 9 too.** `0.9.9` is as far as a pass may count: the next release is a
  `1.0.0` decision, and only the owner makes it, in a conversation, by name. It is never automatic,
  and a pass that reaches the ceiling says so and stops — the same shape as publishing itself.
- **A version is never derived from anything automatic**: not the date, not a build number, not a CI
  run id, not a commit count, not a lint. It is `previous + one`, or it does not change.
- **A bump is its own commit**, `release: <version>`, carrying the manifest and `Cargo.lock` and
  nothing else — so the tag's source is exactly what was tested.

# What `check` looks at

Offline and deterministic, because it runs on every push: the manifest, the lockfile entry that must
agree with it, and every local `bevy_cauldron-v*` tag. A tag with a version outside the ceiling is a
failure here before it is a release nobody can explain.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
MANIFEST = REPO / "crates/bevy_cauldron/Cargo.toml"
LOCK = REPO / "Cargo.lock"
TAG_PREFIX = "bevy_cauldron-v"

#: The highest PATCH a version may carry. Reaching it is what rolls MINOR: see the module note.
PATCH_CEILING = 9
#: The highest MINOR a pass may count to. Past it the release is a `1.0.0` decision, and the owner's.
MINOR_CEILING = 9

_VERSION = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")


class Refusal(Exception):
    """Something the policy does not allow, said in a sentence a person can act on."""


def manifest_version(manifest: Path = MANIFEST) -> str:
    """The `version` in `[package]`, refusing a manifest with none or with more than one."""
    text = manifest.read_text()
    in_package = False
    found = None
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            in_package = stripped == "[package]"
            continue
        if not in_package or not stripped.startswith("version"):
            continue
        _, _, value = stripped.partition("=")
        found = value.strip().strip('"')
    if found is None:
        raise Refusal(f"{manifest} has no [package] version")
    return found


def lock_version(lock: Path = LOCK) -> str | None:
    """The `bevy_cauldron` entry's version in `Cargo.lock`, or `None` when it is not in the file."""
    text = lock.read_text()
    for block in text.split("[[package]]"):
        if not re.search(r'^name = "bevy_cauldron"$', block, re.MULTILINE):
            continue
        match = re.search(r'^version = "([^"]+)"$', block, re.MULTILINE)
        return match.group(1) if match else None
    return None


def local_tags(repo: Path = REPO) -> list[str]:
    """Every local `bevy_cauldron-v*` tag, sorted. A clone with no tags has none, which is fine."""
    try:
        out = subprocess.run(
            ["git", "tag", "-l", f"{TAG_PREFIX}*"],
            cwd=repo, capture_output=True, text=True, check=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError):
        return []
    return sorted(line.strip() for line in out.splitlines() if line.strip())


def parse(version: str) -> tuple[int, int, int]:
    """`X.Y.Z` as numbers, refusing anything else — including a pre-release or build metadata."""
    match = _VERSION.match(version.strip())
    if not match:
        raise Refusal(
            f"`{version}` is not a plain X.Y.Z. A release version here has no pre-release and no "
            "build metadata: those make two downloads of the same code look different, and the "
            "updater compares tags exactly."
        )
    return (int(match.group(1)), int(match.group(2)), int(match.group(3)))


def next_version(current: str) -> str:
    """**The one version after `current`**, by the policy.

    A `1.0.0` is never produced here: at the ceiling this refuses and says whose decision it is,
    because an automated pass must not announce a stable release for a project the owner has not
    called stable.
    """
    major, minor, patch = parse(current)
    if major != 0:
        raise Refusal(
            f"{current} is past 1.0. Past 1.0 the next version is a decision — a breaking change "
            "bumps MAJOR, anything else MINOR — and it is the owner's, not a script's."
        )
    if patch + 1 <= PATCH_CEILING:
        return f"0.{minor}.{patch + 1}"
    if minor + 1 <= MINOR_CEILING:
        return f"0.{minor + 1}.0"
    raise Refusal(
        f"{current} is as far as this line counts (PATCH and MINOR both stop at "
        f"{PATCH_CEILING}/{MINOR_CEILING}). The next release is a `1.0.0` decision, which needs the "
        "owner in a conversation: bumping past that here would declare a stable release nobody asked "
        "for, and a version that keeps counting is the counter this policy exists to stop."
    )


def problems(manifest: Path = MANIFEST, lock: Path = LOCK, tags: list[str] | None = None) -> list[str]:
    """Everything wrong with the version as it stands, as sentences. Empty is a pass."""
    found = []
    try:
        version = manifest_version(manifest)
    except Refusal as refusal:
        return [str(refusal)]
    try:
        major, minor, patch = parse(version)
    except Refusal as refusal:
        return [str(refusal)]
    if major != 0:
        found.append(
            f"the editor is {version}: past 1.0 the digits grow differently, and this policy only "
            "describes the 0.x line. Rewrite `tools/loop/version.py` with the owner when that day comes."
        )
    if patch > PATCH_CEILING:
        found.append(
            f"the editor is {version}, and PATCH may not pass {PATCH_CEILING}: the release after "
            f"0.{minor}.{PATCH_CEILING} rolls MINOR to 0.{minor + 1}.0 instead. A PATCH above the "
            "ceiling is a build counter, which is what makes a version read 0.2.278."
        )
    if minor > MINOR_CEILING:
        found.append(
            f"the editor is {version}, and MINOR may not pass {MINOR_CEILING}: the next release there "
            "is a `1.0.0` decision, and it is the owner's to make."
        )
    locked = lock_version(lock)
    if locked is None:
        found.append(f"{lock} has no `bevy_cauldron` entry, so nothing checks the version it builds")
    elif locked != version:
        found.append(
            f"{manifest} says {version} and {lock} says {locked}: run `cargo metadata` (or any build) "
            "in the same commit, because they are one version in two files."
        )
    for tag in tags if tags is not None else local_tags():
        rest = tag[len(TAG_PREFIX):]
        try:
            tag_major, tag_minor, tag_patch = parse(rest)
        except Refusal as refusal:
            found.append(f"tag {tag}: {refusal}")
            continue
        if tag_major != 0:
            continue
        if tag_patch > PATCH_CEILING or tag_minor > MINOR_CEILING:
            found.append(
                f"tag {tag} is outside the ceiling (PATCH and MINOR stop at "
                f"{PATCH_CEILING}/{MINOR_CEILING}); it would need a rewrite of the policy rather than "
                "a release."
            )
    return found


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="The editor's version policy: what the next version is, and whether this one "
                    "breaks the ceiling.",
    )
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("next", help="print the version this line releases next")
    sub.add_parser("check", help="refuse a version or tag outside the policy (the gate runs this)")
    sub.add_parser("show", help="print the current version")
    args = parser.parse_args(argv)

    try:
        if args.command == "next":
            print(next_version(manifest_version()))
            return 0
        if args.command == "show":
            print(manifest_version())
            return 0
        found = problems()
        if found:
            for line in found:
                print(f"version: {line}", file=sys.stderr)
            return 1
        print("the version is inside the policy")
        return 0
    except Refusal as refusal:
        print(f"version: {refusal}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
