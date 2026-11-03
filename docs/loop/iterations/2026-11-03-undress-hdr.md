# 2026-11-03 environment: #193 closed — the world comes back when the settings file goes

Focus: #193 (bug) · Commits: c880416..b68be59 · Machine: big · Outcome: shipped

## Summary

Last pass fixed the crate half of #193 (`bevy_elements#55`) and left the issue open because nobody had
re-run its repro on Vulkan. This pass ran it end to end, found the editor's own half still broken, fixed
it, measured the fix, and **closed #193** with the evidence its acceptance asked for.

## Baseline

Gate: green before (`c880416`). `stale.py`: all demo GIFs and `editor.png` fresh. Editor built
`--features bevy_elements,bevy_flora` under Xvfb `:99`.

## Bug: #193 step 3 — the world never came back

Must-fix: blocks **y** (six owed clips and every Linux session with an elements file), common **y**
(any project with `family/bevy_elements.ron`), no workaround **y** (only a restart).

**Reproduced first.** The issue's own three steps, measured on the viewport (mean RGB, std, 120×320 patch):

| step | status line | mean | std |
|---|---|---|---|
| 1. no settings file | — | (132,128,121) | 100.0 |
| 2. file written | `bevy_elements settings reloaded` | (149,157,172) | 43.6 |
| 3. file deleted, before | `… removed; its defaults apply` | (19,18,15) | **1.7** |
| 3. file deleted, after | `… removed; its defaults apply` | (97,92,90) | **76.5** |

Std 1.7 is one flat colour, and it was unchanged ten seconds later — not a slow transition. Steps 1 and 2
were healthy all along, which is why the issue's own [INFERENCE] pointed at the undress path.

**Root cause, the editor's half.** `ElementsCamera` arrives with `#[require(Hdr)]`. `bevy_elements#55`
says explicitly that `Hdr` is *"left to the caller (bevy_grimoire removes it itself)"* — and nothing here
ever did: `grep -n 'Hdr' crates/bevy_cauldron/src/*.rs` had no non-comment hit. Removing the marker undid
the crate's dressing but left a camera in **HDR**, the mode this editor's composition does not support.
`dress_camera` now removes it with the marker (`b68be59`).

**Regression check, written first.** `tests/undressing_gives_the_editors_view_back.rs`, feature-gated like
its sibling. Deleting the one `remove::<bevy::camera::Hdr>()` line fails it with *"the editor must remove
the Hdr that ElementsCamera required; leaving it is what drew one flat colour for the rest of the
session"*; restored, it passes; clippy `-D warnings` clean with `--features bevy_elements`.

**Closed with evidence.** The frame after the delete now shows the bed, sofa, bookcase, chairs and the
map's box and grid (`/tmp/r193_fixed.png`, described in the closing comment). #194, the duplicate, was
already closed.

## Test diffs

- `crates/bevy_cauldron/tests/undressing_gives_the_editors_view_back.rs` (new), one `mod` line in
  `tests/all.rs` (standing exception). No existing assertion changed.

## Review

Reviewer could not run (no subagent inside the budget). Five questions, mine: (1) does the test fail
without the fix — yes, measured; (2) is the live claim real — yes, four measured states, and I looked at
the frame; (3) did I close on evidence — yes, step 3 re-run on Vulkan with the file deleted; (4) is the
`Hdr` removal safe for a camera that had it before — it is the editor's own `MainCamera`, which the
editor creates, so no host camera is affected; (5) what remains — the six owed clips are now recordable
but not recorded.

## Release

None: nothing packaged changed (`bevy_cauldron` is `publish = false`).

## Next focus

Record the six clips #193 was blocking — `f4-cover-settings` first, since it demonstrates this fix.
