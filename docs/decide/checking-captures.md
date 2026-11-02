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
