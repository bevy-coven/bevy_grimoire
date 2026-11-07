# 2026-11-06 feel series: G-55 double press built at the source in bevy_gooey

Focus: the feel series, epic #330 (owner's steer for this pass: "focus only epic bevy_grimoire#330, complete one issue, push, publish") · Commits: bevy_gooey 34eec48 (evaluator), f9b125e (G-55), merge c5b3b5a · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
Every bevy_grimoire item of the epic (UI-R01 #331 to UI-R06 #336) waits on bevy_gooey issues filed an hour earlier, so none is ready here. The pass built one of those gooey issues at the source instead: G-55, double press and click counts (bevy-coven/bevy_gooey#62), one of the five UI-R04 (#334) needs. The gooey loop's pass that was running at the same time took the motion items (G-39 onward), so #62 was claimed with `worker:bevy_emerge_loop` and a comment before any code.

## Baseline
bevy_grimoire gate: not run (nothing in this repo's code changed). `accept.py`: not run, same reason. Load 0.1, free disk 112 GB (97% used), available RAM 18 GB.

## Enhancement: G-55 (bevy-coven/bevy_gooey#62)
Goal: a desktop's double click in gooey, so the editor's rows, cards and splits open on the same rule. Files (bevy_gooey): `src/clicks.rs` (new), `src/lib.rs`, `src/split.rs`, `src/table.rs`, `src/widgets.rs`, `tests/double_press.rs` (new), `tests/all.rs` (its one `mod` line), `docs/research/double-press.md`, `docs/guide/pointer.md`, `README.md`, `AGENTS.md`, `CHANGELOG.md`.
- `PointerSettings { double_click_seconds: 0.5, double_click_slop_px: 4.0 }`, `DoublePress { entity }` (bubbles like the click) and `ClickCount(u8)`, kept by five global picking observers on `Time<Real>`; the rules were measured from Rune's runtime (`runtime/input/pointer.rs`, `press_clicks.rs` at `f73f72c`), not copied.
- Clean cutover: the split divider and the table's rows (so the tree's) open on `DoublePress`; nothing reads Bevy's `PointerClick::count` any more.
- Proof: `double_press::a_table_row_opens_on_a_double_press_and_not_on_two_clicks_apart` failed before the cutover (`5 px apart opens nothing`, tests/double_press.rs:355: Bevy's count does not look at the place) and passes after; the issue's acceptance (0.3 s and 3 px: one `DoublePress`, two clicks, the first not delayed; 0.6 s or 5 px: none; three quick clicks: one; a held press then a click: none; Enter twice: none) is the other seven tests in that file. The existing split, table and tree double-click tests pass unchanged (89 of 89 in those four modules).
- bevy_gooey's own gate (fmt, clippy `--all-targets --all-features -D warnings`, `cargo nextest run --all-features --lib --tests`, doctests, wasm check, rustdoc `-D warnings`): green on the merged tree (nextest 1051/1051, doctests 238). The first run went red on `access::a_node_declares_exactly_the_actions_its_widget_takes`. The failure also happens on `main` with any two extra resources: line 757 took the first `TextInputOf` entity, and a text field and its hint both carry one, so the archetype order decides which comes first. Filed as bevy-coven/bevy_gooey#74. The owner approved the one-line fix in conversation (2026-11-07), and it landed as its own `evaluator:` commit (34eec48, `With<TextInput>` added to the filter, no assertion changed).

## Borrowed from Mixar
none (the reference is Rune).

## Performance
not measured: no per-frame system was added (observers run only on pointer events).

## Acceptance
`accept.py` not run: no bevy_grimoire code changed.

## Review
The reviewer subagent ran and returned seven findings. All are fixed: a double press on a tree chevron also opened the row (it now stops at the chevron, and a test checks it); `ClickCount` stuck after a release over nothing (it is now let go on drag end, tested); a press reported on several entities never paired (release now matches the held entity); stale docs about divider drags; a stale doc comment; test gaps (tree row, drag count 2); and per-pointer state that never pruned (now pruned on press and removed on cancel).

## Test diffs
bevy_gooey: one new file, `tests/double_press.rs`, and its `mod double_press;` line in `tests/all.rs` (gooey's own rule for a new test file). No existing test changed.

## Release
None here: nothing in bevy_grimoire's packages changed. bevy_gooey's version and release stay with bevy_gooey's own loop (the two-way street, step 4); its next release carries c5b3b5a (it released v0.38.0 at 36ec345 while this pass ran).

## Next focus
The epic again: the next ready gooey item with no dependency the gooey loop has not taken (G-47, G-48, G-52, G-56; G-39 to G-41 were closed by the gooey loop during this pass), until UI-R04 #334's Needs are closed and it can take the gooey pin.
