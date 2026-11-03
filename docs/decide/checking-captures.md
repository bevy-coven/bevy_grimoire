# Checking captures with the decision model (#112)

`tools/decide/check.py` runs the editor's checks over a directory of captures and says which of them may
fail a gate. It is the command-line half of D4 (#112); the rule it applies lives in Rust, so a test can
prove it.

```sh
# What would be asked, and what may block. No network.
python3 tools/decide/check.py docs/changelog/media --project .

# Ask the model. Exit 1 only when a calibrated check found something.
python3 tools/decide/check.py target/loop/captures/my-run --project . --ask

# JSON, for a script.
python3 tools/decide/check.py target/loop/captures/my-run --project . --ask --json
```

## The three rules, and where they are proved

|Rule|Where|
|---|---|
|A **calibrated** check that finds something fails the run|`crates/bevy_oracle/tests/checks_that_earn_their_authority.rs::a_calibrated_check_that_finds_something_fails_the_run`|
|The **same finding** from an uncalibrated check only prints|`…::the_same_finding_from_an_uncalibrated_check_only_prints`|
|A calibrated check **below the bar** only prints|`…::a_calibrated_check_below_the_bar_only_prints`|
|A missing capture is an **error, never a failure**|`…::a_check_whose_capture_is_missing_is_an_error_and_never_a_failure`|

The policy is `bevy_oracle::run_checks`; `tools/decide/check.py` is a front end that maps the verdict to
an exit code. A rule that lived only in a script could not be tested, and a check that cannot fail — or
fails when it has earned nothing — is the failure mode the issue exists to avoid.

## Where the checks come from

- **Built in** — `crates/bevy_oracle/fixtures/checks.ron`, five yes/no questions about a close-up, a
  before/after pair and a whole view (`floating`, `clipping`, `out_of_area`, `blank_frame`,
  `toast_visible`).
- **The project's** — `assets/grimoire/checks.ron` in the project being checked, merged over the built-in
  set by name, so a project can re-point a threshold without losing the rest.

## Evidence

A capture directory holds `<image name>.png` for each name a check asks for (`closeup.png`, `view.png`,
`before.png`, `after.png`). They are sent as the API's `data:image/png;base64,…` URLs. A name a check asks
for that the directory does not hold is reported and never fails the run: *an error is not evidence of a
problem in a picture.*

## Calibrating a check

A check earns the right to block by being measured against a labelled set of real captures; the result is
written into its `calibration` in `checks.ron` (`agreement`, `set_size`, `model`, `date`). `--bar` (default
0.9) is the agreement a check must reach. Until a check has one, it is **advisory** — printed every run,
never fatal — which is what lets a new check be added to a project on its first day.

The calibration run itself needs a served model: see `serving-clef-flash.md`. `check.py --ask` against a
down endpoint reports every check as an error naming the endpoint, and exits 0: nothing was found because
nothing was asked, and the tool never invents an answer.

## The clip check

`clip_shows_its_title` tests the thing this repository has got wrong before: whether a changelog clip shows
what its line claims. It sends four images — the still beside the MP4 and three frames spread through it
(the API's limit, with the still as the fourth) — and asks one yes/no question: *do these frames fail to show
what the line says?* The line's title is carried in the question, because a model asked "does this match?"
without the claim answers from nothing.

**Its labelled set is data**: `crates/bevy_oracle/fixtures/clip_samples.ron`, every changelog clip with a
clip as a *match* (its own line) and a *mismatch* (the same frames under another line's title). The second
half is what stops a check that always says "that is fine" from scoring 100 %. The images are derived, not
committed; remake them with:

```sh
python3 tools/decide/clip_set.py --out target/clip_set
```

then point the manifest's `dir` fields at `target/clip_set/<id>`. A relative `dir` is resolved against the
manifest, so a committed set works from any clone.

**Measured on 2026-11-02, and it did not earn its authority.** With clef-flash served on
`127.0.0.1:8093`, the clip check scored **61.5 % on 122 samples** (61 clips, each as its own line and as
another line's mismatch), at a chosen threshold of 0.241:

```sh
GRIMOIRE_DECIDE_ENDPOINT=http://127.0.0.1:8093/v1/systemone \
  cargo run -q -p bevy_oracle --bin decide_check -- \
  --calibrate crates/bevy_oracle/fixtures/clip_samples.ron
# clip_shows_its_title   agreement 61.5% on 122 samples, threshold 0.241, model clef-flash
```

**61.5 % is barely better than guessing on a two-sided set**, so the check stays **advisory**: it prints every
run and can never fail one. That is the issue's own rule, not a shortcut — a check that has earned nothing
must not be able to block, and this one has now shown what it is worth rather than leaving the question open.

Two things follow. First, the figure is recorded here and nowhere in the check: a `calibration` is written by
the owner's evaluator pass once a check clears the bar, and this one does not. Second, the honest reading is
about the **samples as much as the model**: a 9B model on four 480 px frames is asked to match a whole
changelog title ("at 23:00 a torch and a brazier burn on two lamps, with their flames, sparks and pooled
light") against a still that shows a room — the mismatch pairs are often about *one clause* of a long line.
A clip check that asks about a short, concrete claim is a different measurement, and it is the next step
before any threshold is trusted.

Until then the check carries no `calibration` and is **advisory**.

What the set *can* be proved for without a model is that it is well formed, and a test does
(`checks_that_earn_their_authority.rs::the_committed_clip_set_is_two_sided_and_well_formed`): it must hold at
least 20 samples, be about half matches and half mismatches, carry whole titles rather than ids, and each
mismatch must genuinely differ from the line its frames belong to.
