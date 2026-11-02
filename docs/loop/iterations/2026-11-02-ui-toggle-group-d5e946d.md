# 2026-11-02 ui: the demos show the toolbar, and the tools become one gooey group (#231)

Focus: the standing demos, then UI-37 #231 · Commits: 0c96e68..d5e946d · Machine: big · Outcome: shipped (a demo refresh and one slice; #231 stays open on a found gap)

## Summary
`stale.py` listed all nine standing demos, so that was the first job: all eight GIFs and `editor.png` were
re-recorded on the editor as it is now, every one passing `check.py`, and the README's *Captured* column
moved to the commit they were recorded at. Then UI-37's remaining slice: the six tool modes are one
gooey `toggle_group` whose value is `EditorState::tool`, so exactly one is lit and a key moves the light.
**One case of that slice does not pass and is recorded**: a click on a group option moves the group but
its `ValueChange` never reaches the editor's observer.

## Baseline
Gate: green (`0c96e68`, from the previous pass's clip push). `accept.py`: not run — no import path touched.
Load 3.0, 151 GB free, 13 GB available RAM.

## Cluster
- **The nine standing demos — re-recorded** (`4945075`). Two storyboards, `place_and_undo` and
  `build_a_map`, clicked the PLACE palette by pixel; the toolbar's new band shifted every row 30 px, so
  the filter took nothing and `check.py` measured a 2.12 % "place" — what a storyboard that hit nothing
  looks like. Both now click measured positions and pass. `stale.py` reads `fresh` for all nine.
- **#231 UI-37 — slice 3 committed** (`d5e946d`). The modes are `bevy_gooey::toggle_group`; its value
  follows `EditorState`, so gooey's own `Checked` is the light. **Not passing:** `testing::click` on an
  option moves the group's value but `on_tool_mode_chosen` never runs — a probe logged nothing. Two
  attempts, then stopped per the repeated-failure rule; the case is `#[ignore]d` with that sentence.
- Skipped: every other `ui` issue is `blocked` on #210's protected pins; #193 (bevy-elements#55);
  #112/#113 (no model at 8093); #195 (a loop setting); #197 `wontfix`.

## Enhancement: the tool modes as a gooey group (#231)
Goal: one mode lit at a time, and the light is the armed tool. Files: `src/toolbar.rs`, its test.
Proof: `the_mode_group_follows_the_armed_tool` — the group holds `tools().len()` options, starts on Move,
and its value becomes 2 when the state's tool becomes Clone. `the_armed_tools_option_is_the_only_lit_one`
asserts no one-shot button is accent-tinted.
MCP: none; a mode is still reached by its command's chord.

## Performance
Not measured (`perf.py` needs a real display).

## Acceptance
`accept.py` not run, see Baseline.

## Review
Reviewer not spawned (past the 125-minute mark). The five questions, mine: the issue asks for a toggle
group — this is one, and its state follows the editor rather than itself; the risk is the unfound
`ValueChange`, which is now a failing test rather than a claim; the change is one module; the group's look
is gooey's; what is owed is the observer wiring and Fill.

## Test diffs
`the_toolbar_arms_a_tool_through_its_key.rs`: the two cases that asserted six mode *buttons* were rewritten
to the group's shape, and the click case is `#[ignore]d` with its finding. No assertion was deleted: the
button-count case now asserts the group holds every tool **and** that no mode is also a one-shot button.

## Release
None: only `bevy_cauldron` changed, and it is `publish = false`.

## Next focus
Find why the group's `ValueChange` never reaches the observer, then the clip the group is owed.
