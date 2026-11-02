# 2026-11-02 demo: check.py scores a click again, and a storyboard can say when a fade has landed

Focus: bugs first (the `check.py` gate that blocked widget demos), then the owed clips · Commits: 05c25ec..89cef33 · Machine: big · Outcome: shipped

## Summary
Last pass I filed #270 believing `tools/demo/check.py` was protected. **It is not** — the protected list
names `tools/loop/accept.py`, `perf.py`, `common.py`, `backlog.py`, `test_accept.py` and `tools/loop/hooks/**`,
and nothing under `tools/demo/`. So the bug was fixable, and the fix unblocked a clip that had failed six
times. A second `check.py` gap (a faded change lands later than its operation's timestamp) was fixed
additively with a `settle` step.

## Baseline
Gate green at 05c25ec. `accept.py` not run: nothing it exercises changed. Load 4.7, free disk 176 GB, RAM 15 GB.

## Bugs fixed
**#270 — closed (e4ddb1f).** `check.py`'s drag rule tested `0 <= t - e["t"] <= 0.1`, which **any** cursor
sharing a click's timestamp satisfies. `record.py`'s click step moves the cursor then presses, so its
cursor is stamped a few microseconds **before** the click (measured: cursor 0.0100667619, click
0.0100711119, delta **-4.3e-06**) — no click could ever be a change moment. Now `1e-6 < t - e["t"] <= 0.1`,
strictly after the press, which is the real distinction: a `drag` stamps the click first and its moves
follow. Proved on a real recording (`check_view.json`): `change_moments → [('place', 1.46)]`, **6.55 %**
changed, and the clip ships as `2026-11-02-111-check-view`.

**A second gap, fixed additively (48b27d4).** A change the editor **fades** has its picture still old at the
operation's timestamp: measured, the sky's first changed frame is 4.13 s while the operation applied at
3.81 s, so `check.py` called a 96 % change 0.05 %. `record.py` now accepts a `settle` step naming the moment
the change arrived, and `check.py` counts it — dropping an agent moment that a settle answers, while an
agent moment with no settle after it is measured as before.

## Enhancement: #111's clip
`docs/changelog/media/2026-11-02-111-check-view.mp4` (5.6 s, 41 KB): the journal's Check view runs the
project's checks and the answer lands there — with no model served, the endpoint that refused and which
checks could not be asked. `check.py` PASS (scene 58.8 %, change 6.55 %, static 1.50 s). Six takes of this
clip were discarded last pass for #270 alone.

## Clips attempted and dropped, with the blocker
- **`96-family-set`**: the `family_set` sky change works (night → dusk → dawn, unmistakable), and the
  `settle` fix makes `check.py` pass it — but a noon→20:00 jump leaves the viewport **white for 12 frames
  (1.5 s)**. Filed as **#271** with the per-frame measurements. A feature's own demo must not be a white
  screen, so the storyboard is not shipped. Hours inside the daylight range (15.0, 17.0) barely change the
  sky: 0.02 %.
- **`115-run-ops`**: `run_add` applies correctly ("ran 9 piece(s) along the line", one UNDO row) but a row of
  thin posts changes 2–5 % of a bedroom-sized frame; three framings each traded scene-fill against
  change-percentage.

## Skipped, and why
#210 + 20 `ui` issues (protected `chrome::NAME` spellings); #235 (deletes pinned assertions); #201
(protected `assert_ne!`); #152 (protected fixture); #193 (bevy-elements#55); #112's calibration and #113
(need the model); #195 (memory item implemented); #197 `wontfix`. The parked pass-218 branch is stale.

## Review
`reviewer` not spawned; self-review: (1) *does the fix do what it says* — the measured delta and the real
clip's moment are both quoted; (2) *anything else changed* — two demo tools and a clip; (3) *protected
files* — none, and I corrected last pass's error about `check.py` being one; (4) *could it mislead* — the
settle could hide a no-op change, so a test asserts a settle with no change still **fails**; (5) *what did
I get wrong* — last pass I read the protected list too broadly and left a fixable bug filed instead of
fixed.

## Test diffs
`tools/demo/test_check.py`: the drag test's synthetic shape (click and cursor at one identical `t`) was one
**no storyboard produces**; it now uses the measured shape and additionally asserts *which* click is the
placement. New: `test_a_widget_click_is_a_change_moment`, `test_a_settle_is_a_change_moment`,
`test_a_moment_with_no_change_still_fails`, `test_a_settle_stands_in_for_the_operation_it_answers`. 9 tests,
all passing.

## Release
None: `bevy_cauldron` is `publish = false`; nothing packaged changed.

## Next focus
#271's exposure transition, or the next owed clip whose change is neither faded nor thin.
