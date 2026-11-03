# 2026-11-03 elements: a blocked bug unblocked at the source

Focus: #193 (blocked bug) · Commits: 591a53b..4ab32c6 · Machine: big · Outcome: fixes only

## Summary

Every open issue in this repo was blocked or unclosable, and four passes had mined one unclosable
mega-issue (#210) instead of saying so. This pass took the actual blocker — `bevy_elements#55`, which
`#193` had been waiting on since 2026-11-02 — and fixed it in the crate that owns it, proved it with a
failing-first test, and moved this repo's pin onto the fix.

## Why the family crate was built here

`docs/loop/focus.md` lists `bevy_elements` as having its own loop on big_mac, which would have meant
leaving it alone. **That loop is not running**: the crate's last commit was 2026-11-01, its last issue
activity 2026-10-31, and #55 had been open and unclaimed since. The owner's instruction is not to leave a
blocked issue alone, so the two-way street's "not listed → build it at the source" applies to a listed
loop that has stopped. `focus.md` should be corrected by whoever owns it.

## Bugs fixed: bevy_elements#55 (unblocks #193)

Must-fix: blocks **y** (six owed clips and #193's step 3), common **n**, no workaround **y**.

**Root cause.** `ElementsDressing` recorded what the crate inserted on a camera, but nothing observed
`Remove<ElementsCamera>` (`grep -n 'Remove<ElementsCamera>' src/` → 0 hits). A camera stayed HDR with
`Msaa::Off`, the atmosphere and its environment map, the depth prepass, both fogs and a celestial-driven
`AmbientLight`. Deleting a project's `family/bevy_elements.ron` therefore changed nothing on screen, and
only a restart brought the plain view back.

**Fix** (`bevy-coven/bevy_elements@41407d6`): the existing `toggle` read backwards. Each component comes
off only when the marker says the crate put it there, so a component the *game* set is never removed;
`Msaa` returns to its default; `SkyBackground` and the marker go with the rest. `Hdr` arrives through
`#[require(Hdr)]`, so it stays the caller's to remove — as the issue asked.

**Regression check, written first.** `tests/gpu_dressing.rs::removing_the_camera_undoes_the_dressing`.
Without the observer: *"AtmosphereSettings must come off the camera when ElementsCamera does, or a host
cannot go back to its own look"*. With it: passes, and the other three dressing tests still pass (4/4).
Clippy `-D warnings` clean.

**Taken here.** Pin moved `2fee42a → 41407d6`; `cargo check -p bevy_cauldron --features bevy_elements`
green. `#193`'s `blocked` label removed and the evidence posted on the issue.

## What is deliberately not done

**#193 is not closed.** Its step 3 (removing the file brings the plain view back) is what the crate fix
addresses, but nobody has re-run the original Vulkan repro end to end on this build. Closing on a build
that was never run on Vulkan is exactly the claim this loop is meant to avoid. A pass with a display
re-runs #193's three steps and closes it with that evidence; the six owed clips then become recordable.

## Also in this pass (before the unblocking work)

`fix(ui)`: `the_colour_table_only_shrinks.rs` accepted any non-alphanumeric left boundary, so
`crate::chrome::text::LABEL` — a *font size* role sharing a name with the colour `LABEL` — counted as
paint: 46 uses, ceiling 518 against a true 403. `:` is now excluded on the left and the ceiling is set to
what the scan measures. Slice 5 also moved five `TextColor(LABEL)` sites in `rebuild_detail` onto the
running skin's `text_muted`. Four sites in other functions were reverted rather than half-converted.

## Test diffs

- `crates/bevy_cauldron/tests/the_colour_table_only_shrinks.rs`: the scan's left boundary excludes `:`.
  This *lowers* the measured count; no assertion is removed, loosened or re-pinned.
- `bevy_elements/tests/gpu_dressing.rs`: one new `#[ignore]`d GPU test, appended. No existing assertion
  changed.

## Review

Reviewer could not run (no subagent spawned inside the budget). Five questions, mine: (1) does the crate
test fail without the fix — yes, measured, quoted above; (2) does the undress touch a game's own
component — no, each removal is gated on the marker, and the test asserts an untouched camera is
untouched; (3) is the pin the commit that contains the fix — `41407d6`, the pushed `main` head;
(4) did I close an issue I did not verify — no, #193 stays open with the reason stated; (5) what remains —
#193's Vulkan re-run and six owed clips.

## Release

None from this repo: nothing packaged changed here (`bevy_cauldron` is `publish = false`). The crate fix
is on `bevy_elements`' `main` and its version/release is that crate's own loop's business, per the
two-way street.

## Next focus

A pass with a display re-runs #193's three steps on Vulkan and closes it, then records the six owed clips.
