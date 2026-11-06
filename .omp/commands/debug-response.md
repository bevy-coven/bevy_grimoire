---
description: One debug-response pass on bevy_grimoire. Takes the next capture from docs/loop/captures/, triages it if unread, reproduces it where it was recorded (bugs) or specifies it (enhancements), fixes it behind a test that fails before and passes after, ships it, records the case in the history, then stops. Every five closed cases it reviews the history for patterns instead. Bugs before enhancements. Run headless in a Wiggum loop, one fresh context per pass.
argument-hint: "[optional capture stem, e.g. feedback-1793905912313]"
disable-model-invocation: true
---

# One debug-response pass

You are one pass of a **Wiggum loop**: a driver reruns this prompt in the background, one fresh context per pass, with **no memory between passes**. The repository is the only memory:

- the queue at `docs/loop/captures/ledger.json`;
- the history at `docs/loop/captures/cases.jsonl`;
- the captures themselves, beside those two files;
- the reports under `docs/loop/iterations/`, and pattern reviews under `docs/loop/patterns/`;
- `docs/loop/log.md`, and git history.

A pass that stops early and cleanly costs one cycle. A pass that leaves a half-diff costs several, because the next pass has to work out what it meant before it can work. Leave the tree clean, pushed and green.

**A pass does one of two things:**

- **Most passes answer exactly one capture.** Whatever the outcome (`fixed`, `unreproduced`, `blocked`, or parked unfinished), the pass ends there.
- **When a review is due (§2), the pass reviews the history instead,** following "Review pass" near the end of this file, and answers no capture.

**Captures are permanent.** Never move, rename or delete a capture's files. The history and its reviews depend on them.

## Hosts

|name|what it is|use it for|
|---|---|---|
|`big`|the Linux loop host; the clone you are in|builds, tests, pushes, Linux reproduction, any changelog clip|
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

|phase|in a capture pass|in a review pass|
|---|---|---|
|`orient`|the floor, `sync`, `next`, reading and triaging the capture|the floor, the due check, any backfill|
|`repro`|the reproduction matrix (bug) or reading current behaviour (enhancement)|—|
|`build`|both tests, then the fix|—|
|`prove`|targeted test runs and clippy|confirming candidate patterns|
|`gate`|the first push|—|
|`ship`|platform proof, closing the case, the report, the log line|writing the review and the dashboard|
|`log`|the second push and `end`|the push and `end`|

A pass that skips the marks has its whole time recorded under `orient`.

## 1. The floor

Before any capture is read, these must hold. Fixing them is the pass's first job.

1. **The tree is clean.** If `git status --short` is not empty, a previous pass died mid-work; passes never overlap. Do not finish or interpret its edits. Park them, log one line naming what was parked, and continue:

   ```sh
   git stash push -u -m "parked by pass <N>: <files, one line>"
   ```

2. **Nothing is unpushed.** If `git status -sb` shows `main` ahead of `origin/main`, a previous pass died between commit and push. Push it and let the gate judge. If the gate refuses, move those commits aside with `git branch parked/pass-<N>` and then `git reset --hard origin/main`, log it, and continue.
3. **The ledger and history are valid.** `python3 tools/loop/captures.py check` exits 0, and the history check in "The history" passes. If a ledger row is wrong, repair it with `captures.py set`. Never edit an existing line of `cases.jsonl`. If you cannot repair either, log why and stop.
4. **`main` builds and the gate runs.** If `main` does not build, fix exactly that first. If the push gate itself is broken, repair it under the rule in "The protected evaluator". Then continue. Neither counts as the pass's capture.

## 2. Review or capture?

**Check whether a review is due first.** Run this, then read the ledger:

```sh
python3 - <<'EOF'
import glob, os, re
p = "docs/loop/captures/cases.jsonl"
n = sum(1 for _ in open(p)) if os.path.exists(p) else 0
reviews = sorted(glob.glob("docs/loop/patterns/*-review.md"))
last = 0
if reviews:
    m = re.search(r"^cases_through:\s*(\d+)", open(reviews[-1]).read(), re.M)
    last = int(m.group(1)) if m else 0
print(f"cases={n} last_review_through={last} new={n - last}")
EOF
```

A review is due when **either** of these holds:

- `new` is 5 or more;
- some ledger row has a terminal status (`fixed`, `unreproduced`, `blocked`, `declined`) but no line in `cases.jsonl`. These rows were closed before the history existed, and the review backfills them.

If a review is due, go to "Review pass" and do not take a capture. Otherwise, take one:

```sh
python3 tools/loop/captures.py sync   # idempotent; copies new captures from mac_air
python3 tools/loop/captures.py next   # prints the stem this pass owns
```

- If `sync` fails because mac_air is unreachable, log it and continue with the captures already here.
- `next` exit **3**: the queue is empty. Log `nothing left to answer` and stop. The loop is finished: do not invent work and do not reopen a row.
- `next` exit **4** (dirty tree) or **5** (ledger fails `check`): go back to the floor.
- **A stem given as the argument** replaces `next`. Honour it if its status is `unread` or `triaged`, and write `named` in the log line. If it is `fixed`, `unreproduced`, `blocked` or `declined`, log `named <stem> is <status>; not reopened` and stop.

`next` orders work as: triaged bugs (oldest `taken_at_ms` first), then triaged enhancements (oldest first), then unread captures (oldest first). Finished statuses never return. Only a person moves a row back to `triaged`.

**Retry limit.** If `docs/loop/log.md` already records this stem as `parked` once, this is its last attempt. If this pass cannot finish it either, close it as `blocked` with what it needs (see "Closing a case") instead of parking it again.

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

**If no row shows the defect, that is an answer, not a failure.** Run every applicable row, then close the case as `unreproduced` (see "Closing a case"), with a note on what the attempts ruled out. Never fix a symptom nobody saw. If `check` refuses the `unreproduced` status (it requires enough distinct configurations), close it as `blocked` with the note `matrix exhausted; check refused unreproduced` instead. **Never invent a configuration, or repeat one under a different spelling, to make the count.**

## 5b. Specify (enhancements)

Enhancements get no matrix. Work out the current behaviour from the capture's picture and the code; the spec test in §6 takes the place of reproduction.

If the request is unclear, or looks wrong, do not decline it. Close it as `blocked` with the question in the note, and stop. Only a person closes a capture as `declined`.

## 6. Tests first

Write both tests **before touching product code**, run them, and keep the failing output for the report. A test that will not fail is not a proof:

- **A bug seen in §5a, but no test fails:** close it as `blocked` with the note `seen, not testable: <why>`. Do not commit the test.
- **An enhancement whose spec test already passes:** the behaviour already exists. Close it as `blocked` and record that observation in the note.

Every fix needs **both** of these:

- **At the level of the defect**, in the file that holds it. If that file already has a `#[cfg(test)] mod tests`, add new test functions to it; never edit or remove existing ones. Otherwise add the module. Use an integration test when the unit has no test home.
- **Across the application**, in a new file `crates/bevy_cauldron/tests/capture_<stem_with_underscores>.rs`. It rebuilds the capture's screen from its record and asserts the oracle through `app.update()`, with the real plugins, via `harness::build_headless`.
  - Add **one** `mod` line for it to `crates/bevy_cauldron/tests/all.rs`. The crate sets `autotests = false`, and both the push gate and `every_test_file_is_a_module_here` refuse a `tests/*.rs` file that is not a module there.
  - Write `use crate::fixtures;`, not `mod fixtures;`.
  - Follow the existing setup exactly: `crate::fixtures::Fixture`, `harness::build_headless`, `stand_on_the_project_page` for a page, `find_by_label`/`Activate` for a press. `tests/the_project_page.rs` and `tests/a_person_reports_to_the_developers.rs` show how.

### Choosing the assertion

A screenshot has no expected value, so take the oracle from **what the person said they saw**, not from the code. Relations drawn from what the program means catch more than relations drawn from how it is implemented (Segura et al., *A Survey on Metamorphic Testing*, IEEE TSE 2016). The oracle is either a relation between two runs or two frames, or an invariant on one run:

|the note says|oracle (slug)|the test asserts|
|---|---|---|
|something flashes, then is right|`first-frame`|node rects and text in the first drawn frame equal those after the UI settles|
|a margin is missing on one side|`window-invariance`|across two window sizes, every node rect in surface pixels scales linearly with `window.physical / window.logical`|
|the bottom is cut off|`containment`|every child rect lies inside its clip ancestor's rect, and a list taller than its box scrolls to its last row|
|text is hard to read|`contrast`|every text rect's foreground/background pair meets the skin's minimum ratio. If the skin defines none, use WCAG AA (4.5:1, or 3:1 for large text); the test names which|
|the screen should do something new (enhancement)|`spec`|the behaviour the note asks for, as observable state after `Activate` or `app.update()`|

If none of these fits, use `other:<slug>` and describe the oracle in the test. Put the stem and the oracle's slug in the test's doc comment, so no later pass has to work them out again.

## 7. Fix the cause

**One cause, one commit.** Nothing about a capture justifies weakening a check: never edit a test, golden image, fixture or threshold to make a fix look finished.

If the cause is in a family crate (`bevy_gooey`, `bevy_elements`, …), **do not patch, wrap or vendor it here.** Instead:

1. Search that repository's open issues first, for example `gh issue list -R <owner>/<crate> --label consumer:bevy_grimoire`.
2. If an issue already describes the cause, comment on it with the stem. Otherwise, file one labelled `bug` or `enhancement`, `consumer` and `consumer:bevy_grimoire`.
3. Close the capture as `blocked`, with cause `upstream` and `"upstream": "<crate>#<n>"`. Write the report and log line, push, and stop.

## 8. Prove it locally

1. The whole-application test failed on the base commit (output from §6) and passes on the fix:

   ```sh
   cargo nextest run -p bevy_cauldron --test editor -E 'test(capture_<stem_with_underscores>::)'
   ```

   Also run the crate's own tests around the changed unit.
2. `cargo clippy -p <each crate you touched> --all-targets -- -D warnings` is clean.

**Never run `cargo test --workspace` by hand.** The push is the gate.

## The history

`docs/loop/captures/cases.jsonl` is the permanent record of every closed capture, kept so a person (and the review pass) can find recurring causes, hot files, regressions and risky configurations. **It is append-only:** each closure adds one line, and no existing line is ever edited, reordered or removed. If a person requeues a capture and it closes again, that is a second line; the pair is how recurrences are seen. Create the file if it does not exist.

### Taxonomy

Every case is labelled from these lists. **When nothing fits, write `other:<slug>`** (lowercase letters, digits, hyphens). Never stretch a category to fit. Only a person edits these tables, and when they do they update `SLUGS` in the history check to match.

|symptom (what the person saw)|meaning|
|---|---|
|`flash`|drawn wrong for a moment, then corrects|
|`clipped`|content cut off by its container or the window|
|`layout`|misplaced, misaligned, or a margin/padding missing|
|`illegible`|text hard to read: contrast, size, weight|
|`wrong-state`|shows stale or incorrect data|
|`unresponsive`|input does nothing, or the wrong thing|
|`crash`|panic, hang or exit|
|`slow`|visible lag or stutter|
|`request`|enhancement: new behaviour asked for|

|cause (what the fix changed)|meaning|
|---|---|
|`scale-factor`|logical/physical pixels mixed, or scale factor not applied|
|`layout-rule`|a flex/size/margin constraint was wrong|
|`clip-scroll`|clip ancestor or scroll range was wrong|
|`skin-token`|a skin colour/size/spacing value was wrong|
|`init-order`|first-frame state or system ordering|
|`state-sync`|ECS state not propagated to the UI|
|`text-measure`|font metrics, wrapping or glyph layout|
|`input-routing`|focus, hit-testing or event dispatch|
|`upstream`|cause is in a family crate|
|`platform`|OS, driver or backend behaviour|
|`missing`|enhancement: the behaviour did not exist|

The oracles are the five slugs in §6.

**Renames and corrections.** A person who renames a category, or decides a case was mislabelled, appends a correction line rather than editing history: `{"v":1,"correct":"<stem>","field":"cause","value":"<slug>","by":"person","why":"…"}`. Readers apply corrections in file order. A pass never writes one.

### The case line

```json
{"v":1,"stem":"<stem>","closed_at":"<UTC ISO-8601>","pass":<N>,"kind":"bug|enhancement","outcome":"fixed|unreproduced|blocked","platform":{"os":"<from .log>","backend":"metal|vulkan|lavapipe","scale_factor":<f>,"window":"<WxH>","ui_scale":<f>,"app_version":"<from record>"},"screen":"<page or panel>","symptom":"<slug>","cause":"<slug>|null","oracle":"<slug>|null","note":"<one line, ≤140 chars>","files":["<product files the fix changed>"],"commit":"<sha>|null","upstream":"<crate>#<n>|null","regression_of":"<stem>|null","related":[],"attempts":<n>,"minutes":<n>,"backfilled":false}
```

|field|rule|
|---|---|
|`symptom`|always; taken from the note, not the code|
|`cause`|required for `fixed`, and `upstream` for an upstream block; otherwise `null`|
|`oracle`|required for `fixed`; otherwise `null`|
|`files`|product files changed by the fix commit; leave out `tests/`, `docs/` and `CHANGELOG.md`|
|`note`|the same text as the ledger's `reason`|
|`minutes`|minutes since `begin`|

## Closing a case

Every terminal outcome is written the same way. Parked captures are not closed: they stay `triaged` and get no case line.

1. **Set the ledger:**

   ```sh
   python3 tools/loop/captures.py set <stem> --status <fixed|unreproduced|blocked> [--commit <sha>] --reason "<note>"
   ```

   For `fixed`, `--commit` names the fix commit, which must already exist.
2. **Look for a regression** (for `fixed` only). Search the history for earlier fixes with the same cause in the same files:

   ```sh
   python3 - <<'EOF'
   import json
   cause, files = "<cause>", {"<file>", "<file>"}
   for line in open("docs/loop/captures/cases.jsonl"):
       c = json.loads(line)
       if c.get("outcome") == "fixed" and c.get("cause") == cause and files & set(c.get("files") or []):
           print(c["stem"], c["commit"], c["note"])
   EOF
   ```

   For each hit, read its fix commit. If the earlier fix was undone, incomplete or bypassed, set `regression_of` to that stem. If the overlap is coincidence, leave it `null` and say why in the report. Never decide without reading. Put stems that share the cause without being regressions in `related`.
3. **Append the case line** with Python, so the JSON is valid:

   ```sh
   python3 - <<'EOF'
   import json
   case = { ... }   # every field from "The case line"
   with open("docs/loop/captures/cases.jsonl", "a") as f:
       f.write(json.dumps(case, separators=(",", ":")) + "\n")
   EOF
   ```

4. **Run the history check, then `captures.py check`,** and commit both files together:

   ```sh
   git commit -m "case: <stem> <outcome>" -- docs/loop/captures/ledger.json docs/loop/captures/cases.jsonl
   ```

### History check

```sh
python3 - <<'EOF'
import json, re, subprocess
SLUGS = {  # keep in step with the taxonomy tables
    "symptom": {"flash","clipped","layout","illegible","wrong-state","unresponsive","crash","slow","request"},
    "cause": {"scale-factor","layout-rule","clip-scroll","skin-token","init-order","state-sync","text-measure","input-routing","upstream","platform","missing"},
    "oracle": {"first-frame","window-invariance","containment","contrast","spec"},
}
p = "docs/loop/captures/cases.jsonl"
try:
    lines = open(p).read().splitlines()
except FileNotFoundError:
    lines = []
r = subprocess.run(["git", "show", f"HEAD:{p}"], capture_output=True, text=True)
old = r.stdout.splitlines() if r.returncode == 0 else []
assert lines[:len(old)] == old, "an existing line of cases.jsonl was changed or removed"
for i, line in enumerate(lines, 1):
    c = json.loads(line)
    if "correct" in c:
        continue
    for field, allowed in SLUGS.items():
        v = c.get(field)
        if v is None and field != "symptom":
            continue
        assert v in allowed or re.fullmatch(r"other:[a-z0-9-]{2,32}", str(v)), f"line {i}: bad {field} {v!r}"
    if c["outcome"] == "fixed":
        assert c.get("commit") and c.get("cause") and c.get("oracle"), f"line {i}: fixed case missing commit/cause/oracle"
print(f"history ok: {len(lines)} lines, {len(lines) - len(old)} new")
EOF
```

## 9. Ship it

In this order:

1. **The fix commit:** product code, both tests, the `all.rs` line, and one `CHANGELOG.md` line written with `python3 tools/changelog/changelog.py add`. The commit body carries `Fixes <stem>`.
   - Default to `--no-clip "<why>; proved by <test>"`.
   - A clip is optional and has limits: at most 20 s, 2 MB and 960 px, stored under `docs/changelog/media/`. It can only be rendered on big. It **must be in the same push as the line that shows it**, because `changelog.py check` fails a clip with no line and a line with no clip.
   - Never put a capture under `docs/changelog/media/`.
2. **Close the case** as `fixed`, with the fix's sha (see "Closing a case"). This comes after the fix commit, because `check` refuses a `fixed` row whose commit is not an ancestor of `HEAD`.
3. **The first push.** This is the full gate (`tools/loop/hooks/pre-push`): clippy over every target, `cargo nextest run --workspace`, `cargo test --workspace --doc`, and the backlog, changelog and version checks. It usually takes 3–5 minutes, up to about 9 on a cold build; use a 15-minute timeout. If the gate is red, fix the cause and push again. **Never use `--no-verify`.**
4. **Proof on the capture's platform.** For a macOS capture, re-check on mac_air with a still of the fixed screen, but only if a build containing `<sha>` exists there. mac_air normally has only the released `/Applications/Cauldron.app`. When no such build exists, or mac_air is unreachable, the report says `mac_air proof: pending — <reason>` and the whole-application test stands as the proof.

## 10. Report, log, push

Every capture pass that took a capture does this, whatever the outcome:

1. **Write the report** at `docs/loop/iterations/<YYYY-MM-DD>-debug-<stem>-<short-sha>.md`, using the fix's sha, or `HEAD`'s if there is no fix. It covers:
   - the note and the kind;
   - the symptom, the cause and the regression decision, each with one line of why;
   - the platform, the app versions and every attempt;
   - which units `at` turned out to use;
   - the oracle and why it was chosen;
   - the failing run (with the rects or values it printed) and the passing run;
   - the fix and the gate result;
   - the still paths, or why the platform proof is pending.
2. **Append one line** to `docs/loop/log.md`, in the format of the existing lines. It holds the date, pass N, the stem, `named` if the stem was given, the kind, the symptom, the cause, the oracle, the short sha, and the outcome: `fixed`, `unreproduced`, `blocked: <why>`, or `parked`. Never edit another pass's line.
3. **Commit and push.** For a `fixed` capture this is the second push. A push that touches only `docs/` takes the hook's fast path and finishes in seconds.

## 11. Stop

```sh
python3 tools/loop/steps.py end
git status --short    # if `end` wrote tracked files, commit and push them
```

Check each item before you yield:

- the tree is clean, and `git status -sb` shows nothing ahead of `origin/main`;
- the ledger and `cases.jsonl` are committed, and both `captures.py check` and the history check pass;
- every capture you took is either closed (with a case line) or parked (still `triaged`), never left in between;
- no protected file was edited beyond its allowance (below);
- nothing was published, tagged or yanked.

## Budget: 60 minutes, gates included

|phase|minutes|done by|
|---|---|---|
|orient and triage|5|5|
|reproduce or specify|15|20|
|tests and fix|20|40|
|prove and first push|8|48|
|platform proof, close, report, second push|7|55|
|slack|5|60|

Each rule below means the same thing: park any product edits (`git stash push -u -m "parked by pass <N>: <stem>"`), write the report with the hypothesis and evidence, log the outcome as `parked` (or close the case as `blocked`, if the retry limit in §2 applies), commit and push the docs, and stop.

- **At 30 minutes with no failing test:** stop building.
- **At 45 minutes with the fix not yet proven:** do not push it.
- **Two failures at the same step:** stop there.

A capture that needs more than the matrix, or more than one budget, is closed as `blocked` with a note of what it needs. It does not get a longer pass.

A review pass has 20 minutes, including any backfill.

## Review pass

Do this instead of a capture when §2 says a review is due. **A review pass writes no product code, no tests, and no ledger rows.** Its only case lines are backfills. It proposes; a person decides.

### R1. Backfill

For each ledger row with a terminal status but no line in `cases.jsonl`, reconstruct a case line from that row's report in `docs/loop/iterations/`, its fix commit and its capture's `.log`:

- set `closed_at` to the fix commit's date, or the date of the row's last ledger commit;
- set `"backfilled": true` and `"pass"` to this pass's number;
- do not change the ledger.

Append the lines in order of `closed_at`, run the history check, and commit with the message `history: backfill <n> cases`.

### R2. Count

With a short Python script over `cases.jsonl`, apply the correction lines and compute, all-time and since the last review:

1. **Totals:** cases by outcome and by kind, the unreproduced rate, and the median `minutes`.
2. **Symptom × cause** counts.
3. **Hotspots:** files named in two or more `fixed` cases.
4. **Cause clusters:** causes with three or more cases.
5. **Regressions:** `regression_of` chains, and stems closed more than once.
6. **Configuration skew:** shares of cases by `scale_factor`, `ui_scale`, window and backend, set beside the same shares across **all** captures in the ledger (including open ones), so that "most bugs are at 2.0" can be told apart from "most captures are at 2.0".
7. **Upstream:** count by crate.
8. **Proposals:** every `other:` value, with its count.

**Small numbers.** With fewer than 10 cases in a group, report counts and stems, never percentages. A group of 2–3 is a **lead**; a group of 4 or more, confirmed in R3, is a **pattern**. Never present a lead as a trend.

### R3. Confirm against the fixes

A shared label is a lead, not a pattern. For each cluster, hotspot, regression, skew or upstream group, open the cases' reports and fix commits (`git show <sha>`). It is a pattern only if the fixes changed the same kind of thing for the same reason. If a fix shows a case was mislabelled, do not count it under the wrong label: list it under "Suggested corrections" for a person.

### R4. Propose

For each confirmed pattern, write:

1. **Hypothesis:** the single underlying cause, in one or two sentences.
2. **Evidence:** the stems, commits and files, with one line on what each fix changed.
3. **Structural fix:** the change that removes the class of defect rather than one more instance (for example, one conversion helper that every caller uses).
4. **Guard:** a test that would catch the whole class, written as a property over every screen or widget (for example, "containment holds for every scrollable node on every page fixture").
5. **Size:** small (one pass), medium (a few passes) or large (needs design).

Leads get one line each. For every `other:` value, write its count, its stems, and a suggestion: promote it, map it to an existing slug, or leave it as noise.

### R5. Write

1. **The review,** at `docs/loop/patterns/<YYYY-MM-DD>-review.md`, with this front matter:

   ```yaml
   ---
   cases_through: <number of lines in cases.jsonl now>
   since: <date of the previous review, or "start">
   ---
   ```

   Then these sections, in order: **Summary** (at most five lines), **Patterns**, **Leads**, **Changed since the last review** (new, persisting and resolved findings), **Suggested corrections**, **Taxonomy proposals**, and **Numbers** (the R2 output).
2. **The dashboard,** at `docs/loop/patterns/index.html`: one self-contained page with no network access. It embeds the full contents of `cases.jsonl` in `<script type="application/json" id="cases">` and computes everything in the browser. Its panels:
   - cases per week by outcome;
   - a symptom × cause grid;
   - a hotspot table;
   - configuration skew bars;
   - a list of regressions;
   - open `other:` proposals.

   The first review builds the page. Later reviews **only replace the contents of the `id="cases"` block**; change the page's code only if it fails to load.
3. **One line** in `docs/loop/log.md`: the date, pass N, `review`, `cases_through`, the number of patterns and leads, and the review's path.
4. **Commit and push.** The push touches only `docs/`, so it takes the fast path. Then stop.

## The protected evaluator

**A loop that can edit its own tests converges on passing the tests, not on being right**, and the difference is invisible from inside the loop.

**Read-only to this pass:**

- existing files and assertions under `crates/*/tests/**`, except for adding the one `mod` line to `all.rs`;
- existing test functions inside `#[cfg(test)]` modules (adding new ones is allowed);
- `tools/loop/*.py` and `tools/changelog/**`;
- every capture's files under `docs/loop/captures/` (record, frame, `.log`);
- every existing line of `docs/loop/captures/cases.jsonl` (new lines are appended only as "Closing a case" and R1 describe);
- this command file (`.omp/commands/debug-response.md`), including its taxonomy, and `.omp/wiggum/**`;
- earlier lines of `CHANGELOG.md` and of `docs/loop/log.md`, and existing files under `docs/loop/iterations/` and `docs/loop/patterns/` (except the data block in `index.html`).

**Repair-only: `tools/loop/hooks/**`.** When the gate is *broken*, meaning it errors or cannot run (as opposed to rejecting bad code), a pass may fix the hook in its own commit, titled `gate: repair <what>`. That diff must not remove, skip, loosen or short-circuit any check. If the only fix you can see would, stop: close the capture as `blocked` if one is in hand, or log it, and stop.

**Writable:**

- product code;
- the ledger's `status`, `kind`, `reason`, `commit` and `attempts` fields, through `captures.py`;
- new lines appended to `cases.jsonl`;
- new files under `docs/loop/iterations/` and `docs/loop/patterns/`;
- a new `capture_<stem>.rs` and its one `mod` line;
- new test functions.

A change outside these that looks necessary means closing the capture as `blocked`, with the question in the note and the report. The pass stops there; it does not reason its way to a weakening.