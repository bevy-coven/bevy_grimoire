# 2026-11-03 shell: every region answers to its name, and the divider is hittable

Focus: shell UI-30 (#224) · Commits: d2756f2..cc2ec9e · Machine: big · Outcome: shipped

## Summary
The frame's four regions now carry `AccessibleLabel`, so a reader, a test or an agent can find them
by name; they were reachable only through the `Frame` resource. The split root lost its
`Pickable::IGNORE`, which had made the divider — the one grabbable node in the frame — unhittable.
The clip the last three audits named as `demo?` now exists.

## Baseline
Gate: green at d2756f2. `accept.py`: not run (no display; this pass measured layout and clips
instead). Load 3.95, free disk 153 GB, available RAM 13 GB.

## Enhancement: the shell's regions and its divider
Goal: the issue's first acceptance item — "a headless test finds each region by its accessible
label" — plus the visible clip it asks for.
Files: `crates/bevy_cauldron/src/chrome.rs`, one new test file, `tools/demo/storyboards/shell_regions.json`.
Proof: `the_shells_regions_are_labelled` failed at HEAD with
`no laid-out node answers to "Viewport"; ... Laid out labels found: ["INSPECTOR", "PLACE", "MAP"]`
and passes now. `the_shells_divider_resizes_the_docks` pins that the split keeps the geometry
`SplitGeometry::read` needs. Full editor suite: 1159 passed, 0 failed. Clippy: clean.

## What was measured, and what stayed blocked
- `Pickable::IGNORE` on the split root made the divider unreachable: `bevy_picking` skips a whole
  subtree below an ignoring ancestor. Measured over BRP: `SplitFraction` 0.22 before and after a
  105-px drag. Fixed with `Pickable::default()`; the panes keep `IGNORE` through
  `adopt_the_shell_split`.
- **A `drag` from the recorder still does not move the divider**, and I stopped after two attempts
  per the stop rule rather than guess a third time. The measured cause: `surface::retarget_pointer`
  re-points the pointer onto the viewport **image** every frame, so an injected cursor's position is
  read in the image's space and window-space UI — the divider included — is never reached. That is
  the next slice, and it is recorded on #224.
- The clip therefore shows what is proven: the shell with the world inside the hole the docks leave,
  with a real pan (10.1% of the frame changes across it). Its caption was rewritten to say that, and
  the changelog title with it — the first title claimed a resize the clip does not show.

## Test diffs
Two new test files, no existing assertion changed:
`crates/bevy_cauldron/tests/the_shell_s_regions_are_labelled_and_the_divider_moves.rs` plus its one
`mod` line in `all.rs` (the owner's exception for a new editor test file).

## Review
Reviewer not spawned; self-review, mine:
- **Does the pickable split leak hover?** No: `Hovered` is what `view::over_ui` reads, and neither
  the split root nor this gooey revision's divider carries it; the panes stay `IGNORE`.
- **Could the label test pass vacuously?** It filters to laid-out nodes (`size > 1 px`) and names all
  four regions, so an unlaid-out label does not satisfy it.
- **Is the divider test meaningful?** It asserts the fraction reaches the panes (0.22 -> 0.40 grows
  the first pane), so a split that kept its components and stopped laying out fails it.
- **Does the clip show the claim?** Watched: docks, viewport, status bar, and a pan; the still was
  re-read from a new path after the first render turned out to be a failed project open.
- **What is still missing?** The recorder-driven drag, named above.

## Release
Two crates had real changes since their tags and are published from a green local gate:

- **bevy_grimoire_core v0.7.0** — https://crates.io/crates/bevy_grimoire_core/0.7.0
  `AgentSettings::launch` gained a parameter (semver-checks: `method_parameter_count_changed`), so
  pre-1.0 it is a MINOR bump: 0.6.5 -> 0.7.0.
- **bevy_grimoire v0.4.2** — https://crates.io/crates/bevy_grimoire/0.4.2
  Its API is unchanged (202 checks pass, no semver update required) but its published `Cargo.toml`
  now requires core 0.7.0, so the crate that ships is different: PATCH.

`bevy_grimoire_rng`, `_solver`, `_anim` and `_snip` have no changed files of their own. `bevy_cauldron`
and `bevy_cauldron_mcp` are `publish = false`.

## Second slice: the split's floor
`a_dock_resize_never_covers_the_viewport`: at `SplitFraction` 0.95 the viewport measured **47 px of a
1280-px body**, because `SplitMin` was its default `(0, 0)`. `SplitMin::new(96.0, DOCK_PANE_MIN_PX)`
— gooey's own dock minimum — takes it to **115 px**. The test sweeps 0.0, 0.05, 0.22, 0.5, 0.95 and
1.0; it fails without the floor and passes with it. Its header says plainly what it does not
promise: the floor is a fixed pixel count, so a narrow window with a wide dock can still leave the
world under a quarter — a guarantee needs a `SplitMin` computed from the body's width, which is a
system this slice does not add.

## Next focus
Make an injected drag reach window-space UI (`retarget_pointer` decides the pointer's space), then
record the divider resize into this clip.
