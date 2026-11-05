# 2026-11-05 the Projects home's "garbled flash": what was measured, and what the measurement overtook

Focus: the owner's report from Cauldron 0.2.5 on `mac_air` — pressing the card for `bevy_grimoire` on the
Projects home flashes something garbled before the project page paints. Machine: big. Outcome: the picture
path is fixed and pinned by a test; **the flash itself did not reproduce**, and the first explanation for it
was measured wrong and withdrawn rather than shipped as if it were the cause.

## The report

`~/Library/Application Support/Grimoire/feedback/feedback-1793901499397.json` on `mac_air`: version 1,
`audience: developers`, `screen: launcher`, `app_version: 0.2.5`, window 2560x1406 physical / 1280x703
logical, `scale_factor 2.0`, `ui_scale 2.4`. The pick is the card's own button, path
`Screen › Split › SplitPane › Surface(0) › … › Button`, `text: "bevy_grimoire"`, rect `[605, 228, 1046, 577]`.
The note: *"When I press this button, there is a garbled flash, before the UI paints correctly."*

This build's `read_reports` refuses a version-1 file by name, so the record was read straight off disk.

## What was built

`crates/bevy_cauldron/src/launcher.rs` gains `ProjectPictures`: **one decoded picture per (path, modified
time)**. `project_picture` used to read, decode and `images.add` on every call — once per card on the home,
once for the page's body, and again on every redraw — so a project's portrait was decoded and uploaded three
or more times for one unchanged file. Both callers now share the handle. `Art` carries the cache,
`fill_the_home`, `redraw_the_cards`, `fill_page` and `redraw` take `ResMut<ProjectPictures>`, and
`PageBody` became `pub` so a test can tell the page from the home.

`crates/bevy_cauldron/tests/the_project_page.rs` gains `the_first_frame_of_the_page_is_whole`, which drives a
real click on a real card and asserts the frame the page first exists: one `LauncherRoot`, a `PageBody` and a
map card, **every picture one the home already uploaded**, and every `SplitDivider` hidden.

## The measurement that overturned the plan

The plan had one expected branch: the card's picture is a texture uploaded the frame it is drawn, so the
first frame of the page samples a texture the GPU has not got. That reading is wrong for Bevy 0.20.

`StateTransition` runs **before** `Update` (`bevy_app-0.20.0/src/main_schedule.rs:32`), and asset extraction
runs after `PostUpdate`. An `Image` added while `OnEnter(Screen::Project)` builds the page is therefore
extracted and prepared in that same frame, before the frame draws. The `Assets::get(...).is_some()` check the
plan proposed is a tautology besides: `Assets::add` inserts into the main world at once
(`bevy_asset-0.20.0/src/assets.rs:401`), so it is `Some` the instant a picture is minted.

Captured on big under Xvfb at 15 fps, `grimoire/capture_sequence` with the click at 0.40 s, against a
**1 MiB 1024x1024** portrait — big enough that a late upload would be visible:

| build | first frame of the page | band? |
|---|---|---|
| base (`703a334`) | `f008` draws the map card's picture correctly | none |
| fixed | `f008` draws the map card's picture correctly | none |

Frame-for-frame pixel diffs over the whole capture are **identical** between the two builds. So the flash is
not this, and the cache is not its fix.

## What the cache is, then

A real waste and now gone: per screen, per redraw, one PNG decode and one GPU upload per call for a file that
had not changed. That is what the changelog line says, with `No clip:` and the reason — the frames prove
there is no visible difference, so claiming a clip would be a lie.

## The flash itself

Not reproduced on big under Xvfb, at 15 fps, with a 1 MiB portrait, on either build. What the frames do show
is a clean two-frame crossing (`f007` home, `f008` the page complete) with no empty window, no band, no wrong
glyphs and no visible divider. Two candidate differences from the owner's machine are worth naming rather
than guessing at:

- **`mac_air` is Metal on an M5; big under Xvfb is llvmpipe on CPU.** A driver-specific first-use cost cannot
  be reproduced here at all.
- **The owner's `ui_scale` was 2.4 with `scale_factor` 2.0** — a 2560x1406 surface. This run is 1080x720 at
  scale 1. A texture-size threshold that only bites at 2560 wide would not show here.

The honest next step is the same capture on `mac_air` against the 0.2.6 build with the report's own window,
which is the one configuration measured to show the defect. That needs a display session there, so it is
the owner's call, not a pass's.

## Verification run

| what | result |
|---|---|
| `the_first_frame_of_the_page_is_whole` on base `703a334` | **FAILS** — `frame 0: every picture on the page is one the home already uploaded — [AssetId{index: 17}]` |
| the same, with the fix | **PASS** |
| `the_project_page` (20 tests) | 20 passed |
| `the_launcher` (21 tests) | 21 passed |
| `cargo clippy -p bevy_cauldron --all-targets -- -D warnings` | clean |
| `changelog.py check` | consistent |

Frames: `/tmp/pp-frames/big/` (base, large texture), `/tmp/pp-frames/after2/` (fixed),
`/tmp/pp-frames/before2/` (base). The base binary used for the captures is `/tmp/bevy_cauldron_base`.

## A finding outside this item: text fields cannot paste

Asked mid-run and answered from the dependency graph, unchanged so far. `bevy_clipboard` reads and writes an
**in-process `String`** unless its `system_clipboard` feature is on, and `bevy`'s `default_platform` enables
`bevy_clipboard` but not that feature — `cargo tree -e features | grep -c system_clipboard` is **0** in this
workspace, and it is not declared anywhere in the manifests. `bevy_ui_widgets` does queue `TextEdit::Paste`
(`text_input.rs:177`, `:196`), so the chord arrives; `Clipboard::fetch_text` then hands back the empty
in-process string. Cauldron's own `arboard` (`crates/bevy_cauldron/Cargo.toml:152`) is used **write-only**, at
`launcher.rs:1003` and `notice.rs:370`, so nothing reads the system clipboard either. A fix is one feature
(`bevy/system_clipboard`) and belongs in its own commit with its own test.
