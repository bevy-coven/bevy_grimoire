# 2026-11-03 label: pass 277's duplicate labeller withdrawn

Focus: correcting pass 277's mistake (#113) · Commits: 3659f88..db9a86d · Machine: big · Outcome: fixes only

## Summary
Pass 277 built a second decision-labelling path without noticing the first already existed and was
already wired. This pass found that, removed the duplicate and everything that existed only for it, and
found one more consequence: a public API it shipped an hour earlier is now dead, so it was removed too.

## The finding
`crates/bevy_cauldron/src/decide_labels.rs` predates the Cauldron rename. It is **wired into the real
label flow** — `labels.rs` asks `LabelPath::load` and calls either `vlm::label_with_retry` or
`decide_labels::label_with_decisions` — and it does more than the module pass 277 added: it splits a
vocabulary too large for one request, and its `suggest` keeps a probability per token.

Worse than the duplication: **both read `GRIMOIRE_LABEL_PATH` and disagreed about it.** `decide_labels`
says unset is `chat`; pass 277's `Path` said unset is `decide`. Two answers to one setting is a defect
whichever answer is right, and the one that was wired was the one pass 277 did not know about.

## What was removed (0cb61dc)
- `src/label_decide.rs` and its `Path` selector.
- `tests/the_decision_labeller_cannot_suggest_a_token_the_project_lacks.rs` — `decide_labels` already has
  `a_token_the_project_lacks_is_never_suggested`, covering the same invariant with a volunteered
  `kind:throne` and a `look:gilded`.
- `tests/the_two_labelling_paths_are_compared.rs`, `fixtures/label_kit.ron`,
  `fixtures/label_comparison.ron`, `examples/label_compare.rs` — the 92%-each measurement, which compared
  the duplicate against a closed-question stand-in rather than the real chat path, and so measured less
  than its name said.
- `Vocabularies::axis`, which only the duplicate used.

## The consequence of shipping it
`axis` went out in `bevy_grimoire_core` **0.7.1** an hour before this pass. A public method nobody calls,
kept only because it once shipped, is a promise to maintain a mistake, so it was removed: semver-checks
reports `inherent_method_missing`, pre-1.0 that is a MINOR bump, and **0.8.0** was published with the
reason on the release commit.

## CHANGELOG.md
Its three pass-277 lines name deleted files and cannot be rewritten (an earlier line may change only to
replace a `not yet recorded` reason with a clip). A new line, `label-decide-withdrawn`, records what became
of them.

## Review
Reviewer not spawned; self-review, mine:
- **Was anything of value lost?** The per-axis `choice` shape and the `axis` accessor, both mine and both
  inferior to what `decide_labels` already does. The vocabulary guard survives in its stronger form.
- **Is the removal safe?** Editor suite 1161 passed (four fewer, all four the removed tests), clippy clean.
- **Did I weaken a check?** No: the surviving guard test asserts more than the deleted one — it includes a
  token volunteered for an axis the request never asked about.
- **Was the 0.8.0 release justified?** Yes: leaving a dead method in a published crate is the worse option,
  and pre-1.0 the removal has a defined bump.
- **What does #113 still need?** Nothing this pass; its remaining item was removing `vlm.rs`, which this
  pass did **not** do — and should not, since `vlm.rs` is the path `decide_labels` still selects when
  `GRIMOIRE_LABEL_PATH` is `chat`, the default.

## Test diffs
Four test files deleted with the module they tested; no existing assertion changed.

## Release
- **bevy_grimoire_core v0.8.0** — https://crates.io/crates/bevy_grimoire_core/0.8.0 (removes
  `Vocabularies::axis`; semver-checks: `inherent_method_missing`, MINOR pre-1.0)
- **bevy_grimoire v0.4.4** — https://crates.io/crates/bevy_grimoire/0.4.4 (API unchanged; requires core 0.8.0)
- **bevy_grimoire_anim v0.1.9** — https://crates.io/crates/bevy_grimoire_anim/0.1.9 (same)

## Next focus
`vlm.rs` stays: it is the selected path by default. What #113 still wants is a measurement of the two
*wired* paths, which pass 277's stand-in comparison did not make.
