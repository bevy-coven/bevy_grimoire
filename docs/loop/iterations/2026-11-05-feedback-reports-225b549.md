# 2026-11-05 feedback reports: one gesture, two channels, and what running it found

Focus: the owner's brief — press a key, point at anything, say what is wrong — with two audiences
(the people who make Cauldron, and whoever works the open project). Commits: `225b549`..`909a5ce`.
Machine: big. Outcome: shipped; the gate passed on `909a5ce` in 217 s, after three refusals that each
found something real.

## What it is

`Cmd+D` (`Ctrl+D`) reports on **Cauldron itself**: every screen, the Projects home included, no
project needed. `Cmd+Shift+D` reports on **the project that is open**, and is refused in a sentence
where there is none. One mechanism (`crates/bevy_cauldron/src/report.rs`), parameterised by one value,
`bevy_grimoire_core::feedback::Audience`, which chooses the folder and is written into the record.
Two folders, two MCP readers, one gesture.

| | Developers | Project |
|---|---|---|
| chord | Cmd+D | Cmd+Shift+D |
| needs a project | no | yes |
| folder | `<data dir>/Grimoire/feedback/` | `<project>/target/feedback/` |
| tool | `grimoire_feedback_list` | `grimoire_project_feedback_list` |

A report is four files under one stem: the whole frame as it was at the key, the same frame with the
subject outlined, the record, and the log tail. A whole-screen report writes three: the second picture
would be the first.

## The record, and why it is in the engine-free crate

`bevy_grimoire_core::feedback` holds the data and the reader and imports no `bevy` — `tests/engine_free.rs`
is the ratchet. That is what lets the MCP server read a report without a renderer, and it is why the
picture encoding lives in the editor: only the editor has a framebuffer.

`subject.kind` is what an agent branches on: `element` (a node, its rectangle, and its ancestor path
with each step's name, gooey role, control id and text), `area` (a dragged box in surface pixels),
`screen`, or `scene` (the world point under the click, its map point, its cell, the placement
`pick_at` chose there, and the camera pose).

## Six defects the live run found, none of which a headless test could

Each is a test now. This is the part worth keeping: the headless suite was green before every one of
them, and every one of them made the feature unusable for a person.

1. **Save could not be clicked.** `report_gates_picking` turned picking off for the whole report, so the
   note box's own `Save` and `Cancel` — gooey buttons, which need picking — never received a click. A
   report could be opened, pointed and described, and then not saved. Fixed by gating on
   `ReportEdit::pointing()` (picking is off only while the person is still pointing), which is the
   inverse of what `claims_pointer` answers for the pointer verbs: the box holds the keyboard and a
   full-window scrim, so nothing behind it is reachable anyway.
2. **Every note began with a newline.** The `Enter` that chose the subject was still on the input queue
   when the box spawned, and the box carries `AutoFocus`. Fixed by recording the frame the subject was
   chosen on (`Open::noted_on`) and spawning the box a frame later.
3. **A closed box's scrim answered the next report's click.** Closing set the state to `None` and left
   the despawn to the next frame, so for one frame a full-window `Dialog` covered everything — the next
   report's click picked `Scrim` rather than the control under the pointer. Fixed by one close path that
   despawns in the same frame (`close_now` / `close_the_box`).
4. **Every unnamed step of a chain carried a descendant's words.** The text fallback searched the whole
   subtree, so the path from the home's `Screen` to a `Button` reported `Welcome to Cauldron` seven
   times and named nothing. Fixed by looking only at control-role children (`labels_of`).

5. **Two gestures wrote `PickingSettings::is_enabled`.** The snip and the report each claimed to be its
   only writer, so whichever ran last won — the first time both features existed,
   `a_snip_armed_over_a_button_does_not_press_it` said so. `picking.rs` is now the one writer; each
   gesture asks (`claims_pointer`, `pointing`) and drops its own presses in flight (B-032). A third
   gesture adds a term to `picking::claimed`, not a fourth writer.
6. **gooey's `ToastStack` answered for the whole window.** It is a full-window anchor that exists
   whether or not a toast is on it, so every report in the editor picked `ToastStack` instead of the
   control under the pointer. Excluded from the chain; a toast that is up is still picked, because its
   own node is inside the stack.

Three were harness facts, not defects: the platform modifier on Linux is Ctrl, not Super
(`keys::MOD_KEYS`); `bevy_fairy/input` types text with `{"kind": "Keyboard", "text": …}`, not one key
tap per character; and a sticky toast covers what it stands over, so a live script has to let it fade
before pointing at what was behind it.

## The gate refused twice, and both refusals found something

| refusal | what it found |
|---|---|
| 4 tests failed | the two-writer `PickingSettings` collision; `clamp_rect` clamping each corner against an edge it was already past; a test scratch folder named `bevy_grimoire_feedback_…` tripping `engine_free`'s scanner; and `Res<State<Screen>>` panicking in an app with no screens (`the_surface_and_the_camera_agree` builds the plugin set around a hand-made window), now `Option` |
| clippy | a field assignment after `Default::default()` in the new `picking` test — the crate's own clippy does not see test code, the gate's `--all-targets -- -D warnings` does |


**`clamp_rect` was the one defect the gate caught that the live run could not**: a drag off the bottom
of the window recorded a y of 4000 in a 720-pixel frame, and no picture would have shown it, because
`draw_outline` clamps when it draws.

## Proof

- `a_person_reports_to_the_developers` — 11 tests: the chord on the Projects home, a click picks the
  button under the pointer (path ends at `ButtonText`, an ancestor is `Button`), the wheel moves the pick
  outward, a drag is the box and `Enter` is the whole screen, saving headless is refused and writes
  nothing for both audiences, `Esc` writes nothing, the project chord needs a project, each chord opens
  its own channel in the editor, an open report keeps the census quiet (`R` does not arm removal), the
  overlay is above `MODAL_Z` and the box is not up while pointing, and a save takes the box away in the
  same frame.
- `a_person_reports_about_this_project` — 7 tests: a click on a piece is the scene with that piece's id,
  its cell and its world point; empty ground is the scene with no placement; a dialog over the map is
  interface, not the ground under it; the project channel names `target/feedback` and never
  `assets/grimoire`; no `BINDINGS` row claims `Cmd+D`.
- `bevy_grimoire_core` feedback — 8 tests: every subject kind round-trips, an unknown field is refused,
  the reader answers newest first, names a broken file and a record with no picture, a missing folder is
  no reports, and `last_lines` keeps its trailing newline.
- `bevy_cauldron_mcp` — 57 tests, including `docs_are_current` and `the_reference_names_the_servers_own_tools`.
  The count test's vocabulary stopped at `twenty`; completed to `twenty-nine` rather than softened.
- **Live, under Xvfb on big, with a real `bevy_cauldron` binary.** Both channels, on both screens:
  `Ctrl+D` on the Projects home, click the New project button, type, Save →
  `feedback-1793882344600.{png,log}` + `_subject.png` + `.json` in `<data dir>/Grimoire/feedback/`,
  the record's `subject.kind` `element` with an eleven-step path, 5,721 accent pixels in the outlined
  picture and 0 difference inside the rectangle. Then `Ctrl+Shift+D` in the editor on a project, a
  click on the map → `Picked: the map at -4.4, -1.1, cell (-5, -2)`, `kind: scene`, written under
  `<project>/target/feedback/`. Both MCP tools read them back over stdio: the project one answered with
  an `image` block and the record (paths filled in), the developers' one with `total: 0` before a report
  was filed there and the record after, and `grimoire_project_feedback_list` with no `--project`
  refused with its own sentence.

## Not done, deliberately

- **No clip.** The gesture is key-driven and there is no standing demo for it; the changelog line says
  so and names the tests. A clip is owed when a storyboard can press a chord and type a note — the
  demo harness types text and presses keys, so this is a storyboard, not new machinery.
- **`--no-clip` reason names two test files rather than one test**, which is what the entry can honestly
  say while the work is proved by seventeen.
- **The report's own text never reaches the `Status` line**, only a gooey toast. That is the launcher's
  idiom and the only one that exists on every screen; `EditorState` does not exist on the home.

## Assumed decisions, recorded here

1. **The chords are outside the key census.** `keys::BINDINGS` is what a *tab* does; these work on the
   Projects home, where there is no tab — the precedent is `command_palette::chords`. A test pins that no
   row claims `Cmd+D`.
2. **`ReportPlugin` is in the shared plugin list**, not `EditorPlugin`: the chord must work where no door
   is open.
3. **`ReportEdit` is `Ownership::Door`.** A report holds a frozen frame of the old door and entity ids
   from its layout, so it cannot survive the change it may be reporting on.
4. **A floating layer over the map is interface** before the map is the scene, so a tooltip, menu,
   popover, dialog or card over the viewport is still pickable.


## After: a design critique of the Projects home, and 0.2.5

The owner asked for a visual review of the first screen. Two isolated assessments (design review; source and
contrast measurement) against the **shipped 0.2.4 app's own smoke screenshot**, because the app cannot be
launched here: the GPU is held by a coding tenant (`gpu-tenant wait` timed out at 15 min twice) and no software
Vulkan driver is installed. The bundled detector returned `[]` — it scans web markup, and this UI is Bevy nodes.

Score **23/40**. Five findings fixed, one **withdrawn**:

| finding | outcome |
|---|---|
| P1 the first thing read on every launch was `Cauldron v0.2.4 is the newest release.` in the danger skin | **my own regression from `9c6cdf0`**, shipped in 0.2.4. `Updater::manual`: a pressed check answers, a launch check that finds nothing says nothing |
| P1 the screen never said what Cauldron is for | the pitch names Bevy 0.20 and the agent; three numbered steps; the tour is on the page |
| P1 "headline = body = 10 px, nav out-sizes the headline" | **withdrawn.** My first pass measured whole-line bands and was wrong. Single glyphs against the skin's `text_size: 11.0` give headline 15, prose 11, nav 11, footnote 10 — the type was already sane |
| P2 the hero is the loudest thing and the whitespace was not composed | top-aligned 560 px column; the crows stay the hero, as the owner asked |
| P2 the sidebar carried one fact twice and gave a tab and a network action one skin | one quiet `v0.2.5 · up to date` line; a button only when `actionable()`; an accent rail on the selected tab |
| P3 the footnote promised a choice the page did not have | it now says what `Open folder…` does not do |

**Two corrections the process caught, both worth more than the findings.**

1. **`no_internal_words_on_screen` refused my first copy.** It named `assets/grimoire`, and the ratchet is
   right: a person never types that path. The sentence now says the *property* — nothing to export — and the
   test asserts both the property's presence and the absence of any `assets/` path.
2. **`Take the tour` exists on two pages now**, by design (the welcome and `Learn`), so the launcher test's
   "exactly one button says this" helper had to be scoped. The duplicate is the finding, not a defect.

**Shipped as 0.2.5** (`166b00b`, gate 209 s; release run 42062946718 passed build, sign, notarize, its own MCP
smoke, and publish). Confirmed in the release's own 0.2.5 screenshot: no toast, `v0.2.5 · up to date` as one
line, the accent rail on `Projects`, the pitch, the three steps, `Take the tour`, and the new footnote.

**Not done:** the example's own thumbnail is not in the hero — the release build's project has no baked
thumbnails at first run, so a real screenshot of the `bedroom` map needs a bake step in the release, or a
committed still. That is the next slice if the owner wants the picture to say what the words now say.
