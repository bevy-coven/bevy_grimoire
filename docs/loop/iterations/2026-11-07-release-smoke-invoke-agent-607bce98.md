# 2026-11-07 release, terminal: the 0.4.4 smoke retries a fresh launch (#338); the terminal starts no agent until Invoke Agent is pressed; gooey G-53 hover groups and G-65 toast entrance

Focus: the owner's asks (release smoke, the agent costing tokens, work through the issues) · Commits: `58cdbe6a..` (`b5371788` smoke, `607bce98` Invoke Agent, release below) · Machine: mac_air (edits), big (builds, gates) · Outcome: shipped

## Summary

The 0.4.4 release workflow built, signed and notarized, then refused at the smoke ("the window never took focus: another app held the foreground"). The artifacts were verified and published by hand with the workflow's own last command; #338 now retries a never-focused window with a fresh launch. The owner's other ask this pass: an agent chosen in the settings started itself the first frame the terminal was raised and cost tokens to warm up, so the terminal now opens empty with an `Invoke Agent` button in the middle and starts nothing until it is pressed. In bevy_gooey (the family crate the UI series waits on), G-53 hover groups (#60) and G-65 toast entrance (#72) are built and closed; G-64 (#71) is released with a finding.

## Baseline

Gate: green at `58cdbe6a` (pass 1). `accept.py`: not run (no editor surface changed that it measures). Big load 47 during the long builds (shared with other loops); `/home` on big had 353 GB free at the start of the pass and 647 GB at 19:15 (other loops churn it); 12 GB free on mac_air.

## Bugs fixed

### #338: the release smoke fails when another app holds the foreground
Must-fix: blocks yes (0.4.4 was built and notarized, then refused), common no, no workaround partly (publish by hand) — the run's log, and `launchctl submit` runs: one passed, the launch 10 s later failed, three later background runs passed.
Root cause: the smoke launches the app once and waits for its window to be frontmost; another app holding the foreground at that moment fails a build that is fine.
Fix: `tools/release/macos/smoke_app.py` retries a `NeverFocused` window up to `LAUNCH_ATTEMPTS` (3) with a fresh launch, `FOCUS_PATIENCE_S` (40) each; any other failure still fails at once. `tools/release/macos/test_smoke_app.py` (6 tests, 3 fail when the retry is mutated away). `Refs #338`, not closed until a release goes through the new smoke.

### 0.4.4 published by hand
`gh release create bevy_cauldron-v0.4.4 …` with the artifacts the workflow built (SHA256SUMS checked, `stapler validate`, `spctl` "Notarized Developer ID", the smoke run by hand on that app passed). Commented on #337.

## Enhancement: the terminal starts no agent until Invoke Agent is pressed (the owner's message to the loop)

Goal: no tokens spent by a person who is only looking around the game. Files: `terminal/mod.rs` (`invites_the_agent` replaces `wants_first_agent` and the `agent_opened` flag), `terminal_view.rs` (`pump_the_terminal` opens a plain shell only when no agent is chosen), `terminal_pane.rs` (`TerminalInvokeButton` and `TerminalInvite`, drawn by `keep_the_agent_on_screen`, `the_button_invokes_the_agent`), docs (`README.md`, `docs/guide/agents.md`, the setting's description).
Behaviour: an agent chosen and nothing open: the pane shows `Invoke Agent` in the middle of its body and spawns nothing; the press runs `open_agent_tab`, as the `Agent` chip does. Closing the last tab brings the button back and starts no shell (`+` still opens one). No agent chosen: unchanged.
Proof: `tests/the_terminal_starts_no_agent_until_it_is_invoked.rs` (3 tests, real pointer input on the editor's image): nothing opens in two seconds and no harness output appears, the button is within 1.5 px of the body's centre, the press opens an agent tab that runs the command and removes the button, closing the tab brings it back with no shell, and no agent chosen opens a shell with no button. Five mutations each fail a test: a shell opening while invited, the press doing nothing, the button staying after a tab opens, the button showing with no agent chosen, the button in a corner. Not recorded as a clip: no display on big this pass (no Xvfb running) and mac_air has 12 GB free, too little for the editor's debug build; the changelog line says so.

## Enhancement: bevy_gooey G-53 hover groups (#60), G-65 toast entrance (#72), G-64 (#71) released

Built on gooey `main` (`d171c33`, notes redated at `036d822`), gate green on big (fmt, clippy `-D warnings`, 1191 tests, doctests, `check`, wasm `check`, `doc -D warnings`). G-53: `HoverGroup` + `GroupReveal` (`src/hover_group.rs`, `tests/hover_group.rs`, six mutations each fail a test); not built, and said so on the issue: every widget's hover colour leaving on `motion.default`. G-65: a new toast starts 48 px toward its stack's edge and settles on `motion.snappy` (`begin_entrances`, `tests/toast_enter.rs`; four mutations, one an equivalent mutant because reduced motion lands every travel anyway). G-64: released with a comment, because the proposal's clean cutover of `PopoverSlide` re-pins eight assertions in `tests/motion.rs` and `tests/reduced_motion.rs`; option 2 (an additive entrance) is recorded as the assumed decision.

## Performance

Not measured (`perf.py` needs a real display).

## Acceptance

`accept.py` not run.

## Review

No reviewer subagent was spawned. Self-review, five questions: (1) does the Invoke Agent path keep the one agent start path? Yes: the button and the `Agent` chip both call `open_agent_tab`; the automatic start is deleted, not kept behind a flag. (2) Can an agent still start by itself anywhere? `grep wants_first_agent` finds nothing. (3) A keyboard-only person: the button is a click target like the bar's chips, with no key; stated here, not fixed. (4) Does closing the last tab strand a person with no shell? The button and `+` both remain. (5) Did any assertion weaken? One test's setup gained a step (below).

## Test diffs

- `crates/bevy_cauldron/tests/an_agent_tab_runs_its_self_test.rs`: the test presses `Invoke Agent` (`invoke_the_agent`, new, one call) before its assertions, which are unchanged: the first tab is still the agent's and every check is as before. Owner exception, the owner's own request ("you don't immediately engage the agent"), recorded in the commit message; none was written for it in advance, so this is an assumed owner exception.
- `crates/bevy_cauldron/tests/all.rs`: one `mod` line. gooey `tests/all.rs`: two.

## Release

`bevy_cauldron` 0.4.5 (`version.py next`; the Invoke Agent change is in it, the smoke retry is in the workflow's script), as its own commit `release: 0.4.5` at the end of this pass's push with the tag `bevy_cauldron-v0.4.5`. Not a crates.io crate (`publish = false`). The mac_air runner builds it; this pass does not watch or re-run the workflow. If its smoke refuses it again, the artifacts are in `~/actions-runner-bevy_grimoire/_work/bevy_grimoire/bevy_grimoire/dist` and `gh release create` publishes them as it did for 0.4.4.

## Disk and leftovers

Pass 1 removed 486 GB of regenerable build output on big and 12 GB of caches on mac_air (owner-granted). This pass removed its own worktrees (`~/p2-gooey-wt` and `~/p2-cauldron-wt` on big, `~/p2-gooey-wt` on mac) and `/tmp/p2_*`. Listed, not removed: worktrees `~/g17_base`, `~/g17_parked`, `~/g17s_base`, `~/bevy_elements_i70`, `~/bevy_elements_base70`, `~/bevy_before_target`, `~/bevy_lights_base_target` on big, and the `p1-*` and `p2-*` branches in `~/bevy_emerge_loop` and `~/bevy_gooey_loop` (a branch delete needs the owner's word).

## Next focus

gooey G-61 (#68) as the opt-in `SelectionIndicator` the assumed decision describes; then the UI-R04 to R06 items as their gooey issues close.
