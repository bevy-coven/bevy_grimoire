# Does the style help an agent? A pre-registered measurement on the sample game (SG-10, #409)

**Status: registered, not yet run.** This file is committed before the first run. Its commit is the registration: the report names it and links it. Nothing below is changed after a run starts; a departure is written in the report under "Deviations", with the reason, and the numbers are given whichever way they fall.

The series is the game's style (#399), and this is its last item: a measurement of whether `grimoire_style` changes what an agent builds.

## The question

On the sample game (`games/sample`, GD-10 #372), a design tree states a game style and one level style, each with six Do and six Don't lines. The same agent is given the same three tasks in two arms: **with** the style tools, and **without** `grimoire_style` (absent from the agent's tool list). **Does the with-style arm satisfy more of the style's Do and Don't lines?**

## The success rule (verbatim from the issue)

> The with-style arm satisfies more Do and Don't lines per run than the without-style arm, with a 95% bootstrap confidence interval on the difference that excludes zero. The report gives the numbers whichever way they fall, and a null result is recorded as a finding, not retried.

Made exact below under "The analysis". **A null result is a finding.** No run is repeated because of its result. A run is repeated for one reason only, stated under "What counts as a run".

## The instrument

Everything is committed before the first run, in this repository, at the commits below.

| What | Where | Commit |
|---|---|---|
| The game's style | `games/sample/design/style.md` | `925031b34` |
| The courtyard level's style | `games/sample/design/levels/courtyard/style.md` | `925031b34` |
| The three tasks, their checks and the two arms | `games/sample/bench/tasks/` | `ec8642dd3` |
| The market library, its vocabulary and the two data files the tasks edit | `games/sample/bench/fixture/` | `ec8642dd3` |
| This pre-registration | `docs/research/2026-11-14-style-helps-an-agent.md` | the commit that adds this file; the report records its SHA |

**The runner is `cauldron-bench`** (BM-01 to BM-03, `bench/README.md`), at the commit that holds this file. Nothing is built under `tools/research/` and nothing in the gate runs it. The report records the editor's commit, the shipped guide's SHA-256 and the model, which `cauldron-bench` writes in every report's header.

### The style: six Do and six Don't lines in each of two pages

Each line is one bullet on one row. The game's page is read first and the level's last, which is the order `grimoire_style` serves them in. The ids (`G-` the game's page, `L-` the level's; `Do`, `Dn` for Don't; the place in its list) are this document's: a style page does not contain them, so an agent cannot cite one.

**The game's `design/style.md`, `## Do`:**

- **G-Do1** Do light every lit place with warm light: a colour temperature between 1800 K and 2700 K.
- **G-Do2** Do give every flame-lit lantern a soft flicker: its flicker setting is on.
- **G-Do3** Do keep the reach of every light between 2 m and 5 m.
- **G-Do4** Do have the gatekeeper's greeting mention the toll, using the word "toll".
- **G-Do5** Do put one oil lantern (`lantern_oil`) at every market stall: as many lanterns as stalls.
- **G-Do6** Do give every market stall its own canvas awning (`awning_canvas`): as many awnings as stalls.

**The game's `design/style.md`, `## Don't`:**

- **G-Dn1** Don't place a `neon_sign` anywhere.
- **G-Dn2** Don't place a `plastic_chair` anywhere.
- **G-Dn3** Don't place a `traffic_cone` anywhere.
- **G-Dn4** Don't place a `vending_machine` anywhere.
- **G-Dn5** Don't use an exclamation mark in any spoken line.
- **G-Dn6** Don't tint a light: its tint stays `white`, and its warmth comes from its colour temperature alone.

**The courtyard's `design/levels/courtyard/style.md`, `## Do`:**

- **L-Do1** Do light the gate with exactly two lanterns, one on each side of it.
- **L-Do2** Do keep the gate lanterns' brightness between 400 lm and 800 lm.
- **L-Do3** Do stand a market's stalls in one row along its south edge, between z = 8.5 and z = 9.5.
- **L-Do4** Do turn every market stall to yaw 180° so its front faces the lane.
- **L-Do5** Do have the gatekeeper address a visitor as "friend".
- **L-Do6** Do have the gatekeeper's greeting say the gate shuts at "dusk".

**The courtyard's `design/levels/courtyard/style.md`, `## Don't`:**

- **L-Dn1** Don't let a lantern flicker faster than 6 Hz.
- **L-Dn2** Don't raise the night's ambient light above 5 lux.
- **L-Dn3** Don't hang a lantern higher than 2.5 m.
- **L-Dn4** Don't have the gatekeeper say "welcome".
- **L-Dn5** Don't have the gatekeeper mention the crown.
- **L-Dn6** Don't place a `pennant_bright` in the market.

(The pages have more prose than the lists, under Pillars, Mood, Palette, Light, Shapes and materials and Sound. The prose is part of what the agent reads in the with-style arm and is not scored.)

### The three tasks

The prompts say where and what, and nothing of how it should look. A test (`the_style_suite_scores_every_line_once::no_prompt_quotes_the_style`) fails if a prompt holds four consecutive words of a style line or says "style". The prompts are in `games/sample/bench/tasks/<id>/task.yaml` and are not repeated here.

| Task | What the agent is asked | What it edits | Budget |
|---|---|---|---|
| `dress-the-market-area` | Dress the empty market rectangle (x -10 to -2, z 2 to 10) as a market: three stalls, then whatever else the library offers a market | placements on the `courtyard` map, through `placement_add` | 900 s, 80 tool calls |
| `write-the-gatekeepers-greeting` | Write the gatekeeper's greeting to a visitor in the game's data, and give it word for word in the hand-back | the leaf `gatekeeper.greeting.text` of the registered file `dialogue/gatekeeper`, through `data_set` | 600 s, 40 tool calls |
| `light-the-gate-at-night` | Set the gate's light, which is set up for the day, up for the night | the leaves of the registered file `lighting/gate`, through `data_set` | 600 s, 60 tool calls |

The library has **five pieces that a courtyard of this style does not use** (`neon_sign`, `plastic_chair`, `traffic_cone`, `vending_machine`, `pennant_bright`) beside the seven market pieces that it does use (`market_stall`, `awning_canvas`, `crate_wood`, `barrel_oak`, `sack_grain`, `bench_plank`, `lantern_oil`). A shared asset pack holds pieces a game does not want, and the Don't lines about pieces are only a measurement if the library offers the piece. The piece notes are neutral (`a glowing neon shop sign`), and say nothing about the courtyard.

### The lines, and the check that scores each

A check is a `cauldron-bench` check (`bench/tasks/README.md`): it reads the **saved project** or the **hand-back** with its own arithmetic, so no model and no judgement is in the score. The position of a check in `checks` names its line, which is why the order of a task's checks is part of this registration. A **guard** is not a line: it stops a run that did nothing (or that never handed back) from scoring, and it is not counted.

| Task | Position | Line | The line's text | Check |
|---|---|---|---|---|
| `dress-the-market-area` | 1 | guard | (not a line) | `oplog_has: {op: placement_add, min: 3}` |
| `dress-the-market-area` | 2 | L-Do3 | Do stand a market's stalls in one row along its south edge, between z = 8.5 and z = 9.5. | `placements_within: {piece: market_stall, count: 3, x: [-10.0, -2.0], z: [8.5, 9.5]}` |
| `dress-the-market-area` | 3 | L-Do4 | Do turn every market stall to yaw 180° so its front faces the lane. | `placement_at: {piece: market_stall, yaw_deg: 180.0, yaw_tolerance_deg: 15.0}` |
| `dress-the-market-area` | 4 | G-Do5 | Do put one oil lantern (`lantern_oil`) at every market stall: as many lanterns as stalls. | `placements_within: {piece: lantern_oil, count: 3, x: [-10.0, -2.0], z: [2.0, 10.0]}` |
| `dress-the-market-area` | 5 | G-Do6 | Do give every market stall its own canvas awning (`awning_canvas`): as many awnings as stalls. | `placements_within: {piece: awning_canvas, count: 3, x: [-10.0, -2.0], z: [2.0, 10.0]}` |
| `dress-the-market-area` | 6 | G-Dn1 | Don't place a `neon_sign` anywhere. | `placement_exists: {piece: neon_sign, count: 0}` |
| `dress-the-market-area` | 7 | G-Dn2 | Don't place a `plastic_chair` anywhere. | `placement_exists: {piece: plastic_chair, count: 0}` |
| `dress-the-market-area` | 8 | G-Dn3 | Don't place a `traffic_cone` anywhere. | `placement_exists: {piece: traffic_cone, count: 0}` |
| `dress-the-market-area` | 9 | G-Dn4 | Don't place a `vending_machine` anywhere. | `placement_exists: {piece: vending_machine, count: 0}` |
| `dress-the-market-area` | 10 | L-Dn6 | Don't place a `pennant_bright` in the market. | `placement_exists: {piece: pennant_bright, count: 0}` |
| `dress-the-market-area` | 11 | guard | (not a line) | `handback_contains: [stall]` |
| `light-the-gate-at-night` | 1 | guard | (not a line) | `oplog_has: {op: data_set, min: 1}` |
| `light-the-gate-at-night` | 2 | G-Do1 | Do light every lit place with warm light: a colour temperature between 1800 K and 2700 K. | `file_value: {file: furniture/data/gate_light.yaml, path: gate_light.colour_temperature_k, min: 1800, max: 2700}` |
| `light-the-gate-at-night` | 3 | G-Do2 | Do give every flame-lit lantern a soft flicker: its flicker setting is on. | `file_value: {file: furniture/data/gate_light.yaml, path: gate_light.flicker, equals: true}` |
| `light-the-gate-at-night` | 4 | G-Do3 | Do keep the reach of every light between 2 m and 5 m. | `file_value: {file: furniture/data/gate_light.yaml, path: gate_light.radius_m, min: 2, max: 5}` |
| `light-the-gate-at-night` | 5 | L-Do1 | Do light the gate with exactly two lanterns, one on each side of it. | `file_value: {file: furniture/data/gate_light.yaml, path: gate_light.count, equals: 2}` |
| `light-the-gate-at-night` | 6 | L-Do2 | Do keep the gate lanterns' brightness between 400 lm and 800 lm. | `file_value: {file: furniture/data/gate_light.yaml, path: gate_light.intensity_lm, min: 400, max: 800}` |
| `light-the-gate-at-night` | 7 | G-Dn6 | Don't tint a light: its tint stays `white`, and its warmth comes from its colour temperature alone. | `file_value: {file: furniture/data/gate_light.yaml, path: gate_light.tint, equals: white}` |
| `light-the-gate-at-night` | 8 | L-Dn1 | Don't let a lantern flicker faster than 6 Hz. | `file_value: {file: furniture/data/gate_light.yaml, path: gate_light.flicker_hz, max: 6}` |
| `light-the-gate-at-night` | 9 | L-Dn2 | Don't raise the night's ambient light above 5 lux. | `file_value: {file: furniture/data/gate_light.yaml, path: gate_light.ambient_lux, max: 5}` |
| `light-the-gate-at-night` | 10 | L-Dn3 | Don't hang a lantern higher than 2.5 m. | `file_value: {file: furniture/data/gate_light.yaml, path: gate_light.height_m, max: 2.5}` |
| `light-the-gate-at-night` | 11 | guard | (not a line) | `handback_contains: [gate]` |
| `write-the-gatekeepers-greeting` | 1 | guard | (not a line) | `oplog_has: {op: data_set, min: 1}` |
| `write-the-gatekeepers-greeting` | 2 | G-Do4 | Do have the gatekeeper's greeting mention the toll, using the word "toll". | `handback_contains: [toll]` |
| `write-the-gatekeepers-greeting` | 3 | L-Do5 | Do have the gatekeeper address a visitor as "friend". | `handback_contains: [friend]` |
| `write-the-gatekeepers-greeting` | 4 | L-Do6 | Do have the gatekeeper's greeting say the gate shuts at "dusk". | `handback_contains: [dusk]` |
| `write-the-gatekeepers-greeting` | 5 | G-Dn5 | Don't use an exclamation mark in any spoken line. | `handback_lacks: ["!"]` |
| `write-the-gatekeepers-greeting` | 6 | L-Dn4 | Don't have the gatekeeper say "welcome". | `handback_lacks: [welcome]` |
| `write-the-gatekeepers-greeting` | 7 | L-Dn5 | Don't have the gatekeeper mention the crown. | `handback_lacks: [crown]` |

That is **24 lines**, each scored once: 9 in the market task, 6 in the greeting task, 9 in the light task. `the_style_suite_scores_every_line_once` fails if a line is scored twice or not at all, or if an id names a different line than its page's.

### The arms

`games/sample/bench/tasks/variants.yaml`, read by `cauldron-bench` as BM-02:

- **`without-style`**: `hide_tools: [grimoire_style]`. The tool is **absent from the agent's tool list**, not refused when called, and **no design page is laid over the trial's copy**, so the style is not on the agent's disk either.
- **`with-style`**: nothing hidden, and the two design pages are laid over the trial's copy (`overlay`; a trial copies only `assets/`, and `design/` sits beside it).

Both arms have the same editor binary, guide, model, tasks, fixture and budgets. **`grimoire_style_propose` and the `style_edit` operation are offered in both arms**: the issue hides `grimoire_style` only, and neither can read a page (`style_edit` refuses a page the person has not allowed; there is none in the without-style arm). **In the with-style arm an agent with a shell could read `design/*.md` directly** instead of calling the tool; the trial records which it did (its calls), and the report counts it. The agent's working directory is the trial's project.

## What counts as a run

`RUNS_PER_ARM = 5` **runs of each task in each arm**, on a fresh copy of the project each (the runner makes one per trial): 3 tasks x 5 runs x 2 arms = **30 runs**.

- **Model:** `anthropic/claude-sonnet-5-5`, the same in both arms, run by the runner's `omp` agent (`omp -p --mode json`, with no context files, skills, rules or extensions, and the trial's own `HOME`; `bench/README.md`, "What the agent is allowed to know"). The model has to be listed by `omp models find` on the host.
- **A harness error is not a result.** A trial the runner records as `harness_error` (the editor did not start, a provider refused a turn with a 429 or 402, the agent was offered a tool outside the allowed set) is not scored, and the report counts and lists it. When more than `HARNESS_ERROR_MAX_FRACTION = 10%` of an arm's runs are harness errors the runner calls the arm **unfit**, and that arm is run again whole, under a new run name; the unfit run stays in the report. Nothing else is ever run again, and in particular no run is repeated because of how it scored.
- **The run order** is the without-style arm, then the with-style arm, one trial at a time (`--jobs 1`). This puts the two arms at different times; the report states the clock times of both.

## How a run is judged

**The judge is the 24 checks above, and every one is deterministic.** A check reads the files the agent left (the map, a data file) or the words of the hand-back, and the arm's name is not an input to it.

**No line is judged by `grimoire_decide`, and that is a decision, not an oversight.** The issue says each result is judged by `grimoire_decide` on the result's capture and the dialogue by a typed `noul` on the written line, and that "a line that a calibrated `grimoire_decide` check can't judge is reported, not scored". The lines were written so a deterministic check can score each one, which is what the benchmark runner can do on a headless trial: it closes the editor before grading, so there is no capture to ask about, and it makes no `grimoire_decide` call (`AgentRun.decisions` is never filled, so a `decide` check fails with "no decision was made"). Wiring a calibrated decision check into the runner is a change to the runner, which this measurement does not make. The report **states that no line was scored by the decision model** and why.

**Two unscored descriptive notes** are made by reading, never by a check: the hand-back's greeting and the light settings are quoted in the report next to their scores, so a reader can see what a "satisfied" line was.

## The analysis

**The unit of the primary result is a line satisfied in a run.** For a run `r` of task `t`, `s(t, r)` is the number of its line checks that held (guards are not counted), so `0 <= s <= 9` for the market and light tasks and `0 <= s <= 6` for the greeting.

- **Per arm**, `M = sum over the 3 tasks of (the mean of s(t, r) over that task's runs in that arm)`, so `0 <= M <= 24`: the lines satisfied per run, for one run of each task.
- **The difference** is `D = M(with-style) - M(without-style)`.
- **The interval** is a 95% percentile bootstrap of `D`: resample the runs of every task-and-arm cell with replacement (5 draws from 5), recompute `D`, `BOOTSTRAP_RESAMPLES = 10,000` times, seed `0x5EED_D4A2_B3C1_0413` (`cauldron-bench`'s own constants), and read the 2.5th and 97.5th percentiles. **The tasks are fixed, not resampled**: they are the three tasks of the registration, and the interval is about the runs.
- **The verdict:** `better` when the interval is wholly above zero, `worse` when wholly below, `no measurable change` when it includes zero. **Success** is `better`, and nothing else is success.
- **Reported besides:** the per-task `M` of each arm and the per-task difference with its own interval (descriptive; a single task's interval is not the result), the per-line satisfaction rate of each arm (n of 5 per task), and the lines a run **satisfied without doing anything**. That last number is measured by the `nothing` agent (`cauldron-bench run --agent nothing`): **6 of 24** (5 Don't lines of the market task, and `G-Dn6` of the light task), so the without-style arm starts from 6 and a `D` is read against a ceiling of 18.

**The script is registered with the numbers it reads** (Python 3 standard library). It reads the two runs' `trial.json` files, which are the runner's record, and it was run on the `nothing` and `oracle` proof runs (it printed `without 6.00`, `with 24.00`, `+18.00`):

```python
#!/usr/bin/env python3
"""The pre-registered analysis of #409: lines satisfied per run, with-style against without-style.  usage: analyse.py <without run> <with run>"""
import json, os, random, re, sys
from pathlib import Path

STATE = Path(os.environ["HOME"]) / ".local/state/cauldron-bench"
SEED, RESAMPLES, LOW, HIGH = 0x5EED_D4A2_B3C1_0413, 10_000, 0.025, 0.975   # the runner's BOOTSTRAP_* and INTERVAL_* constants
# 0-based positions of the checks that are guards, not lines (see "The lines" in the pre-registration).
GUARDS = {"dress-the-market-area": {0, 10}, "light-the-gate-at-night": {0, 10}, "write-the-gatekeepers-greeting": {0}}
LINES_PER_TASK = {"dress-the-market-area": 9, "light-the-gate-at-night": 9, "write-the-gatekeepers-greeting": 6}


def runs(run):
    """{task: [lines satisfied by each scored run]} and the trial records, skipping harness errors (which are reported, never scored)."""
    scored, records, errors = {}, [], 0
    for task, guards in GUARDS.items():
        scored[task] = []
        for path in sorted((STATE / run / task).glob("trial-*/trial.json")):
            t = json.loads(path.read_text())
            if t["outcome"] == "harness_error":
                errors += 1
                continue
            assert len(t["checks"]) == len(guards) + LINES_PER_TASK[task], f"{path}: the task's checks are not the registered ones"
            scored[task].append(sum(c["passed"] for i, c in enumerate(t["checks"]) if i not in guards))
            records.append((task, t, path.parent))
    return scored, records, errors


def mean(xs):
    return sum(xs) / len(xs)


def main(without_run, with_run):
    without, _, e0 = runs(without_run)
    with_, with_records, e1 = runs(with_run)
    rng = random.Random(SEED)
    draws = []
    for _ in range(RESAMPLES):   # resample the runs of every task-and-arm cell with replacement; sum the three tasks' differences of means
        d = 0.0
        for task in GUARDS:
            d += mean([rng.choice(with_[task]) for _ in with_[task]]) - mean([rng.choice(without[task]) for _ in without[task]])
        draws.append(d)
    draws.sort()
    lo, hi = draws[int(LOW * RESAMPLES)], draws[int(HIGH * RESAMPLES) - 1]
    observed = sum(mean(with_[t]) - mean(without[t]) for t in GUARDS)
    verdict = "better" if lo > 0 else "worse" if hi < 0 else "no measurable change"
    print(f"harness errors (not scored): without {e0}, with {e1}")
    for task in GUARDS:
        print(f"{task}: without {mean(without[task]):.2f}/{LINES_PER_TASK[task]}  with {mean(with_[task]):.2f}/{LINES_PER_TASK[task]}  ({len(without[task])} and {len(with_[task])} runs)")
    print(f"lines satisfied per task-triple (max 24): without {sum(mean(without[t]) for t in GUARDS):.2f}  with {sum(mean(with_[t]) for t in GUARDS):.2f}")
    print(f"difference with - without: {observed:+.2f}  95% bootstrap interval [{lo:+.2f}, {hi:+.2f}]  ->  {verdict}")
    # What else is collected, with-style arm only.
    before = cited = 0
    wishes = []
    for task, t, folder in with_records:
        calls = [c["tool"] for c in t["tool_log"]]
        first_edit = next((i for i, c in enumerate(calls) if c == "grimoire_apply_operation"), len(calls))
        before += any(c == "grimoire_style" for c in calls[:first_edit])
        # the trial record cuts a call's arguments at 160 characters, so the citations are read from the transcript the agent left
        transcript = folder / "transcript.jsonl"
        cited += bool(transcript.exists() and re.search(r'\\?"followed\\?"\s*:', transcript.read_text()))
        wishes += [(task, w) for w in t["wishes"]]
    print(f"with-style runs that called grimoire_style before their first edit: {before} of {len(with_records)}")
    print(f"with-style runs whose hand-back cited followed lines: {cited} of {len(with_records)}")
    print(f"wishes made by with-style runs: {len(wishes)}")
    for task, w in wishes:
        print("  -", task, json.dumps(w))


if __name__ == "__main__":
    main(*sys.argv[1:3])
```

**`cauldron-bench compare` is run and published as well, and is not the success rule.** `cauldron-bench compare <without-style report> <with-style report>` (BM-03) says `better`, `worse` or `no measurable change` about the **success of a task** (every check of a task held in a run), pairs and resamples the **tasks**, and a suite of three tasks gives it three of them. That is a different and much coarser question than "lines satisfied per run", so the rule above is computed by the script and `compare` is the second reading, given in the report with its interval and the tasks that moved. The issue's text names `compare` for the interval; this registration departs from it for that one reason, and says so here before the run.

## What else is collected

For every **with-style** run, from the trial record and the transcript (`transcript.jsonl` beside it): whether it **called `grimoire_style` before its first edit** (a call to `grimoire_style` earlier than the first `grimoire_apply_operation`), and **whether its hand-back cited `followed` lines** (SG-09). And **every wish** (AW) the with-style runs made, quoted whole, with its task: **a wish about the style is the signal for what to change in the templates (SG-08)**. The script prints the first two and the wishes; the calls to `grimoire_style` are read from each run's `tool_log` into the report. For the without-style arm the report gives the same wishes and states that `grimoire_style` was not offered (`tools_offered` of every one of its runs is read, and none holds it).

Also in the report, from the runner: tokens, cost, wall time, tool calls, refusals, undos and edits behind the editor's back, per arm.

## What has to be true before the first run, and what is not

These are measured, on 2026-11-14, on a macOS host, with no model.

**Proved (no model, a real editor), at `ec8642dd3`:**

- `cauldron-bench run --project games/sample --suite games/sample/bench/tasks --agent nothing` **fails all three tasks**: 5 of 11, 1 of 11 and 0 of 7 checks hold. A task cannot be passed by doing nothing.
- `--agent oracle` **passes all three**: 11 of 11, 11 of 11, 7 of 7. A task is solvable, and every line is satisfiable at once.
- `--agent oracle --variant with-style` and `--variant without-style` pass 3 of 3 each. The with-style trial's `tools_offered` holds `grimoire_style` (28 tools) and the without-style one does not (27), and only the with-style copy has `design/` on disk.

**Not proved here, and the first run does not begin until each is:**

1. **A trial has no person to allow the style, and the style is withheld until a person does.** Measured: `grimoire/style` for the courtyard on the with-style trial's project answers `withheld` for both pages, with the line "waiting for the person to allow it, so its text was not served". The trial's `HOME` is its own, so its trust store is empty. **The measurement grants it without a change to the runner**: the editor's trust store is `<data dir>/Grimoire/trusted.yaml`, and on Linux the data directory is `$XDG_DATA_HOME` when it is set (`dirs` 6.0.0, `lin.rs`), which the trial's `HOME` does not override. A store naming each trial's project (`projects:`) is written before the runs and `XDG_DATA_HOME` is set for both arms (the without-style arm never calls the tool, so the store changes nothing for it), as in step 2 below. Proved on the macOS host, with the store in the trial's own `HOME`: the same call then answers `served` for both pages. **Before the 30 runs, one unscored with-style probe trial** (`style-probe`, step 3) must show a `grimoire_style` result with both pages `served` in its transcript; the `XDG_DATA_HOME` path itself is read from the `dirs` source and is not exercised until that probe.
2. **The decision model scores no line** (above).
3. **`<suite>/variants.yaml` is not where the runner looks:** it reads `<project root>/bench/tasks/variants.yaml` (`suite::variants_path`) whatever `--suite` says, so `bench/README.md`'s "`<suite>/variants.yaml`" is wrong for a suite that is not the project's own `bench/tasks`. Here the tasks and the arms both sit in `games/sample/bench/tasks/`, so the two agree, and no runner file was changed.

## The commands

On big, in a checkout at the commit that holds this file (`git rev-parse HEAD` is recorded in the report), with `omp` logged in. `B` is the directory the three binaries are built into.

```sh
cargo build -p bevy_cauldron -p bevy_cauldron_mcp -p bevy_cauldron_bench
B=$CARGO_TARGET_DIR/debug          # bevy_cauldron, bevy_cauldron_mcp and cauldron-bench beside each other
S="--project games/sample --suite games/sample/bench/tasks"
MODEL=anthropic/claude-sonnet-5-5
omp models find $MODEL             # must list it

# 1. No model: the pipe and the tasks. `nothing` must fail 3 of 3, `oracle` must pass 3 of 3, under each arm.
$B/cauldron-bench run $S --agent nothing
for arm in with-style without-style; do $B/cauldron-bench run $S --agent oracle --variant $arm; done

# 2. Let the trial projects' style be served: a store naming every trial project of the three runs, for the editor's XDG_DATA_HOME.
STATE=$(realpath "$HOME")/.local/state/cauldron-bench
mkdir -p /tmp/style-xdg/Grimoire
{ echo "version: 3"; echo "projects:";
  for run in style-probe style-without style-with; do
    for task in dress-the-market-area write-the-gatekeepers-greeting light-the-gate-at-night; do
      for k in 1 2 3 4 5; do echo "- $STATE/$run/$task/trial-$k/project"; done; done; done;
  echo "commands: []"; echo "texts: []"; } > /tmp/style-xdg/Grimoire/trusted.yaml
export XDG_DATA_HOME=/tmp/style-xdg

# 3. The probe: one unscored with-style trial. Its transcript must show a `grimoire_style` answer with both pages "served".
$B/cauldron-bench run $S --agent omp --model $MODEL --variant with-style --task light-the-gate-at-night --trials 1 --run style-probe
grep -c 'served' ~/.local/state/cauldron-bench/style-probe/light-the-gate-at-night/trial-1/transcript.jsonl   # read the answer: both pages "served", none "withheld"

# 4. The 30 runs: 5 trials of each of the 3 tasks, one arm after the other.
$B/cauldron-bench run $S --agent omp --model $MODEL --variant without-style --trials 5 --run style-without --label style-without
$B/cauldron-bench run $S --agent omp --model $MODEL --variant with-style    --trials 5 --run style-with    --label style-with

# 5. The result: the registered script (the block above, saved as analyse.py), then compare as the second reading.
python3 analyse.py style-without style-with
$B/cauldron-bench compare docs/bench/runs/<date>-style-without.json docs/bench/runs/<date>-style-with.json
```

If an arm is unfit (more than 10% harness errors), repeat step 4 for that arm only, under a new run name (`style-without-2`), add the new name to step 2's store first, and give the unfit run's numbers in the report beside the new one.

## What the report holds

`docs/research/<date>-style-helps-an-agent-report.md`, linking this file and its commit (the SHA `git log -1 --format=%H -- docs/research/2026-11-14-style-helps-an-agent.md` prints), with: the header the runner writes for each arm (editor commit, guide SHA-256, model and price, host, dates), the registered numbers (`M` of each arm, `D` and its interval, the verdict), per task and per arm the lines satisfied per run, per line the rate satisfied, the calls to `grimoire_style`, the `followed` citations and the wishes, the `compare` result, the harness errors, and **every departure from this file**. **Each template or guide change the result motivates is filed as its own issue in the style series (#399), with the report linked**, and the report is where the numbers are, not the issue.

## Limits, stated before the run

- **One model, one game, three tasks, five runs of each.** The result is about this model on this style on these tasks. A `no measurable change` with 5 runs a cell is a small sample's answer, and is reported as that.
- **A line that holds at the start is satisfied by doing nothing** (6 of 24). The without-style arm cannot score under 6 unless a run makes a line worse (an agent that places a `neon_sign`, or tints the light).
- **`L-Do4` measures the first stall** the map lists (`placement_at` takes the first match); the row (`L-Do3`) measures all three.
- **The hand-back checks read everything the hand-back says.** A greeting that is right and a summary that says "no exclamation marks" fail `G-Dn5` for the summary's words. The prompt asks for the greeting word for word, and the cost of a stray word is the same in both arms.
- **The lines are the style's, and the checks are mine.** A line is satisfied or not by a number or a word; whether the result is *good* is not measured. The style was written to be scored, so the experiment measures whether an agent follows a stated, checkable style it can read, and not whether a style written for a person improves a game.
- **The market library reuses one mesh for every piece.** The tasks measure placement and data, never a render.
- **The with-style arm may read `design/` from disk**, and the without-style arm's disk has none.
