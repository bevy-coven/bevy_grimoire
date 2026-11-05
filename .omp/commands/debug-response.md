---
description: One debug-response pass on bevy_grimoire. Takes the next unhandled debug capture from docs/loop/captures/, reproduces it on the platform it was recorded on, fixes it with a test that fails before and passes after, ships it, records it in the ledger, then stops. Bugs before enhancements. Designed to be run headless in a Wiggum loop, one fresh context per pass.
argument-hint: "[optional capture stem, e.g. feedback-1793905912313]"
disable-model-invocation: true
---

# One debug-response pass

You are one pass of a **Wiggum loop**: a dumb outer shell reruns this same prompt forever, in the background, with **no memory carried between passes**. The repository is the only memory: the queue in `docs/loop/captures/ledger.json`, the captures themselves, the reports under `docs/loop/iterations/`, `docs/loop/log.md` and git history.

So a pass that stops early and cleanly costs one cycle, and a pass that leaves a half-diff costs several: the next pass must work out what you meant before it can work on a capture. Leave the tree clean, pushed and green.

## What a pass does, in one line

`captures.py next` → read the capture → reproduce it on its own platform → two tests → fix → prove → ship → record → stop.

## Begin and end the pass

```sh
python3 tools/loop/steps.py begin          # first command of the pass
python3 tools/loop/steps.py mark <phase>   # as each phase starts
python3 tools/loop/steps.py end            # last command of the pass
```

The phases are `orient`, `repro`, `build`, `prove`, `gate`, `ship`, `log`. `begin` numbers the pass so its time is recorded and it works whether a driver, a person or Oh My Pie launched you; `end` closes it. The same contract `build-loop.md` states, for the same reason: a driven pass is timed, and a pass that skips the marks has every step land in `orient`.

## The floor, before the capture

Two things must be true before a capture is read, and a red floor is **the pass's first job**, ahead of any capture:

1. `git status --short` is empty. If it is not, finish it or park it first — the driver already parks a dirty tree, so a pass that finds one has found a real problem, not a routine one. Do not start a capture on top of somebody's half-done edit.
2. `python3 tools/loop/captures.py check` exits 0. The ledger is the queue; a pass must not choose work from a ledger it cannot validate, and `next` refuses (exit 5) rather than guessing.

A broken push gate or a `main` that does not build also outranks the capture: fix exactly that, then take the capture in the same pass, because no fix can ship without it.

## Take the capture

```sh
python3 tools/loop/captures.py sync          # idempotent; reads mac_air
python3 tools/loop/captures.py next          # the capture this pass owns
```

`sync` copies any capture that is not here yet from the machine it was filed on and adds its ledger row; running it twice changes nothing. **`next` exiting 3 means the queue is empty: log `nothing left to answer` and stop.** The loop is finished, not blocked — do not invent work, and do not reopen a finished row.

`next` exits 4 when the tree is dirty (naming the files) and 5 when the ledger fails `check`. Both are the floor above, not a reason to skip the pass.

A stem given as the argument (`/debug-response feedback-1793905912313`) replaces `next`'s choice. Honour it, and say in the log line that a stem was named.

The order `next` uses is the order captures are worked: a `triaged` **bug** (oldest `taken_at_ms` first), then a `triaged` **enhancement** (oldest first), then an `unread` capture (oldest first, so it is classified before more work is chosen off it). `fixed`, `unreproduced`, `blocked` and `declined` never return; only a person moving a row back to `triaged` requeues it.

## Read it, then classify it if it is unread

```sh
python3 tools/loop/captures.py show <stem>
```

Read the note, the pick's path and the window it was taken at. Then **look at the picture**: crop the frame to the pick's `rect_px`, convert surface pixels to logical with the record's `scale_factor` (the `at` line `show` prints is already logical), and **copy it to a fresh path before reading it** — the read tool caches an image by path, so a re-read of the same path shows the old pixels and you will conclude nothing changed.

`show` reads either record version: version 2 has `picks`, version 1 has `subject`, and both are the same list to a reader. `read_reports` in `bevy_grimoire_core` refuses a version-1 file by name because this build reads 2 — **do not change `FEEDBACK_VERSION` and do not re-pin that reader.** `captures.py` reads the JSON itself, which is the whole point.

### The classification rule

Decided at the first pass that reads a capture and written into the ledger, so the order is a property of the file rather than of each pass's mood:

> **A note that says something *is* wrong is a `bug`. A note that says something *should be* is an `enhancement`.** The test is the mood of the main verb (the indicative or the imperative, "there is" / "there is no" / "this is", versus the optative, "can we" / "could we"), not the presence of a request. A bug whose note also suggests a fix is still a bug, because the complaint stands without the suggestion.

Write it, and commit that alone:

```sh
python3 tools/loop/captures.py set <stem> --kind <bug|enhancement> --status triaged --reason "<why, in one line>"
git commit -m "triage: <stem> is a <kind>"
```

**A bug whose suggested fix is a separate feature does not file that feature in the same pass.** The suggestion goes in `reason` and the pass moves on: one capture, one pass.

The five captures already synced were triaged this way, and the ledger records it:

|stem|kind|why|
|---|---|---|
|`…499397`|`bug`|"there is a garbled flash" is a defect.|
|`…311065`|`enhancement`|"Can we make this a download bar" only proposes new behaviour.|
|`…698793`|`bug`|"hard to read", "the prose are bad" are defects; the decision-model sentence is the suggested *fix*, recorded in `reason`, not the kind.|
|`…798698`|`bug`|"no margin on the right side" is a defect.|
|`…912313`|`bug`|"the bottom gets cut off and is not visible" is a defect.|

## Reproduce it, on its own platform

**A capture is reproduced where it was recorded**, read from the capture's own `.log`, which carries `SystemInfo { os: …, cpu: … }` and `AdapterInfo { … backend: … }`:

|`os` in the log|reproduce on|backend|
|---|---|---|
|`macOS`|`mac_air` over `ssh` (the owner's preference for macOS work — never `big_mac`, which is old and nearly full)|native Metal|
|`Linux`|the loop's host|native Vulkan, else Mesa lavapipe (CPU Vulkan, so no `gpu-tenant` lease)|
|unreadable|the loop's host, then `mac_air`|as above|

The matrix, **tried in this order, stopping at the first configuration that shows the defect**, and recording every row attempted because `check` counts them:

1. the capture's own `window.physical` and `ui_scale`, native backend;
2. that window at the host's `scale_factor` with `ui_scale` 1.0;
3. the fitted default window (`window_frame::fit` of the primary monitor);
4. on Linux only: rows 1–3 again under lavapipe, `VK_ICD_FILENAMES` pointing at a staged `lvp_icd.json` and `LD_LIBRARY_PATH` at its `libvulkan_lvp.so` and `libLLVM` (big's staging is `/tmp/lvp/`).

Each configuration that does not show it is one row:

```sh
python3 tools/loop/captures.py attempt <stem> --host <H> --backend <B> --window <WxH> --ui-scale <F> \
    --outcome not_reproduced --saw "<what was on the screen, in a person's words>"
```

and the one that does is `--outcome reproduced --saw "<what the defect looked like>"`.

**Driving the editor reuses what exists and writes no second client:** `tools/demo/record.py`'s `Injector`, `brp`, `widget_centre` and `McpClient`, and `tools/loop/common.py`'s `binary`, `free_port`, `make_scratch` and `Editor`. The click goes to the pick's `at` (surface pixels ÷ `scale_factor`), or to `widget_centre(port, <the pick's text>)` when the pick names words — never to a guessed pixel.

**If no configuration shows the defect, that is an answer, not a failure.** Complete every row, then:

```sh
python3 tools/loop/captures.py set <stem> --status unreproduced --reason "<what the attempts ruled out>"
```

commit the ledger, write the pass's log line, and **stop**. Never write a fix for a symptom nobody saw: an `unreproduced` row needs at least three distinct `(host, backend, window)` attempts and a reason, and `check` refuses the row otherwise. That gate exists so giving up early cannot pass as an answer.

## Write the failing test first

The fix comes after a test that fails *because of the defect*, and the failing output is pasted into the report. **A test that will not fail is not a proof**: if you cannot make one fail on the base commit, the capture goes `unreproduced` (with its attempts) and the test is not committed.

### At the level, and across the application

Every fix needs **both**:

- **At the level** — a `#[cfg(test)]` module in the file that holds the defect, or an integration test when the unit has no test home.
- **Across the application** — a new file `crates/bevy_cauldron/tests/capture_<stem_with_underscores>.rs` that rebuilds the capture's screen from its record and asserts the invariant through `app.update()` with the real plugins and `harness::build_headless`.

That new file needs **one line** in the module list of `crates/bevy_cauldron/tests/all.rs` (`autotests = false`; both the push gate and `every_test_file_is_a_module_here` refuse a `tests/*.rs` that is not a module there), and writes `use crate::fixtures;`, not `mod fixtures;`. Setup follows the existing convention exactly — `crate::fixtures::Fixture`, `harness::build_headless`, `stand_on_the_project_page` for a page, `find_by_label`/`Activate` for a press — as `tests/the_project_page.rs` and `tests/a_person_reports_to_the_developers.rs` already do.

### Finding the assertion when the note is a complaint

A picture has no expected output, so the oracle is a **relation between two runs** rather than a value: metamorphic testing (Segura, Fraser, Sánchez & Ruiz-Cortés, *A Survey on Metamorphic Testing*, IEEE TSE 2016, `10.1109/TSE.2016.2532875`). The survey's finding that governs which relation to pick is that relations "strongly inspired by the semantics of the program under test" are the effective ones, while relations close to the implementation strategy are not — so the relation is chosen from **what the person said they saw**, not from the code:

|the note says|relation|property|first capture it applies to|
|---|---|---|---|
|something flashes, then is right|**idempotence**|the frame the screen first draws equals the frame it settles to|`…499397`|
|a margin is missing on one side|**invariance under window size**|every node's rect in surface px scales linearly with `window.physical / window.logical`|`…798698`|
|the bottom is cut off|**containment**|a child's rect lies inside its clip ancestor's rect, and a list taller than its box scrolls to its last row|`…912313`|
|text is hard to read|**contrast bound**|every text rect's foreground/background ink pair meets the skin's own minimum ratio|`…698793`|
|a control should become a progress bar|*none — it is an enhancement*|—|`…311065`|

Put the chosen relation in the test's doc comment with the stem, so it is never re-derived by a later pass.

## Fix the cause

**One cause, one commit.** If the cause is in a family crate (`bevy_gooey`, `bevy_elements`, …) then the fix goes there, not here: file the issue on that repository labelled `enhancement` (or `bug`), `consumer` and `consumer:bevy_grimoire`, set the capture `blocked` with the issue number in `reason`, and **never patch, wrap or vendor it here** — then take the next capture in the same pass.

Nothing about a capture is a reason to weaken a check: no test, golden image, fixture or threshold is edited to make a fix look finished.

## Prove it

Three things, all observed in this pass:

1. the whole-application test fails on the base commit and passes on the fix — the failing run's output goes in the report, with the node rects or values it printed;
2. the behaviour is exercised on the platform the capture came from, with a still or a state dump whose path goes in the report (a capture recorded on `mac_air` is re-checked there; a launch that no longer shows the defect is proof that outlives the test);
3. `cargo clippy -p bevy_cauldron --all-targets -- -D warnings` is clean.

**Never run `cargo test --workspace` by hand** — the push is the gate. Run only what you touched: `cargo nextest run -p bevy_cauldron --test editor -E 'test(capture_<stem>::)'`, and the crate's own tests around the changed unit.

## Ship it

- The commit body carries `Fixes <stem>` (the capture's stem, not an issue number), so `git log` says which filed report it answers.
- One `CHANGELOG.md` line, written with `python3 tools/changelog/changelog.py add`: a clip for a change a person can see (at most 20 s, 2 MB, 960 px, under `docs/changelog/media/`), and `--no-clip "<reason>; proved by <test>"` for one they cannot. Never place a capture under `docs/changelog/media/`.
- The ledger row is written **after** the commit exists, because `check` refuses a `fixed` row whose commit is not an ancestor of `HEAD`:

  ```sh
  python3 tools/loop/captures.py set <stem> --status fixed --commit <sha>
  python3 tools/loop/captures.py check
  ```

- **Push once.** The push is the full gate (`tools/loop/hooks/pre-push`: clippy over every target, `cargo nextest run --workspace`, `cargo test --workspace --doc`, the backlog, changelog and version checks). Give it a generous timeout. A red gate is fixed and pushed again; it is never skipped with `--no-verify`.

## Report and stop

Write `docs/loop/iterations/<date>-debug-<stem>-<short-sha>.md`: the report, the platform and the attempts, the relation chosen and why, the failing run and the passing run, the fix, and the paths of the stills.

Append **one line** to `docs/loop/log.md` naming the capture, its classification, the relation, the commit and the outcome (`fixed`, `unreproduced`, `blocked on <crate>#N`, `declined`). Append; never rewrite another pass's line.

Then the stop checklist, each item checked before you yield:

- the tree is clean **and pushed**;
- the ledger is updated and committed, and `check` exits 0;
- no protected file was edited (below);
- nothing was published, tagged or yanked;
- the pass is inside its budget.

## Budget

A pass is 45 minutes, and running long does not buy progress: the driver starts the next pass regardless, and an unpushed half-diff is the most common way a headless pass goes wrong.

|Phase|Minutes|
|---|---|
|orient and triage|5|
|reproduce (the matrix)|15|
|test and fix|15|
|ship (changelog, ledger, push)|10|
|stop at|45|

Early stops, in this order: **at 20 minutes** commit verified fixes only and log the rest; **at 40 minutes** finish the commit and push; **at 45** stop. **Two failures at the same step means stop** — log the hypothesis and the evidence, exactly as `build-loop.md` says. A capture that needs more than the matrix is a `blocked` row with what it needs, not a longer pass.

## The protected evaluator

**A loop that can edit its own tests converges on passing the tests, not on being right**, and the difference is invisible from inside the loop. Read-only to this pass: `crates/*/tests/**` (an existing assertion), `#[cfg(test)]` blocks that already exist, `tools/loop/*.py`, `tools/loop/hooks/**`, `tools/changelog/**`, `CHANGELOG.md`'s earlier lines, `docs/loop/log.md`'s earlier lines, `docs/loop/iterations/**` that already exist, and **this command file**.

**You may write** product code, the ledger's `status`/`kind`/`reason`/`commit`/`attempts` fields, new files under `docs/loop/iterations/`, and **new** test files (a new `capture_<stem>.rs` and its one `mod` line).

A protected change that looks necessary means `--status blocked` with the reason in the row and the question on the report: the pass stops there, it does not reason its way to a weakening.
