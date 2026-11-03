# 2026-11-03 shell: a pointer over a dock targets the window

Focus: #224 UI-30, the drag's real cause · Commits: e0d1e2c..35d74cc · Machine: big · Outcome: fixes only

## Summary
Found the cause pass 273 spent three attempts failing to identify: `retarget_pointer` put **every**
pointer on the surface *image*, so the UI picker — which keeps a pointer only for cameras whose
`RenderTarget` matches — never reached the split's divider. A pointer over the frame's furniture now
names the window. The drag itself still does not move the dock; that is the next step, and it is now a
much smaller question.

## The bug (35d74cc)
Must-fix: **blocks** #224's second acceptance item (a clip of the docks resized), **common** (no dock
can be resized, by a person or by `tools/demo/record.py`), **no workaround** (there is no other way to
press the divider).

Root cause: `surface::retarget_pointer` writes `PointerLocation { target: Image(surface) }` for every
pointer, which is right over the viewport (the map's picking reads it there) and wrong over a dock:
`bevy_ui`'s backend keeps a pointer only for cameras whose `RenderTarget` equals the pointer's own, so
an image-targeted pointer never reaches a window-space node.

**Measured, twice** — pass 273 and again here — on the fixture project: the pointer sat at the
divider's own centre (218.5, 398.0) and its target was `Image(..)`. Pass 273 believed `inject_clicks`
was at fault and reverted that change three times.

The fix asks the one thing that already knows the boundary: **the viewport slot's computed rect**. Over
it the pointer stays on the image; anywhere else in the frame it names the window through `WindowRef`'s
own `normalize` — the call `mouse_pick_events` makes on a cursor event — rather than a second way to say
"the primary window".

**`view::OverUi` was tried first and cannot work here**, which is worth recording: it iterates `Hovered`
nodes, and the frame's furniture carries none by design (`spawn_frame`'s header), so it answers false
over the divider by construction.

Regression check: `tests/an_injected_pointer_over_a_dock_is_pickable.rs` — failed with that same
`Image(..)` target, passes now, and asserts the other half too (a pointer over the viewport stays on the
image, or the map would stop taking clicks). **Editor suite 1166 passed, 0 failed**, including
`a_shift_click_that_picks_no_piece_says_what_to_do_instead`, the test pass 273 could not make pass.

## What this does not yet do
The recorded drag still leaves `SplitFraction` at 0.22 and the docks where they were. Two measurements
say why the next attempt starts elsewhere: 169 entities carry `Hovered` and **the divider is not one of
them**, so the picker is still not reaching it even with the window target. That is the question the
next pass opens with — whether `bevy_ui`'s backend has run for this pointer at all, which is a
different question from where the pointer points.

Stopped after two attempts at the live drag, per the rule, rather than guessing a third time.

## Review
Reviewer not spawned; self-review, mine:
- **Is the fix narrow?** Yes: one branch in one system, keyed on a rect that already existed.
- **Could it break the map?** The suite says no, and the test asserts the viewport half explicitly.
- **Why is `OverUi` not used?** Measured: the frame's furniture has no `Hovered`, so it answers false.
- **Is the changelog honest?** It says "so the shell's UI picker can see it" and no more.
- **Is the failed drag logged as failed?** Yes, with the measurement that narrows it.

## Test diffs
One new test file and its `mod` line. No existing assertion changed.

## Release
None: no packaged crate changed.

## Next focus
Why the picker does not hover the divider even with a window target — start from
`bevy_ui`'s backend and whether it runs for a pointer that arrived through `retarget_pointer`.
