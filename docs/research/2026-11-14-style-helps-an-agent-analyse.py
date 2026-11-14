#!/usr/bin/env python3
"""The pre-registered analysis of #409 (docs/research/2026-11-14-style-helps-an-agent.md): lines satisfied per run, with-style against without-style.

usage:  analyse.py <without run>[,<run>...] <with run>[,<run>...]     two or more run names per arm are combined (a round-by-round run, or a repeated unfit arm)
        analyse.py --selftest                                          the script on synthetic data, with known answers
        analyse.py --routes <run>[,<run>...]                           each trial's route, citations and attempts at the hidden tool (the probe: READ this and check it by hand against the transcript before the 30 runs)
        analyse.py --assemble <name> <run>,<run>...                    one run directory <name> from one-trial rounds (trials renumbered, headers merged, refused if the rounds were run differently), for `report` and `compare`

Python 3 standard library only.  It reads `trial.json` (the runner's record) and `transcript.jsonl` (the agent's own stream) under ~/.local/state/cauldron-bench/<run>/.
"""
import itertools, json, math, os, random, re, shutil, sys, tempfile
from collections import Counter
from fractions import Fraction
from pathlib import Path

STATE = Path(os.environ["HOME"]) / ".local/state/cauldron-bench"
SEED, RESAMPLES, LOW, HIGH = 0x5EED_D4A2_B3C1_0413, 10_000, 0.025, 0.975   # the runner's BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED, INTERVAL_LOW, INTERVAL_HIGH
TOOL_SHARE_MIN = 0.70            # the share of with-style runs that must have called `grimoire_style` for a claim about the tool
# 0-based positions of the checks that are guards, not lines, and how many lines each task has (see "The lines" in the pre-registration).
GUARDS = {"dress-the-market-area": {0, 10}, "light-the-gate-at-night": {0, 10}, "write-the-gatekeepers-greeting": {0}}
LINES_PER_TASK = {"dress-the-market-area": 9, "light-the-gate-at-night": 9, "write-the-gatekeepers-greeting": 6}
GENERIC_TOOLS = {"read", "bash", "grep", "find", "glob", "eval", "ast_edit", "edit", "lsp"}   # the harness's own tools that can open a file
ROUTES = ("tool", "file", "both", "neither")


def percentile(sorted_values, p):
    """Linear interpolation between the two nearest ranks: the runner's `stats::percentile`."""
    pos = p * (len(sorted_values) - 1)
    below, above = math.floor(pos), math.ceil(pos)
    return sorted_values[below] + (sorted_values[above] - sorted_values[below]) * (pos - below)


def mean(xs):
    return sum(xs) / len(xs)


def events(transcript):
    """The agent's calls in the order it made them: ('mcp', tool name, args) for an editor tool, ('generic', tool name, arguments) for the harness's own."""
    if not transcript.exists():
        return []
    out = []
    for row in transcript.read_text().splitlines():
        try:
            e = json.loads(row)
        except ValueError:
            continue
        xdev = ((e.get("result") or {}).get("details") or {}).get("xdev")
        if e.get("type") == "tool_execution_end" and xdev:
            name = ((xdev.get("inner") or {}).get("mcpToolName"))
            if name:
                out.append(("mcp", name, xdev.get("args")))
        inner = e.get("assistantMessageEvent") or {}
        if e.get("type") == "message_update" and inner.get("type") == "toolcall_end":
            call = inner.get("toolCall") or {}
            args = call.get("arguments") or {}
            if call.get("name") in GENERIC_TOOLS and not str(args.get("path", "")).startswith("xd://mcp__"):
                out.append(("generic", call["name"], args))
            elif call.get("name") in ("read", "write") and str(args.get("path", "")).startswith("xd://mcp__"):
                out.append(("device", call["name"], args))   # an editor tool asked for by its device path, which the transcript shows before the tool's own answer
    return out


def hidden_attempts(calls):
    """(writes, reads) of the device path of `grimoire_style` itself (never `grimoire_style_propose`): an agent calling, or looking up, a tool; in the without-style arm that tool is absent."""
    own = [(name, args) for kind, name, args in calls if kind == "device" and re.search(r"grimoire_style(?!_)", str(args.get("path", "")))]
    return sum(n == "write" for n, _ in own), sum(n == "read" for n, _ in own)


def route_of(calls):
    """`tool`: called grimoire_style and opened no design page by a generic tool; `file`: the reverse; `both`; `neither`."""
    tool = any(kind == "mcp" and name == "grimoire_style" for kind, name, _ in calls)
    file = any(kind == "generic" and re.search(r"design[/\\]", json.dumps(args)) for kind, _, args in calls)
    return {(True, False): "tool", (False, True): "file", (True, True): "both", (False, False): "neither"}[(tool, file)]


def cited_followed(calls):
    """Whether the agent's own `grimoire_present` call carried a non-empty `followed` list (never a tool result or the agent's prose)."""
    return any(kind == "mcp" and name == "grimoire_present" and isinstance(args, dict) and args.get("followed") for kind, name, args in calls)


def load(runs, state=None):
    """{task: [run records]} for one arm over one or more run names; a harness error is counted and never scored."""
    cells, errors = {t: [] for t in GUARDS}, 0
    for run in runs.split(","):
        for task, guards in GUARDS.items():
            for path in sorted(((state or STATE) / run / task).glob("trial-*/trial.json")):
                t = json.loads(path.read_text())
                if t["outcome"] == "harness_error":
                    errors += 1
                    continue
                assert len(t["checks"]) == len(guards) + LINES_PER_TASK[task], f"{path}: the task's checks are not the registered ones"
                calls = events(path.parent / "transcript.jsonl")
                cells[task].append({
                    "lines": sum(c["passed"] for i, c in enumerate(t["checks"]) if i not in guards),
                    "route": route_of(calls), "followed": cited_followed(calls),
                    "style_before_edit": next((n == "grimoire_style" for k, n, _ in calls if k == "mcp" and n in ("grimoire_style", "grimoire_apply_operation")), False),
                    "wishes": t["wishes"], "offered": t["tools_offered"],
                    "hidden_writes": hidden_attempts(calls)[0], "hidden_reads": hidden_attempts(calls)[1],
                })
    return cells, errors


def usable(without, with_):
    """The tasks with at least one scored run in BOTH arms. A task with none in an arm is unfit and is left out of both arms' M, and said so."""
    return [t for t in GUARDS if without[t] and with_[t]]


def difference(without, with_, tasks):
    return sum(mean([r["lines"] for r in with_[t]]) - mean([r["lines"] for r in without[t]]) for t in tasks)


def bootstrap(without, with_, tasks, rng, resamples=RESAMPLES):
    """95% percentile interval of D: the runs of every task-and-arm cell are resampled with replacement; the tasks are fixed."""
    w = {t: [r["lines"] for r in with_[t]] for t in tasks}
    o = {t: [r["lines"] for r in without[t]] for t in tasks}
    draws = sorted(sum(mean([rng.choice(w[t]) for _ in w[t]]) - mean([rng.choice(o[t]) for _ in o[t]]) for t in tasks) for _ in range(resamples))
    return percentile(draws, LOW), percentile(draws, HIGH)


def permutation_p(without, with_, tasks):
    """The exact two-sided permutation p-value: every way of giving each task's runs to the two arms (the arm labels exchanged within a task), convolved over the tasks."""
    total = Counter({Fraction(0): 1})
    for t in tasks:
        runs = [r["lines"] for r in with_[t]] + [r["lines"] for r in without[t]]
        n_w, n_o = len(with_[t]), len(without[t])
        per_task = Counter()
        for pick in itertools.combinations(range(len(runs)), n_w):
            s_w = sum(runs[i] for i in pick)
            per_task[Fraction(s_w, n_w) - Fraction(sum(runs) - s_w, n_o)] += 1
        nxt = Counter()
        for (a, x), (b, y) in itertools.product(total.items(), per_task.items()):
            nxt[a + b] += x * y
        total = nxt
    observed = sum(Fraction(sum(r["lines"] for r in with_[t]), len(with_[t])) - Fraction(sum(r["lines"] for r in without[t]), len(without[t])) for t in tasks)
    return float(Fraction(sum(c for d, c in total.items() if abs(d) >= abs(observed)), sum(total.values())))


def analyse(without, with_, errors, rng=None):
    """Every number the pre-registration reports, as a dict."""
    rng = rng or random.Random(SEED)
    tasks = usable(without, with_)
    out = {"errors": errors, "tasks": tasks, "excluded": [t for t in GUARDS if t not in tasks]}
    if not tasks:
        return out
    out["max_lines"] = sum(LINES_PER_TASK[t] for t in tasks)
    out["m_without"] = sum(mean([r["lines"] for r in without[t]]) for t in tasks)
    out["m_with"] = sum(mean([r["lines"] for r in with_[t]]) for t in tasks)
    out["d"] = out["m_with"] - out["m_without"]
    out["lo"], out["hi"] = bootstrap(without, with_, tasks, rng)
    out["verdict"] = "better" if out["lo"] > 0 else "worse" if out["hi"] < 0 else "no measurable change"
    out["p_permutation"] = permutation_p(without, with_, tasks)
    runs = [r for t in tasks for r in with_[t]]
    out["routes"] = {k: sum(r["route"] == k for r in runs) for k in ROUTES}
    out["tool_share"] = (out["routes"]["tool"] + out["routes"]["both"]) / len(runs)        # the runs that called the tool, whether or not they also read a page
    out["tool_only_share"] = out["routes"]["tool"] / len(runs)                              # the runs that called the tool and opened no page: reported beside it
    out["tool_claim_allowed"] = out["verdict"] == "better" and out["tool_share"] >= TOOL_SHARE_MIN
    out["by_route"] = {}
    for k in ROUTES:   # D of the with-style runs that took one route against every without-style run, over the tasks that have both
        sub = {t: [r for r in with_[t] if r["route"] == k] for t in tasks}
        ts = [t for t in tasks if sub[t]]
        if ts:
            lo, hi = bootstrap(without, sub, ts, random.Random(SEED))
            out["by_route"][k] = {"n": sum(len(sub[t]) for t in ts), "tasks": ts, "d": difference(without, sub, ts), "lo": lo, "hi": hi}
    out["style_before_edit"] = sum(r["style_before_edit"] for r in runs)
    out["followed"] = sum(r["followed"] for r in runs)
    out["wishes"] = [(t, w) for t in tasks for r in with_[t] for w in r["wishes"]]
    out["without_offered_style"] = sum("grimoire_style" in r["offered"] for t in tasks for r in without[t])
    out["without_attempts"] = {"runs": sum(r.get("hidden_writes", 0) > 0 for t in tasks for r in without[t]), "writes": sum(r.get("hidden_writes", 0) for t in tasks for r in without[t]),
                               "reads": sum(r.get("hidden_reads", 0) for t in tasks for r in without[t]), "of": sum(len(without[t]) for t in tasks)}
    out["per_task"] = {t: (mean([r["lines"] for r in without[t]]), mean([r["lines"] for r in with_[t]]), len(without[t]), len(with_[t])) for t in tasks}
    return out


def report(a):
    print(f"harness errors (not scored): {a['errors']}")
    if a["excluded"]:
        print(f"UNFIT, left out of both arms (a task with no scored run in an arm): {', '.join(a['excluded'])}")
    if not a["tasks"]:
        print("no task has a scored run in both arms: there is no result")
        return
    for t, (o, w, n_o, n_w) in a["per_task"].items():
        print(f"{t}: without {o:.2f}/{LINES_PER_TASK[t]}  with {w:.2f}/{LINES_PER_TASK[t]}  ({n_o} and {n_w} runs)")
    print(f"lines satisfied per task-triple (max {a['max_lines']}): without {a['m_without']:.2f}  with {a['m_with']:.2f}")
    print(f"PRIMARY  difference with - without: {a['d']:+.2f}  95% bootstrap interval [{a['lo']:+.2f}, {a['hi']:+.2f}]  ->  {a['verdict']}")
    print(f"second reading, exact permutation test (arm labels exchanged within a task), two-sided p = {a['p_permutation']:.4f}")
    print("route of the with-style runs: " + ", ".join(f"{k} {v}" for k, v in a["routes"].items()) + f"  (called grimoire_style, with or without a page: {a['tool_share']:.0%}; the tool alone, no page: {a['tool_only_share']:.0%}; needed {TOOL_SHARE_MIN:.0%} of the first)")
    for k, r in a["by_route"].items():
        print(f"  D over the {r['n']} with-style runs by route `{k}` ({len(r['tasks'])} tasks): {r['d']:+.2f}  [{r['lo']:+.2f}, {r['hi']:+.2f}]  (descriptive, a thin cell)")
    print("CLAIM: " + ("`grimoire_style` changes what an agent builds: the tool share was met and D is better" if a["tool_claim_allowed"] else "the style, available to the agent, " + ("changed what it built" if a["verdict"] == "better" else "did not measurably change what it built") + "; nothing is claimed about the tool in particular"))
    print(f"with-style runs that called grimoire_style before their first edit: {a['style_before_edit']} of {sum(a['routes'].values())}")
    print(f"with-style runs whose own grimoire_present call cited followed lines: {a['followed']} of {sum(a['routes'].values())}")
    print(f"without-style runs offered grimoire_style (must be 0): {a['without_offered_style']}")
    w = a["without_attempts"]
    print(f"without-style runs that tried the hidden grimoire_style by its device path: {w['runs']} of {w['of']} ({w['writes']} writes, {w['reads']} reads)")
    print(f"wishes made by with-style runs: {len(a['wishes'])}")
    for task, w in a["wishes"]:
        print("  -", task, json.dumps(w))



SAME_IN_EVERY_ROUND = ("commit", "guide_sha256", "agent", "agent_version", "model", "variant", "suite", "host")


def assemble(dest, runs, state=None):
    """One run directory `dest` from the one-trial rounds `runs`, for `cauldron-bench report` and `compare` (which read one run per arm).

    The trials of each task are renumbered in round order and only `trial.json` and `transcript.jsonl` are kept. The merged `run.json` is the first round's with `label` = dest and
    `trials` = the rounds per task; it is only made if every round was run alike (the editor commit, the guide, the agent and its version, the model, the arm, the suite and the host
    are equal), because a header that said one commit over trials of two would be false. The date is the first round's."""
    state = state or STATE
    heads = [json.loads((state / r / "run.json").read_text()) for r in runs]
    for key in SAME_IN_EVERY_ROUND:
        values = sorted({json.dumps(h.get(key)) for h in heads})
        if len(values) > 1:
            raise SystemExit(f"the rounds were not run alike: `{key}` is {', '.join(values)}; rerun them, never assemble them")
    out = state / dest
    if out.exists():
        shutil.rmtree(out)
    counts = {}
    for run in runs:
        for task_dir in sorted(p for p in (state / run).iterdir() if p.is_dir()):
            for trial in sorted(task_dir.glob("trial-*"), key=lambda p: int(p.name.split("-")[1])):
                if not (trial / "trial.json").is_file():
                    continue
                n = counts[task_dir.name] = counts.get(task_dir.name, 0) + 1
                target = out / task_dir.name / f"trial-{n}"
                target.mkdir(parents=True)
                record = json.loads((trial / "trial.json").read_text())
                record["trial"] = n
                (target / "trial.json").write_text(json.dumps(record, indent=2))
                if (trial / "transcript.jsonl").is_file():
                    shutil.copy(trial / "transcript.jsonl", target / "transcript.jsonl")
    head = dict(heads[0], label=dest, trials=max(counts.values()))
    (out / "run.json").write_text(json.dumps(head, indent=2))
    return head, counts


# ---------------------------------------------------------------------------------------------------------------------------------------------
def selftest():
    """The script on synthetic data whose answers are known: a known D, a task with an empty cell, ties, a thin route, and the permutation test."""
    def run(lines, route="neither", followed=False, errored=False):
        return {"lines": lines, "route": route, "followed": followed, "style_before_edit": route in ("tool", "both"), "wishes": [], "offered": []}
    # known D: with-style 9,9,9,8,9 -> 8.8 ; without 5,5,6,5,5 -> 5.2 ; greeting with 6x5 -> 6 ; without 0x5 -> 0 ; light with 9x5 ; without 1x5
    without = {"dress-the-market-area": [run(x) for x in (5, 5, 6, 5, 5)], "write-the-gatekeepers-greeting": [run(0)] * 5, "light-the-gate-at-night": [run(1)] * 5}
    with_ = {"dress-the-market-area": [run(x, "tool") for x in (9, 9, 9, 8, 9)], "write-the-gatekeepers-greeting": [run(6, "both")] * 5, "light-the-gate-at-night": [run(9, "file")] * 5}
    a = analyse(without, with_, 0)
    assert abs(a["d"] - ((8.8 - 5.2) + 6 + 8)) < 1e-9, a["d"]
    assert a["verdict"] == "better" and a["lo"] > 0, a
    assert a["routes"] == {"tool": 5, "file": 5, "both": 5, "neither": 0} and abs(a["tool_share"] - 10 / 15) < 1e-9 and not a["tool_claim_allowed"]   # 67% < 70%
    assert a["by_route"]["file"]["d"] == 8 and a["by_route"]["file"]["tasks"] == ["light-the-gate-at-night"]
    # the exact permutation p-value: fully separated samples of 5 and 5 in a task give 1 assignment of 252 at each extreme, so in three tasks only the all-with and the all-without
    # assignments reach |D| = 9, and the two-sided p is 2 / 252 ** 3
    sep_without = {t: [run(0)] * 5 for t in GUARDS}
    sep_with = {t: [run(3)] * 5 for t in GUARDS}
    p = analyse(sep_without, sep_with, 0)["p_permutation"]
    assert abs(p - 2 / 252 ** 3) < 1e-12, p
    # ties: when every run scores alike the interval is the point, the verdict is "better" only if D > 0, and a null difference is "no measurable change"
    flat = {t: [run(2)] * 5 for t in GUARDS}
    a = analyse(flat, flat, 0)
    assert a["d"] == 0 and a["verdict"] == "no measurable change" and a["p_permutation"] == 1.0, a
    # an empty cell must not crash: the task is unfit, left out of BOTH arms, and said so
    gap = dict(with_)
    gap["write-the-gatekeepers-greeting"] = []
    a = analyse(without, gap, 3)
    assert a["excluded"] == ["write-the-gatekeepers-greeting"] and a["max_lines"] == 18 and abs(a["d"] - ((8.8 - 5.2) + 8)) < 1e-9 and a["errors"] == 3
    assert analyse({t: [] for t in GUARDS}, with_, 0)["tasks"] == []
    # unequal cells (a harness error in one cell) still give an exact p-value and an interval
    uneven = dict(without)
    uneven["dress-the-market-area"] = without["dress-the-market-area"][:3]
    a = analyse(uneven, with_, 2)
    assert 0 < a["p_permutation"] <= 1 and a["lo"] <= a["d"] <= a["hi"] + 1e-9
    # the percentile is the runner's: interpolated between ranks
    assert percentile([0.0, 10.0], 0.25) == 2.5 and percentile([1.0, 2.0, 3.0], 0.5) == 2.0
    # a transcript: the route and the citations come from the agent's own calls, in its own tool arguments only
    mcp = lambda name, args: {"type": "tool_execution_end", "result": {"details": {"xdev": {"args": args, "inner": {"mcpToolName": name}}}}}
    gen = lambda name, args: {"type": "message_update", "assistantMessageEvent": {"type": "toolcall_end", "toolCall": {"name": name, "arguments": args}}}
    assert route_of(events_from([mcp("grimoire_style", {"map": "courtyard"})])) == "tool"
    assert route_of(events_from([gen("read", {"path": "design/style.md"})])) == "file"
    assert route_of(events_from([gen("bash", {"command": "cat design/levels/courtyard/style.md"}), mcp("grimoire_style", {})])) == "both"
    assert route_of(events_from([gen("read", {"path": "assets/grimoire/maps/courtyard.map.yaml"}), mcp("grimoire_style_propose", {})])) == "neither"
    assert route_of(events_from([gen("write", {"path": "xd://mcp__cauldron_grimoire_style", "content": "{}"})])) == "neither"   # the device path is counted as the MCP call, not as a file
    assert cited_followed(events_from([mcp("grimoire_present", {"title": "t", "followed": [{"path": "design/style.md", "line": 3}]})]))
    assert not cited_followed(events_from([mcp("grimoire_present", {"title": "t"}), mcp("grimoire_state", {"followed": [1]}), gen("read", {"path": "followed"})]))
    # editor tools asked for by their device path (`write xd://mcp__<server>_grimoire_style`) are the agent trying a tool: in the without-style arm that tool is absent
    assert hidden_attempts(events_from([gen("write", {"path": "xd://mcp__cauldron_grimoire_style", "content": "{}"})])) == (1, 0)
    assert hidden_attempts(events_from([gen("read", {"path": "xd://mcp__cauldron_grimoire_style"}), gen("write", {"path": "xd://mcp__cauldron_grimoire_style_propose", "content": "{}"})])) == (0, 1)
    assert hidden_attempts(events_from([mcp("grimoire_state", {})])) == (0, 0)
    # the 70% rule counts the runs that only used the tool separately from the runs that used both, and reports both numbers
    a = analyse(without, with_, 0)
    assert abs(a["tool_only_share"] - 5 / 15) < 1e-9 and abs(a["tool_share"] - 10 / 15) < 1e-9
    # assembling one-trial rounds into one run directory: the trials renumbered, the header merged, a round run differently refused
    with tempfile.TemporaryDirectory() as d:
        state = Path(d)
        head = {"version": 1, "label": "x", "commit": "abc", "guide_sha256": "g", "agent": "omp", "agent_version": "1", "model": "m", "variant": "with-style", "suite": "s", "trials": 1, "host": "h", "date": "2026-11-14", "price": None}
        for r in ("r1", "r2", "r3"):
            for task in ("dress-the-market-area", "light-the-gate-at-night"):
                (state / r / task / "trial-1").mkdir(parents=True)
                (state / r / task / "trial-1" / "trial.json").write_text(json.dumps({"task": task, "trial": 1, "outcome": "pass"}))
                (state / r / task / "trial-1" / "transcript.jsonl").write_text("{}")
            (state / r / "run.json").write_text(json.dumps(dict(head, commit="abc" if r != "r3" else "other")))
        merged, counts = assemble("all", ["r1", "r2"], state)
        assert counts == {"dress-the-market-area": 2, "light-the-gate-at-night": 2} and merged["trials"] == 2 and merged["label"] == "all" and merged["commit"] == "abc"
        assert [json.loads((state / "all" / "dress-the-market-area" / f"trial-{n}" / "trial.json").read_text())["trial"] for n in (1, 2)] == [1, 2]
        assert json.loads((state / "all" / "run.json").read_text())["trials"] == 2
        try:
            assemble("bad", ["r1", "r3"], state)
            raise AssertionError("rounds built at different commits were assembled")
        except SystemExit as stop:
            assert "commit" in str(stop)
    print("selftest passed")


def events_from(rows):
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "transcript.jsonl"
        p.write_text("\n".join(json.dumps(r) for r in rows))
        return events(p)


if __name__ == "__main__":
    if sys.argv[1:] == ["--selftest"]:
        selftest()
    elif sys.argv[1:2] == ["--routes"]:
        for run in sys.argv[2].split(","):
            for path in sorted((STATE / run).glob("*/trial-*/transcript.jsonl")):
                calls = events(path)
                print(f"{path.parent.relative_to(STATE)}: route {route_of(calls)}, followed {cited_followed(calls)}, hidden-tool attempts {hidden_attempts(calls)}, {len(calls)} calls: " + ", ".join(f"{k}:{n}" for k, n, _ in calls[:40]))
    elif sys.argv[1:2] == ["--assemble"]:
        head, counts = assemble(sys.argv[2], sys.argv[3].split(","))
        print(f"assembled {sys.argv[2]}: {counts} trials per task, header trials = {head['trials']}, commit {head['commit']}, date {head['date']}")
    else:
        without, e0 = load(sys.argv[1])
        with_, e1 = load(sys.argv[2])
        report(analyse(without, with_, e0 + e1))
