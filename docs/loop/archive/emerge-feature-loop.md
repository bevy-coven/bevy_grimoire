> **SUPERSEDED — do not run.** `build-loop` replaced this command. That is not cosmetic: this file's
> own description says "publish the release to crates.io" and it carries a full Phase 6 publish
> procedure, while `build-loop` forbids publishing outright ("Never publish, tag, yank, or change crate
> ownership from this loop. E-006 is open and publication is the human's decision"). Publishing cannot
> be undone — crates.io has no unpublish — so a command capable of it must not sit in a live commands
> directory. Kept for its history only.

---
description: One iteration of the bevy_grimoire usability loop. Fix confirmed bugs, ship at most one verified usability enhancement with visual proof, publish the release to crates.io, then stop.
argument-hint: "[optional focus area, e.g. compositions | wfc | labeller | tile-kits | maps]"
disable-model-invocation: true
---

# bevy_grimoire: one loop iteration

## Non-negotiables

Read these first. They are repeated as a checklist at the end.

1. **Evidence over claims.** Every claim in your report points to something a reviewer can re-run or open: a command and its output, a test name, a file, or a frame index.
2. **Deterministic checks gate everything.** Build, clippy, tests, the parity test, and the protected checks must pass. No reviewer opinion, VLM judgment, or screenshot overrides a failing check.
3. **Never weaken a check to pass it.** Do not edit, skip, or loosen tests, assertions, golden images, or protected checks to make them pass. Any diff to test code must be justified in the report.
4. **At most one enhancement per iteration, or one step of the open series — and shipping nothing is allowed.** If you cannot verify it, commit only the bug fixes, log what blocked you, and stop.
5. **Stop on repeated failure.** If two attempts at the same step fail, stop, log your hypothesis and the evidence, and end the iteration. The next iteration starts with a fresh context.
6. **Human work is sacrosanct.** Never modify or delete real user content, and never let an agent operation change human-authored or locked regions unless that operation explicitly targets them.
7. **Content is data, and data hot-reloads.** Anything a developer or agent authors or tunes is data. It changes in the running editor through MCP/BRP or by editing a file, takes effect in about a second, and needs no recompile or restart. Needing a rebuild to change content is a bug.
8. **Publishing is irreversible.** A crates.io version can never be overwritten or deleted, only yanked. Publish only from a green gate, a clean working tree, and a pushed, tagged commit. Never use `--allow-dirty`, never skip the dry run, and never yank anything yourself.

## Context

You are running one iteration of a long, repeated loop on **bevy_grimoire**, a Bevy 0.20 editor for authoring game worlds: maps, tile kits, compositions, WFC (wave function collapse) generation, a VLM-assisted labeller, and an agent-drivable UI. Each iteration starts with a fresh context. Your only memory of earlier iterations is the backlog, the loop log, and git history.

A game developer sits in front of this editor. The goal is a truly expressive editor where **the human describes and the agent drives**. The target workflow has four steps, and every iteration should make one of them faster or more reliable:

1. **Describe.** The developer says what they want quickly, in words, a rough sketch, a selection, or a reference, without needing to know which panels or parameters are involved.
2. **Set up.** The in-editor agent turns that into typed editor operations, never free-form code. It reads the current editor state before acting, keeps its changes to existing work as small as possible, and leaves pinned or locked regions alone. When the request is open-ended, it offers 2–3 alternatives instead of one.
3. **Inspect.** The agent checks its result before showing it. Objective properties are checked by automated validators: connectivity, WFC validity, adjacency rules, and label agreement. Visual properties are checked with stills, frame sequences, and state dumps. For generated content, the agent runs several seeds and reports how varied the results are.
4. **Present.** The agent shows the developer a short, visual summary:
   - what it did;
   - what the developer specified versus what the agent inferred;
   - the 1–3 specific places most worth checking;
   - uncertainty, stated only where it is real and tied to a specific region or item.

   Every change can be undone in one step. Operations that are irreversible or wide in scope are previewed and need explicit confirmation before they commit.

The agent reaches the editor only through **the bevy_grimoire MCP server** (see "MCP server"). Gaps in any of these four steps are first-class usability problems.

How the repo is laid out (check against the current README):
- `bevy_grimoire_core` holds the engine-free schema, grammar learning, WFC and validation.
- `bevy_grimoire` is the runtime that puts a library and a map into a Bevy world.
- `bevy_cauldron` is the editor application. It opens a project directory and writes `<root>/assets/grimoire`, which games then load.

That project directory is where the editor's data lives (see "Data-driven and hot-reloadable").

Values, in order when they conflict: usability, then performance, then aesthetics. Never ship a regression in any of them to gain another.

Focus for this iteration: `$ARGUMENTS`. If empty, choose based on the evidence you gather.

## Direction

**Bevy is the app; Blender is a tool.** Scenes are authored in bevy_grimoire because that is where the game runs. Blender is driven through the golem MCP for what a DCC does (meshes, UVs, bakes, painting, rigging), and its results enter bevy_grimoire as kit pieces. Do not fork Blender or rebuild DCC features in Bevy.

**Collaborate the way Mixar does.** The agent shows what it is doing as steps narrated by measured effect (the oplog's `result` lines, never raw logs), hangs captures on the step that made them, groups a turn so one action takes it back, and hands results to the person with explicit keep/undo answers that come back to the agent. Before building any of it, follow `docs/loop/borrowing-from-mixar.md`: it has the licence rule, the pinned source, the procedure, and the catalogue of beats with their status.

**Emergence at every scale.** Mesh → piece in a kit → composition (assembly with rules) → generated layout → scene. Every scale must be reachable by an agent through a registered operation. A scale an agent cannot reach is a parity gap of the first rank.

**The headline number** is `reached` from `python3 tools/loop/accept.py`: how far an agent gets from "a mossy ruined courtyard with two exits" to a presented, saved result. Rank work that raises it above work that does not.

## Definition of done

This iteration is done when all of the following are true, or when you have taken the "ship nothing" exit (Non-negotiable 4):

1. Every confirmed must-fix bug (see "Phase 1") is fixed, or recorded as blocked with the reason.
2. At most one usability enhancement is implemented — or one step of the open series — and it meets the user-facing goal written in its plan. "It builds" does not count as meeting the goal.
3. Each fix and the enhancement has evidence:
   - a check that **failed before the change and passes after it**, kept as a regression test;
   - for anything visible, before/after stills and frame sequences you have opened and inspected.
4. All deterministic checks pass: build, `cargo clippy --all-targets -- -D warnings`, tests, MCP-client integration tests, the parity test, and the protected checks.
5. The iteration report and its curated visuals are committed with the code (see "Documentation").
6. If the iteration changed published crate code, a new version is published to crates.io and verified there (see "Phase 6"). Otherwise the report says why no release was made.
7. `tools/loop/accept.py` ran before and after; both reached counts are in the report, and a lower count than the previous iteration's is a must-fix bug.
8. `tools/loop/perf.py` ran before and after, and both are in the report.
9. The independent review (Phase 4.4) ran. An iteration without it does not merge.

Then stop. Log any further enhancement ideas as backlog candidates for a later iteration.

## Phase 0: Orient (keep it brief)

- Read `AGENTS.md` (and `CLAUDE.md` if present) for build, test, and run commands. Where they conflict with mechanics in this file, they win, except for the Non-negotiables.
- Read `docs/loop/backlog.json`, the last 5 entries of `docs/loop/LOG.md`, and `git log --oneline -20`. Create the state files if they are missing (formats at the end).
- Check the previous release:
  - `cargo info <crate>` shows the version that `LOG.md` says was published;
  - its docs.rs build succeeded;
  - its git tag exists on the remote.

  A failed docs.rs build or a missing release is a must-fix bug for this iteration.
- Run the baseline: build, clippy, and tests. If the baseline is red, that failure is your first bug.
- Record `uptime` and `df -h /`. If the load average is above the core count, run tests with `-- --test-threads=4`. If free disk is under 20 GB, ask the human before deleting anything; a linker `errno=28` is a full disk, not a code failure.
- Run `python3 tools/loop/perf.py --source ~/sanctuary/assets --out target/loop/<date>-<sha>/perf-before.json` and `python3 tools/loop/accept.py --source ~/sanctuary/assets --out target/loop/<date>-<sha>/accept-before.json`. If either cannot run, fixing it is this iteration's first must-fix bug.
- Run `python3 tools/loop/backlog.py check`.

If you cannot launch the editor, reach it through MCP, or capture stills and sequences, fix that first and treat it as a must-fix bug.

Bootstrapping, if these pieces don't exist yet:
- **No MCP server.** Build the minimum: a state summary tool, a still-capture tool, and a sequence-capture tool. That counts as the iteration's enhancement.
- **No shared operation layer.** Do not retrofit the whole editor at once. Introduce the registry and dispatch path, migrate one workflow (UI and MCP both), and add the parity test with the unmigrated actions on an explicit allowlist that later iterations shrink.

## Data-driven and hot-reloadable

The editor exists so that a human describes, and an agent sets things up quickly. Recompiling a Rust project for every change is too slow for that loop. The editor's design target is that **authoring never needs a rebuild**. Recompiles are for new capability, never for new or changed content.

**What is data.** Everything authored or tuned lives as versioned data files in the project directory, loaded through Bevy's asset system. That covers:
- libraries and tile kits;
- maps and compositions;
- WFC rules, weights, pins and negative examples;
- label vocabularies and labels;
- generation seeds;
- visual and lighting parameters;
- the settings of the family crates the editor uses (see "Use the ladvien crate family");
- editor preferences.

If a value that someone would want to tweak is hard-coded in Rust, that is a usability bug. Move it into data.

**Two paths to the same state, both live.**
- **File path.** Editing a data file on disk updates the running editor in place, through Bevy asset hot reload (the `file_watcher` feature) or an equivalent watcher. There is no restart. Camera, selection and the operation log survive the reload.
- **Live path.** An agent changes the same value through an MCP operation, or through BRP on a registered `Reflect` resource or component. The change shows up immediately and **is written back to the data file**, so nothing exists only in memory.

Both paths must converge. Test it: a live change, saved and reloaded from disk, reproduces the same state, and a file edit shows up in MCP state queries.

**Bad data never takes the editor down.** A malformed or invalid file is rejected with its file, field and line, reported through MCP and in the UI. The last good state stays loaded. Schema changes are versioned and ship with a migration.

**Speed is measured.** Track **edit-to-visible latency**: the time from a file save or MCP call until the first presented frame that shows the change. Measure it for both paths, on the same representative scene used for frame time. Target about one second or less for single-asset changes. Report it in every iteration alongside frame time. A regression is a bug.

**When code is the right answer.** New operation kinds, new validators, and new component or asset types need code. When you add one, make its parameters data from the start, register its types for reflection so BRP can see and change them, and add it to the operation registry. Do not make a value data-driven unless it is actually authored or tuned. Avoid building a scripting language inside RON files. Keep data-driven features simple, because over-built data-driven tooling costs more than it saves.

**Bevy specifics, as of 0.20.**
- BSN scenes cannot yet be loaded from `.bsn` files. Bevy lists that as upcoming work, so use serde assets (RON or JSON) now, and keep the schema simple enough to move later.
- Bevy's subsecond hotpatching (introduced experimentally in 0.17, covering ECS systems only) may speed up your own code iteration. It is never the mechanism for content changes.
- For dev builds, use `bevy/dynamic_linking` and a fast linker to keep the unavoidable rebuilds short.

## Use the ladvien crate family

Games built with this editor, and the editor itself, use the author's own Bevy crates first. Don't write a new system or add a third-party crate for something a family crate already does. Doing so needs a logged reason.

| Need                                                                       | Crate                      |
| -------------------------------------------------------------------------- | -------------------------- |
| Sun, moon, sky, weather, wind, clouds, fog, terrain erosion, wetness, snow | `bevy_elements`            |
| Procedural trees and undergrowth that grow, with seasons                   | `bevy_flora`               |
| SDF to mesh, voxel terrain, carving, booleans, debris                      | `isomesh` / `bevy_isomesh` |
| Runtime gore, fracture, wounds, blood                                      | `bevy_carnage`             |
| Speech and thought balloons, conversation graphs, dialog box               | `bevy_speech_bubbles`      |
| Illuminance field for creature AI                                          | `bevy_light_grid`          |
| Stigmergic influence fields                                                | `bevy_stigmergy`           |
| Local collision avoidance (ORCA)                                           | `bevy_orca`                |
| Agent debugging of a running game over BRP                                 | `bevy_debugger_mcp`        |
| A frame on demand                                                          | `bevy_devshot`             |

Rules:
- **Configure, don't fork.** Use each crate's presets and parameters as data in the project directory, editable live like everything else. If a family crate's settings can't be changed at runtime, log it against that crate rather than wrapping it in hard-coded values.
- **Fix at the source.** A bug or missing feature in a family crate is fixed in that crate's canonical repository, in its own commits and with its own tests. Never vendor or patch a copy here. Some of these repos are read-only mirrors of `foundation_vs_slop`, so check the crate's README for where changes belong. If the fix is outside this iteration's scope, log it in the backlog with the crate named.
- **Versions.** Keep the family crates on the same Bevy version as the editor. A version mismatch is a must-fix bug.

## MCP server

bevy_grimoire ships an MCP server built on the official Rust SDK (`rmcp`). Every agent, including you, drives the editor through it. If you find yourself working around it (raw BRP calls, editing files directly, simulating clicks), that is a parity gap. Log it as an enhancement candidate. Screenshots are for perception and verification only. Never drive the UI by clicking.

**Interoperability and safety**
- Stdio transport at minimum. Rely only on the MCP specification.
- Tool descriptions are factual and complete: what the tool does, what it doesn't do, when to use it, and what it returns. No persuasive wording such as "always prefer this tool".
- Set MCP tool annotations (`readOnlyHint`, `destructiveHint`, `idempotentHint`) accurately.
- Destructive or bulk operations take a `dry_run` flag and require confirmation. Examples: deleting a layer or kit, overwriting a map, regenerating a whole region.
- File and path parameters are confined to the project directory.

**One operation layer, two front ends**
- Every user action is defined once, as a typed operation in a single registry: parameters, validation, apply, and undo. UI widgets and MCP tools are both thin front ends that build an operation and dispatch it. Neither changes editor state any other way.
- As a result, user and agent actions match one-to-one by construction: same name, same parameters, same validation, same errors, same undo.
- MCP tools and `docs/mcp-tools.md` are generated from the registry, or mechanically checked against it.
- Continuous gestures such as drags and brush strokes are not operations. The operation each gesture emits when it completes (for example, a stroke with its points) is what MCP exposes.
- Every operation can run in **preview mode**: it shows its result without committing. Preview is the default for agent proposals that are irreversible or wide in scope. Everything else applies directly and relies on undo.

**Undo and the operation log**
- Each operation declares its undo strategy:
  - an inverse delta, for cheap edits;
  - a snapshot or re-seed, for procedural operations such as `grimoire_run_wfc`.

  Do not assume undo is cheap and uniform.
- An agent's multi-step action is grouped into one undo step, so a single undo reverts the whole agent step.
- Define and test what happens when a human edits inside a region the agent generated, and that generation is later undone or re-run.
- One append-only operation log is the source of truth, and editor state is derived from it. Each entry records:
  - source (human or agent);
  - session or agent ID;
  - operation and parameters;
  - RNG seed and rules hash, where relevant;
  - result summary.
- Undo is recorded as a new event, not a deletion. The agent can read what the human just did, and any session can be replayed.

**Parity is tested, not assumed**
- A parity test enumerates the registry and fails if any operation lacks an MCP tool or a UI entry point, or if the two expose different parameters.
- Any UI code path that changes editor state without dispatching a registered operation is a must-fix bug.

**Tool design for agents**
- Tools speak in domain terms: `grimoire_create_map`, `grimoire_paint_tiles`, `grimoire_place_composition`, `grimoire_run_wfc`, `grimoire_label_tiles`, `grimoire_capture_sequence`. They do not speak in entity IDs or component JSON.
- Every tool is prefixed `grimoire_`.
- Keep the always-loaded toolset small. Put rarely used tools behind a discovery or category tool.
- A mutating tool returns a short state delta, plus an image only when one is useful.
- Large results are capped and paginated. When a result hits its cap, the response says how to narrow the query.
- Invalid calls are rejected atomically, with a message saying what was wrong and what to change.
- Relevant validators run after every mutation. Their failures appear in the tool result.
- Every mutating tool returns an operation ID that the agent can cite when presenting and the human can use to revert.
- Capture tools return images as MCP image content, plus file paths to the full-resolution frames.
- A presentation tool puts the agent's result in front of the developer: summary, specified versus inferred, the places to check, before/after, and accept, adjust, or undo.

**Testing new tools**
When you add or change a tool, test it as an outside agent would: list the tools, read only the description, call the tool, and check the result is usable with no project knowledge. Add an integration test that calls it through an MCP client.

## Visual capture

Many usability problems only show up over time: flicker, layout jumping, progress that stalls, stale state, lagging hover or focus, a WFC run that thrashes. Capture three things.

- **Stills** for layout, via `grimoire_capture_still`. Under the hood this uses Bevy's `Screenshot::primary_window()` with the `save_to_disk` observer. Bevy needs its `png` feature or the files come out empty.
  - Capture at native resolution.
  - For fine layout checks, review full-resolution crops of the region, not downscaled whole frames.
- **Sequences** for anything that changes over time, via `grimoire_capture_sequence { seconds, fps, label }`.
  - Default to **4 fps** (one frame every 250 ms). Use 2 fps only for long, slow operations where nothing short-lived can happen.
  - Capture the interaction plus about 2 seconds to settle. Start capture before triggering the action, so frame 0 is the state before it.
  - Name frames `<label>/f000_t0.00s.png`, `f001_t0.25s.png`, and so on.
  - Stitch frames into contact sheets of **at most 8 frames** (2 rows × 4 columns, about 2 seconds at 4 fps). Label each cell with its index and timestamp.
  - Review the sheets first, then open full-resolution frames or crops wherever a cell looks off.
- **State dumps** alongside every still or sequence: the UI layout rectangles, visible widget states, and the relevant entity or tile summary, as text. A screenshot without a state dump is half the evidence.

Reviewing a sequence:
- Compare it against a **reference**: the same interaction on the pre-change build, or the last known-good frame. Do not judge frames in isolation.
- Ask specific questions about named elements. For example: "Does the WFC progress bar advance between f002 and f006?" or "Is the Apply button enabled in the final frame?" Do not ask "does anything look wrong?"
- Check for:
  - visible feedback for every action;
  - flicker, jumps, or reflow;
  - progress that advances and then resolves;
  - a final state that matches the expected end state and stays stable through the settle time;
  - nothing stuck in a hover, loading, or disabled state.

Save raw captures under `target/loop/<YYYY-MM-DD>-<short-sha>/`. They are working files and are not committed.

## Measuring responsiveness

Frame captures cannot measure latency. Instrument it: timestamp each input event and the frame where its effect is first presented, and report the difference. Targets:

| Interaction                                                             | Target, input to presented frame                   |
| ----------------------------------------------------------------------- | -------------------------------------------------- |
| Continuous manipulation: painting, dragging, gizmos, camera             | Within one frame, and never above 50 ms            |
| Discrete command acknowledged (button, menu, tool call reflected in UI) | At most 100 ms                                     |
| Operation taking longer than about 1 s                                  | Shows progress within 100 ms, and can be cancelled |

Measure frame time and latency on runs without capture, because capture adds its own cost.

## Phase 1: Bugs (a bounded sweep)

Find candidates in this order:
1. Open bugs in `backlog.json`.
2. Failing or flaky checks, clippy errors, and runtime `warn!`/`error!` output or panics, including Bevy system-ordering and query-conflict warnings.
3. Parity breaks: user actions with no MCP equivalent, and UI code that changes state without dispatching an operation. Search the UI code for direct state mutation.
4. A hands-on pass through MCP: create a map, paint with a tile kit, place a composition, run WFC, label tiles, save, reload, and confirm nothing changed. Record sequences of anything that animates, generates, loads, or responds to input, and a still plus state dump of each result. Bias this pass toward the focus area, if one was given.

**Reproduce before fixing.** For timing or transition bugs, the reproduction is a sequence that shows the bug. For anything generated, record the full generation context:
- seed, rules or tileset hash, grid size, pinned cells, build version, and thread count;
- for the labeller, the recorded VLM responses.

A seed alone does not make a run reproducible. If you cannot reproduce a bug, log it as `unconfirmed`.

**Decide must-fix with three yes/no checks.** Do not use a single severity score. Record the evidence behind each answer:
- **Blocks:** does it block or corrupt a core workflow (crash, data loss, broken save, wrong result)?
- **Common:** does it occur in ordinary use, not just an edge case?
- **No workaround:** is there no reasonable workaround?

A bug is **must-fix** if Blocks is yes, or if any two answers are yes. If the call is borderline, get a second independent judgment from a fresh-context subagent given only the evidence. If the two judgments disagree, treat the bug as must-fix and flag it in the report. Fix bugs that aren't must-fix only if they are trivial and in code you are already touching. Otherwise, log them.

**Fixing rules**
- Write the regression check first and watch it fail. For ECS logic, a minimal `App` with the plugin under test, `update()`, and an assertion is usually enough. For visual bugs, the before sequence is the reproduction and the after sequence of the same interaction is the proof. Pair them with a state-dump assertion where you can.
- Fix the root cause. Do not silence warnings, swallow errors, or loosen assertions.
- One commit per bug: `fix(<area>): <what broke>, <why>`.

**Failure triage.** A red test in a full run is a suspect, not a verdict.
- Rerun that test binary alone. Load-dependent failures are the commonest false attribution this loop makes.
- If it passes alone, run it on the base commit: `git worktree add /tmp/grimoire_base <sha>` with `CARGO_TARGET_DIR=/tmp/grimoire_base_target`, then remove both. A failure the base also has is pre-existing: log it, and do not attribute it to this iteration's change.
- Never use `git stash` or `git checkout -- <file>` over uncommitted work to recreate a "before". A worktree at the base commit is the only honest "before", and it cannot touch the tree you are working in.

**Budget.** If there are more must-fix bugs than you can fix well (roughly five or more, or your context is getting heavy), fix the most damaging, log the rest, skip Phase 2, and say so in the report.

## Phase 2: Choose the enhancement

If `backlog.json` holds an open item of kind `series`, its next open step is this iteration's enhancement; skip the candidate list.

List 2–3 candidates drawn from:
- the backlog;
- friction you hit in Phase 1;
- playtest notes in the repo;
- the lenses below.

For each candidate, write one line covering:
- the user problem;
- how often a developer hits it;
- the expected saving in time or errors;
- the risk.

Rank the candidates against each other and pick the top one. Record the others in the backlog with their line.

Lenses (look for problems, not features):
- **The describe → set up → inspect → present loop.** Phrase a realistic request the way a developer would ("a mossy ruined courtyard with two exits"). Find the step where manual work, guessing, unverified output, or an unhelpful presentation gets in the way.
- **Collaboration (Mixar).** The first catalogue row in `docs/loop/borrowing-from-mixar.md` whose status is `backlog` or `unexamined` is a candidate. If it is chosen, the procedure in that file is part of the build.
- **Parity:**
  - a UI path that bypasses the operation layer;
  - a missing MCP tool;
  - a tool description an outside agent would misread;
  - an agent action with no visible feedback;
  - an irreversible operation with no preview.
- **Agent self-inspection.** Can the in-editor agent capture stills, sequences, and state dumps, and run validators itself? The capture tooling must be an editor capability, not just a harness for this loop.
- **Data and hot reload:**
  - authoring or tuning values hard-coded in Rust;
  - anything that needs a restart or rebuild to see a content change;
  - live MCP or BRP changes that aren't written back to disk;
  - bad data that crashes or resets the editor;
  - slow edit-to-visible latency;
  - family-crate settings that can't be edited live.
- **Designer control.** Pinning and locking regions; a weight brush; "never generate this" negative examples; alternatives offered when the designer's own constraints conflict.
- **WFC as constraint solving.** Do contradictions trigger local repair or backtracking instead of a full restart? Are committed regions frozen? Does the tileset pass a completeness check before generation (for every pair of edges, at least one tile fits both)? Does a contradiction produce a structured trace: the cell, its emptied set of options, and the chain of neighbour constraints that emptied it?
- **Label confidence.** Is confidence computed from agreement across several samples, prompts, or the tile kit's adjacency rules, rather than the model's self-reported number? Are low-agreement tiles shown to the human first?
- **Visibility of system status, error recovery, and consistency** with Bevy Feathers conventions.
- **Responsiveness** against the targets in "Measuring responsiveness".
- **Expressiveness.** Across several seeds, how varied are the outputs on emergent metrics (empty-space %, isolated regions, tile entropy, connectivity)? Does one constraint collapse the variety?

Not candidates: refactors with no user-visible effect, new subsystems, dependency upgrades, or save-format changes without a migration. Log those.

Before writing code, add a short plan to the iteration report:
- the change;
- the files and functions to touch;
- the **user-facing goal** in one sentence;
- the check that will prove it: one that fails now and passes after.

### Series

A series is a backlog item `{id: "S-###", kind: "series", title, goal, approved_by, acceptance: [accept.py step ids it must turn to pass], steps: [{id, title, status}]}`.

- A series is opened only for work that cannot fit one iteration, and only with the human's approval recorded in `approved_by`. Without that string there is no series, whatever the plan says.
- At most one series is open at a time. A second one is a second trunk, and the loop would have no rule left about breadth.
- Each iteration does exactly one step, with its own failed-before check, report and merge. A step that cannot close is marked `blocked` with `last_failure`, and the series carries on with the next step only if it does not depend on the blocked one.
- The series closes when its `acceptance` steps pass in `tools/loop/accept.py`. Closing it is a backlog edit, not a separate iteration.

## Phase 3: Build

- Follow existing patterns. Reuse Feathers widgets and the editor's theme values.
- A new or changed user action is defined once as a registered operation, with its undo strategy and preview behaviour. Both the UI and the MCP tool dispatch it. Do not implement behaviour in either front end.
- Keep the diff to the plan. If it needs to grow, stop and re-scope.
- Add tests for new logic. Keep the plan's failing check as a regression test.

## Phase 4: Prove it

Run these in order. Each later layer runs only after the earlier one passes.

1. **Deterministic gate.**
   - Build, clippy, tests, parity test, MCP integration tests, and protected checks all pass.
   - The plan's check failed before the change and passes after it. The failed-before run is captured when the test is written, before the fix, and its failing assertion is quoted in the report. A test that would have passed before the change is not a regression test.
   - Confirm the tests actually exercise the new behaviour, not just that they pass.
   - List every diff to test files or assertions, with its justification.
2. **Visual proof.**
   - After stills and state dumps at the same state and window size as the baseline, at the default size and one smaller size.
   - A sequence of the changed workflow end to end, reviewed against the reference sequence using the checklist in "Visual capture".
   - The enhancement is not proven until a sequence shows it working across time.
3. **Responsiveness and performance.** Measure frame time, input latency, and edit-to-visible latency without capture, and compare with the baseline. Run `tools/loop/perf.py` again and compare with `perf-before.json`. If the change touched data or assets, also prove hot reload with the editor running and no restart:
   - edit the affected data file and capture a sequence showing the update;
   - make the same change through MCP and show the data file's diff on disk;
   - feed a malformed file and show the error report, with the last good state still loaded. A regression beyond run-to-run noise is a bug: fix it, or explain it with numbers.
4. **Independent review.** This step is mandatory. Use the task tool's `reviewer` agent with the diff, the plan and the evidence paths. If no subagent can be spawned, ship nothing and say why. Give it only:
     - the diff and the plan;
     - the before/after stills, contact sheets, and state dumps;
     - the gate output.
   - It reports only gaps that affect correctness, the stated goal, or visual quality. Each gap must cite a frame index, crop, or diff hunk.
   - Then run a skeptic pass. For each reported gap, try to give a benign explanation. A gap blocks shipping only if it survives that pass.
   - Fix real gaps, with at most two review rounds. Reviewer approval never overrides a failing check.
5. **Acceptance.** Run `tools/loop/accept.py` again and report `reached` before → after.

If a still or sequence shows something wrong, it is wrong, whatever the tests say. If the enhancement cannot be proven, take the "ship nothing" exit.

## Phase 5: Ship, report, stop

- Run the final gate once more.
- Commit as `feat(<area>): <user-visible change>`, with a body covering the problem, the change, and where the evidence is.
- Write the iteration report and commit its curated visuals with the code (see "Documentation").
- Regenerate `docs/mcp-tools.md`.
- Write the report after the feature commit, and name it and its asset directory with that commit's short sha.
- Add the iteration's line to `LOG.md` and update `backlog.json`.
- Run `python3 tools/loop/backlog.py archive`, then `check`.
- New backlog ids come from `python3 tools/loop/backlog.py next-id <B|E|S>`.
- Go on to Phase 6. Do not push or open a PR except as Phase 6 describes, unless `AGENTS.md` says otherwise.

## Phase 6: Release to crates.io

While `E-006` is open (nothing in this workspace is published), skip this phase and write "No release: E-006 open" in the report.

Release when this iteration changed code or metadata of a published crate: a bug fix, the enhancement, or both. Do not release for docs-only or loop-state-only changes, or when nothing shipped. Say which case applies in the report.

**1. Decide the version.** Run `cargo semver-checks check-release` against the latest published version (install it with `cargo install cargo-semver-checks --locked` if missing).

The crate is pre-1.0, so a version is written 0.MINOR.PATCH:
- **Breaking change** (anything semver-checks flags, a Bevy version bump, a save-format change, a removed or renamed MCP tool or parameter): increment MINOR and reset PATCH. 0.4.2 becomes 0.5.0.
- **Anything else:** increment PATCH. 0.4.2 becomes 0.4.3.
- Never go to 1.0.0. That is the human's call.
- Never ignore a semver-checks finding by bumping less than it requires.

In a workspace, bump only the crates whose published contents changed. Bump the version requirements of internal dependents to match.

**2. Prepare the release commit.**
- Update the version in `Cargo.toml`, and refresh `Cargo.lock` if the repo commits it.
- Add a `CHANGELOG.md` entry under the new version in Keep a Changelog style (Added / Changed / Fixed / Breaking). Write it from the developer's point of view, link the iteration report, and give migration steps for breaking changes.
- Check the README's install snippet and Bevy compatibility table still match.
- Commit as `release(<crate>): v<version>`.

**3. Pre-publish gate.** All of these must pass, in order, on a clean working tree:
1. The full Phase 4 deterministic gate, rerun on the release commit.
2. `cargo doc --no-deps` with `RUSTDOCFLAGS="-D warnings"`.
3. `cargo package --list`, reviewed by you. The package must not contain:
   - `target/`, `docs/loop/` (iteration GIFs and stills), or protected test fixtures;
   - secrets or `.env` files;
   - any large asset the crate doesn't need at runtime.

   Fix this with `include`/`exclude` in `Cargo.toml`. Keep the package well under the crates.io 10 MB limit.
4. `cargo publish --dry-run`, or `cargo publish --workspace --dry-run` for a workspace (Rust 1.90+, which also publishes crates in dependency order).
5. Required package metadata is present: `license`, `description`, `repository`, `readme`, `keywords`, `categories`, `rust-version`.

If any step fails, fix it on a new commit and rerun the gate from step 1. If you cannot get it green, do not publish. Log the blocker and stop. The release goes out next iteration.

**4. Tag and push.**
- Create an annotated tag on the release commit: `v<version>`, or `<crate>-v<version>` in a workspace.
- Push the commit and the tag to the default branch.
- If `AGENTS.md` forbids pushing, stop here with the release prepared and say so in the report.

**5. Publish.** Use the first channel that applies:
- **CI with trusted publishing.** This is preferred. Use it if `.github/workflows/release.yml` exists and runs `rust-lang/crates-io-auth-action` on tag push.
  - Do not publish locally. Pushing the tag triggers the workflow.
  - Wait for the workflow with `gh run watch`, then go on to verification.
  - If the workflow fails, read the logs and report the failure. Do not fall back to a local publish for the same version.
- **Local publish.** Use this only if no release workflow exists, `AGENTS.md` permits local publishing, and `CARGO_REGISTRY_TOKEN` is set in the environment. Run `cargo publish` (or `--workspace`).
  - Never print, log, commit, or write the token to a file.
- **Neither is available.** Stop with the tag pushed. Log a must-fix item to set up the release workflow and its trusted publisher on crates.io, which needs the human for the first manual publish and crates.io configuration.

**6. Verify.**
- `cargo info <crate>@<version>` resolves from crates.io.
- In a scratch directory outside the repo, a new project depending on `<crate> = "=<version>"` builds.
- Record the crates.io URL and the docs.rs URL in the iteration report. docs.rs builds asynchronously, so the next iteration's Phase 0 checks its result.

**If something goes wrong after publishing:**
- A published version that turns out to be broken is fixed forward with a new patch release in the next iteration.
- Flag it in the report so the human can decide whether to yank it. Never yank yourself.
- Never reuse a version number, and never try to republish a version that failed partway through. Investigate it and report.

**Final message.** Cover:
- bugs fixed, with their evidence;
- the enhancement (problem, change, evidence), or the reason nothing shipped;
- frame time and latency before and after;
- the released version and its crates.io link, or why there was no release;
- what was deferred;
- a recommended next focus.

Report failures plainly. An honest partial result is more useful to the next iteration than an overstated one.

Then stop.

## Documentation

Every iteration leaves a short written record, with visual proof, that a person can read without running anything.

- **Iteration report:** `docs/loop/iterations/<YYYY-MM-DD>-<area>-<short-sha>.md`, written for every iteration, including those that shipped nothing. Use the template below. Keep it concise. Show evidence rather than narrating it.
- **Curated visuals:** `docs/loop/iterations/assets/<YYYY-MM-DD>-<short-sha>/`, embedded with relative links.
  - **GIFs** for anything over time. Build them from the sequence frames at real timing (250 ms per frame at 4 fps) and hold the last frame for about 2 seconds. Use ffmpeg with `palettegen`/`paletteuse`, about 960 px wide, under about 3 MB each. Make one before and one after GIF of the same interaction.
  - **Stills** (compressed PNG) for layout, before and after.
  - **Contact sheets** where a reviewer needs every frame at once.
  - Commit only what the report embeds. Use Git LFS if the repo already does.
- **Feature docs:** when the enhancement changes how the editor is used, update its page under `docs/features/` with the new still or GIF. That page describes the editor as it is now, not the history of changes.
- **MCP reference:** `docs/mcp-tools.md` is generated from the registry. A stale copy fails a test.
- **Check before committing:** every image link resolves to a committed file, and every image shows what its caption claims.

## Guardrails

- Never modify or delete real user content. Test on scratch copies.
- Close an editor you launched by its pid; never `pkill` by name, because the human may have one open. Live checks use scratch copies of the project.
- No destructive git operations: no `reset --hard`, force push, history rewriting, or moving or deleting a pushed tag.
- Never yank, change crate ownership, or edit crates.io settings. Those are the human's decisions.
- Do not add dependencies without a logged reason.
- Use recorded or fixture VLM responses where the labeller supports them. Never loop live model calls.
- Protected checks (golden stills, golden state dumps, and anything under `tests/protected/`) are read-only for this loop. If one must change, stop and flag it for the human.

## State files

Keep state files short and focused on the next action. They are what the next fresh context reads first, and long, ever-growing logs degrade.

`docs/loop/backlog.json` is the working list. Edit it item by item and do not rewrite it wholesale. Move `fixed`, `rejected` and stale items into `docs/loop/backlog-archive.json` so the active list stays short.

```json
{
  "items": [
    {
      "id": "B-001",
      "kind": "bug",
      "area": "wfc",
      "title": "Contradiction on 3-way corner tiles freezes the panel",
      "must_fix": { "blocks": true, "common": false, "no_workaround": true, "evidence": "seq/wfc-corner f004-f011; seed 1337, rules a3f9" },
      "status": "open",
      "evidence": "tests/wfc_corner.rs::contradiction_recovers",
      "last_failure": "",
      "first_seen": "YYYY-MM-DD"
    }
  ]
}
```

`kind` is `bug`, `enhancement` or `series`. `status` is one of `open`, `fixed`, `blocked`, `unconfirmed`, `deferred`, `rejected`, `shipped`. `last_failure` holds the hypothesis and evidence when an iteration stopped on repeated failure.

A series carries four more fields, described under Phase 2's "Series":

```json
{ "id": "S-001", "kind": "series", "title": "...", "goal": "...", "approved_by": "owner YYYY-MM-DD",
  "acceptance": ["tools", "save"], "steps": [{ "id": "S1.1", "title": "...", "status": "open" }] }
```

A series step's `status` is `open`, `fixed` or `blocked`. `python3 tools/loop/backlog.py check` enforces all of this, and the loop gates on it.

`docs/loop/LOG.md` is an index with one line per iteration, newest at the bottom:

```markdown
- YYYY-MM-DD <short-sha> <area>: <one-line outcome> — v<version> | no release — [report](iterations/YYYY-MM-DD-<area>-<short-sha>.md)
```

Iteration report template:

```markdown
# YYYY-MM-DD <area>: <one-line outcome>

Focus: <area or "none"> · Commits: <sha>..<sha> · Outcome: shipped | fixes only | nothing shipped

## Summary
<Two or three sentences.>

## Baseline
Gate: <status>. Frame time <n> ms; latency p95 <n> ms (continuous) / <n> ms (discrete); edit-to-visible <n> ms (file) / <n> ms (MCP) on <scene>.

## Bugs fixed
### B-###: <title>
Must-fix: blocks <y/n>, common <y/n>, no workaround <y/n> — <evidence>
Fix: ... (root cause: ...) · Regression check: `path::test_name` (failed before, passes after)
| Before                                                | After                                               |
| ----------------------------------------------------- | --------------------------------------------------- |
| ![before](assets/<YYYY-MM-DD>-<sha>/B-###-before.gif) | ![after](assets/<YYYY-MM-DD>-<sha>/B-###-after.gif) |

## Enhancement: <title>
Plan: <change> · Files: <...> · Goal: <one sentence> · Proof: <check>
Candidates considered: <the 2–3, one line each>
MCP: <operations/tools added or changed, undo strategy, preview behaviour>

## Borrowed from Mixar
<the six-line behaviour note, or "none">

| Before                                              | After                                             |
| --------------------------------------------------- | ------------------------------------------------- |
| ![before](assets/<YYYY-MM-DD>-<sha>/enh-before.gif) | ![after](assets/<YYYY-MM-DD>-<sha>/enh-after.gif) |

## Performance
`perf.py` before → after: frame <n> → <n> ms; key p95 <n> → <n> ms; file <n> → <n> ms; MCP <n> → <n> ms; load <n>.

## Acceptance
`reached <k>/10 → <k>/10`; first failure `<id>`.

## Review
<Gaps reported (with frame or hunk), skeptic outcome, what was fixed.>

## Test diffs
<Every change to test code or assertions, with justification, or "none".>

## Release
Version: <old> → <new> (<patch | minor-breaking>) · semver-checks: <result> · Channel: <CI trusted publishing | local | not published, reason>
Tag: <tag> · crates.io: <url> · docs.rs: <url> (verified next iteration)
Package: <file count>, <size> · Changelog: <link to entry>

## Deferred / open
- ...

## Next focus
<recommendation and why>
```

## Before you stop: checklist

- [ ] Every claim in the report points to a command output, test, file, or frame.
- [ ] Every deterministic check passes, and no reviewer or VLM judgment overrode a failure.
- [ ] No check was weakened. Test diffs are listed and justified.
- [ ] At most one enhancement, proven by a check that failed before and passes after and by a sequence. Otherwise the report says nothing shipped and why.
- [ ] No step was retried more than twice. Repeated failures are logged in `last_failure`.
- [ ] Human-authored and locked content was not touched by agent operations.
- [ ] New authoring or tuning values are data, hot-reload without a restart, and persist when changed live. No content change needs a rebuild.
- [ ] `accept.py` and `perf.py` ran before and after, and their numbers are in the report.
- [ ] The independent review ran.
- [ ] `backlog.py check` passes.
- [ ] Capabilities a family crate provides came from that crate. Fixes to family crates were made, or logged, in their own canonical repos.
- [ ] The report, curated visuals, `mcp-tools.md`, `LOG.md`, and `backlog.json` are committed with the code.
- [ ] If published crate code changed:
  - [ ] the version bump matches the semver-checks result;
  - [ ] the changelog entry is written;
  - [ ] the package contents were reviewed and the dry run passed;
  - [ ] the tag is pushed;
  - [ ] the release is published and resolves from crates.io.
  
  Otherwise, the report says why there was no release.
- [ ] Nothing was yanked, republished, published with `--allow-dirty`, or tagged twice.