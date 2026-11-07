# 2026-11-07 feel series: G-51 button variants built in bevy_gooey, released as v0.40.2

Focus: epic #330 (owner's steer) · Commits: bevy_gooey b749025, release a2413c5 · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
UI-R01 and UI-R02 are closed, so the next ready work for UI-R03 (#333) is in bevy_gooey. This pass built G-51 (bevy-coven/bevy_gooey#58), the first of UI-R03's four open needs (G-51, G-57, G-58, G-59 and G-66), and released v0.40.2.

## Baseline
bevy_grimoire was not touched, so its gate and `accept.py` did not run.

## Enhancement: G-51
- **Change:** `src/button_variant.rs`.
  - **API:** `ButtonVariant` (nine variants) on a `button` or `icon_button`; `GooeySkin::variant_fill`, `variant_ink` and `warning`; `Loading(bool)`.
  - **Checks:** `problems()` and `high_contrast_problems()` hold every new variant's ink on its fill, and the fills' lightness steps.
  - **Loading:** swaps the label for a spinner, keeps the width, and takes no click.
- **Assumed decisions (on the issue):**
  - **`PrimaryButton` stays.** `primary_button` and `PrimaryButton` stay as the look of Submit, because the issue's clean cutover would delete the protected `tests/primary_button.rs`.
  - **No sheen.**
  - **Disabled keeps no hue.**
  - **Gallery still owed:** no display on big.
- **Proof:** `tests/button_variant.rs`, 7 tests.
  - **Fills:** every variant's three fills differ in every preset.
  - **Skin checks:** the skin passes its own checks, and a skin whose danger has no lift is refused by name.
  - **Drawn:** a running button draws its fill and ink at rest and held, and a changed variant restyles.
  - **Loading:** a loading button keeps its width, hides its label, shows one spinner and takes no click.
  - **Held look:** a held quiet button is seen to change on the panel.
  - **Host disabling:** a host-disabled button stays disabled after loading.
  - **Release:** a removed `Loading` releases the hold.
  - The module did not exist before this change.
- **bevy_gooey gate:** fmt, clippy -D warnings, nextest 1150/1150, doctests 254, wasm32 check, rustdoc -D warnings, all green.

## Borrowed from Mixar
None. The reference is Rune: `crates/rune-kit/src/actions.rs:27-39`.

## Performance
Not measured.

## Acceptance
`accept.py` not run: no bevy_grimoire code changed.

## Review
An independent `reviewer` ran: 6 P2 and 4 P3. All the P2s and P3s were fixed except the P3 that a label or icon added during a load stays visible, which is documented.
- A held Ghost, Outline, GhostMuted or GhostFaint button looked the same as rest.
  - **Cause:** a skin's `pressed()` is the panel itself.
  - **Fix:** the held fill is now the panel moved away from the text, with a test per preset.
- Outline drew no frame. It draws the skin's outline now.
- Three Loading-hold bugs are fixed:
  - a host-disabled button looked enabled after the load;
  - disabling during a load was undone when it ended;
  - removing `Loading` left the button stuck.
- The spinner's lit quarter was invisible on Submit. It is the button's ink now.
- `warning()` fell outside sRGB. Its chroma is pulled in until it fits.
- A restyle test could not fail. It does now.

The fixes were not re-reviewed. The gate and 7 tests cover them.

My new step checks also refused two existing presets (Desk light's plain-button hover step is 3.4 L*), which the gate caught. Those steps are now checked only where this change makes the fill.

## Test diffs
New `tests/button_variant.rs` and its `mod` line in `tests/all.rs`. No existing test changed.

## Release
bevy_gooey **v0.40.2**, https://crates.io/crates/bevy_gooey/0.40.2 (tag v0.40.2, commit a2413c5; `cargo info bevy_gooey@0.40.2` resolves). `cargo semver-checks check-release`: no semver update required, so PATCH. This repo's crates did not change.

## Next focus
G-53 hover groups (#60) and G-59 the dialog's default button (#66, needs G-51, now closed), then G-57, G-58, G-66 for UI-R03; UI-R04 needs G-53, G-54 (closed), G-55 (closed), G-56 (closed), G-57.
