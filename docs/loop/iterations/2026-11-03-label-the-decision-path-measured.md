# 2026-11-03 label: kit labelling on closed questions, measured equal to the old path

Focus: #113 D5, all three acceptance items · Commits: 2715ecf..3c555c8 · Machine: big · Outcome: shipped

## Summary
Moved kit labelling onto closed `choice` questions over the project's own vocabulary, made the path a
config choice, and measured both paths on the same 12-piece labelled kit: **92% each**. D5's rule — the
old path goes only when the new one is equal or better — now permits its removal. That removal (2483
lines, 39 call sites in five modules) is the next pass's work, not the last ten minutes of this one.

## Baseline
Gate: green at 2715ecf. `accept.py`: not run (no behaviour claim this pass). Demos: all nine fresh.
Load 8.4, free disk 147 GB, available RAM 13 GB.

## Acceptance items

**1. On the same labelled kit, both paths' agreement is measured and recorded, and a test reads it.**
`fixtures/label_kit.ron` (12 real pieces, each labelled with the project's own `kind` reading from
`assets/grimoire/library_from_manifests.ron`), `examples/label_compare.rs`, and
`fixtures/label_comparison.ron`: **decide 11/12 (92%), vlm 11/12 (92%)**, `clef-flash`, 2026-11-03.
`tests/the_two_labelling_paths_are_compared.rs` reads it and refuses a record whose counts do not add up,
whose set is not the committed one, or that shows the new path behind.

**2. Both paths exist behind config, and the test selects the path.** `label_decide::Path` reads
`GRIMOIRE_LABEL_PATH`: unset is `decide`, `vlm` is the old path, and any other value is refused, naming
the bad value and both paths. A typo that quietly keeps the old path is how a migration stalls.

**3. A vocabulary token the project lacks can never be suggested.** `request_for` builds the options *from*
the vocabulary, so the request has no room for another word; `suggestion_from` still refuses one that
arrives anyway (a stale cache, a fixture, a misbehaving server) because a suggestion that reaches a
descriptor is a piece no rule can match.

## What the `vlm` number is, and is not
A decision endpoint cannot be asked for free-form text, so the run cannot reproduce `vlm.rs`'s prompt. It
asks both paths the same closed question over the same options and compares the decisions, which makes
`vlm`'s 92% a **ceiling** — the real path also pays a reprompt for every token the vocabulary rejects. It
is enough for D5's rule and not enough to claim 92% for the old path. Said in the example, in the test,
and here.

## Bugs found by running it
`request_for` sent empty `instructions`, and the server refuses the whole request with *"the
instructions and the options of a question must not be empty"* — every decision request failed until the
axis's own question was carried. Caught only because the run was made rather than the code assumed.

## Review
Reviewer not spawned; self-review, mine:
- **Is the vocabulary guard load-bearing?** Deleting it lets `{"kind": "throne"}` through, and the test
  goes red with exactly that value.
- **Does the config test prove selection?** It covers unset, empty, both names and a typo, and the typo case
  asserts the refusal names both the value and the variable.
- **Is the comparison honest?** Its limit is written where the number is made; the test also asserts the new
  path is not behind, so a worse measurement cannot quietly ship as a removal.
- **Did I delete anything early?** No: `vlm.rs` is untouched, because its removal is a pass of its own.
- **Any protected file changed?** No.

## Test diffs
Two new test files and their two `mod` lines in `all.rs`. One new fixture set. No existing assertion changed.

## Release
`bevy_grimoire_core` gained `Vocabularies::axis`, which the labeller uses, so three crates went out in
dependency order from a green local gate:

- **bevy_grimoire_core v0.7.1** — https://crates.io/crates/bevy_grimoire_core/0.7.1 (an addition;
  semver-checks: 202 checks pass, no semver update required, so PATCH)
- **bevy_grimoire v0.4.3** — https://crates.io/crates/bevy_grimoire/0.4.3 (API unchanged; its published
  `Cargo.toml` now requires core 0.7.1)
- **bevy_grimoire_anim v0.1.8** — https://crates.io/crates/bevy_grimoire_anim/0.1.8 (same)

`bevy_cauldron` is `publish = false`.

## Next focus
Remove `vlm.rs`: 2483 lines, 39 call sites in five modules, now permitted by the measurement.
