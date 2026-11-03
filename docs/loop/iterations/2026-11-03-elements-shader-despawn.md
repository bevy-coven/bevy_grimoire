# 2026-11-03 elements: the editor with bevy_elements dies and is fixed

Focus: #282 / owed clips · Commits: 0161a96..96ac8ef · Machine: big · Outcome: fixes only

## Summary

Trying to record the `98-clock-scrub` clip turned up **three defects in `bevy_elements`**, two of them
regressions I introduced in `41407d6` last pass. Together they killed the editor with `EXIT=101` about
90 s into every run with the elements feature — which is why no clip needing an elements sky has ever
been recordable here. All three are fixed at the source and pinned; the editor now runs clean.

## Bugs fixed (in `bevy_coven/bevy_elements`, pinned by `96ac8ef`)

### 1. `src/noise.wesl` declared two functions twice (`0a285de`)

`perlin3_periodic` at lines 156 and 235, `fbm3_periodic` at 215 and 235, byte-identical — a rebase
duplicated both blocks. **WESL compiles when a pipeline is first used**, so `cargo build`, `cargo test`
and CI all stayed green while the editor logged `error: duplicate declaration of perlin3_periodic`
every frame. `2fee42a` (the pin before mine) has one of each; `41407d6` (mine) has two.

Caught from now on by `tests/no_shader_declares_a_name_twice.rs`, which names the file, the function and
both line numbers, and **skips `@if`/`@else` branches** — `terrain/render.wesl` uses that idiom
deliberately (`sand_colour` defined once per branch), so flagging it would be a false positive on the
crate's own design. Proved load-bearing: re-adding the duplicate fails it with
`noise.wesl: fn perlin3_periodic at lines [156, 440]`.

### 2. `redress_cameras` dressed a despawning camera (`94e36d9`)

`Changed<ElementsCamera>` includes the entity whose component was just **removed**, often while the
entity is despawning; `dress` then queues `insert`/`remove` at a dead entity, and Bevy's default handler
escalates that to a panic. Now dresses only cameras that still have the marker.

### 3. The `Msaa` restore could hit a despawned camera (`5d71dbd`)

The last of `undress_camera`'s commands to need it. `get_entity` only proves the camera was alive when
the command was *queued*; a despawn queued in the same frame lands first, so this insert is
`queue_silenced`.

**Verified live:** editor `--features bevy_elements` under Xvfb `:99`, project with `family.ron`
declaring the crate — **0 exits and 0 `Entity despawned` over 80 s**, where before it died every run
with `Entity despawned: The entity with ID 961v0 is invalid`.

## Progress on the clip (#282)

With the editor stable, the re-cut storyboard got further than any previous attempt: the wheel over the
panel content scrolled the ENVIRONMENT section into view and `click_label: "6:00"` **found and clicked
the button** — the first time that has worked. The run stopped at `click_label: "18:30"` with *"no
visible widget"*: the panel re-scrolled between clicks. That is the harness fix #282 already names —
ask the editor for the widget's position *and* the pane's scroll each time, not once at the start.

**`98-clock-scrub` keeps its `not yet recorded` reason.** One click landing is not a clip.

## Test diffs

- `bevy_elements/tests/no_shader_declares_a_name_twice.rs` (new) — the duplicate-declaration ratchet.
- `bevy_elements/tests/gpu_dressing.rs` — one new `#[ignore]`d test for the despawn path.
- Nothing in this repository's tests changed.

## Review

Reviewer could not run. Five questions, mine: (1) is the shader fix load-bearing — yes, re-adding the
duplicate fails the new test by name and line; (2) did I distinguish my regressions from pre-existing
defects — yes, `2fee42a` has one definition, `41407d6` two, so the duplication is mine; (3) is the
despawn fix verified live rather than only in a test — yes, 80 s with zero exits; (4) did I ship a clip I
could not show — no; (5) what remains — the per-click scroll in the harness, then the clip.

## Release

None here: nothing packaged changed (`bevy_cauldron` is `publish = false`). The crate fixes are on
`bevy_elements`' `main`; its version and release are that crate's own loop's business.

## Next focus

Make `record.py` re-query a widget's position and its pane's scroll before each click, then record
`98-clock-scrub` and close #282.
