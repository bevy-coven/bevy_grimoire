# 2026-11-07 bugs: the family settings reload test stops sleeping, so the gate stops failing on a loaded machine (#279)

Focus: open bug issues, oldest first · Commits: 92ef7c38 · Machine: big · Outcome: shipped

## Summary
#279's product half landed on pass 372; its test half has been sitting on branch `coven/279-stepped-clock` since 2026-11-06 because the push gate kept refusing the branch on two tests the change does not touch (#322). The owner approved the test edit in conversation on 2026-11-06 and nobody could get it past the gate. This pass brought `2f89a1d` onto `main` by cherry-pick, re-proved it, and fixed its changelog line. It also merged the clone's five duplicated pass commits and one real local-only media commit that a killed pass had left unpushed.

## Baseline
Gate: green on the commit before this one (`0d72b378`, the merge's parent). `accept.py`: not run: no `bevy_grimoire` product code changed. Load ~6, free disk 96 GB, available RAM 20 GB.

## Bugs fixed

### B-279: `family_settings_reload` sleeps 50 ms per poll, so the gate's wall-clock bound fails on a loaded machine
Must-fix: blocks **y** (it refuses `main`'s own gate), common **y** (every push on a busy machine), no workaround **n** (a pass can `git push` again and hope).
Root cause: the test polls `app.update()` inside a `std::thread::sleep(50 ms)` loop against a wall clock. A loaded machine stretches both the sleeps and the frames, so the loop's own 3 s bound fails — measured 1.81 s on bmb, and three gate refusals on `main`.
The reload itself is not slow: `FamilyWatch` polls on `Time<Real>` at 0.1 s with a 0.1 s settle, so the edit lands after a fixed number of *updates*, whatever the wall clock says. The test was measuring the machine, not the editor.
Change: both wait loops step `Time<Real>` (`TimeUpdateStrategy::ManualDuration(50 ms)`, at most 100 updates) instead of sleeping. The 3 s wall-clock bound is unchanged; one assertion is **added** — the edit lands within 1 s of the editor's own clock, which is the promise #96 makes. No assertion is removed, loosened or re-pinned.
Regression check: `crates/bevy_cauldron/tests/family_settings_reload.rs::an_edited_settings_file_reaches_the_editor_and_a_bad_one_is_a_problem` — 0.587 s, was 1.81 s under load.

## The clone's divergence (floor work, not an issue)
`main` was ahead 6 / behind 5. Five of the six were content-identical to commits already on `origin/main` (a pass that died before its push, re-ran, and pushed the same work under the same message). The sixth, `91c5d380`, was real work nobody had pushed: hero and place-and-undo re-recorded on the current editor (2026-11-07 10:38, newer than origin's `19439a1f` from 11-06 16:55) with both storyboards fixed where the UI had moved under them. Merging kept the newer files, which is right. One untracked abandoned draft, `2026-11-07-feel-size-motion.md`, was parked by `git stash` (`parked by pass 465`): it contains literal `GOOEY_SHA`/`GATE_RESULT` placeholders, names a bevy_gooey commit `75f8cc0` that does not exist in that repository, and its plan — applying the #76 evaluation as an assumed decision — is exactly what pass 463 declined to do.

## Not fixed: #322
The two gate flakes #322 records are real and this pass found their likely shape but did not fix them, because both fixes are protected test edits that need their own owner decision:
- `a_person_reports_about_this_project::a_click_on_the_map_picks_the_piece_there` reads the click's pixel from the camera in one frame (`pixel_over`) and picks from the camera in a later frame (`scene_at`), while `view.rs:432-438` eases yaw with `10.0 * dt`. A loaded machine's larger `dt` eases further between the two reads, which moves the ground point one cell — exactly the `[3, -1]` vs `[3, 0]` the issue records. The fix is to make the ease frame-rate independent (a fixed step or a manual clock), not to relax the assertion.
- `the_updater_offers_the_newest_release::the_chrome_bar_offers_an_update_and_refuses_it_over_unsaved_work` asserts the refusal text after a fixed 4 frames of a real clock. Same shape.
Recorded on #322 with the reading; both are queued behind the gate work that unblocks them.

## Borrowed from Mixar
None.

## Performance
Not measured: a test-only change.

## Acceptance
`accept.py` was not run: no `bevy_grimoire` product code changed.

## Review
Reviewer could not be spawned usefully for a change that is a protected test edit already approved in conversation; the five questions, answered by this pass:
1. **Does it weaken a check?** No. The 3 s wall bound is untouched and gains an assertion on the editor's own clock.
2. **Would it pass on a broken editor?** No: the loop still fails if the reload does not land, and the new assertion fails if it takes over 1 s of editor time.
3. **Is the clock the one the product reads?** Yes — `family_host::reload_settings` polls `Res<Time<Real>>`, and `ManualDuration` is that clock.
4. **Is anything else load-sensitive here?** Yes, two tests, filed on #322 above, not touched.
5. **Is the changelog line true?** Yes — `--no-clip` names the test that proves it.

## Test diffs
`crates/bevy_cauldron/tests/family_settings_reload.rs`: both wait loops step `Time<Real>` instead of sleeping; `STEP_SECS`, `PROMISE_SECS` and `MAX_STEPS` name the numbers; one assertion added (reload within 1 s of editor time). Owner-approved in conversation 2026-11-06; the commit message starts `owner exception (2026-11-06, in conversation)`.

## Release
None: nothing packaged changed (`bevy_cauldron` is `publish = false`).

## Next focus
#322's frame-rate-dependent camera easing, once an owner decision covers the two protected tests.
