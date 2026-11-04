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

## Tuning 3 — a click that does nothing: ask the picker what it hit, before forming a theory (pass 382)

**Measured.** The inspector's `23:00` button "could not be clicked" for at least six passes (273, 280, 281, 283, 378–381): each blamed a different layer (`Pickable::IGNORE` on an ancestor, the picker's target, a hover rule, the sky not following) and spent builds and a test file on it. Pass 382 found the cause in about ten minutes by reading one BRP resource, `world.get_resources` on `bevy_picking::hover::HoverMap`, after pressing at the button's own reported centre: the entity under the pointer was the **viewport** (size 724 x 582), because the left pane was 204 px wide around a 456 px panel. The first draft in the tree had blamed `IGNORE`; `bevy_ui`'s `picking_backend.rs` (read in two minutes) does not stop at an ignoring ancestor, so that theory was falsifiable from the source without a build.

**Change.** When an injected click changes nothing, the first two steps are `world.get_resources` for `HoverMap` (then `world.get_components` for the hit entity's size and `UiGlobalTransform`), and reading the vendored `bevy_picking`/`bevy_ui` source for the rule a theory rests on. A theory that cannot name which entity the picker did hit is not yet a theory.

**Watch.** No issue about an unresponsive control is carried across more than one pass; and a log line that blames a picking rule names the vendored file and line it rests on.
