# Does the style help an agent? A pre-registered measurement on the sample game (SG-10, #409)

**Status: registered, not yet run.** This file is committed before the first run. Its commit is the registration: the report names it and links it. Nothing below is changed after a run starts; a departure is written in the report under "Deviations", with the reason, and the numbers are given whichever way they fall.

The series is the game's style (#399), and this is its last item: a measurement of whether `grimoire_style` changes what an agent builds.

## Revisions before the first run

This file was first committed as `35ca9306d`. **No run has been made**: the first commit was reviewed for method before any agent ran, and it is committed again, changed, as the final commit of the branch. The revised file replaces the first, and its SHA is the registration the report records. A revision after a run starts is a "Deviation" in the report, never an edit of this file. What changed, and why:

1. **The primary result is stated as "the style was available", and a claim about the tool has a pre-registered condition** (section "What the arms differ by"). The arms differ in two things, not one, and the first draft hid that.
2. **The greeting task scores the saved leaf, not the hand-back.** The first draft scored six lines on everything the hand-back said, which differs between an agent that was told the forbidden words and one that was not. The runner's `file_value` check gained `contains` and `lacks` (additive, with a test), and the six checks read `gatekeeper.greeting.text`.
3. **The arms alternate by trial** (5 rounds of 1 trial per task per arm, the arm order alternating), so provider drift is spread over both arms instead of being one arm's.
4. **The analysis script is a registered file with a self-test** (`docs/research/2026-11-14-style-helps-an-agent-analyse.py`): an empty cell, several run names per arm, the route stratification, an exact permutation p-value as a second reading, citations read only from the agent's own `grimoire_present` call, and the runner's own percentile.
5. **Smaller:** `cauldron-bench` publishes under `<project root>/docs/bench`, so the registered commands publish the merged reports with `report --publish docs/bench`; the guard wording; the checks that are weaker than their line are listed; a cost and time estimate; the seeded trust store's version is a tested constant.

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
| `write-the-gatekeepers-greeting` | Write the gatekeeper's greeting to a visitor in the game's data | the leaf `gatekeeper.greeting.text` of the registered file `dialogue/gatekeeper`, through `data_set` | 600 s, 40 tool calls |
| `light-the-gate-at-night` | Set the gate's light, which is set up for the day, up for the night | the leaves of the registered file `lighting/gate`, through `data_set` | 600 s, 60 tool calls |

The library has **five pieces that a courtyard of this style does not use** (`neon_sign`, `plastic_chair`, `traffic_cone`, `vending_machine`, `pennant_bright`) beside the seven market pieces that it does use (`market_stall`, `awning_canvas`, `crate_wood`, `barrel_oak`, `sack_grain`, `bench_plank`, `lantern_oil`). A shared asset pack holds pieces a game does not want, and the Don't lines about pieces are only a measurement if the library offers the piece. The piece notes are neutral (`a glowing neon shop sign`), and say nothing about the courtyard.

### The lines, and the check that scores each

A check is a `cauldron-bench` check (`bench/tasks/README.md`): it reads the **saved project** with its own arithmetic, so no model and no judgement is in the score. **Every line is scored on the saved project**, including the six greeting lines, which read the leaf `gatekeeper.greeting.text` with the check's `contains` and `lacks` (added to `file_value` for this study, additive, tested by `a_data_file_string_is_checked_by_what_it_says`); no line reads the hand-back. The position of a check in `checks` names its line, which is why the order of a task's checks is part of this registration (the table counts from 1; the script's `GUARDS` count the same positions from 0). A **guard** is not a line and is not counted: it records that the run did something through the editor (a `placement_add` or `data_set` in the log) or handed back. A run that does nothing still satisfies the lines that hold in the starting scene, and the script counts those lines whether or not a guard held; that baseline is intended and is reported (9 of 24).

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
| `write-the-gatekeepers-greeting` | 2 | G-Do4 | Do have the gatekeeper's greeting mention the toll, using the word "toll". | `file_value: {file: furniture/data/gatekeeper.yaml, path: gatekeeper.greeting.text, contains: toll}` |
| `write-the-gatekeepers-greeting` | 3 | L-Do5 | Do have the gatekeeper address a visitor as "friend". | `file_value: {file: furniture/data/gatekeeper.yaml, path: gatekeeper.greeting.text, contains: friend}` |
| `write-the-gatekeepers-greeting` | 4 | L-Do6 | Do have the gatekeeper's greeting say the gate shuts at "dusk". | `file_value: {file: furniture/data/gatekeeper.yaml, path: gatekeeper.greeting.text, contains: dusk}` |
| `write-the-gatekeepers-greeting` | 5 | G-Dn5 | Don't use an exclamation mark in any spoken line. | `file_value: {file: furniture/data/gatekeeper.yaml, path: gatekeeper.greeting.text, lacks: "!"}` |
| `write-the-gatekeepers-greeting` | 6 | L-Dn4 | Don't have the gatekeeper say "welcome". | `file_value: {file: furniture/data/gatekeeper.yaml, path: gatekeeper.greeting.text, lacks: welcome}` |
| `write-the-gatekeepers-greeting` | 7 | L-Dn5 | Don't have the gatekeeper mention the crown. | `file_value: {file: furniture/data/gatekeeper.yaml, path: gatekeeper.greeting.text, lacks: crown}` |

That is **24 lines**, each scored once: 9 in the market task, 6 in the greeting task, 9 in the light task. **Some checks are weaker than their line**, and are listed under "Limits". `the_style_suite_scores_every_line_once` fails if a line is scored twice or not at all, or if an id names a different line than its page's.

### The arms

`games/sample/bench/tasks/variants.yaml`, read by `cauldron-bench` as BM-02:

- **`without-style`**: `hide_tools: [grimoire_style]`. The tool is **absent from the agent's tool list**, not refused when called, and **no design page is laid over the trial's copy**, so the style is not on the agent's disk either.
- **`with-style`**: nothing hidden, and the two design pages are laid over the trial's copy (`overlay`; a trial copies only `assets/`, and `design/` sits beside it).

Both arms have the same editor binary, guide, model, tasks, fixture and budgets. **`grimoire_style_propose` and the `style_edit` operation are offered in both arms**: the issue hides `grimoire_style` only, and neither can read a page (`style_edit` refuses a page the person has not allowed; there is none in the without-style arm).

## What the arms differ by, and what a result may claim

**The arms differ in two things, not one.** In the with-style arm the agent has the tool **and** the pages are on its disk, so it can read `design/*.md` with a generic tool (`read`, `bash`, `grep`) and never call `grimoire_style`. The without-style arm has neither. So the primary result answers **"does having the style available change what an agent builds?"**, and is stated as that.

**Route stratification.** Every with-style run is put in one route, from the agent's own calls in `transcript.jsonl` (the script, `route_of`): `tool` (it called `grimoire_style` at some point and opened no design page by a generic tool), `file` (it opened a design page by a generic tool, a call whose arguments name `design/`, and never called the tool), `both`, `neither`. The report gives **D overall and D per route** (the with-style runs of one route against every without-style run, over the tasks that have a run in that route; a thin cell, descriptive), and the share of with-style runs that called the tool.

**A claim about the tool is allowed only on a pre-registered condition.** "`grimoire_style` changes what an agent builds" may be written only if **the verdict is `better` and at least `TOOL_SHARE_MIN = 70%` of the with-style runs called `grimoire_style`** (routes `tool` and `both`). Otherwise the report says "the style, available to the agent, changed (or did not measurably change) what it built", and says nothing about the tool. **The follow-up if the share is low** is a third arm, `A'`: the pages on disk and `grimoire_style` hidden. It would separate the tool from the files, it is **not** part of this design (it would add 15 runs), and it is registered here only as what to run next.

**What the without-style agent is told.** The shipped guide, served by `grimoire_guide` and not changed here, has a section "Know the style before you shape a place" that tells an agent to call `grimoire_style` and to cite `followed` lines. The tool being hidden does not hide that text, so **the without-style agent is told to call a tool it does not have** (a call to a hidden name is refused as an unknown tool). That can cost it a call or a wish, and it can make it look for the style elsewhere: it has no `design/` on disk to find. The with-style agent reads the same guide and has the tool. The text is the product's and is not edited for the study; the report records what each arm's agent did about it (its calls to the tool and its wishes).

**The trust gate is opened by the study, and is not what is tested.** A project's style is withheld until a person allows it, and a trial has no person. The registered commands pre-open the gate for the study's own trial paths (step 2), so the with-style agent is served the pages as an agent whose person said yes would be. The gate has its own tests (`the_style_of_a_place_is_served_behind_the_persons_trust`); nothing here measures it.

## What counts as a run

`RUNS_PER_ARM = 5` **runs of each task in each arm**, on a fresh copy of the project each (the runner makes one per trial): 3 tasks x 5 runs x 2 arms = **30 runs**.

- **Model:** `anthropic/claude-sonnet-5-5`, the same in both arms, run by the runner's `omp` agent (`omp -p --mode json`, with no context files, skills, rules or extensions, and the trial's own `HOME`; `bench/README.md`, "What the agent is allowed to know"). The model has to be listed by `omp models find` on the host.
- **The arms alternate by trial.** Five rounds; in each round every task is run once in each arm (`--trials 1`, the three tasks of an arm at once, `--jobs 3`), the arm order alternating between rounds (without-style first in rounds 1, 3 and 5, with-style first in rounds 2 and 4). A provider that is slower or less available at some hour then touches both arms alike. The report states the clock times of every round.
- **A harness error is not a result.** A trial the runner records as `harness_error` (the editor did not start, a provider refused a turn with a 429 or 402, the agent was offered a tool outside the allowed set) is not scored, and the report counts and lists it. **A task with no scored run in an arm is unfit** and is left out of both arms' `M`, and the report says so. When more than `HARNESS_ERROR_MAX_FRACTION = 10%` of an arm's runs are harness errors the runner calls the arm unfit, and **the missing runs are made up in further rounds** (a round-by-round run, `style-r6-...`, whose names are added to step 2's store and combined by the script). Nothing else is ever run again, and in particular no run is repeated because of how it scored.

## How a run is judged

**The judge is the 24 checks above, and every one is deterministic and reads the saved project.** A check reads the files the agent left (the map, a data file), and the arm's name is not an input to it. Nothing a run *said* is scored.

**No line is judged by `grimoire_decide`, and that is a decision, not an oversight.** The issue says each result is judged by `grimoire_decide` on the result's capture and the dialogue by a typed `noul` on the written line, and that "a line that a calibrated `grimoire_decide` check can't judge is reported, not scored". The lines were written so a deterministic check can score each one, which is what the benchmark runner can do on a headless trial: it closes the editor before grading, so there is no capture to ask about, and it makes no `grimoire_decide` call (`AgentRun.decisions` is never filled, so a `decide` check fails with "no decision was made"). Wiring a calibrated decision check into the runner is a larger change to the runner than this measurement makes. The report **states that no line was scored by the decision model** and why.

**The one change to the runner** is `file_value`'s `contains` and `lacks` (a case-insensitive substring of a string leaf, an empty word refused), which scores the greeting where the agent saved it. It adds two optional fields and changes no existing check.

## The analysis

**The unit of the primary result is a line satisfied in a run.** For a run `r` of task `t`, `s(t, r)` is the number of its line checks that held (guards are not counted), so `0 <= s <= 9` for the market and light tasks and `0 <= s <= 6` for the greeting.

- **Per arm**, `M = sum over the 3 tasks of (the mean of s(t, r) over that task's runs in that arm)`, so `0 <= M <= 24`: the lines satisfied per run, for one run of each task.
- **The difference** is `D = M(with-style) - M(without-style)`.
- **The interval** is a 95% percentile bootstrap of `D`: resample the runs of every task-and-arm cell with replacement (5 draws from 5), recompute `D`, `BOOTSTRAP_RESAMPLES = 10,000` times, seed `0x5EED_D4A2_B3C1_0413` (`cauldron-bench`'s own constants), and read the 2.5th and 97.5th percentiles, interpolated between ranks as the runner's `stats::percentile` does. **The tasks are fixed, not resampled**: they are the three tasks of the registration, and the interval is about the runs.
- **The verdict:** `better` when the interval is wholly above zero, `worse` when wholly below, `no measurable change` when it includes zero. **Success** is `better`, and nothing else is success. **What a success is a claim about** is fixed in "What the arms differ by": the style being available, and the tool only on the 70% condition.
- **A second reading, the exact permutation test:** the arm labels are exchanged within each task in every possible way (252 per task of 5 and 5, convolved over the tasks), and the two-sided p-value is the share of assignments whose `|D|` is at least the observed. A percentile bootstrap of 5 runs a cell is **anti-conservative** (its interval is too narrow for so few runs), so the permutation p-value is the reading to trust when the two disagree; the report gives both and does not choose after seeing them. The verdict above stays the registered success rule.
- **Reported besides:** the per-task `M` of each arm, the route stratification above, the per-line satisfaction rate of each arm, and the lines a run **satisfied without doing anything**, measured by the `nothing` agent (`cauldron-bench run --agent nothing`): **9 of 24** (the five Don't lines about pieces and `G-Dn6` in the market and light tasks, and the three greeting Don't lines, which a starting greeting that says nothing satisfies), so the without-style arm starts from 9 and a `D` is read against a ceiling of 15.
- **A task with no scored run in an arm** is left out of both arms' `M` (above); the script says which.

**The script is the registered file `docs/research/2026-11-14-style-helps-an-agent-analyse.py`** (Python 3 standard library). `python3 <it> --selftest` runs it on synthetic data with known answers (a known `D`, a task with an empty cell, ties, a thin route, the permutation p-value, the percentile, and the transcript reading) and has to print `selftest passed` before it is used. It reads each run's `trial.json` (the runner's record) and `transcript.jsonl` (the agent's own stream), takes several run names per arm joined by commas, and prints every number the report gives.

**`cauldron-bench compare` is run and published as well, and is not the success rule.** `compare <without-style report> <with-style report>` (BM-03) says `better`, `worse` or `no measurable change` about the **success of a task** (every check of a task held in a run), pairs and resamples the **tasks**, and a suite of three tasks gives it three of them. That is a different and much coarser question than "lines satisfied per run", so the rule above is computed by the script and `compare` is the second reading, given with its interval and the tasks that moved. The issue's text names `compare` for the interval; this registration departs from it for that one reason. It needs one run per arm, so the five rounds of an arm are assembled into one run directory first (step 5), and the reports are published by `report --publish docs/bench`, because a `--label` on `run` publishes under `<project root>/docs/bench`, which here is `games/sample/docs/bench`.

## What else is collected

For every **with-style** run, from the transcript (`transcript.jsonl` beside the trial's record): its **route** (above); whether it **called `grimoire_style` before its first edit** (a `grimoire_style` call earlier than the first `grimoire_apply_operation`); and **whether its own `grimoire_present` call carried a non-empty `followed` list** (SG-09), read from that call's arguments and nowhere else (never a tool's answer or the agent's prose). And **every wish** (AW) the with-style runs made, quoted whole, with its task: **a wish about the style is the signal for what to change in the templates (SG-08)**. For the without-style arm the report gives its wishes too (the ones about the missing tool are the ones the guide's text causes) and states that `grimoire_style` was not offered (`tools_offered` of every one of its runs is read, and none holds it).

Also in the report, from the runner: tokens, cost, wall time, tool calls, refusals, undos and edits behind the editor's back, per arm.

## What has to be true before the first run, and what is not

These are measured, on 2026-11-14, on a macOS host, with no model.

**Proved (no model, a real editor), at the revised suite:**

- **At the first draft's suite (`ec8642dd3`), with a real editor on a macOS host:** `cauldron-bench run --project games/sample --suite games/sample/bench/tasks --agent nothing` failed all three tasks (5 of 11, 1 of 11 and 0 of 7 checks held), `--agent oracle` passed all three (11 of 11, 11 of 11, 7 of 7), and `--agent oracle` under each `--variant` passed 3 of 3, with `tools_offered` holding `grimoire_style` in the with-style trial (28 tools) and not in the without-style one (27), and `design/` on disk in the with-style copy only. The market and light tasks and the arms have not changed since.
- **At the revised suite, with no editor** (the new test `the_greeting_task_is_scored_on_the_saved_leaf` loads the real greeting task and grades a trial copy laid out as the runner lays it): a greeting never written fails the three Do lines and satisfies the three Don't lines; a good greeting satisfies all six whatever the hand-back says, even one that names every forbidden word; a bad greeting fails all six even when the hand-back states every Do word. Failing first on the task as it was.
- **Not re-run on a real editor at the revised suite**: the host's loopback was out of ephemeral ports when the proofs were repeated (every local connect failed with `Can't assign requested address`, about 34,000 sockets stuck in `TIME_WAIT` that no process of this study owns), so the editor could not answer `grimoire/state`, and the `nothing` and `oracle` runs of the revised greeting task, the round-by-round runs and the assembled-run `report` and `compare` of step 5 were **not** exercised. Step 1 of the commands is that proof, and it is the first thing run on big: `nothing` must fail 3 of 3 (the greeting with 3 of 7 checks held), `oracle` must pass 3 of 3, and step 5's assembly must produce a report that `compare` reads, before any model is run.

**Not proved here, and the first run does not begin until each is:**

1. **A trial has no person to allow the style, and the style is withheld until a person does.** Measured: `grimoire/style` for the courtyard on the with-style trial's project answers `withheld` for both pages, with the line "waiting for the person to allow it, so its text was not served". The trial's `HOME` is its own, so its trust store is empty. **The measurement grants it without a change to the runner**: the editor's trust store is `<data dir>/Grimoire/trusted.yaml`, and on Linux the data directory is `$XDG_DATA_HOME` when it is set (`dirs` 6.0.0, `lin.rs`), which the trial's `HOME` does not override. A store naming each trial's project (`projects:`) is written before the runs and `XDG_DATA_HOME` is set for both arms (the without-style arm never calls the tool, so the store changes nothing for it), as in step 2 below. Proved on the macOS host, with the store in the trial's own `HOME`: the same call then answers `served` for both pages. The store's `version` is a tested constant (`the_seeded_trust_store_is_one_the_editor_reads`). **Before the 30 runs, one unscored with-style probe trial** (`style-probe`, step 3) must show a `grimoire_style` answer with both pages `served` in its transcript, and the script's route reading is checked on it (`--routes`); the `XDG_DATA_HOME` path itself is read from the `dirs` source and is not exercised until that probe.
2. **The decision model scores no line** (above).
3. **`<suite>/variants.yaml` is not where the runner looks:** it reads `<project root>/bench/tasks/variants.yaml` (`suite::variants_path`) whatever `--suite` says, so `bench/README.md`'s "`<suite>/variants.yaml`" is wrong for a suite that is not the project's own `bench/tasks`. Here the tasks and the arms both sit in `games/sample/bench/tasks/`, so the two agree.
4. **The route reading is checked against a real transcript only by the probe.** The script reads an editor tool call as `details.xdev.inner.mcpToolName` and a generic tool call as a `toolcall_end` of `read`, `bash`, `grep`, `find`, `glob`, `eval`, `ast_edit`, `edit` or `lsp`, the shapes `run.rs` reads; the self-test builds them from that code, not from a transcript.

## The commands

On big, in a checkout at the commit that holds this file (`git rev-parse HEAD` is recorded in the report), with `omp` logged in and nothing else heavy running (`jobs 3` starts three editors and three displays at once). `B` is the directory the three binaries are built into.

```sh
cargo build -p bevy_cauldron -p bevy_cauldron_mcp -p bevy_cauldron_bench
B=$CARGO_TARGET_DIR/debug          # bevy_cauldron, bevy_cauldron_mcp and cauldron-bench beside each other
S="--project games/sample --suite games/sample/bench/tasks"
MODEL=anthropic/claude-sonnet-5-5
omp models find $MODEL             # must list it
A=docs/research/2026-11-14-style-helps-an-agent-analyse.py
python3 $A --selftest              # must print: selftest passed

# 1. No model: the pipe and the tasks. `nothing` must fail 3 of 3, `oracle` must pass 3 of 3, under each arm.
$B/cauldron-bench run $S --agent nothing
for arm in with-style without-style; do $B/cauldron-bench run $S --agent oracle --variant $arm; done

# 2. Let the trial projects' style be served: a store naming every trial project of the runs below, for the editor's XDG_DATA_HOME.
STATE=$(realpath "$HOME")/.local/state/cauldron-bench
TASKS="dress-the-market-area write-the-gatekeepers-greeting light-the-gate-at-night"
mkdir -p /tmp/style-xdg/Grimoire
{ echo "version: 3"; echo "projects:";
  for task in $TASKS; do echo "- $STATE/style-probe/$task/trial-1/project"; done;
  for k in 1 2 3 4 5 6 7 8; do for arm in without-style with-style; do
    for task in $TASKS; do echo "- $STATE/style-r$k-$arm/$task/trial-1/project"; done; done; done;
  echo "commands: []"; echo "texts: []"; } > /tmp/style-xdg/Grimoire/trusted.yaml
export XDG_DATA_HOME=/tmp/style-xdg

# 3. The probe: one unscored with-style trial. Its transcript must show a `grimoire_style` answer with both pages "served".
$B/cauldron-bench run $S --agent omp --model $MODEL --variant with-style --task light-the-gate-at-night --trials 1 --run style-probe
grep -c 'served' $STATE/style-probe/light-the-gate-at-night/trial-1/transcript.jsonl   # read the answer: both pages "served", none "withheld"

# 4. The 30 runs: five rounds, each task once in each arm, the arm order alternating, the three tasks of an arm at once.
for k in 1 2 3 4 5; do
  if [ $((k % 2)) = 1 ]; then order="without-style with-style"; else order="with-style without-style"; fi
  for arm in $order; do
    $B/cauldron-bench run $S --agent omp --model $MODEL --variant $arm --trials 1 --jobs 3 --run style-r$k-$arm
  done
done

# 5. The result. The registered script first (run names joined by commas), then the published reports and `compare`.
W=$(printf 'style-r%s-without-style,' 1 2 3 4 5); W=${W%,}
R=$(printf 'style-r%s-with-style,' 1 2 3 4 5); R=${R%,}
python3 $A $W $R
for arm in without-style with-style; do                      # one run directory per arm, for `report` and `compare`
  rm -rf $STATE/style-$arm
  for k in 1 2 3 4 5; do for task in $TASKS; do
    mkdir -p $STATE/style-$arm/$task; cp -R $STATE/style-r$k-$arm/$task/trial-1 $STATE/style-$arm/$task/trial-$k
  done; done
  $B/cauldron-bench report $STATE/style-$arm --label style-$arm --publish docs/bench
done
$B/cauldron-bench compare docs/bench/runs/<date>-style-without-style.json docs/bench/runs/<date>-style-with-style.json
```

If an arm is unfit (more than 10% harness errors), make the missing runs up in further rounds (`k` 6, 7, 8, which step 2's store already names), add them to the script's run names, and give the unfit rounds' numbers in the report.

**Cost and time (an estimate, not a measurement of this suite).** The model's measured cost on small benchmark tasks has been about 0.12 to 0.29 USD a trial, so the 30 runs and the probe are about **3.7 to 9 USD**, and a repeated arm adds about 2 to 4.5 USD. A round runs an arm's three tasks together, so the wall time of a round is about its slowest task (the budgets are 900, 600 and 600 s): **about 0.5 to 1.5 hours for the ten invocations, 2.5 hours at the worst**.

## What the report holds

`docs/research/<date>-style-helps-an-agent-report.md`, linking this file and its commit (the SHA `git log -1 --format=%H -- docs/research/2026-11-14-style-helps-an-agent.md` prints), with: the header the runner writes for each arm (editor commit, guide SHA-256, model and price, host, dates), the registered numbers (`M` of each arm, `D` and its interval, the verdict, the permutation p-value), the route stratification and the share that called the tool, per task and per arm the lines satisfied per run, per line the rate satisfied, the calls to `grimoire_style`, the `followed` citations and the wishes, the `compare` result, the harness errors, and **every departure from this file**. **Each template or guide change the result motivates is filed as its own issue in the style series (#399), with the report linked**, and the report is where the numbers are, not the issue.

## Limits, stated before the run

- **One model, one game, three tasks, five runs of each.** The result is about this model on this style on these tasks. A `no measurable change` with 5 runs a cell is a small sample's answer, and is reported as that. A percentile bootstrap at 5 runs per cell is anti-conservative; the permutation p-value is reported beside it for that reason.
- **The arms differ by the tool and by the files**, so the primary result is about the style being available (above), and the route stratification is descriptive, not randomised: an agent chooses its route.
- **A line that holds at the start is satisfied by doing nothing** (9 of 24). The without-style arm cannot score under 9 unless a run makes a line worse (an agent that places a `neon_sign`, tints the light, or writes `welcome`).
- **These checks are weaker than their line**, because the fixture offers nothing finer, and each is stated so the result is not read as more than it is:
  - `L-Do1` (two lanterns, one on each side of the gate) checks `count == 2`, not that one is on each side: the lighting data has no side.
  - `G-Do1`, `G-Do2`, `G-Do3` (every lit place) check the gate's light, the one lit place the data names.
  - `G-Do5`, `G-Do6` (a lantern and an awning at every stall) check that there are three of each in the market, not that each stall has its own.
  - `L-Do4` (every stall turned to yaw 180) measures the first stall the map lists (`placement_at` takes the first match); the row (`L-Do3`) measures all three.
  - `L-Do3` and `L-Do4` use a stall's centre, not its footprint.
- **The substring checks read the greeting without case**: `friend` is also found in `friendly`, and `crown` in `crowned`. The cost is the same in both arms.
- **The lines are the style's, and the checks are mine.** A line is satisfied or not by a number or a word; whether the result is *good* is not measured. The style was written to be scored, so the experiment measures whether an agent follows a stated, checkable style it can read, and not whether a style written for a person improves a game.
- **The market library reuses one mesh for every piece.** The tasks measure placement and data, never a render.
- **The with-style arm may read `design/` from disk**, and the without-style arm's disk has none (the route stratification counts it).
- **The without-style agent is told by the guide to call a tool it does not have** (above).
