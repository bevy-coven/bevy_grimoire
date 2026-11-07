# 2026-11-07 update: updating in place no longer freezes the app or strands the swap (#337); G-60 switch motion built in bevy_gooey

Focus: the owner's report in the prompt (bugs first), then epic #330 · Commits: f766fb33 (bevy_grimoire), 5f4ab2f + merge 3e73c98 (bevy_gooey) · Machine: mac_air for editing, big for every build and test · Outcome: shipped

## Summary
The owner reported that updating the app in place freezes it until it is killed and restarted. The machine's own files show the swap never ran for 0.4.2 (15:10) or 0.4.3 (16:42): `/Applications/Cauldron.app` is still 0.4.1, both staged folders still hold `unpacked/Cauldron.app` and an empty `swap.log`, and the window and audio were torn down at 16:42:45 with the process still running ten seconds later. Where the process was stuck is **not known**, so each wait that can strand an update is bounded: a closing terminal, the swap script, and the quit itself. In bevy_gooey, G-60 (the switch's knob travels on `motion.snappy`) was built beside it.

## Baseline
Gate: not run by hand (the push is the gate). mac_air had 1.2 GB free (`df /System/Volumes/Data` 100%), so nothing was built there: every build ran on big in worktrees `~/p1-gr-wt` and `~/p1-gooey-wt` against the shared target, with files rsynced from mac_air and the commits moved over ssh. `accept.py` and `perf.py` were not run: nothing they measure changed (no drawing, no registry operation).

## Bugs fixed
### #337: updating in place freezes the app and the swap never runs
Must-fix: blocks yes (an update cannot complete), common yes (every update from 0.4.1), no workaround other than killing the app by hand.
Evidence (all from files that were already on the machine): `/Applications/Cauldron.app` 0.4.1 against staged 0.4.2 and 0.4.3; `cauldron.log` relaunches 2 s after the window on the 0.4.0 and 0.4.1 updates and none after 0.4.2 and 0.4.3; the unified log (`log show`) has the process alive and `running-active` at 16:42:55 after its window and audio went at 16:42:45; the agent tab's `omp` child was alive at 16:49. 0.4.1 is the release that added the agent tab.
Not established: which destructor held the main thread. A bare PTY and a zsh with `omp` in the foreground both died on `SIGHUP` in 0.07 s and 0.10 s when tried by hand on this Mac, so the terminal's unbounded `wait` (alacritty's `Pty::drop`) is one possible cause and not a proven one.
Change: `terminal/pty.rs` (a bounded `Drop` for `TerminalSession`), `update.rs` (`swap.sh` waits at most `SWAP_KILL_AFTER_S`, asks then makes the old process end, and `Swap::start` runs it in its own process group), `exit_watchdog.rs` (the binary's first `AppExit` starts a thread that records `sample` of the stuck process to `exit-hang.txt` beside the log and ends it after `EXIT_GRACE_S`).
Regression checks (new files, three `mod` lines in `tests/all.rs`, the owner's exception for a new editor test file), each mutation-checked on a copy of the tree:
- `closing_a_terminal_never_waits_on_its_child::a_child_that_ignores_the_hangup_does_not_hold_the_drop`: 1.07 s with the fix, 20.0 s and a failure without the `Drop`.
- `an_update_swaps_even_when_the_old_app_will_not_leave::the_swap_ends_an_old_app_that_will_not_leave_and_finishes` and `::the_script_does_not_share_the_apps_process_group`: both fail with the TERM and KILL lines removed (30 s, the test's own limit), and the second alone with `process_group(0)` removed.
- `a_quit_that_does_not_finish_is_ended::nothing_is_armed_until_the_app_exits_and_a_second_exit_arms_nothing_more`: fails with the arming removed.
- Before: the old script, run against a stand-in that ignores `SIGTERM`, was still waiting after 8 s with the old app installed; only `SIGKILL` of the old app let it swap.
Also green: the 36 lib tests of `terminal`, the 29 updater, download, terminal and new tests of the `editor` binary that the filter `test(terminal::) | test(update)` and the three new files select, and `cargo clippy -p bevy_cauldron --all-targets -- -D warnings`.
**What this does not fix:** an app already running the old code (0.4.1 to 0.4.3) still quits with the old script and needs one kill and restart; the update after that one is the first that is covered. The staged 0.4.3 app is in `~/Library/Caches/Cauldron/updates/0.4.3/unpacked/Cauldron.app`; nothing was moved in `/Applications`.
Not closed: `Refs #337`, not `Fixes`, because the end-to-end claim (an update from a build that has this, on a Mac) cannot be checked until a release with it exists.

## Enhancement: G-60 switch motion (bevy-coven/bevy_gooey#67)
Goal: a toggle's knob travels on `motion.snappy` instead of jumping · Files: `src/layout_motion.rs` (`TravelSpring`, the knob marked), `src/lib.rs`, `tests/switch_motion.rs`, `docs/research/switch-motion.md`, `docs/guide/motion.md`, `AGENTS.md`, `CHANGELOG.md`.
Proof: 6 new tests; an unmarked knob and a knob on the default spring each fail the two spring tests. Gate on big: fmt, clippy `--all-targets --all-features`, nextest 1164/1164, doctests 256, `check`, wasm `check`, `doc -D warnings`. Merged with pass 473's G-57 (`760405b`); the CHANGELOG conflict kept both lines.
Not built, said on the issue: the menu check mark's entrance, and Rune's hover stretch of the thumb.
**G-61 (#68), claimed and released:** the issue's design (one indicator behind the options, the lit option's own fill removed) re-pins protected assertions (`tests/tabs.rs:570,583,601`, `tests/toolbar.rs:235,248` hold the lit option's `BackgroundColor` at the accent). The additive form is an opt-in `SelectionIndicator` on the strip, which makes the lit option's fill transparent only there; it was not started.

## Review
Reviewer not spawned. Self-review, my five questions: (1) Can the watchdog end a healthy quit? It arms on the first `AppExit` and fires after 3 s; a quit that takes longer than 3 s is ended after a `sample`, which is the point. (2) Can the swap script end an unrelated process? Only the pid it was given, and only while `ps` still names the program it named at the start and the state is not a zombie. (3) Does the `Drop` change a compliant shell's close? No: `waitid` with `NOWAIT` sees it leave and returns (0.08 s in the test). (4) Does the watchdog end a test run? It is added in `main.rs` only. (5) Is `SIGKILL` of the whole process acceptable? Its children get the hangup from the PTY closing; nothing is written at that point that is not already on disk (the window frame is saved in `Last` on the same `AppExit`, before the watchdog's grace).

## Test diffs
New files only, plus the three `mod` lines at the end of `crates/bevy_cauldron/tests/all.rs` (the owner's exception). No existing assertion changed.

## Release
Not this pass's: only `bevy_emerge_loop` releases. `bevy_cauldron` has files changed since 0.4.3 (`f766fb33`), so the next release pass must cut 0.4.4 (`python3 tools/loop/version.py next`); the update-freeze fix reaches the owner only with that release and one manual restart.

## Next focus
G-61 as the opt-in `SelectionIndicator`, then G-62, G-63 and G-65 for UI-R06; G-67 (`InspectMixed`, #77) is unclaimed and needs no other issue.
