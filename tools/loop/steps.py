#!/usr/bin/env python3
"""Every step of a loop pass, with how long it took, read from the pass's own omp session.

A pass is one `omp -p` run, and omp records its whole session as JSONL under
`~/.omp/agent/sessions/`: each model turn, each tool call's start (`tool_execution_start`) and end
(`toolResult`), and each background job's completion (`async-result`, with its `durationMs`). This
turns that record into a timeline, so the slow steps of a loop can be found and optimised. Nothing has
to remember to log: a pass that is killed still leaves its session, and an old pass can be read after
the fact (`backfill`).

What a row is:

- `model`: the model thinking and writing between the end of the last step and its next call.
- `tool`: one tool call, from its start to its result (a batch of parallel calls is one row each, so
  their seconds overlap; `wall` in the report counts the batch once).
- `wait`: the pass ended its turn and sat until a background job delivered (the job's own run time is
  in the job's `bg` row).
- `bg`: a background job, from launch to completion, as the job reported it.

Each row also carries a `category` (what kind of work: `cargo test`, `push gate`, `remote wait`, ...)
and a `phase`. Phases are marked by the pass itself with `python3 tools/loop/steps.py mark <phase>`
(`build-loop.md` says when); rows before the first mark are phase `orient`.

    steps.py mark build                         # a pass marks a phase (prints one line)
    steps.py record --repo DIR --since EPOCH --pass N --state STATEDIR
                                                # the driver, after a pass: append rows to STATEDIR/steps.tsv,
                                                # and its host, wall time and settings to STATEDIR/config.tsv
    steps.py backfill --repo DIR --state STATEDIR [--last K]
                                                # every past session of that clone, numbered by start time
    steps.py report --state STATEDIR [--last K] # where the time goes, across the last K passes
    steps.py audit --repo DIR --from SHA --pass N --state STATEDIR
                                                # the driver, after a pass: did it commit, push, release,
                                                # document and demo? one row in STATEDIR/audit.tsv
    steps.py publish --repo DIR --state STATEDIR [--branch loop-stats]
                                                # the driver, after record: push steps/<clone>.tsv and
                                                # steps/<clone>.md to the repo's loop-stats branch

Stdlib only.
"""

import argparse
import collections
import datetime as dt
import glob
import json
import os
import re
import sys
from pathlib import Path

SESSIONS = Path(os.path.expanduser("~/.omp/agent/sessions"))
COLUMNS = ["pass", "seq", "phase", "kind", "category", "tool", "start", "seconds", "detail"]
MARK = re.compile(r"steps\.py\s+mark\s+([A-Za-z0-9_.:-]+)")

# First match wins: the command text of a bash call (or a background job's label) to a category.
CATEGORIES = [
    ("phase mark", r"steps\.py\s+mark"),
    ("push gate", r"git\b[^\n]*\bpush\b"),
    ("remote wait", r"^\s*(sleep\s+\d+\s*;|until\b|while\b).*(ssh|log|tail|grep)"),
    ("sleep", r"^\s*sleep\s+\d+"),
    ("cargo test", r"cargo\s+(\+\S+\s+)?(nextest|test)\b"),
    ("cargo clippy", r"cargo\s+(\+\S+\s+)?clippy\b"),
    ("cargo build", r"cargo\s+(\+\S+\s+)?(build|check|run|doc)\b"),
    ("cargo publish", r"cargo\s+(\+\S+\s+)?publish\b"),
    ("clip record/render", r"tools/demo/(record|render)\.py|ffmpeg|changelog\.py\s+clip"),
    ("loop tools", r"tools/loop/|tools/changelog/"),
    ("remote command", r"^\s*ssh\b|\bssh\s+(big|big_mac|bmb|mac_air|sib)\b"),
    ("github", r"\bgh\s+(issue|pr|api|run|label|repo)\b"),
    ("git", r"^\s*(cd\s+\S+\s*&&\s*)?git\b"),
    ("homestill", r"home.?still|distill|scribe"),
    ("shell", r"."),
]


def parse_ts(s):
    return dt.datetime.fromisoformat(s.replace("Z", "+00:00"))


WAITED_ON = [("gate", r"gate|push"), ("tests", r"test"), ("build", r"build|compil|link|target"),
             ("release", r"release|publish"), ("clip", r"clip|record|render|capture"),
             ("review", r"review"), ("subagent", r"agent|task")]


def category(tool, text, intent=""):
    if tool == "wait":
        for name, pattern in WAITED_ON:
            if re.search(pattern, intent or "", re.I):
                return f"wait: {name}"
        return "wait: other"
    if tool != "bash":
        return {"read": "read files", "grep": "search", "find": "search", "glob": "search",
                "edit": "edit files", "write": "edit files", "ast_edit": "edit files",
                "task": "subagents", "web_search": "web"}.get(tool, tool)
    for name, pattern in CATEGORIES:
        if re.search(pattern, text or "", re.S | re.M):
            return name
    return "shell"


def detail_of(tool, args):
    """The step's stated intent (omp's `i`) and what it ran or read."""
    if not isinstance(args, dict):
        return str(args)[:160]
    intent = str(args.get("i", "")).strip()
    for key in ("command", "path", "pattern", "query", "input"):
        if key in args:
            what = " ".join(str(args[key]).split())
            return ((intent + " :: ") if intent else "") + what[:160]
    return intent or " ".join(json.dumps(args).split())[:160]


def read_session(path):
    """The session's entries, in file order (which is time order)."""
    out = []
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            try:
                out.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return out


def session_cwd(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            for _, line in zip(range(5), f):
                o = json.loads(line)
                if o.get("type") == "session":
                    return o.get("cwd"), parse_ts(o["timestamp"])
    except (OSError, json.JSONDecodeError, KeyError, ValueError):
        pass
    return None, None


def sessions_for(repo):
    """Every session whose cwd is `repo`, oldest first: (start, path)."""
    repo = os.path.realpath(repo)
    found = []
    for path in glob.glob(str(SESSIONS / "*" / "*.jsonl")):
        cwd, start = session_cwd(path)
        if cwd and start and os.path.realpath(cwd) == repo:
            found.append((start, path))
    return sorted(found)


def steps(entries):
    """The timeline of one session: a list of row dicts (without `pass`)."""
    rows, calls = [], {}
    phase = "orient"
    last_end = None
    turn_ended_at = None
    for o in entries:
        kind = o.get("type")
        ts = o.get("timestamp")
        if not ts:
            continue
        t = parse_ts(ts)
        if kind == "message":
            m = o.get("message", {})
            role = m.get("role")
            if role == "user":
                last_end = t
            elif role == "assistant":
                if last_end is not None:
                    rows.append(dict(phase=phase, kind="model", category="model", tool="", start=last_end,
                                     seconds=(t - last_end).total_seconds(), detail=""))
                last_end = t
                content = m.get("content") if isinstance(m.get("content"), list) else []
                has_call = False
                for c in content:
                    if c.get("type") == "toolCall":
                        has_call = True
                        args = c.get("arguments") or {}
                        text = args.get("command", "") if isinstance(args, dict) else ""
                        hit = MARK.search(text)
                        if hit:
                            phase = hit.group(1)
                        calls[c.get("id")] = dict(phase=phase, tool=c.get("name", ""), args=args, start=t)
                turn_ended_at = None if has_call else t
            elif role == "toolResult":
                call = calls.pop(m.get("toolCallId"), None)
                if call is None:
                    continue
                details = m.get("details") or {}
                job = (details.get("async") or {}).get("jobId") if isinstance(details, dict) else None
                text = call["args"].get("command", "") if isinstance(call["args"], dict) else ""
                intent = call["args"].get("i", "") if isinstance(call["args"], dict) else ""
                rows.append(dict(phase=call["phase"], kind="tool", category=category(call["tool"], text, intent),
                                 tool=call["tool"], start=call["start"],
                                 seconds=(t - call["start"]).total_seconds(),
                                 detail=(f"[launched {job}] " if job else "") + detail_of(call["tool"], call["args"])))
                last_end = t
        elif kind == "custom" and o.get("customType") == "tool_execution_start":
            data = o.get("data") or {}
            call = calls.get(data.get("toolCallId"))
            if call and data.get("startedAt"):
                call["start"] = parse_ts(data["startedAt"])
        elif kind == "custom_message" and o.get("customType") == "async-result":
            for job in ((o.get("details") or {}).get("jobs") or []):
                secs = (job.get("durationMs") or 0) / 1000
                label = job.get("label", "")
                rows.append(dict(phase=phase, kind="bg", category=category(job.get("type", "bash"), label),
                                 tool=job.get("type", ""), start=t - dt.timedelta(seconds=secs), seconds=secs,
                                 detail=f"{job.get('jobId', '')} {job.get('status', '')}: " + " ".join(label.split())[:150]))
            if turn_ended_at is not None:
                rows.append(dict(phase=phase, kind="wait", category="wait for background job", tool="",
                                 start=turn_ended_at, seconds=(t - turn_ended_at).total_seconds(), detail=""))
                turn_ended_at = None
            last_end = t
    for seq, r in enumerate(rows, start=1):
        r["seq"] = seq
    return rows


def pass_starts(driver_log):
    """`pass N START` lines of a driver log: [(utc start, N)]. The log writes local time."""
    out = []
    try:
        with open(driver_log, encoding="utf-8", errors="replace") as f:
            lines = f.readlines()
        for line in lines:
            m = re.match(r"(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d) pass (\d+) START\b", line)
            if m:
                local = dt.datetime.fromisoformat(m.group(1)).astimezone()
                out.append((local.astimezone(dt.timezone.utc), int(m.group(2))))
    except OSError:
        pass
    return out


def pass_for(session_start, starts, fallback):
    """The driver's pass whose START is the last one at or before the session began (within 5 min)."""
    best = None
    for start, n in starts:
        if start <= session_start + dt.timedelta(seconds=5) and session_start - start <= dt.timedelta(minutes=5):
            best = n
    return best if best is not None else fallback


def drop_passes(path, passes):
    """Remove the rows of the given passes, so rewriting a pass replaces it rather than doubling it."""
    if not path.exists():
        return
    lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    keep = [l for i, l in enumerate(lines) if i == 0 or l.split("\t", 1)[0] not in {str(p) for p in passes}]
    if len(keep) != len(lines):
        path.write_text("".join(keep), encoding="utf-8")


def write_rows(path, pass_no, rows):
    new = not path.exists()
    with open(path, "a", encoding="utf-8") as f:
        if new:
            f.write("\t".join(COLUMNS) + "\n")
        for r in rows:
            f.write("\t".join([str(pass_no), str(r["seq"]), r["phase"], r["kind"], r["category"], r["tool"],
                               r["start"].strftime("%Y-%m-%dT%H:%M:%SZ"), f"{r['seconds']:.1f}",
                               r["detail"].replace("\t", " ")]) + "\n")


CONFIG_COLUMNS = ["pass", "at", "host", "seconds", "model", "think", "build_jobs", "test_threads"]
CONFIG_ENV = ["WIGGUM_MODEL", "WIGGUM_THINK", "CARGO_BUILD_JOBS", "RUST_TEST_THREADS"]


def write_config(state, pass_no, since):
    """One row in STATE/config.tsv: where the pass ran and with what settings, so passes can be compared
    by host, model, thinking level and build parallelism. `record` runs right after the pass, so the
    wall time is now minus the pass's start. A re-recorded pass replaces its row."""
    import socket
    import time
    path = Path(state) / "config.tsv"
    path.parent.mkdir(parents=True, exist_ok=True)
    drop_passes(path, {pass_no})
    new = not path.exists()
    env = [os.environ.get(k) or "-" for k in CONFIG_ENV]
    with open(path, "a", encoding="utf-8") as f:
        if new:
            f.write("\t".join(CONFIG_COLUMNS) + "\n")
        f.write("\t".join([str(pass_no), dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%MZ"),
                           socket.gethostname().split(".")[0], str(round(time.time() - since)), *env]) + "\n")


def config_report(state, out=sys.stdout):
    """The passes grouped by (host, model, thinking, build jobs, test threads): how many, mean and median
    minutes, in the order each group first appeared."""
    import statistics
    path = Path(state) / "config.tsv"
    if not path.exists():
        print("no configuration recorded yet", file=out)
        return
    groups = {}
    for r in load_tsv(path):
        key = (r["host"], r["model"], r["think"], r["build_jobs"], r["test_threads"])
        groups.setdefault(key, []).append(float(r["seconds"]) / 60)
    print(f"{'host':<10} {'model':<28} {'think':<8} {'jobs':<5} {'threads':<8} {'passes':>6} {'mean min':>9} {'median min':>11}", file=out)
    for (host, model, think, jobs, threads), mins in groups.items():
        print(f"{host:<10} {model:<28} {think:<8} {jobs:<5} {threads:<8} {len(mins):>6} "
              f"{statistics.mean(mins):>9.1f} {statistics.median(mins):>11.1f}", file=out)


def summary(rows, top=3):
    total = sum(r["seconds"] for r in rows if r["kind"] in ("model", "tool", "wait"))
    by = collections.Counter()
    for r in rows:
        if r["kind"] in ("model", "tool", "wait"):
            by[r["category"]] += r["seconds"]
    worst = ", ".join(f"{c} {s:.0f}s" for c, s in by.most_common(top))
    return f"{len(rows)} steps, {total:.0f}s; most time: {worst}"


def load_tsv(path):
    with open(path, encoding="utf-8") as f:
        head = f.readline().rstrip("\n").split("\t")
        for line in f:
            cells = line.rstrip("\n").split("\t")
            if len(cells) == len(head):
                row = dict(zip(head, cells))
                row["seconds"] = float(row["seconds"])
                yield row


def report(state, last, out=sys.stdout):
    path = Path(state) / "steps.tsv"
    rows = list(load_tsv(path))
    passes = sorted({int(r["pass"]) for r in rows})
    if last:
        keep = set(passes[-last:])
        rows = [r for r in rows if int(r["pass"]) in keep]
        passes = sorted(keep)
    timed = [r for r in rows if r["kind"] in ("model", "tool", "wait")]
    total = sum(r["seconds"] for r in timed) or 1.0
    print(f"{len(passes)} pass(es), {len(rows)} steps, {total / 3600:.1f} h of pass time\n", file=out)

    def table(title, key):
        by, n = collections.Counter(), collections.Counter()
        for r in timed:
            by[key(r)] += r["seconds"]
            n[key(r)] += 1
        print(f"{title:<28} {'hours':>7} {'share':>6} {'steps':>6} {'mean s':>7}", file=out)
        for k, s in by.most_common(15):
            print(f"{k:<28} {s / 3600:7.2f} {100 * s / total:5.1f}% {n[k]:6d} {s / n[k]:7.1f}", file=out)
        print(file=out)

    table("category", lambda r: r["category"])
    table("phase", lambda r: r["phase"])
    print("slowest single steps", file=out)
    for r in sorted(rows, key=lambda r: -r["seconds"])[:15]:
        print(f"  pass {r['pass']:>4} {r['seconds']:7.0f}s  {r['kind']:<5} {r['category']:<20} {r['detail'][:90]}", file=out)


def git(repo, *args, env=None, input=None):
    import subprocess
    return subprocess.run(["git", "-C", repo, *args], capture_output=True, text=True, env=env, input=input)


AUDIT_COLUMNS = ["pass", "at", "commits", "unpushed", "package_files", "released", "docs_files", "demo_files", "missing"]
PACKAGE = re.compile(r"^(src/|crates/[^/]+/src/|crates/[^/]+/Cargo\.toml$|Cargo\.toml$|build\.rs$|crates/[^/]+/build\.rs$|assets/[^/]*shader|.*\.wgsl$|.*\.wesl$)")
DOCS = re.compile(r"(^|/)(README[^/]*\.md|CHANGELOG\.md)$|^docs/(?!loop/|loop-log)|^crates/[^/]+/(README|CHANGELOG)\.md$")
DEMO = re.compile(r"\.(gif|mp4|webm|png|jpg|webp)$", re.I)


def unreleased_crates(repo, head, package, tags):
    """The published crates whose shipped files changed with no release tag for them in the range.

    A workspace tags each crate `<crate>-v<version>`; a single-crate repo tags `v<version>` and is named
    `.` here. A crate whose manifest says `publish = false` ships nothing, so it needs no release."""
    crates = {}
    for f in package:
        m = re.match(r"crates/([^/]+)/", f)
        crates.setdefault(m.group(1) if m else ".", True)
    out = []
    for crate in sorted(crates):
        manifest = "Cargo.toml" if crate == "." else f"crates/{crate}/Cargo.toml"
        text = git(repo, "show", f"{head}:{manifest}").stdout
        if re.search(r"(?m)^\s*publish\s*=\s*false", text) or not re.search(r"(?m)^\[package\]", text):
            continue  # unpublished, or a workspace root that is no crate
        name = re.search(r'(?m)^\s*name\s*=\s*"([^"]+)"', text)
        want = "v" if crate == "." else f"{name.group(1) if name else crate}-v"
        if not any(t_.startswith(want) or (crate == "." and re.match(r"v\d", t_)) for t_ in tags):
            out.append(crate if crate == "." else (name.group(1) if name else crate))
    return out


def audit(repo, start, pass_no, state):
    """What a pass shipped, read from git: commits it put on origin/main, whether any are still only
    local, whether it changed the package and tagged a release, and whether it touched docs and demos.
    `missing` names what the commands require and the pass did not do."""
    git(repo, "fetch", "-q", "--tags", "origin")
    head = git(repo, "rev-parse", "origin/main").stdout.strip()
    rng = f"{start}..{head}"
    commits = int(git(repo, "rev-list", "--count", rng).stdout.strip() or 0)
    unpushed = int(git(repo, "rev-list", "--count", "origin/main..HEAD").stdout.strip() or 0) \
        if git(repo, "symbolic-ref", "-q", "--short", "HEAD").stdout.strip() == "main" else 0
    files = [f for f in git(repo, "diff", "--name-only", rng).stdout.split() if f]
    added = [f for f in git(repo, "diff", "--name-only", "--diff-filter=AM", rng).stdout.split() if f]
    package = [f for f in files if PACKAGE.search(f)]
    docs = [f for f in files if DOCS.search(f)]
    demo = [f for f in added if DEMO.search(f) and not f.startswith(("docs/loop/", "target/"))]
    tags = [x for x in git(repo, "tag", "--merged", head, "--no-merged", start).stdout.split() if x]
    missing = []
    if unpushed:
        missing.append("push")
    unreleased = unreleased_crates(repo, head, package, tags)
    if unreleased:
        missing.append("release(" + " ".join(unreleased) + ")" if unreleased != ["."] else "release")
    if package and not docs:
        missing.append("docs")
    if package and not demo:
        missing.append("demo?")  # a change nobody can see needs none; a visible one does
    if commits == 0 and not unpushed:
        missing.append("nothing shipped")
    row = [str(pass_no), dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%MZ"), str(commits), str(unpushed),
           str(len(package)), ",".join(tags) or "-", str(len(docs)), str(len(demo)), ",".join(missing) or "-"]
    path = Path(state) / "audit.tsv"
    new = not path.exists()
    with open(path, "a", encoding="utf-8") as f:
        if new:
            f.write("\t".join(AUDIT_COLUMNS) + "\n")
        f.write("\t".join(row) + "\n")
    return (f"audit: {commits} commit(s) on main, {unpushed} unpushed, {len(package)} package file(s), "
            f"released {row[5]}, {len(docs)} doc file(s), {len(demo)} demo file(s); missing: {row[8]}")


def publish(repo, state, branch):
    """Commit this clone's step log and its report to `branch` and push it, without touching the clone.

    The loop's working tree, index and `main` are the pass's; the driver writes nothing there (a loop's
    bookkeeping must never appear in a pass's `git status`). So this builds the commit with plumbing in a
    throwaway index and pushes it to a branch of its own, which only this writes. The pre-push gate guards
    `main` only, so it does not run here.
    """
    import io
    import tempfile
    name = os.path.basename(os.path.realpath(repo))
    tsv = Path(state) / "steps.tsv"
    if not tsv.exists():
        return f"steps: nothing to publish ({tsv} missing)"
    buf = io.StringIO()
    buf.write(f"# Where {name}'s time goes\n\nWritten by `tools/loop/steps.py publish` after every pass; ")
    buf.write(f"the rows are in [`{name}.tsv`]({name}.tsv). Updated {dt.datetime.now(dt.timezone.utc):%Y-%m-%d %H:%MZ}.\n\n")
    audit_tsv = Path(state) / "audit.tsv"
    if audit_tsv.exists():
        lines = audit_tsv.read_text(encoding="utf-8").splitlines()
        buf.write(f"## What the last passes shipped\n\nFrom [`{name}-audit.tsv`]({name}-audit.tsv): commits on main, ")
        buf.write("unpushed commits, package files changed, release tags, doc and demo files, and what was missing.\n\n")
        head, rows = lines[0].split("\t"), [l.split("\t") for l in lines[1:]][-12:]
        buf.write("| " + " | ".join(head) + " |\n|" + "---|" * len(head) + "\n")
        for r in rows:
            buf.write("| " + " | ".join(r) + " |\n")
        buf.write("\n")
    if (Path(state) / "config.tsv").exists():
        buf.write(f"## How each configuration did\n\nFrom [`{name}-config.tsv`]({name}-config.tsv): one row per pass with its host, ")
        buf.write("wall time, model, thinking level and build settings.\n\n```text\n")
        config_report(state, out=buf)
        buf.write("```\n\n")
    for title, last in (("The last 12 passes", 12), ("Every pass recorded", 0)):
        buf.write(f"## {title}\n\n```text\n")
        report(state, last, out=buf)
        buf.write("```\n\n")
    ref = f"refs/remotes/origin/{branch}"
    for attempt in (1, 2):
        git(repo, "fetch", "-q", "origin", f"+refs/heads/{branch}:{ref}")
        base = git(repo, "rev-parse", "-q", "--verify", f"{ref}^{{commit}}").stdout.strip()
        with tempfile.TemporaryDirectory() as d:
            env = dict(os.environ, GIT_INDEX_FILE=os.path.join(d, "index"))
            if base:
                git(repo, "read-tree", base, env=env)
            files = [(f"steps/{name}.tsv", tsv.read_text(encoding="utf-8")), (f"steps/{name}.md", buf.getvalue())]
            config_tsv = Path(state) / "config.tsv"
            if config_tsv.exists():
                files.append((f"steps/{name}-config.tsv", config_tsv.read_text(encoding="utf-8")))
            audit_tsv = Path(state) / "audit.tsv"
            if audit_tsv.exists():
                files.append((f"steps/{name}-audit.tsv", audit_tsv.read_text(encoding="utf-8")))
            for path, text in files:
                blob = git(repo, "hash-object", "-w", "--stdin", input=text).stdout.strip()
                git(repo, "update-index", "--add", "--cacheinfo", f"100644,{blob},{path}", env=env)
            tree = git(repo, "write-tree", env=env).stdout.strip()
        if base and git(repo, "rev-parse", f"{base}^{{tree}}").stdout.strip() == tree:
            return f"steps: {branch} already up to date"
        args = ["commit-tree", tree, "-m", f"steps: {name}"] + (["-p", base] if base else [])
        commit = git(repo, *args, env=dict(os.environ, GIT_AUTHOR_NAME="wiggum", GIT_AUTHOR_EMAIL="wiggum@localhost",
                                           GIT_COMMITTER_NAME="wiggum", GIT_COMMITTER_EMAIL="wiggum@localhost")).stdout.strip()
        pushed = git(repo, "push", "-q", "origin", f"{commit}:refs/heads/{branch}")
        if pushed.returncode == 0:
            return f"steps: pushed steps/{name}.tsv and .md to {branch} ({commit[:9]})"
    return f"steps: push to {branch} failed: {(pushed.stderr or '').strip()[:160]}"


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    m = sub.add_parser("mark", help="a pass marks the phase it is entering")
    m.add_argument("phase")
    r = sub.add_parser("record", help="append one pass's steps to STATE/steps.tsv")
    r.add_argument("--repo", required=True)
    r.add_argument("--since", type=float, required=True, help="the pass's start, epoch seconds")
    r.add_argument("--pass", dest="pass_no", type=int, required=True)
    r.add_argument("--state", required=True)
    b = sub.add_parser("backfill", help="record every past session of a clone")
    b.add_argument("--repo", required=True)
    b.add_argument("--state", required=True)
    b.add_argument("--last", type=int, default=0)
    p = sub.add_parser("report", help="where the time goes")
    p.add_argument("--state", required=True)
    p.add_argument("--last", type=int, default=0)
    au = sub.add_parser("audit", help="what a pass shipped: commits, push, release, docs, demos")
    au.add_argument("--repo", required=True)
    au.add_argument("--from", dest="start", required=True, help="origin/main when the pass started")
    au.add_argument("--pass", dest="pass_no", type=int, required=True)
    au.add_argument("--state", required=True)
    u = sub.add_parser("publish", help="push this clone's step log to the repo's loop-stats branch")
    u.add_argument("--repo", required=True)
    u.add_argument("--state", required=True)
    u.add_argument("--branch", default="loop-stats")
    a = ap.parse_args(argv)

    if a.cmd == "mark":
        print(f"phase: {a.phase} at {dt.datetime.now(dt.timezone.utc):%H:%M:%SZ}")
        return 0
    if a.cmd == "record":
        write_config(a.state, a.pass_no, a.since)
        since = dt.datetime.fromtimestamp(a.since - 60, dt.timezone.utc)
        found = [p for start, p in sessions_for(a.repo) if start >= since]
        if not found:
            print(f"steps: no session for {a.repo} since {since:%H:%M:%SZ}")
            return 0
        rows = steps(read_session(found[0]))
        drop_passes(Path(a.state) / "steps.tsv", {a.pass_no})
        write_rows(Path(a.state) / "steps.tsv", a.pass_no, rows)
        print(f"steps: {summary(rows)}")
        return 0
    if a.cmd == "backfill":
        found = sessions_for(a.repo)
        if a.last:
            found = found[-a.last:]
        starts = pass_starts(Path(a.state) / "driver.log")
        out = Path(a.state) / "steps.tsv"
        found = [(pass_for(start, starts, -i), path) for i, (start, path) in enumerate(found, start=1)]
        # A backfill rebuilds the passes it read and keeps every other pass's rows, so running it twice
        # does not double them and `--last K` does not erase older passes.
        drop_passes(out, {n for n, _ in found})
        for n, path in found:
            write_rows(out, n, steps(read_session(path)))
        print(f"steps: backfilled {len(found)} session(s) into {out}")
        return 0
    if a.cmd == "report":
        report(a.state, a.last)
        return 0
    if a.cmd == "audit":
        print(audit(a.repo, a.start, a.pass_no, a.state))
        return 0
    if a.cmd == "publish":
        print(publish(a.repo, a.state, a.branch))
        return 0
    return 2


if __name__ == "__main__":
    sys.exit(main())
