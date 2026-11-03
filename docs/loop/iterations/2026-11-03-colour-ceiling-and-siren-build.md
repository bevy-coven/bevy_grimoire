# 2026-11-03 UI: the colour ceiling, and a feature that never compiled

Focus: UI-02 (#210) · Commits: 46b1875..HEAD · Machine: big · Outcome: shipped

## Summary

`#210` is finally **ready** — every `Needs` on it (gooey#6, #7, #33) is closed as completed, and so is
`#209`. Its slices 1–3 were already in the tree; slice 4 (28 constants, 551 product uses) cannot finish in
a pass, so this one shipped the ratchet that stops it growing. Along the way, the `bevy_siren` feature
turned out **never to have compiled**, which is why #274's clip is still owed.

## Baseline

Gate: green before (`46b1875`). `accept.py`: not run (no solver path touched). Disk 97 GB free,
available RAM 13 GB, load ~4.8.

**Environment defect fixed mid-pass, worth recording:** the build broke with
`clang: error: invalid linker name in argument '-fuse-ld=mold'`. `~/.cargo/config.toml` pins mold, clang
23 resolves `-fuse-ld=mold` by looking for **`ld.mold`**, and `~/.local/bin` had only `mold`. A symlink
`~/.local/bin/ld.mold -> mold` restored every build; no repo file, no `RUSTFLAGS`, no config edited.

## Bugs fixed

### #274: `--features bevy_siren` did not compile
Must-fix: blocks **y** (the panel, its tests and its clip), common **n**, no workaround **y**.
Root cause: `aea2456` declared `pub mod audio_panel;` twice (`lib.rs:53` unconditional, `lib.rs:110`
gated) — `E0428` — and added `crate::audio_panel::AudioPanelPlugin` from `editor.rs:2429` while nothing
defined it — `E0425`. Regression check: `cargo build -p bevy_cauldron --features bevy_siren` failed
before, finishes in 1m 55s after; `--test editor audio_panel` runs **4 passed**, which could not build at
all before.

### #279: corrected, not fixed
The gate's wall-clock bug. My earlier body claimed the reload "reads the file every frame", which is
false: `SETTINGS_POLL_SECS` and `SETTINGS_SETTLE_SECS` are 0.1 s each, so 200 ms is a floor. The
correction is on the issue. Not fixed because both remedies take the protected test (deleting its sleeps)
or need a test that fails on the wall clock first — my attempt at the latter passed on the wall clock too
and was reverted unshipped.

## Enhancement: the colour table only shrinks (#210, slice 4)

Goal: make the migration's direction one-way · Files:
`crates/bevy_cauldron/tests/the_colour_table_only_shrinks.rs` (new), `tests/all.rs` (one `mod`) · Proof:
fails when a 29th `pub const … : Color` is added (`declares 29 colour constants, ceiling 28`), passes at 28.

Measured: **28** constants, **551** uses in `src/` outside `chrome.rs`, **268** in `tests/`. The ceiling is
one-sided so every later slice lowers it and none has to negotiate with it.

MCP: none. Preview/undo: n/a.

## The clip that could not be recorded

`#274`'s AUDIO section is inside `chrome::scroll_list(_, MapControlsPane)` in the left INSPECTOR panel,
below the fold on both doors. Four attempts with `wheel` at four points did not move it (the palette in
the other dock did move on attempt 1). The status line does read `LAST bevy_siren set` in every run, so
the panel's state is right and only the scroll is missing. Stopped at two attempts at one step, as the
rule says; reported on #274 with the three honest fixes (a `wheel_at` naming a node, the section moved
above the fold, or keyboard focus). `tools/demo/storyboards/audio_panel.json` is committed so the next
pass starts from it.

One clip **was** recorded and measured, then deliberately not shipped: the `gooey_skin` storyboard on a
live editor shows the accent edit reaching the UI — blue pixels 10,341 → 57,850 after the blue save, then
green 0 → 45,900 after the green save. It duplicates `#147`'s existing `2026-11-02-gooey-skin` clip, so it
would have been a second clip for one behaviour. `stale.py` reports all eight standing demos fresh.

## Test diffs

- `crates/bevy_cauldron/tests/all.rs`: one `mod the_colour_table_only_shrinks;` (the standing exception).
- New test file. No existing assertion changed or removed.

## Review

Reviewer could not run (no subagent spawned inside the budget). Five questions, mine: (1) does the ratchet
fail when it should — yes, measured; (2) can it pass by reading nothing — no, an empty scan asserts; (3)
did I edit a protected file — no; (4) is the siren claim real — yes, the build and 4 tests; (5) what is
unfinished — 28 constants remain, and #274's clip is owed with the reason on the issue.

## Release

None: no publishable crate's files changed (`bevy_cauldron` is `publish = false`).

## Next focus

`#210` slice 5: migrate one constant group (the 12 tones are the natural first) and lower the ceiling.
