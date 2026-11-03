# 2026-11-02 ops: the snip is a view verb — migrated (#129, E-002)

Focus: #129 E-002 · Commits: d6e28ad..505dea7 · Machine: big · Outcome: shipped

## Summary
`Snip` was flagged in pass 259 as possibly misclassified, with a guess of `Camera`. It is neither: the key
is a **toggle** (arm to capture, press again to cancel), which is the `tool_arm` shape. Migrated as
`snip_arm` on `View::snip_armed`. **Ratchet 87 → 86.**

## Baseline
Gate: green (`d6e28ad`). `accept.py`: not run. Load 8.1, 150 GB free, 15 GB available.

## Enhancement: `snip_arm`
Goal: put the third view verb on the template. · Files: `ops_view.rs` (`View::snip_armed`), `snip.rs`
(dispatch), `ops.rs` (`snip_arm`, parity row), `keys.rs` (mapping).
Proof: `operations_have_two_front_ends` **9/9**; full editor suite **1154 passed, 0 failed**; clippy clean;
gate passed in 238 s.

Design points:
- `writes()` empty and `Undo::None`: the *file* a completed rectangle writes is `snip_commit`'s work, not
  this operation's. Arming a capture mode is not a change to the grimoire.
- A drag already in progress is cancelled by the key before dispatching — that is the key's own business,
  not a value an agent can hold.

## The parity scans, wrong in the same way again
Two scans name *the UI* as a list of files and had reached five (`editor`, `chooser`, `tiles`, `screen`,
`notice`). `snip.rs` owns this dispatch, so both reported a live entry point as missing — **the identical
false failure** `notice.rs` produced in pass 256, and `piece_import` before that. Both now read six. An
addition to the check's reach, not a loosening.

**Worth stating plainly:** I have guessed a verb's kind twice in two passes and been wrong twice. The
answer is one line of code away — `pressed` vs `just_pressed`, a toggle vs a write — so the next pass
should read the handler **before** classifying.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* The parity tests failed until the scans read `snip.rs`.
(2) *Is the failure the claim?* The claim is "both front ends dispatch one operation"; those tests are it.
(3) *Right layer?* The toggle lives on `View`, where the other two view verbs are. (4) *Protected files?*
One `UNMIGRATED` row removed, ceiling lowered by one, two scans widened. (5) *What remains?* 55 `Project`
verbs, and `FoldPack`/`Rescan`/`SuggestLabels` have handlers and are next.

## Test diffs
- `UNMIGRATED`: `Snip` removed with its comment; `UNMIGRATED_MAX` 87 → 86.
- Two UI-source scans widened from five files to six (`snip.rs` added).

## Release
None — `bevy_cauldron` is `publish = false`.

## Next focus
`FoldPack` and `Rescan`: read the handler first, then classify.
