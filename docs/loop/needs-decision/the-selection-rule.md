# The selection rule: one item per iteration cannot reach the trunk

**Status:** decided 2026-10-25 — options B and C adopted: trunk work runs as an approved series, and tools/loop/accept.py's reached count is the loop's headline metric. See the loop brief, "Series".

## The problem

The loop's own rule is *"at most one enhancement per iteration, proven by a check that failed before
and passes after"*. That rule is good against thrash. **But combined with a 55-item backlog it
becomes a filter that selects for smallness.**

Anything worth doing in this editor that is larger than one afternoon cannot be picked, because it
cannot be *closed* in one iteration. So the loop keeps choosing what fits: a dock width, a file size,
a second Enter key. Not because anyone decided the trunk did not matter — because the selection rule
cannot see it.

Measured 2026-10-24, from the repo:

| | count |
|---|---|
| operations in the registry (what an agent can call) | **11** |
| actions on the unmigrated allowlist (no operation) | **124** |
| share of the editor's verbs with an agent path | **8 %** |

And the eleven are: `map_create`, `map_set_settings`, `placement_set_pin`, `placement_set_yaw`,
`placement_remove`, `stamp_remove`, `map_rename`, `brush_arm`, `door_open`, `piece_select`,
`label_set`.

**Not one of them generates anything.** From `tests/operations_have_two_front_ends.rs`:
`Action::Generate`, `Action::GenerateDeclared`, `Action::GenerateComposed`, `Action::Fill` and
`Action::Save` are all still on the unmigrated list.

## What that costs, in the brief's own terms

The brief's step 2 is *"the in-editor agent turns that into typed editor operations"* and its example
request is **"a mossy ruined courtyard with two exits"**.

An agent given that request today can call `map_create` (added 2026-10-24, E-008) and gets an empty
40 × 6 × 40 map. Then it has **nothing to fill it with**: the WFC and composition generators are on
the other side of the parity gap. So the workflow stops exactly at the step the brief says every
iteration should make faster or more reliable.

`E-002` — *"An operation per editor verb, with the UI and the agent both dispatching it"* — has been
open since 2026-10-21 and is the trunk the whole design rests on. It has never been picked, because
it is not one iteration.

## Two supporting rot points

Both are honest omissions, repeated often enough to have become policy nobody wrote down.

**1. The gate is not currently trustworthy.** On 2026-10-24 three consecutive `cargo test
--workspace` runs each failed a *different* sequencer test, and clean `main` failed the same way.
Cause: five tests loop `app.update()` against a 20-second real-time deadline without sleeping
(`a_sequence_turn_is_on_the_receipt.rs:49`) while the machine sat at load average 17–37 on 10 cores.
The same tree then ran 126 binaries clean at load 6. **While that holds, no iteration's "gate green"
means what it claims.** Logged as `E-065`.

**2. Review and measurement are being skipped.** Of the last eight iteration reports, **5 have no
independent review** and **8 have no frame-time or latency measurement**; each says so plainly, which
is better than pretending — but the brief requires both every iteration. An exception honestly
recorded eight times is a change to the process that should be decided rather than accumulated.

## The options

### A. Keep the rule exactly as written

- **Cost:** the 124 unmigrated verbs stay unmigrated, and the loop continues to spend its iterations
  on things a developer would not notice. The 8 % never becomes 50 %.
- **Gains:** every iteration is small, provable, and cheap to review. No half-finished work.

### B. Let trunk work span a **series** — 2–4 iterations on one named theme, reported as one report

The rule stays for everything else; a series is opened explicitly, with its own plan and its own
closing condition, and the loop works it until done or abandoned.

- **Cost:** an iteration can end without a shippable increment, which the rule currently forbids.
  Needs a way to leave the tree clean and green at each boundary (which is doable: each operation in
  a series is independently committable).
- **Gains:** `E-002` becomes reachable. A series "**make the generator agent-drivable**" =
  `generate` + `generate_declared` + `generate_composed` + `fill` + `save` as operations, each with
  its own failing-before check, then a scripted MCP client that takes the courtyard request end to
  end. **That series is the difference between a demo and a tool.**

### C. Change what the loop measures, without changing the rule

Stop grading "did you ship one item" and grade **"can a scripted MCP client take a sentence to a
layout and a picture?"** — one script, run every iteration, whose failure point is the report line.

- **Cost:** the script has to exist and be kept honest; it will be red for a while.
- **Gains:** the loop's selection pressure flips. Right now nothing rewards touching the trunk; under
  C nothing else counts.

## Recommendation

**B and C together**, and `E-065` first because it is cheap and everything else is unverifiable
without it.

In order:

1. **`E-065`** — the flaky sequencer tests. One iteration. Until the gate means green, no evidence in
   any report is worth the paper.
2. **A series: "the agent can generate."** `generate`, `generate_declared`, `generate_composed`,
   `fill`, `save` as registered operations, one commit and one failing-before check each. This is
   `E-002`'s first real bite and it is the brief's step 2.
3. **The acceptance script (C)**, written *before* the series ends so the series has a target it can
   fail: a client that says *"a mossy ruined courtyard with two exits"*, makes the map, generates,
   saves, captures a still, and reports what it could not do.

## What the loop must not do

- Do not keep picking items because they close cleanly. That is the failure mode this file exists to
  name.
- Do not start the series without `E-065` fixed, or its "failed before / passes after" evidence will
  be untrustworthy in exactly the way this iteration's gate was.
- Do not treat "no review this iteration" as a settled practice. Either restore the review step or
  decide to drop it.

## Owner decisions needed

1. **Does the one-enhancement rule get an explicit series exemption (B)?** If no, `E-002` is
   unreachable and the 8 % is permanent by construction.
2. **Is `E-065` the next iteration, ahead of any feature?** (Recommended.)
3. **Is the acceptance script (C) the loop's headline metric**, replacing "one item shipped"?
