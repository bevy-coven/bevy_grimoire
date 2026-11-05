#!/usr/bin/env python3
"""The debug captures a person filed against the editor: one queue, one ledger, six verbs.

A capture is what `Cmd+D` writes — a record (`feedback-<millis>.json`), the frame it was taken on,
the pictures the picks are outlined in, and the tail of the log. They are filed on the machine a
person ran the editor on, so before a pass can read one they have to be here: `sync` copies them into
`docs/loop/captures/<stem>/` **as ordinary git blobs**, never under `docs/changelog/media/` (which
`.gitattributes` puts in LFS), because a pass must be able to read a capture's frame with no LFS
install and no second host.

**The ledger is the queue**, `docs/loop/captures/ledger.json`: one row per capture, its `kind`
(bug or enhancement) and its `status`. `next` reads it and prints the stem this pass owns; nothing
else decides what is worked on, so the order is a property of the file rather than of each pass's
mood — bugs before enhancements, oldest first inside a class.

**`check` is the gate the ledger has to pass**, and the reason it is strict is that a ledger nobody
validates is prose: a `fixed` row has to name a commit that is really an ancestor of `HEAD`, and an
`unreproduced` row has to carry three distinct attempts, which is a person's "capture more
information" made checkable. Giving up early cannot pass.

Python 3 standard library only, as everything under `tools/loop/` is. Everything it does to a
capture's source is a read over one ssh pipe; it never writes to the machine a person filed on.

usage:
  python3 tools/loop/captures.py sync    [--host mac_air] [--from DIR]
  python3 tools/loop/captures.py next
  python3 tools/loop/captures.py show <stem>
  python3 tools/loop/captures.py set <stem> --status S [--kind K] [--reason TEXT] [--commit SHA]
  python3 tools/loop/captures.py attempt <stem> --host H --backend B --window WxH --ui-scale F \
                                             --outcome reproduced|not_reproduced --saw TEXT
  python3 tools/loop/captures.py check

exit codes, so a caller can branch instead of parsing prose:
  0  done (for `next`: the queue had an item)
  1  `check` found violations
  2  usage: an unknown verb value, or a stem the ledger does not hold
  3  `next`: the queue is empty — the loop is finished, not blocked
  4  `next`: the working tree is dirty, and which files
  5  `next`: `check` fails, so the ledger cannot be trusted to pick from
"""
from __future__ import annotations

import argparse
import io
import json
import re
import shlex
import subprocess
import sys
import tarfile
from pathlib import Path

from common import REPO

# **Where the ledger and the captures live**, under the repository root. Both are ordinary git
# blobs: a capture's frame is committed beside its record so a pass reads the pixels that were
# actually on the screen, not a caption of them.
CAPTURES = Path("docs") / "loop" / "captures"
LEDGER = CAPTURES / "ledger.json"

# **The two classes of note**, and the rule that decides one, stated here so a caller need not read
# the command: a note that says something *is* wrong is a bug; a note that says something *should be*
# is an enhancement. The test is the mood of the main verb, not the presence of a request.
KINDS = ("bug", "enhancement")

# **`unread` means unclassified**, and an empty `kind` is legal only for it: a row past triage has to
# say which class it is in, because `next` orders by it.
STATUSES = ("unread", "triaged", "fixed", "unreproduced", "blocked", "declined")

# **A finished row never comes back**: only a person moving a row to `triaged` requeues it.
FINISHED = ("fixed", "unreproduced", "blocked", "declined")

OUTCOMES = ("reproduced", "not_reproduced")

# **How many configurations an `unreproduced` claim needs**, distinct by `(host, backend, window)`:
# "it did not reproduce" is only an answer once more than one place has been looked at, which is the
# checkable form of "capture more information" rather than "I could not see it".
MIN_ATTEMPTS_UNREPRODUCED = 3

# **The files a capture is made of**, from the names the writer uses (`bevy_grimoire_core::feedback`'s
# own helpers): the record, its frame, the log tail, and the pictures a pick is outlined in — one of
# `_picks`/`_subject` depending on the record's version, and it is genuinely absent for some captures,
# so a missing optional file is not an error.
RECORD_SUFFIX = ".json"
REQUIRED_SUFFIXES = (".json", ".png")
OPTIONAL_PICTURES = ("_picks.png", "_subject.png")
LOG_SUFFIX = ".log"

WINDOW = re.compile(r"^\d+x\d+$")
STEM = re.compile(r"^feedback-(\d+)$")


class Refusal(Exception):
    """A verb refusing to do something, with the sentence a caller should read and an exit code."""

    def __init__(self, code, message):
        super().__init__(message)
        self.code = code
        self.message = message


def ledger_path(root):
    return Path(root) / LEDGER


def captures_dir(root):
    return Path(root) / CAPTURES


def load_ledger(root):
    """The ledger as a dict with an `items` list.

    **A ledger that will not read is not an empty ledger.** Treating a damaged queue as "nothing to
    do" is how a loop quietly stops working, so this raises rather than returning `{"items": []}`.
    """
    path = ledger_path(root)
    if not path.is_file():
        return {"items": []}
    text = path.read_text(encoding="utf-8")
    if not text.strip():
        return {"items": []}
    value = json.loads(text)
    if not isinstance(value, dict) or not isinstance(value.get("items"), list):
        raise ValueError(f"{path} is not a ledger: expected an object with an `items` list")
    return value


def save_ledger(root, ledger):
    """Write the ledger the one way it is ever written: two-space JSON, newline at the end, sorted rows
    kept in the order they are given (so a pass's diff touches only the row it changed)."""
    path = ledger_path(root)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(ledger, indent=2) + "\n", encoding="utf-8")


def row_of(ledger, stem):
    for row in ledger["items"]:
        if row.get("stem") == stem:
            return row
    return None


def stem_millis(stem):
    """`feedback-1793901499397` is also its timestamp, so a record that will not parse still has one."""
    hit = STEM.match(stem)
    return int(hit.group(1)) if hit else 0


# ---------------------------------------------------------------- talking to the machine that filed


def _ssh(host, script, allow_failure=False):
    """One remote shell script, with `bash` named explicitly: ssh runs the login shell, and a
    person's login shell differs per host (mac_air's is zsh)."""
    out = subprocess.run(
        ["ssh", host, f"bash -c {shlex.quote(script)}"],
        capture_output=True,
    )
    if out.returncode != 0 and not allow_failure:
        raise Refusal(
            2,
            f"{host}: the command failed ({out.returncode}): {out.stderr.decode(errors='replace').strip()}",
        )
    return out.returncode, out.stdout, out.stderr.decode(errors="replace")


# **Where a person's reports live on each platform**, `dirs::data_dir()`'s two shapes. Asked of the
# host rather than assumed, because `$XDG_DATA_HOME` may be set and because macOS's Library path has a
# space in it.
_FEEDBACK_DIR = (
    'if [ "$(uname -s)" = Darwin ]; then '
    'd="$HOME/Library/Application Support/Grimoire/feedback"; '
    'else d="${XDG_DATA_HOME:-$HOME/.local/share}/Grimoire/feedback"; fi; '
)


def remote_list(host):
    """The folder on `host` and the stems of the records in it, in one round trip."""
    # **No `--` before the glob and no `ls`.** BSD `ls` on macOS does not read `--` as end-of-options,
    # and a glob that matches nothing stays literal, so the `-f` test is what makes an empty folder an
    # empty list rather than a file named `*.json`.
    script = (
        _FEEDBACK_DIR
        + 'cd "$d" || { echo "no $d" >&2; exit 2; }; printf "%s\\n" "$PWD"; '
        'for f in *.json; do [ -f "$f" ] && printf "%s\\n" "$f"; done'
    )
    _code, data, complaint = _ssh(host, script)
    lines = data.decode("utf-8", "replace").splitlines()
    if not lines:
        raise Refusal(2, f"{host}: the reports folder could not be read: {complaint.strip()}")
    folder = lines[0].strip()
    stems = sorted(line.strip()[: -len(RECORD_SUFFIX)] for line in lines[1:] if line.strip().endswith(RECORD_SUFFIX))
    return folder, stems


def remote_tar(host, folder, names):
    """One capture's files over one ssh pipe, as a tar stream.

    **A missing optional file is not a failure, and it is not a silent miss either.** macOS's `tar`
    writes its complaint to stderr, emits every member it could stat, and exits 1 (measured against
    `mac_air`, 2026-11-05: `feedback-1793905912313` has no `_picks.png`), so a non-zero status is
    expected here and the archive is what is checked — a stream that will not open is the failure.
    """
    quoted = " ".join(shlex.quote(name) for name in names)
    # `--` is accepted by macOS's BSD `tar` as well as GNU's (measured against `mac_air`, 2026-11-05),
    # so a member name can never be read as an option.
    script = f"cd {shlex.quote(folder)} && tar cf - -- {quoted} 2>/dev/null"
    _code, data, _complaint = _ssh(host, script, allow_failure=True)
    members = {}
    try:
        archive = tarfile.open(fileobj=io.BytesIO(data))
    except tarfile.TarError as error:
        raise Refusal(2, f"{host}: {folder} did not answer with a tar stream ({error})") from error
    with archive:
        for member in archive.getmembers():
            if not member.isfile():
                continue
            handle = archive.extractfile(member)
            if handle is not None:
                members[Path(member.name).name] = handle.read()
    return members


def local_list(folder):
    folder = Path(folder)
    if not folder.is_dir():
        raise Refusal(2, f"{folder} is not a directory")
    stems = sorted(p.name[: -len(RECORD_SUFFIX)] for p in folder.glob("*" + RECORD_SUFFIX))
    return str(folder), stems


# ---------------------------------------------------------------- the six verbs


def new_row(stem, record, host):
    """The ledger row a freshly synced capture gets: unread, unclassified, with the record's own
    versions. `record_version` is the capture's `version` field, never this build's."""
    if isinstance(record, dict):
        return {
            "stem": stem,
            "taken_at_ms": int(record.get("taken_at_ms") or stem_millis(stem)),
            "app_version": str(record.get("app_version") or ""),
            "record_version": int(record.get("version") or 0),
            "kind": "",
            "status": "unread",
            "host": host,
            "reason": "",
            "commit": "",
            "attempts": [],
        }
    return {
        "stem": stem,
        "taken_at_ms": stem_millis(stem),
        "app_version": "",
        "record_version": 0,
        "kind": "",
        "status": "blocked",
        "host": host,
        "reason": record,
        "commit": "",
        "attempts": [],
    }


def sync(root, host="mac_air", from_dir=None):
    """Copy every capture not here yet, and append a row for each. Rerunning it changes nothing."""
    root = Path(root)
    ledger = load_ledger(root)
    if from_dir:
        folder, stems = local_list(from_dir)
    else:
        folder, stems = remote_list(host)
    if not stems:
        print(f"no captures in {folder} on {host if not from_dir else 'disk'}")
        return 0

    dest_root = captures_dir(root)
    dest_root.mkdir(parents=True, exist_ok=True)
    added = 0
    for stem in stems:
        dest = dest_root / stem
        # **The files and the ledger row are each made once, and neither standing in for the other.**
        # A capture copied by an earlier version of this tool has files and no row; a row with no files
        # is what `show` refuses by name. So the copy is skipped when the record is here, and the row is
        # added whenever the ledger has not got it.
        if not (dest / (stem + RECORD_SUFFIX)).is_file():
            names = [stem + RECORD_SUFFIX, stem + ".png", stem + LOG_SUFFIX]
            names += [stem + picture for picture in OPTIONAL_PICTURES]
            if from_dir:
                members = {}
                for name in names:
                    path = Path(folder) / name
                    if path.is_file():
                        members[name] = path.read_bytes()
            else:
                members = remote_tar(host, folder, names)
            dest.mkdir(parents=True, exist_ok=True)
            for name, data in members.items():
                (dest / name).write_bytes(data)
            print(f"copied: {stem}")
        missing = [suffix for suffix in REQUIRED_SUFFIXES if not (dest / (stem + suffix)).is_file()]
        if missing:
            raise Refusal(2, f"{stem}: {folder} has no {stem}{missing[0]}, so it is not a capture")
        if row_of(ledger, stem) is not None:
            continue
        try:
            record = json.loads((dest / (stem + RECORD_SUFFIX)).read_text(encoding="utf-8"))
        except (OSError, ValueError) as error:
            # **Never silently skipped.** A record that will not parse is a row, so it is visible in
            # the queue, and `blocked` is what it is until a person says otherwise.
            record = f"the record does not read: {error}"
        ledger["items"].append(new_row(stem, record, host if not from_dir else "disk"))
        added += 1
        print(f"queued: {stem}")

    ledger["items"].sort(key=lambda row: row["taken_at_ms"])
    save_ledger(root, ledger)
    print(f"{added} new, {len(ledger['items'])} in the queue")
    return 0


def ordered(ledger):
    """The queue in the order a pass takes it: a triaged bug, then a triaged enhancement, then an
    unclassified capture (so it is classified before further work), oldest first inside each class."""
    def key(row):
        status = row.get("status")
        if status == "triaged":
            return (0 if row.get("kind") == "bug" else 1, row.get("taken_at_ms") or 0)
        return (2, row.get("taken_at_ms") or 0)

    live = [row for row in ledger["items"] if row.get("status") not in FINISHED and row.get("status") in ("triaged", "unread")]
    live.sort(key=key)
    return live


def working_tree_dirty(root):
    out = subprocess.run(["git", "-C", str(root), "status", "--short"], capture_output=True, text=True)
    if out.returncode != 0:
        return []
    return [line for line in out.stdout.splitlines() if line.strip()]


def next_stem(root):
    root = Path(root)
    dirty = working_tree_dirty(root)
    if dirty:
        listing = "\n".join(dirty[:20])
        raise Refusal(
            4,
            f"the working tree is dirty, so this pass would work on somebody's half-done edit:\n{listing}",
        )
    ledger = load_ledger(root)
    violations = check(root)
    if violations:
        raise Refusal(5, "the ledger does not pass `check`, so it cannot choose the item:\n" + "\n".join(violations))
    live = ordered(ledger)
    if not live:
        raise Refusal(3, "nothing left to answer")
    print(live[0]["stem"])
    return 0


def show(root, stem):
    """The record, as a person would read it, plus where the pixels are on this machine."""
    root = Path(root)
    ledger = load_ledger(root)
    row = row_of(ledger, stem)
    if row is None:
        raise Refusal(2, f"{stem} is not in {LEDGER}")
    folder = captures_dir(root) / stem
    for suffix in REQUIRED_SUFFIXES:
        path = folder / (stem + suffix)
        if not path.is_file():
            raise Refusal(2, f"{stem}: {path} is missing, so there is nothing to look at; run `sync`")
    record = json.loads((folder / (stem + RECORD_SUFFIX)).read_text(encoding="utf-8"))
    window = record.get("window") or {}
    physical = window.get("physical") or [0, 0]
    logical = window.get("logical") or [0.0, 0.0]
    scale = float(window.get("scale_factor") or 1.0)
    print(f"{stem}  record version {record.get('version')}, app {record.get('app_version') or '?'}")
    print(f"status: {row.get('status')}  kind: {row.get('kind') or '(unclassified)'}  host: {row.get('host')}")
    print(f"screen: {record.get('screen') or '-'}  project: {record.get('project') or '-'}  map: {record.get('map') or '-'}")
    print(
        f"window: {physical[0]}x{physical[1]} physical, {logical[0]:g}x{logical[1]:g} logical, "
        f"scale_factor {scale:g}, ui_scale {float(window.get('ui_scale') or 1.0):g}"
    )
    print(f"note: {(record.get('note') or '').strip()}")
    # **Version 1 calls it `subject` and version 2 calls it `picks`**; both are a list of picks to a
    # reader, so both are printed by one loop. Reading it here is why `read_reports`' refusal of a
    # version-1 file costs this tool nothing: `FEEDBACK_VERSION` is not this file's business.
    picks = record.get("picks")
    if picks is None and record.get("subject"):
        picks = [{"subject": record["subject"], "at": None}]
    for index, pick in enumerate(picks or []):
        subject = pick.get("subject") or {}
        rect = subject.get("rect_px") or []
        at = pick.get("at")
        where = ", ".join(
            f"{step.get('name') or step.get('role') or '?'}"
            + (f" \"{step['text']}\"" if step.get("text") else "")
            for step in (subject.get("path") or [])
        )
        print(f"pick {index}: rect_px {rect}  kind {subject.get('kind') or '-'}")
        if at:
            print(f"  at {at[0]:.1f},{at[1]:.1f} surface px = {at[0] / scale:.1f},{at[1] / scale:.1f} logical px")
        print(f"  path: {where or '(none)'}")
    pictures = [record.get("picture"), record.get("picks_picture"), record.get("subject_picture")]
    for entry in pictures:
        if entry and entry.get("file"):
            print(f"picture: {folder / entry['file']}")
    print(f"log: {folder / (stem + LOG_SUFFIX)}")
    if row.get("reason"):
        print(f"reason: {row['reason']}")
    for attempt in row.get("attempts") or []:
        print(
            f"attempt: {attempt.get('host')} {attempt.get('backend')} {attempt.get('window')} "
            f"ui_scale {attempt.get('ui_scale')} -> {attempt.get('outcome')}: {attempt.get('saw')}"
        )
    return 0


def validate_row(row):
    """The rules a single row has to meet, as sentences. `check` runs this over every row; `set`
    runs it over the row it is about to write, so a caller hears about it while it can still fix it."""
    problems = []
    stem = row.get("stem") or "(no stem)"
    kind, status = row.get("kind", ""), row.get("status", "")
    if status not in STATUSES:
        problems.append(f"{stem}: status {status!r} is not one of {', '.join(STATUSES)}")
    if kind not in KINDS and not (kind == "" and status == "unread"):
        problems.append(f"{stem}: kind {kind!r} is not one of {', '.join(KINDS)} (or empty while unread)")
    attempts = row.get("attempts") or []
    reproduced = [a for a in attempts if a.get("outcome") == "reproduced"]
    if status == "fixed":
        if not row.get("commit"):
            problems.append(f"{stem}: status fixed with no commit")
        if not reproduced:
            problems.append(f"{stem}: status fixed with no attempt that reproduced the defect")
    if status == "unreproduced":
        distinct = {(a.get("host"), a.get("backend"), a.get("window")) for a in attempts}
        if len(distinct) < MIN_ATTEMPTS_UNREPRODUCED:
            problems.append(
                f"{stem}: status unreproduced with {len(distinct)} distinct (host, backend, window) "
                f"attempt(s); {MIN_ATTEMPTS_UNREPRODUCED} are needed, because giving up early is not an answer"
            )
        if not (row.get("reason") or "").strip():
            problems.append(f"{stem}: status unreproduced with an empty reason")
    return problems


def is_ancestor(root, sha):
    if not sha:
        return False
    exists = subprocess.run(["git", "-C", str(root), "cat-file", "-e", f"{sha}^{{commit}}"], capture_output=True)
    if exists.returncode != 0:
        return False
    out = subprocess.run(["git", "-C", str(root), "merge-base", "--is-ancestor", sha, "HEAD"], capture_output=True)
    return out.returncode == 0


def check(root):
    """Every violation in the ledger, in the order the rows are in. Exit 1 if there is one."""
    root = Path(root)
    ledger = load_ledger(root)
    problems = []
    seen = set()
    for row in ledger["items"]:
        stem = row.get("stem")
        if stem in seen:
            problems.append(f"{stem}: two rows with the same stem")
        seen.add(stem)
        problems += validate_row(row)
        if row.get("status") == "fixed" and row.get("commit") and not is_ancestor(root, row["commit"]):
            problems.append(f"{stem}: status fixed with commit {row['commit']}, which is not an ancestor of HEAD")
    return problems


def run_check(root):
    problems = check(root)
    if problems:
        for problem in problems:
            print(problem, file=sys.stderr)
        return 1
    print("the ledger is honest")
    return 0


def set_status(root, stem, status=None, kind=None, reason=None, commit=None):
    root = Path(root)
    ledger = load_ledger(root)
    row = row_of(ledger, stem)
    if row is None:
        raise Refusal(2, f"{stem} is not in {LEDGER}")
    if status is not None:
        row["status"] = status
    if kind is not None:
        row["kind"] = kind
    if reason is not None:
        row["reason"] = reason
    if commit is not None:
        row["commit"] = commit
    problems = validate_row(row)
    if problems:
        raise Refusal(2, "\n".join(problems))
    save_ledger(root, ledger)
    print(f"{stem}: status {row['status']}, kind {row['kind'] or '(unclassified)'}")
    return 0


def add_attempt(root, stem, host, backend, window, ui_scale, outcome, saw):
    root = Path(root)
    if not WINDOW.match(window):
        raise Refusal(2, f"--window {window!r} is not WxH, e.g. 2560x1406")
    if outcome not in OUTCOMES:
        raise Refusal(2, f"--outcome {outcome!r} is not one of {', '.join(OUTCOMES)}")
    ledger = load_ledger(root)
    row = row_of(ledger, stem)
    if row is None:
        raise Refusal(2, f"{stem} is not in {LEDGER}")
    row.setdefault("attempts", []).append(
        {
            "host": host,
            "backend": backend,
            "window": window,
            "ui_scale": float(ui_scale),
            "outcome": outcome,
            "saw": saw,
        }
    )
    save_ledger(root, ledger)
    print(f"{stem}: {outcome} on {host} {backend} {window} (ui_scale {float(ui_scale):g})")
    return 0


# ---------------------------------------------------------------- the command line


def build_parser():
    parser = argparse.ArgumentParser(prog="captures.py", description="The queue of filed debug captures.")
    verbs = parser.add_subparsers(dest="verb", required=True)

    p = verbs.add_parser("sync", help="copy captures that are not here yet and queue them")
    p.add_argument("--host", default="mac_air")
    p.add_argument("--from", dest="from_dir", default=None, help="a local folder instead of a host")

    verbs.add_parser("next", help="the capture this pass owns; exit 3 when the queue is empty")

    p = verbs.add_parser("show", help="print one capture's record and where its pixels are")
    p.add_argument("stem")

    p = verbs.add_parser("set", help="write a row's status, kind, reason or commit")
    p.add_argument("stem")
    p.add_argument("--status")
    p.add_argument("--kind")
    p.add_argument("--reason")
    p.add_argument("--commit")

    p = verbs.add_parser("attempt", help="record one configuration a capture was tried on")
    p.add_argument("stem")
    p.add_argument("--host", required=True)
    p.add_argument("--backend", required=True)
    p.add_argument("--window", required=True, help="WxH, the physical size the editor ran at")
    p.add_argument("--ui-scale", dest="ui_scale", required=True, type=float)
    p.add_argument("--outcome", required=True)
    p.add_argument("--saw", required=True, help="what was on the screen, in a person's words")

    verbs.add_parser("check", help="exit 1 listing every violation in the ledger")
    return parser


def main(argv=None, root=None):
    args = build_parser().parse_args(argv)
    root = Path(root) if root else REPO
    try:
        if args.verb == "sync":
            return sync(root, host=args.host, from_dir=args.from_dir)
        if args.verb == "next":
            return next_stem(root)
        if args.verb == "show":
            return show(root, args.stem)
        if args.verb == "set":
            return set_status(root, args.stem, args.status, args.kind, args.reason, args.commit)
        if args.verb == "attempt":
            return add_attempt(
                root, args.stem, args.host, args.backend, args.window, args.ui_scale, args.outcome, args.saw
            )
        if args.verb == "check":
            return run_check(root)
    except Refusal as refusal:
        print(refusal.message, file=sys.stderr)
        return refusal.code
    except ValueError as error:
        print(str(error), file=sys.stderr)
        return 1
    return 2


if __name__ == "__main__":
    sys.exit(main())
