---
description: One debug-response pass on bevy_grimoire. Takes the next capture from docs/loop/captures/, triages it if unread, reproduces it where it was recorded (bugs) or specifies it (enhancements), fixes it behind a test that fails before and passes after, ships it, records it in the ledger, then stops. Bugs before enhancements. Run headless in a Wiggum loop, one fresh context per pass.
argument-hint: "[optional capture stem, e.g. feedback-1793905912313]"
disable-model-invocation: true
---

# One debug-response pass

You are one pass of a **Wiggum loop**: a driver reruns this prompt in the background, one fresh context per pass, with **no memory between passes**. The repository is the only memory: the ledger at `docs/loop/captures/ledger.json`, the captures beside it, the reports under `docs/loop/iterations/`, `docs/loop/log.md`, and git history.

A pass that stops early and cleanly costs one cycle. A pass that leaves a half-diff costs several, because the next pass has to work out what it meant before it can work. Leave the tree clean, pushed and green.

**One pass works exactly one capture.** Whatever the outcome (`fixed`, `unreproduced`, `blocked`, or parked unfinished), the pass ends there. The loop will run again.

## Hosts

|name|what it is|use it for|
|---|---|---|
|`big`|the Linux loop host; the clone you are in|builds, tests, both pushes, Linux reproduction, any changelog clip|
|`mac_air`|macOS, over `ssh`|reproducing macOS captures against the installed `/Applications/Cauldron.app`; PNG stills|
|`big_mac`|old macOS machine, nearly full|**never**|

mac_air has no bevy_grimoire checkout of its own, no `ffmpeg`, `git-lfs` or `gh`, and Python 3.9. Anything that needs those runs on big.

## Pass skeleton

```sh
date +%s                                   # note the start; the budget is measured from here
python3 tools/loop/steps.py begin          # first command; prints the pass number N
python3 tools/loop/steps.py mark <phase>   # as each phase starts
python3 tools/loop/steps.py end            # last command (see "Stop")
```

|phase|covers|
|---|---|
|`orient`|the floor, `sync`, `next`, reading and triaging the capture|
|`repro`|the reproduction matrix (bug) or reading current behaviour (enhancement)|
|`build`|both tests, then the fix|
|`prove`|targeted test runs and clippy|
|`gate`|the first push|
|`ship`|the platform proof, the report, the log line|
|`log`|the second push and `end`|

A pass that skips the marks has its whole time recorded under `orient`.

## 1. The floor

Before any capture is read, these must hold. Fixing them is the pass's first job.

1. **The tree is clean.** If `git status --short` is not empty, a previous pass died mid-work; passes never overlap. Do not finish or interpret its edits. Park them, log one line naming what was parked, and continue:

   ```sh
   git stash push -u -m "parked by pass <N>: <files, one line>"
   ```

2. **Nothing is unpushed.** If `git status -sb` shows `main` ahead of `origin/main`, a previous pass died between commit and push. Push it and let the gate judge. If the gate refuses, move those commits aside with `git branch parked/pass-<N>` and then `git reset --hard origin/main`, log it, and continue.
3. **The ledger is valid.** `python3 tools/loop/captures.py check` exits 0. If not, repair the offending row with `captures.py set`. If you cannot, log why and stop.
4. **`main` builds and the gate runs.** If `main` does not build, fix exactly that first. If the push gate itself is broken, repair it under the rule in "The protected evaluator". Then continue with the capture. Neither counts as the pass's capture.

## 2. Take the capture

```sh
python3 tools/loop/captures.py sync   # idempotent; copies new captures from mac_air
python3 tools/loop/captures.py next   # prints the stem this pass owns
```

- If `sync` fails because mac_air is unreachable, log it and continue with the captures already here.
- `next` exit **3**: the queue is empty. Log `nothing left to answer` and stop. The loop is finished: do not invent work and do not reopen a row.
- `next` exit **4** (dirty tree) or **5** (ledger fails `check`): go back to the floor.
- **A stem given as the argument** replaces `next`. Honour it if its status is `unread` or `triaged`, and write `named` in the log line. If it is `fixed`, `unreproduced`, `blocked` or `declined`, log `named <stem> is <status>; not reopened` and stop.

`next` orders work as: triaged bugs (oldest `taken_at_ms` first), then triaged enhancements (oldest first), then unread captures (oldest first). Finished statuses never return. Only a person moves a row back to `triaged`.

**Retry limit.** If `docs/loop/log.md` already records this stem as `parked` once, this is its last attempt. If this pass cannot finish it either, set it `blocked` with what it needs instead of parking it again.

## 3. Read it

```sh
python3 tools/loop/captures.py show <stem>
```

Read the note, the pick's path and the window it was taken at. Then look at the picture:

- Crop the frame to the pick's `rect_px` (surface pixels).
- **Copy the crop to a fresh path before reading it.** The image reader caches by path, so re-reading a path shows the old pixels.
- **Check the units of `at` before using it.** If it falls inside `rect_px ÷ scale_factor`, it is logical. If it falls only inside `rect_px`, it is in surface pixels: divide by `scale_factor` before clicking. At scale 1 the two coincide. Write which you found in the report.

`show` reads both record versions: version 2 has `picks`, version 1 has `subject`, and both are the same list to a reader. `read_reports` in `bevy_grimoire_core` refuses version 1 by name. **Do not change `FEEDBACK_VERSION` and do not re-pin that reader.**

**The platform comes from the capture's `.log`:** `SystemInfo { os: … }` and `AdapterInfo { … backend: … }`. The record JSON has no `os` field, and the ledger's `host` is where the capture was synced from, not where it was recorded.

## 4. Triage (unread captures only)

> A note that describes how the screen **is** is a `bug`. A note that describes how it **should become** is an `enhancement`.

- "the bottom gets cut off", "there is no margin on the right", "this is hard to read": bugs.
- "can we make this a download bar", "make this a download bar", "add a search box": enhancements, however they are phrased.
- A complaint that also suggests a fix is a bug, because the complaint stands without the suggestion. Put the suggestion in `reason` and do not file it as a separate capture.

```sh
python3 tools/loop/captures.py set <stem> --kind <bug|enhancement> --status triaged --reason "<why, one line>"
git commit -m "triage: <stem> is a <kind>" -- docs/loop/captures/ledger.json
```

Then carry on with this capture. Triage is not the whole pass.

## 5a. Reproduce (bugs)

Reproduce the bug where it was recorded:

|`os` in the log|host|backend|
|---|---|---|
|macOS|mac_air, against `/Applications/Cauldron.app`|`metal`|
|Linux|big|`vulkan`. Use `lavapipe` only if big has no native Vulkan: `VK_ICD_FILENAMES` → `/tmp/lvp/lvp_icd.json`, `LD_LIBRARY_PATH` → its `libvulkan_lvp.so` and `libLLVM`|
|unreadable|big, then mac_air|as above|

Note the installed app's version next to the capture's `app_version` in the report. A mismatch is worth stating; it is not a reason to stop.

**If the capture needs mac_air and mac_air is unreachable,** log `mac_air unreachable` and stop, leaving the row as it is. This is not `nothing left to answer`.

The matrix, tried in order, stopping at the first configuration that shows the defect:

1. the capture's own `window.physical` and `ui_scale`;
2. the same window at the host's scale factor with `ui_scale` 1.0;
3. the fitted default window (`window_frame::fit` of the primary monitor).

Skip a row that is identical to one already run, and say so in the report. Record every run, writing backend names in lowercase (`metal`, `vulkan`, `lavapipe`):

```sh
python3 tools/loop/captures.py attempt <stem> --host <big|mac_air> --backend <metal|vulkan|lavapipe> \
    --window <WxH> --ui-scale <F> --outcome <reproduced|not_reproduced> \
    --saw "<what was on the screen, in a person's words>"
```

**Drive the editor with what exists; write no second client.** Use `tools/demo/record.py` (`Injector`, `brp`, `widget_centre`, `McpClient`) and `tools/loop/common.py` (`binary`, `free_port`, `make_scratch`, `Editor`). Click at the pick's `at` in logical units (see §3), or at `widget_centre(port, "<the pick's text>")` when the pick names words. Never click a guessed pixel.

**If no row shows the defect, that is an answer, not a failure.** Run every applicable row, then:

```sh
python3 tools/loop/captures.py set <stem> --status unreproduced --reason "<what the attempts ruled out>"
```

Commit the ledger, write the report and the log line, push, and stop. Never fix a symptom nobody saw. If `check` refuses the `unreproduced` row (it requires enough distinct configurations), set `blocked` with the reason `matrix exhausted; check refused unreproduced` instead. **Never invent a configuration, or repeat one under a different spelling, to make the count.**

## 5b. Specify (enhancements)

Enhancements get no matrix. Work out the current behaviour from the capture's picture and the code; the spec test in §6 takes the place of reproduction.

If the request is unclear, or looks wrong, do not decline it. Set `blocked` with the question in `reason` and stop. Only a person sets `declined`.

## 6. Tests first

Write both tests **before touching product code**, run them, and keep the failing output for the report. A test that will not fail is not a proof:

- **A bug seen in §5a, but no test fails:** set `blocked`, reason `seen, not testable: <why>`. Do not commit the test.
- **An enhancement whose spec test already passes:** the behaviour already exists. Set `blocked` and record that observation.

Every fix needs **both** of these:

- **At the level of the defect**, in the file that holds it. If that file already has a `#[cfg(test)] mod tests`, add new test functions to it; never edit or remove existing ones. Otherwise add the module. Use an integration test when the unit has no test home.
- **Across the application**, in a new file `crates/bevy_cauldron/tests/capture_<stem_with_underscores>.rs`. It rebuilds the capture's screen from its record and asserts the oracle through `app.update()`, with the real plugins, via `harness::build_headless`.
  - Add **one** `mod` line for it to `crates/bevy_cauldron/tests/all.rs`. The crate sets `autotests = false`, and both the push gate and `every_test_file_is_a_module_here` refuse a `tests/*.rs` file that is not a module there.
  - Write `use crate::fixtures;`, not `mod fixtures;`.
  - Follow the existing setup exactly: `crate::fixtures::Fixture`, `harness::build_headless`, `stand_on_the_project_page` for a page, `find_by_label`/`Activate` for a press. `tests/the_project_page.rs` and `tests/a_person_reports_to_the_developers.rs` show how.

### Choosing the assertion

A screenshot has no expected value, so take the oracle from **what the person said they saw**, not from the code. Relations drawn from what the program means catch more than relations drawn from how it is implemented (Segura et al., *A Survey on Metamorphic Testing*, IEEE TSE 2016). The oracle is either a relation between two runs or two frames, or an invariant on one run:

|the note says|oracle|the test asserts|
|---|---|---|
|something flashes, then is right|first frame equals settled frame|node rects and text in the first drawn frame equal those after the UI settles|
|a margin is missing on one side|invariance under window size|across two window sizes, every node rect in surface pixels scales linearly with `window.physical / window.logical`|
|the bottom is cut off|containment|every child rect lies inside its clip ancestor's rect, and a list taller than its box scrolls to its last row|
|text is hard to read|contrast bound|every text rect's foreground/background pair meets the skin's minimum ratio. If the skin defines none, use WCAG AA (4.5:1, or 3:1 for large text); the test names which|
|the screen should do something new (enhancement)|spec|the behaviour the note asks for, as observable state after `Activate` or `app.update()`|

Put the stem and the chosen oracle in the test's doc comment, so no later pass has to work them out again.

## 7. Fix the cause

**One cause, one commit.** Nothing about a capture justifies weakening a check: never edit a test, golden image, fixture or threshold to make a fix look finished.

If the cause is in a family crate (`bevy_gooey`, `bevy_elements`, …), **do not patch, wrap or vendor it here.** Instead:

1. Search that repository's open issues first, for example `gh issue list -R <owner>/<crate> --label consumer:bevy_grimoire`.
2. If an issue already describes the cause, comment on it with the stem. Otherwise, file one labelled `bug` or `enhancement`, `consumer` and `consumer:bevy_grimoire`.
3. Set the capture `blocked` with the issue number in `reason`, commit the ledger, write the report and log line, push, and stop.

## 8. Prove it locally

1. The whole-application test failed on the base commit (output from §6) and passes on the fix:

   ```sh
   cargo nextest run -p bevy_cauldron --test editor -E 'test(capture_<stem_with_underscores>::)'
   ```

   Also run the crate's own tests around the changed unit.
2. `cargo clippy -p <each crate you touched> --all-targets -- -D warnings` is clean.

**Never run `cargo test --workspace` by hand.** The push is the gate.

## 9. Ship it

In this order:

1. **The fix commit:** product code, both tests, the `all.rs` line, and one `CHANGELOG.md` line written with `python3 tools/changelog/changelog.py add`. The commit body carries `Fixes <stem>`.
   - Default to `--no-clip "<why>; proved by <test>"`.
   - A clip is optional and has limits: at most 20 s, 2 MB and 960 px, stored under `docs/changelog/media/`. It can only be rendered on big. It **must be in the same push as the line that shows it**, because `changelog.py check` fails a clip with no line and a line with no clip.
   - Never put a capture under `docs/changelog/media/`.
2. **The ledger,** which comes after the fix commit because `check` refuses a `fixed` row whose commit is not an ancestor of `HEAD`:

   ```sh
   python3 tools/loop/captures.py set <stem> --status fixed --commit <sha>
   python3 tools/loop/captures.py check
   git commit -m "ledger: <stem> fixed in <short-sha>" -- docs/loop/captures/ledger.json
   ```

3. **The first push.** This is the full gate (`tools/loop/hooks/pre-push`): clippy over every target, `cargo nextest run --workspace`, `cargo test --workspace --doc`, and the backlog, changelog and version checks. It usually takes 3–5 minutes, up to about 9 on a cold build; use a 15-minute timeout. If the gate is red, fix the cause and push again. **Never use `--no-verify`.**
4. **Proof on the capture's platform.** For a macOS capture, re-check on mac_air with a still of the fixed screen, but only if a build containing `<sha>` exists there. mac_air normally has only the released `/Applications/Cauldron.app`. When no such build exists, or mac_air is unreachable, the report says `mac_air proof: pending — <reason>` and the whole-application test stands as the proof.

## 10. Report, log, second push

Every pass that took a capture does this, whatever the outcome:

1. **Write the report** at `docs/loop/iterations/<YYYY-MM-DD>-debug-<stem>-<short-sha>.md`, using the fix's sha, or `HEAD`'s if there is no fix. It covers:
   - the note and the kind;
   - the platform, the app versions and every attempt;
   - which units `at` turned out to use;
   - the oracle and why it was chosen;
   - the failing run (with the rects or values it printed) and the passing run;
   - the fix and the gate result;
   - the still paths, or why the platform proof is pending.
2. **Append one line** to `docs/loop/log.md`, in the format of the existing lines. It holds the date, pass N, the stem, `named` if the stem was given, the kind, the oracle, the short sha, and the outcome: `fixed`, `unreproduced`, `blocked: <why>`, or `parked`. Never edit another pass's line.
3. **Commit both and push.** A push that touches only `docs/` takes the hook's fast path and finishes in seconds.

## 11. Stop

```sh
python3 tools/loop/steps.py end
git status --short    # if `end` wrote tracked files, commit and push them
```

Check each item before you yield:

- the tree is clean, and `git status -sb` shows nothing ahead of `origin/main`;
- the ledger is committed and `captures.py check` exits 0;
- no protected file was edited beyond its allowance (below);
- nothing was published, tagged or yanked.

## Budget: 60 minutes, gates included

|phase|minutes|done by|
|---|---|---|
|orient and triage|5|5|
|reproduce or specify|15|20|
|tests and fix|20|40|
|prove and first push|8|48|
|platform proof, report, second push|7|55|
|slack|5|60|

Each rule below means the same thing: park any product edits (`git stash push -u -m "parked by pass <N>: <stem>"`), write the report with the hypothesis and evidence, log the outcome as `parked` (or `blocked`, if the retry limit in §2 applies), commit and push the ledger and docs, and stop.

- **At 30 minutes with no failing test:** stop building.
- **At 45 minutes with the fix not yet proven:** do not push it.
- **Two failures at the same step:** stop there.

A capture that needs more than the matrix, or more than one budget, becomes `blocked` with a note of what it needs. It does not get a longer pass.

## The protected evaluator

**A loop that can edit its own tests converges on passing the tests, not on being right**, and the difference is invisible from inside the loop.

**Read-only to this pass:**

- existing files and assertions under `crates/*/tests/**`, except for adding the one `mod` line to `all.rs`;
- existing test functions inside `#[cfg(test)]` modules (adding new ones is allowed);
- `tools/loop/*.py` and `tools/changelog/**`;
- this command file (`.omp/commands/debug-response.md`) and `.omp/wiggum/**`;
- earlier lines of `CHANGELOG.md` and of `docs/loop/log.md`, and existing files under `docs/loop/iterations/`.

**Repair-only: `tools/loop/hooks/**`.** When the gate is *broken*, meaning it errors or cannot run (as opposed to rejecting bad code), a pass may fix the hook in its own commit, titled `gate: repair <what>`. That diff must not remove, skip, loosen or short-circuit any check. If the only fix you can see would, stop: set the capture `blocked` if one is in hand, or log it, and stop.

**Writable:**

- product code;
- the ledger's `status`, `kind`, `reason`, `commit` and `attempts` fields, through `captures.py`;
- new files under `docs/loop/iterations/`;
- a new `capture_<stem>.rs` and its one `mod` line;
- new test functions.

A change outside these that looks necessary means `blocked`, with the question in the report. The pass stops there; it does not reason its way to a weakening.