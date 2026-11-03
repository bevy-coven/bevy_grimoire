# 2026-11-03 ui: the wheel scrolls a panel (#283)

Focus: #283 · Commits: 2b5f2a0..23755a0 · Machine: big · Outcome: shipped

## Summary

Chasing last pass's owed clip turned up a real UI defect and fixed it: **no panel in this editor answered
the mouse wheel**, because `bevy_ui_widgets::ScrollAreaPlugin` was never installed. Every scrollable box
drew a scrollbar and clipped its content, and everything below `EDGES` in the INSPECTOR was unreachable —
to a person as much as to a storyboard. One plugin, and the wheel works.

## Baseline

Gate: green before (`2b5f2a0`). `stale.py`: all demos fresh. Editor `--features bevy_elements` under Xvfb
`:99`, 1280×720.

## Bug: #283 — the INSPECTOR's content runs past the window and did not scroll

Must-fix: blocks **y** (two owed clips and any section below `EDGES`), common **y** (every session at this
window size), no workaround **n** (sections are still reachable if the window is tall enough).

**Measured before the fix,** asking the running editor where each `Text` sits:

| node | y |
|---|---|
| `INSPECTOR` | 107 |
| `EDGES` | 564 |
| `ENVIRONMENT` | **898** — past the 720 px window |
| `6:00` … `23:00` | ~927 |

Ten wheel notches over the panel left `ENVIRONMENT` at `898.5`. The panel is correctly bounded (no node in
the tree is taller than the window), so the content overflowed inside a box that never listened.

**Root cause.** `grep -rn 'UiWidgetsPlugins\|ScrollAreaPlugin' src/` → nothing. `DefaultPlugins` does not
include `bevy_ui_widgets`' plugin group, and the editor never added the one plugin whose observers turn a
wheel event into a `ScrollPosition`.

**Fix** (`23755a0`): `ui::ScrollWheelPlugin`, added by `EditorUiPlugin` — the one place the editor's UI
library is installed (UI-01), guarded so a second add is a no-op.

**Live proof after the fix**, wheel at `(120, 300)` over the inspector: `EDGES` 564 → **−355.5**,
`ENVIRONMENT` 898 → **−21.5**.

**Regression check, failing first.** `tests/the_wheel_scrolls_a_panel.rs` could not compile before the
plugin existed (`cannot find value ScrollWheelPlugin in module bevy_cauldron::ui`) and passes now. Clippy
`-D warnings` clean.

## Still open on #283

The inspector's scroll **lane** is narrow: a wheel at `(215, 400)` — over the scrollbar gutter rather than
the content — scrolled the palette in the *right* dock, and the sky went dark as a side effect. So the
wheel is live but a wheel near the panel's right edge reaches the wrong pane. Reported, not guessed at.

## Also corrected: #282

Last pass's #282 said the sky does not follow the clock. **It does**: setting the hour *through the file*
gives sky `(128,157,194)` at 12:00 and `(27,30,35)` at 23:00. The defect was reach — the clock buttons sat
below the fold and no storyboard could get to them — which #283's fix removes. #282 now carries that
shape: re-cut `clock_scrub.json` to wheel at the panel content, and if `click_label: "6:00"` then finds its
button the issue is a stale storyboard and closes with its clip.

## Test diffs

- `crates/bevy_cauldron/tests/the_wheel_scrolls_a_panel.rs` (new), one `mod` line in `tests/all.rs`
  (standing exception). No existing assertion changed.

## Review

Reviewer could not run. Five questions, mine: (1) does the test fail without the fix — it did not compile,
which is the strongest form of failing; (2) is the live claim real — yes, two node positions before and
after in one session; (3) is the plugin safe to add twice — guarded, and `is_plugin_added` is checked;
(4) did I claim more than I proved — no, the gutter hit-test problem is reported as still open; (5) what
remains — the gutter, and re-cutting `clock_scrub.json`.

## Release

None: nothing packaged changed (`bevy_cauldron` is `publish = false`).

## Next focus

Re-cut `clock_scrub.json` with a wheel over the panel content, then record `98-clock-scrub` and close #282.
