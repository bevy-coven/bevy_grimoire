# 2026-11-03 fix: a settings reload is measured on a clock a test can drive (#279)

Focus: UI-02 (#210)'s owed slice, then the open bug · Commits: `488fdd8..22ef281` · Machine: big · Outcome: shipped

## Summary

`#279` is the gate's own reliability bug: `reload_settings` polled `FamilyWatch` with an
`Instant` it created privately, so the one knob that decides when a settings reload lands was pinned to
the wall clock and no test could drive it. It now reads `Res<Time<Real>>`, so
`TimeUpdateStrategy::ManualDuration` steps it and the reload path is testable with no sleep. A running
editor is unchanged — `Time<Real>` *is* the wall clock unless a test replaces it.

Two things to know before reading further: **a second hand-driven pass (pid 2095292) was live in this
same clone**, and **I first wrote the wrong proof for this fix and corrected it from a measurement.**

## Baseline

Gate: green before (`9ff6ed8`). `accept.py`: not run (no solver path touched). Disk 93 GB free,
12 GB available RAM, load 1.9.

## The concurrent pass, which shaped this report

`ps` shows **two** agent sessions in `bevy_emerge_loop`: this one (`omp -p /build-loop`, pid 3064119,
driver pass 372, started 13:34:44) and `omp --resume` (pid 2095292, started 10:41, hand-driven pass 371).
Both were editing `crates/bevy_cauldron/src/chrome.rs` within seconds of each other; my writes and its
writes interleaved, and my `GroundRole`/`repaint_grounds` pair was a duplicate of its better-integrated
`PanelGround` marker plus `dim_the_ground` reading the skin.

What I did about it, in order: **removed my duplicate twice** (the second time because its next write
restored my symbols from a stale buffer), left its design standing, and **committed only my own files**.
Then, when I staged `all.rs` with my one `mod` line, its `git commit` swept my staged files into
`488fdd8` — a shared index, not a mistake I could avoid. My content is intact there (`git show
488fdd8:crates/bevy_cauldron/tests/a_reload_is_measured_on_the_clock_the_editor_exposes.rs`), and
`22ef281` carries the changelog line.

**This is a configuration bug, not an item failure, and it is worth fixing before the next pass:**
`wiggum.sh one` was started by hand at 13:34 while a `--resume` session was already open on the same
clone. The command's own "Parallel workers" section assumes one clone per worker for exactly this reason.

## Bugs fixed

### #279: `reload_settings` polled the watch on a private `Instant`

Must-fix: blocks **y** (it has failed the push gate at least three times), common **n** (one test),
no workaround **n** (the owner can delete the sleeps) — but blocks, so it is must-fix.

Root cause: `family_host.rs` kept `started: std::time::Instant` and polled `held.watch.poll(now)` with
`held.started.elapsed()`. `FamilyWatch::poll` and `FileWatch::poll` have always taken `now_secs` and
never asked where it came from, so the defect was entirely in the caller.

Change: the `Held` local keeps `made_at: f64` (the `Time<Real>` reading when the watch was made) instead
of an `Instant`, and every poll is `real.elapsed_secs_f64() - made_at`. Subtracting the origin keeps the
intervals the watch is configured with (0.1 s poll, 0.1 s settle) meaningful, rather than measuring from
the app's start.

Regression check:
`crates/bevy_cauldron/tests/a_reload_is_measured_on_the_clock_the_editor_exposes.rs::stepping_the_clock_past_the_poll_and_the_settle_reloads_the_file`
— **failed at `9ff6ed8`** with `left: ...hour: 6.0... right: ...hour: 18.5...`, passes after.

## The proof I first got wrong, and the measurement that corrected it

I wrote the file claiming the *negative* test (`a_clock_that_does_not_advance_reloads_nothing`) was the
failing-first proof. I then actually ran both at `9ff6ed8` in a detached worktree with its own
`CARGO_TARGET_DIR`:

| test | wall clock (`9ff6ed8`) | `Time<Real>` |
|---|---|---|
| `stepping_the_clock_past_the_poll_and_the_settle_reloads_the_file` | **FAILED** | passes |
| `a_clock_that_does_not_advance_reloads_nothing` | **passed** | passes |

So the negative test passed at HEAD — on an idle machine 40 fast updates had simply not accumulated the
10 ms the wall clock still needed. It is a **regression guard that stays true under load**, not a
demonstration, and the file's header now says exactly that with the table above. The positive test is the
proof: three `app.update()` calls are **0 s** of real time, so the 0.1 s poll is never crossed and the
edit cannot land.

## Not shipped, and why

The protected test `crates/bevy_cauldron/tests/family_settings_reload.rs` is **untouched**: its two
`sleep(50ms)` lines and its 3 s bound stand. Deleting them is the owner's call (the issue's
recommendation 1), and this commit is what makes recommendation 2 possible in the same edit — the clock
is injectable now, so those sleeps can become stepped time rather than deleted assertions.

## Test diffs

One new test file, which the owner exception allows: `a_reload_is_measured_on_the_clock_the_editor_exposes.rs`
(2 tests) and its one `mod` line in `tests/all.rs`. **No existing test was edited, removed or re-pinned.**

## Review

Reviewer could not be spawned this pass (rate limit); these are my own five answers.

1. **Does the fix change a running editor?** No — `Time<Real>` is advanced from `Instant` by `TimePlugin`;
   `ManualDuration` exists only in a test that inserts it. Verified: the editor suite's 1218 other tests
   pass unchanged.
2. **Is the subtraction of `made_at` right?** It is the origin the watch was created at, so
   `poll(now)` sees the same *intervals* it is configured with. Without it the first poll would compare
   against the app's uptime, which is arbitrary relative to the watch.
3. **Can the new test pass vacuously?** The positive test fails at `9ff6ed8` — measured, quoted above.
   The negative test can, and its header says so.
4. **Is the shared tree's other content mine to ship?** No: `488fdd8` is another pass's; I removed my
   duplicate of it and committed only my two files plus one `mod` line.
5. **What did the interleaving cost?** Two re-edits of `chrome.rs` to remove my duplicate, and a shared
   index that put my files in another pass's commit. No content was lost; the report says so.

## Release

None. `bevy_cauldron` is `publish = false`, and no packaged crate's files changed.

## Next focus

Close #210's last item or take the next ready `ui` issue; and **stop two passes sharing one clone**.
