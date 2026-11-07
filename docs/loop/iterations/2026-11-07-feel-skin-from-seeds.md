# 2026-11-07 feel series: G-47 a skin from three seeds, built in bevy_gooey

Focus: epic #330 (owner's steer) · Commits: bevy_gooey 39730eb · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
No bevy_grimoire item of the epic is ready (UI-R01 to UI-R06, #331 to #336, wait on bevy_gooey). This pass built G-47, a skin from three seeds (bevy-coven/bevy_gooey#54), which is one of UI-R02 (#332)'s four needs. The work was done in `~/family_src/bevy_emerge_loop/bevy_gooey`, and the issue was claimed first.

## Baseline
bevy_grimoire gate and `accept.py` were not run, because no bevy_grimoire code changed. Load at start: 19. Free disk: 60 GB (99% used).

## Enhancement: G-47 (bevy-coven/bevy_gooey#54)
- **Goal:** `GooeySkin::from_seed(PaletteSeed { background, text, accent })` derives a whole skin that passes `problems()`, or refuses the seed.
- **Files (bevy_gooey):**
  - `src/seed.rs` (new)
  - `src/skin.rs` (`SkinProblem::TranslucentSeed`, a breaking addition to an exhaustive enum)
  - `src/lib.rs`
  - `tests/seed.rs` (new) and its `mod` line
  - `docs/research/skin-from-seeds.md`
  - `docs/guide/skins.md`, `README.md`, `AGENTS.md`, `CHANGELOG.md`
- **What it borrows:** Rune's `palette.rs` at `f73f72c` supplies the ladder offsets, the dark floor, the hue rule, the muted-text mix, the ink alphas and the blue focus ring, measured from source. Where gooey's checks ask for more, each colour is solved for its floor instead (see the research note).
- **Where this crate's rules won:** of Rune's four gallery seeds, three pass (dark, black, warm). The light seed is refused: its `#e5484d` accent carries words that neither black nor white reads at 4.5:1 across a primary button's rest, hover and press fills. The issue's footer says gooey's WCAG checks win over the reference, so the test pins that refusal. A white paper with a blue accent covers the light case instead.
- **Proof:** `tests/seed.rs` has seven tests: the ladders, the derived fills, a warm hue kept, grey-on-grey refused naming `text on surface`, Rune's light seed refused for its accent words, a translucent or NaN-alpha seed refused, and a saturated ladder that may never step back toward the background. `PaletteSeed` and `from_seed` did not exist before, so the file does not compile on `origin/main` 2b4dd73.
- **bevy_gooey gate:** fmt, clippy `--all-targets --all-features -D warnings`, nextest 1064/1064, doctests 239, wasm check and rustdoc `-D warnings`, all green.

## Borrowed from Mixar
None (the reference is Rune).

## Performance
Not measured: this is a constructor, and nothing runs per frame.

## Acceptance
`accept.py` was not run, because no bevy_grimoire code changed.

## Review
The reviewer subagent ran. It ported the derivation to Python to check it, and its numbers matched the patch. All seven findings are fixed:
1. **Ladder (bug):** a saturated seed (`#ffffff`, `#004d00`, `#1d4ed8`) produced a ladder that stepped back toward the background, and `problems()`, which compares only the size of a step, passed it. Each step now starts no nearer the background than the one below, the direction is checked, and a short step is returned as `ElevationStep`. A test covers that seed.
2. **NaN alpha (bug):** a seed with a NaN alpha passed the opacity check. The check is now `!= 1.0`, and a test covers it.
3. **Focus ring fallback (risk):** when no blue reaches 3:1 on every ground, each ground it misses is now reported as a problem.
4. **Danger:** a dark seed now takes `tool()`'s danger.
5. **Docs:** the accent-words rule, the danger reason, the refusal of Rune's light seed in the research note, and the guide wording now match the code.
6. **Tests:** the vacuous assertions are gone, and the focus ring is now checked on the background and every step.

## Test diffs
bevy_gooey: one new file, `tests/seed.rs`, and its `mod seed;` line in `tests/all.rs`. No existing test changed.

## Release
None here: nothing in bevy_grimoire's packages changed. bevy_gooey's release is its own loop's.

## Next focus
G-48 (elevation as data) or G-52 (text on the device-pixel grid) for UI-R02. After those, G-49 (the Desk preset, which needs G-47 and G-48).
