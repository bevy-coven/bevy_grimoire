# How the loop command has been tuned

**This file is the tunable half of `.omp/commands/build-loop.md`.** A pass appends a section here when
a measurement shows a cheaper way to work; the next pass reads it in Orient and works that way. The
command's frozen core — the budget, the protected evaluator, one gate per push, `Fixes #N`, the
two-way street, the owner's overrides — is not tunable and a tuning that weakens any of it is refused.

The rules for changing it are in the command, "The command improves itself — and its core cannot be
edited": one tuning a pass, a named metric, and a revert when the metric does not move in two passes.

## Tuning 1 — record the timing off the driver's path (2026-11-03)

**Measured.** `~/.local/state/wiggum/bevy_emerge_loop/steps.tsv` had no row for any pass after 251,
while commits named passes up to 291. `backfill` mapped a session to a pass by a `pass N START` line
in `driver.log`, and a hand-driven pass writes none, so it fell through to a **negative** number and
every report ignored those rows. The largest single recoverable sink the analysis did find:
**50 `wait` steps sitting at exactly 30:00**, 24.98 h, every one in Orient.

**Change.** `steps.py begin` / `steps.py end` number and time a pass with no driver at all; the driver
calls the same two. `backfill` now cuts a session at each `one … pass` marker, so a hand-driven night
is 57 passes and not one 26-hour one.

**Watch.** Every pass in `steps.tsv` after a run — hand or driven; `steps.py report --last 12` showing
per-pass minutes under 45; and no `wait` step at exactly 30:00, which is a wait that hit its ceiling
rather than a job that finished.

## Tuning 2 — a 30-minute wait is a failed wait (2026-11-03)

**Measured.** The same analysis: 50 of the editor's 31,117 steps ran 30:00 and returned nothing. A
wait that reaches its ceiling has told you the thing you waited for is not coming.

**Change.** A `wait` is given the time the thing it waits for plausibly needs, and a result at the
ceiling is treated as a refusal: name what did not arrive in the log line and carry on with the next
issue rather than waiting again.

**Watch.** Zero steps at 30:00, and no pass whose log line says it waited twice for the same thing.
