# 2026-11-02 ui: the settings, keymap and project configuration screens, and #214 and #233 closed

Focus: the editor's UI on bevy_gooey · Commits: 6bc0cea..HEAD · Machine: big · Outcome: shipped

## Summary
No `ui` issue on the shell's path was ready (every one of them waits on #210's first acceptance item, which needs protected tests re-pinned), so the pass took the three ready issues the earlier passes had not listed: UI-71 (#249), UI-72 (#250) and UI-74 (#252, first slice). They hang off UI-70 (#248, closed), not off the shell. Also closed #214 (the click was erased by the editor's own pointer plumbing) and #233 (its acceptance list was met by pass 210's slice).

## Baseline
Gate: green at 2784c14. `accept.py`: 10/10 on `assets` before and after (no change to anything it exercises). Load 1.9, free disk 175 GB, available RAM 16 GB.

## Issues in the cluster
- **#214 UI-06 closed** (6bc0cea): `surface::retarget_pointer` cleared `PointerLocation` every frame when there was no window; it leaves it alone now. bevy_gooey pin 91953463 (v0.25.1). `tests/a_widget_is_clicked_by_its_label.rs` clicks the launcher row by its words; it failed before (Handoff stayed None). The issue asked for an existing chooser test to be rewritten; tests/** is protected, so it is a new test (assumed decision on the issue).
- **#233 UI-39 closed**: its two acceptance items were met by slice 1; the notification centre and Undo-on-notes (a re-pin of `headless.rs` and `the_journal_draws_the_agents_steps.rs`) moved to #260.
- **#249 UI-71 closed** (dd0c703, 0f92711): `settings_screen.rs`, `setting_catalogue.rs`. Cmd/Ctrl+, and the launcher's Settings button; search, tabs, default, description, Modified, Reset, User/Project switch; one key written atomically, a file that does not read is never written over. Two keys that act now (`show_grid`, `grid_strength`). Clip `2026-11-02-249-settings-screen`.
- **#250 UI-72 closed** (75c9c74): `keymap_screen.rs`. Rebinding by chord capture, a clash refused with nothing saved, `keymap` per command id in settings.ron/editor.ron. Clip `2026-11-02-250-keymap-clash`. Not done and filed as #265: the key dispatcher does not read the keymap yet (assumed decision on #250).
- **#252 UI-74 slice** (dd0c703, e54b9cd): `project_config.rs`. A switch per family crate writes family.ron atomically, reads it back through the reload contract's `family_list` check and shows `family.ron: applied`. Clip `2026-11-02-252-family-toggle`. Open: policy exclusions and kit edits on the screen.
- **Skipped**: #210 (item 1 needs `chrome::NAME` spellings changed in protected tests; blocks #211, #224 and everything behind the shell), #235 (protected re-pin), #251 and #254 (depend on #210), #223 and the panels (depend on #224), #129 (the ratchet's migrated-action check reads editor.rs only, and Accept's dispatch is in tiles.rs), #152 (its remaining red tests are fixture text in protected files), #193/#194 (wait on bevy_elements#55), #201 (in-place split needs a protected `assert_ne!` changed), #103, #112/#113 (need the decision model), #195 (laptop), #197 (wontfix).

## Review
`reviewer` subagent over `git diff 2784c14..HEAD -- crates/`; findings in the log line.

## Test diffs
None to an existing test. New files: `a_widget_is_clicked_by_its_label`, `the_settings_screen_searches_and_resets`, `the_keymap_refuses_a_clash_and_saves_nothing`, `the_project_configuration_toggles_a_family_crate` (each one `mod` line in `all.rs`, the owner's exception). Editor binary: 1095 tests.

## Demos
All eight standing demos and `editor.png` re-recorded at c1e8005 on fresh copies of `assets/`; `check.py` passes on all eight; `stale.py` reads fresh. They show the workbench as before: the three new screens are dialogs and appear in none of them.

## Release
None: only `bevy_cauldron` (publish = false) changed.

## Next focus
#210 needs an owner decision on the protected `chrome::NAME` spellings; until then the shell (#224) and everything behind it waits. Ready without it: #252's remainder, #265.
