# 2026-11-07 ui: the shell's arrangement is a person's, kept across a restart (#227)

Focus: the `ui` series, first ready issue (`build-loop.md`, "The owner's next override") · Commits: `cdd0c2d1` · Machine: big · Outcome: shipped (one slice)

## Summary
UI-33 (#227) asked that Kit, Map and Rigs be saved `DockLayout`s with a person's changes persisted in
`settings.ron` and Window ▸ Reset layout restoring the default. The slice ships all three parts: the
layout is a settings key, a drag writes it, a file's layout is what the editor opens on, and a Reset
layout row puts the default back on screen and out of the file. Only one issue was workable this pass
(see "Cluster").

## Baseline
Gate: green on `b550459f`. Load 21.5 (65 over five minutes), 45 GB free, 13 GB available RAM.
`accept.py` not run: no import path touched.

## Enhancement: UI-33, saved dock layouts (#227)
- **Goal:** each door's arrangement is data a person edits, and survives a restart.
- **Files:** `settings.rs` (the `shell_layout` key on both `EditorSettings` and `SettingsLayer`, and its
  `resolve`), `ui.rs` (`save_the_layout`, `follow_the_saved_layout`, and their registration),
  `chrome.rs` (`spawn_frame` opens the dock on the settings' layout), `app_verbs.rs`
  (`AppVerb::ResetLayout` and `reset_the_layout`), `menu_bar.rs` (the View row).
- **Proof:** `a_saved_arrangement_is_the_one_the_editor_shows`, 3 tests. Before the change they do not
  compile (`no field shell_layout`, `no variant ResetLayout`). The three:
  1. a restart — a second application on the same file — opens on the arrangement the file holds;
  2. Reset restores `ui::shell_layout` on screen, clears the key, and the running settings agree;
  3. a `DockLayoutChanged` a dock reports is written to the file.
- **Not a setting in the catalogue:** a person drags a layout, they do not type it, and the Settings
  screen has no control for structure. `View ▸ Reset layout` is the way back, and the guide says so.
- **`Window` or `View`:** `docs/ui/vision.md` §4 says "Window ▸ Reset layout", but the bar has no Window
  menu (`menu_bar::MENUS`). The row sits in **View**, beside Show statistics — the other row there that
  is a setting and not a command. Assumed decision, recorded here; the owner may move it.
- **Nothing is written without a `UserSettingsFile`**, so a headless test that drags a divider writes no
  real config. The write goes through `setting_catalogue::edit_layer` — the same read/refuse/read-back/
  atomic path as `write_key` — rather than through `Value`, which is the Settings screen's vocabulary of
  switches, numbers and named choices; adding a fourth variant would have forced `Value` to lose `Copy`
  and broken a protected test's compile for no behaviour.

## Cluster
The batch rule asks for five or six. This pass had one: every `feel`-series issue (UI-R01..R06,
#331–#336) still waits on `bevy_gooey`, the remaining `ui` items wait on each other or on a panel that
does not exist yet, #210 was released as blocked on the owner, and #327/#322 are more than one pass's
share. The other live session in this clone (pass 458) is on `bevy_gooey` G-52.

## Review
The reviewer subagent was not spawned: this pass ran two builds and the gate inside the budget, and the
fallback questions were answered here instead.
- *Does the write path refuse a file that does not read?* Yes — `edit_layer` reads first and leaves a
  refusing file alone; the test's `saved()` helper re-reads through `SettingsLayer::read`, so a bad
  write would fail there.
- *Does a settled editor write every frame?* No: `follow_the_saved_layout` compares before assigning,
  and `no_system_writes_every_frame` passes.
- *Can a test reach the real config?* No: no `UserSettingsFile` means no write.
- *Is the default still the default?* Yes: `shell_layout: None` resolves to `ui::shell_layout`, and Reset
  writes `None`, not a copy of the default.
- *What breaks if gooey's `DockLayout` changes shape?* The key is RON of gooey's own type, so a change
  there is a settings read the editor refuses by name (`SettingsLayer::read`), not a silent default.

## Test diffs
None to an existing test. One new file, its one `mod` line in `all.rs`.

## Release
`bevy_cauldron` is `publish = false`; nothing packaged changed, so nothing was published.

## Next focus
The next ready `ui` issue once gooey closes another of UI-R01..R06's needs; until then #327 (a settings
edit during a capture sequence) is the oldest workable bug.
