# 2026-11-07 ui: a toast stands under the menu bar (#305)

Focus: #208's open bug issues, oldest first · Commits: `e222ef63` · Machine: big · Outcome: shipped

## Summary
The tour's offer toast covered the menu bar's titles. `bevy_gooey`'s toast stack is `TopCenter` at the
window's top and honours `ToastSettings::inset`; the editor set the anchor and never an inset. One
system now reads the bar's own laid-out height into the inset. Bug issue #305 closed; no other issue
was ready this pass (the `feel` series' UI-R01..R06 all wait on `bevy_gooey`, and every ready-looking
`ui` item is either claimed or needs a slice first).

## Baseline
Gate: green on `f4697da2`. Load 9.8, 57 GB free (99% used), 18 GB available RAM. `accept.py` not run:
no import path changed.

## Bugs fixed
### #305: the tour offer toast draws over the menu bar
Must-fix: blocks nothing, common (every toast takes this path on a screen with no editor host), no
workaround (the bar's menus are unreachable while a toast covers them) — two of three.
- **Root cause:** the editor's *own* toast host (`chrome::problem_toast`) hangs inside the viewport,
  which already starts below the bar, so refusals were fine. A toast raised with no host — the tour's
  offer, spawned on `OnEnter(Screen::Editor)` before the frame's first pass — makes gooey spawn its own
  stack at the window's root, and `ToastSettings::inset` was `default()`: 0 on every side.
- **Measured before:** bar `0..28.33` logical px, tour toast `9.17..55.83`. After: the test's two cases
  pass, so the toast's top is at or below `28.33`.
- **Regression check:** `a_toast_never_stands_over_the_menu_bar::the_tours_offer_does_not_cover_the_menu_bar`
  failed on `f4697da2` (`the toast "New here? Take the 2-minute tour" covers the menu bar`) and passes on
  `e222ef63`; `a_refusals_toast_does_not_cover_the_menu_bar` passed both ways, which is what says the
  fix did not move the host that was already right.

## The fix
`chrome::keep_toasts_under_the_menu_bar` (ungated by screen, after `FrameSystems`) reads
`ComputedNode::size().y * inverse_scale_factor()` of `menu_bar::EditorMenuBar` and writes
`ToastInset::top(that)`. The bar's height comes from the skin's type size and padding, so a text-scale or
skin change moves the inset with it; the write is compared first, so a settled editor costs a comparison.

**Pin:** `bevy_gooey` moves `1cc4c7d` → `c6b714c` (v0.36.0), the revision whose `ToastInset` is the widget
this needs (bevy-coven/bevy_gooey#43, closed as completed). No other gooey API this crate uses changed.

## Proven
- `a_toast_never_stands_over_the_menu_bar` — 2 tests (the tour's offer, a refusal).
- The toast, menu and tour suites beside them: 62 tests, 62 passed.
- `cargo clippy -p bevy_cauldron --all-targets -- -D warnings`: clean.
- The full gate: the first push (see the log line for its result).

## Clip
`--no-clip`: a toast moved 28 px down and the screen is otherwise identical; a 20 s clip of a toast
appearing would say less than the two rects above. Recorded in `CHANGELOG.md` with the tests named.

## Test diffs
None to an existing test. One new file, its one `mod` line in `all.rs`.

## Release
`bevy_cauldron` is `publish = false`, so nothing packaged changed and nothing is published. The
`release:` bump follows `tools/loop/version.py next` when a publishable crate changes.

## Next focus
The oldest open bug that is reproducible: #327 (a settings edit during a capture sequence does not reach
the terminal's text size) or #322 (two gate-flaky tests, which needs an owner's word on the assertions).
