# 2026-11-02 ops: an agent can already undo — pass 263's claim corrected

Focus: #129 E-002 · Commits: 7ed5c35..ffe2bb5 · Machine: big · Outcome: fixes only

## Summary
Pass 263 told the record that *"an agent can place, remove, turn, pin and save a map over MCP — and it
cannot undo."* Reading `ops_brp.rs` to build the recommended `Ctx::deferred` found that false:
`grimoire/operation_undo` and `grimoire/run_command` already give an agent every one of the thirteen
`Change::World` verbs. The kind's own doc is corrected and a test now holds it false.

## Baseline
Gate: green (`7ed5c35`). `accept.py`: not run. Load 6.3, 150 GB free, 15 GB available.

## Bug fixed: a false claim in the record
Must-fix: blocks no, common **yes**, no workaround yes — a wrong capability claim sends the next pass to
build something that exists.
Evidence: `ops_brp.rs:1647` (`operation_undo`, by `operation_id` or `turn: true`, `dry_run`),
`ops_brp.rs:1988` (`run_command`, any registered id as its key would), and `ops::undo_only_refusal`, whose
text already points callers at the first.
Fix: `Change::World`'s doc now says what is missing — **no `Ctx` operation, so no logged id and no
undo-of-an-undo path** — rather than what is not (agent reach). Narrower, and true.

**A test now holds it false.** `a_world_verb_is_reachable_by_an_agent_through_a_door_that_is_not_the_registry`
asserts both doors are **registered** (`DebugCommandInfo::new` with its handler), that `edit.undo` and
`edit.redo` resolve through `commands::by_id`, and that every `World` verb has a row in `commands.rs`.

**Its first version was worthless and a probe proved it.** It searched the source for the door's *name*;
that string appears in six places — two doc comments, `command_palette.rs`, `command_run.rs`, `keys.rs` and
four sites in `ops_brp.rs` — so renaming the one that registers it changed nothing it could see, and it
passed with the registration broken. Rewritten to check the registration and the resolver; a probe that
renames the registration now fails with *"`grimoire/run_command` is not registered with its handler"*.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* Measured — the old test passed under a broken registration.
(2) *Is the failure the claim?* "Both doors exist and every `World` verb has an id" is exactly the claim.
(3) *Right layer?* The doc lives on the kind that was wrong. (4) *Protected files?* A test was **added**; no
existing assertion changed. (5) *What remains?* `Ctx::deferred` for symmetry, and the 15 project verbs —
neither urgent.

## Test diffs
One new test in `operations_have_two_front_ends.rs`. Nothing removed or loosened.

## Release
None — `bevy_cauldron` is `publish = false`.

## Next focus
One of the 15 `Project` verbs with a handler that already fits `Ctx`.
