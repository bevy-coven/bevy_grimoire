# 2026-11-07 feel: UI-R03 buttons, UI-R04 desktop conventions and UI-R05 scroll fades adopted on the gooey pin; gooey G-68 type-select groups; release 0.4.6

Focus: the UI series, feel group (#333, #334, #335 as one cluster) · Commits: `07ee7da9..880709c8`, then the clips and docs · Machine: mac_air (edits), big (builds, gates, recordings) · Outcome: shipped

## Summary

One cluster of three feel-series issues behind one gooey pin move (0.40.1 to 0.43.0, gooey `70126ef`). UI-R05 (#335): every `chrome::scroll_list` viewport takes gooey's `ScrollGlide` and `ScrollEdgeFade`. UI-R03 (#333): `AccentButton` and its repaint are replaced by gooey's `ButtonVariant::Submit`, toolbar icon buttons are `Ghost`, a destructive confirmation's agreeing answer is `Danger`, and the two dialogs with a text field name `DefaultButton`. UI-R04 (#334): the launcher's cards type-select, press-scale and open on a double press, and a tooltip waits 0.5 s. UI-R04 needed gooey to type-select a card grid, so the pass built that in gooey (G-68, #78, closed). 0.4.6 carries the pass and the Enter key from pass 2.

## Baseline

Gate: green at `07ee7da9` (3144 tests). Pin move alone: `cargo check --all-targets` clean, then 1891 of 1892 editor tests passed; the one failure was `the_launcher_and_the_command_line_agree::copy_command_line_shows_the_line_for_the_row_even_when_the_clipboard_cannot_take_it` (below). `accept.py` not run (no map surface changed). Load 18 to 50 on big; 625 GB free on `/home` at the end.

## Cluster

| Issue | State | What remains |
|---|---|---|
| #335 UI-R05 scroll fades | closed (`Fixes`) | none |
| #334 UI-R04 conventions | closed (`Fixes`) | `HoverGroup`/`GroupReveal` on row actions: I found no row action the editor reveals on hover, so there is nothing to adopt |
| #333 UI-R03 buttons | closed (`Fixes`) | inspector controls at `ControlSize::Small`: the shell has no inspector panel yet (UI-50, #236) |

## Enhancement: the feel series on the launcher

Files: `chrome.rs` (`scroll_list`), `ui.rs` (`submit_button`, `paint_marked_buttons` now only the selected rail), `launcher.rs`, `new_project.rs`, `project_page.rs`, `empty_state.rs`, `toolbar.rs`, `confirm.rs`.
Proof: `tests/the_project_list_fades_at_the_edges_that_hide_rows.rs` (2), `tests/the_buttons_say_what_they_do.rs` (3: Return creates the project from the Name field, every toolbar button is Ghost, a destructive answer is Danger and an ordinary one is not), `tests/the_launcher_takes_the_desktop_conventions.rs` (4: a tooltip is not shown at 0.4 s and is at 0.64 s on a 16 ms clock, a pointer double press opens a card, `br` moves focus to the bravo card, a card is `PressScale(0.97)`).
Six mutations of the editor changes (no edge fade, Create not the default button, a toolbar button not Ghost, a destructive answer not Danger, the cards not a type-select group, no press scale) each fail the test that names them and nothing else.

Two things the work found: a keyboard-only test needs a `Window` entity in the headless editor, or `FocusedInput` never reaches the text box and Return never reaches the default button (the first Return test failed for a reason that had nothing to do with `DefaultButton`); and `Submit` buttons with the same words can sit on other pages (the first-run welcome's `New project` is plain), so a test finds a main action by its variant.

## bevy_gooey

G-68 (#78): `TypeSelectGroup`, a public marker; one `Or<(With<ListView>, With<TypeSelectGroup>)>` in the type-select system's group lookup, 4 tests, two mutations that fail them. Gate: fmt, clippy `-D warnings` (it caught a `type_complexity`, fixed with an alias), 1204 tests (two time-based flakes at load 50, `elevation::each_surface_casts_its_heights_stack_from_the_skin` and `split::a_double_click_collapses_the_first_pane_and_a_second_gives_it_back`, both pass alone), doctests, `check`, wasm `check`, `doc -D warnings`. On gooey `main` at `70126ef`; not in a gooey release yet.

## Performance

Not measured (`perf.py` needs a real display).

## Acceptance

`accept.py` not run.

## Review

No reviewer subagent spawned. Self-review, five questions: (1) Is there one path to each behaviour? Yes: `submit_button` is the only primary button; the `AccentButton` marker, its helper and its repaint are deleted, not kept. (2) Does a pinned behaviour change under the pin move? The launcher's second press still opens (`the_launcher_opens_a_project_by_its_name` pins it); gooey's `DoublePress` is therefore not a second path, and a double press opens through the existing one. (3) Does the fade cost anything at rest? gooey writes nothing for an edge with nothing hidden. (4) Keyboard parity: Return presses Create from the Name field; cards take Tab, arrows and type-select; a toolbar `Ghost` is still a focusable button. (5) Did any assertion weaken? One test's lookup changed (below).

## Test diffs

- `the_launcher_opens_a_project_by_its_name.rs`: `AccentButton` is replaced by `ButtonVariant::Submit` through one helper, `submit_buttons`; every assertion is the same. The issue's own acceptance names this ("assumed owner exception: a type rename in a protected test").
- `the_launcher_and_the_command_line_agree.rs`: the card's menu is read from the card (`ContextMenu` on the card) instead of the first `ContextMenu` in the world. The terminal pane has one too, and which one a query yields first depends on archetype order, which the gooey pin moved. The assertion is unchanged; assumed owner exception, in the commit message.
- `all.rs`: three `mod` lines.

## Release

`bevy_cauldron` 0.4.6 (`880709c8`, tag `bevy_cauldron-v0.4.6`; not a crates.io crate). The workflow is not watched. 0.4.5's run passed its smoke on the first attempt (run 42435425188), so #338 was closed.

## Clips and demos

Two changelog clips, recorded on big under its own Xvfb from the 0.4.6 build with the new storyboards `launcher_conventions.json` (the scroll fades, a card chosen, `al` moving focus to alpha, a double press opening it) and `new_project_return.json` (New project, a typed name, Return, the project open). Both contact sheets and stills were looked at. The standing GIFs were last recorded by the other loop at `3e9d9bb8`, before this cluster, so `stale.py` listed all eight and `editor.png`. Seven were retaken on the 0.4.6 build with `tools/demo/retake.sh` (`a4c4e21a`): hero, agent-drives, agent-removes-then-undo, frame-the-map, move-the-camera, place-and-undo and review-labels each passed `check.py` on the first take, and each contact sheet was looked at; `docs/media/README.md` has their new commit and frame counts. Owed: `build-a-map.gif` (four takes, three with a static run of 1.8 s against the 1.5 s limit and one with a 2.4% change for a step that needs 5%, so its storyboard's timing wants a look) and `editor.png` (a window screenshot taken by hand, which `retake.sh` does not do).

A first retake passed `check.py` and was wrong: two editors of my own from earlier single takes were still alive on BRP port 15710 (the pid file held a subshell's pid, so my `kill` missed them), the new editor could not bind it, and the recorder drove the old one, whose bed an earlier take had already removed. The sheet showed `placement_remove: no placement is called bed`. Both stray editors were mine (their cwd was my scratch project) and were killed by pid; the retake was then redone from clean. `check.py` is necessary, not sufficient.

## Disk and leftovers

This pass's worktrees (`~/p3-wt`, `~/p3-gooey-wt` on big, `~/p3-gooey-wt` on mac), `/tmp/p3_*` and the demo scratch are removed. Listed, not removed: `~/g17_base`, `~/g17_parked`, `~/g17s_base`, `~/bevy_elements_i70`, `~/bevy_elements_base70`, `~/bevy_before_target`, `~/bevy_lights_base_target`, and the `p1-*`, `p2-*` and `p3-*` branches in `~/bevy_emerge_loop` and `~/bevy_gooey_loop` (a branch delete needs the owner's word).

## Next focus

UI-R06 (#336) once gooey's #68 (G-61), #69 (G-62), #70 (G-63) and #71 (G-64) are closed; until then the UI-5x panels, whose gooey needs are now smaller.
