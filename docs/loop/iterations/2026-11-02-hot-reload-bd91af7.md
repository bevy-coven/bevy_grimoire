# 2026-11-02 hot reload: a moved piece keeps its entity (#201 item 5), and #152's fixtures are real settings

Focus: the two issues the owner unblocked on 2026-11-02 · Commits: 424e3d3..HEAD · Machine: big · Outcome: shipped

## Summary
The owner's grants lifted #152 and #201, and both are done to the point their acceptance allows. #152 was
fixture text — the family-settings tests wrote `(sun: (hour: 6.0))`, which the host's typed read refuses,
so they asserted on a file the crate throws away. #201 item 5 was the in-place split: a move used to
respawn a piece, restarting every run's people and every fire's flicker on it.

## Bugs fixed
### #152: the family settings fixtures were not `ElementsSettings`
Must-fix: blocks y (3 red tests), common n, no workaround y — the tests could not pass while the file
they wrote was one the host refused.
Root cause: `(sun: (hour: 6.0))` predates #200's typed read; the host says `Unexpected field named sun` and
never stores it, so `text["bevy_elements"]` has no key and the test panicked one step earlier than the
issue recorded. · Regression check: the three tests now read `tests/fixtures/bevy_elements.ron`
(`ElementsSettings::defaults().to_ron()`, hour set) — `family_settings` runs **7 passed, 0 failed**.
`diff` between the two fixtures is **exactly one line**, which is what makes the edit an edit.

## Enhancement: an edit that only moves a piece keeps its entity (#201, acceptance item 5)
Goal: move, turn, retint and retune preserve identity; a change of kind respawns that piece only.
Files: `crates/bevy_grimoire/src/live.rs` (the plan split, the in-place retarget, riders),
`crates/bevy_grimoire/src/lib.rs` (`placed_transform`, shared by spawn and move).
Proof: on the base, `an_edit_respawns_only_what_changed` fails with
`assertion left != right failed: a moved placement is respawned`; with the change, `bevy_grimoire` is
**40 passed, 0 failed** across six suites, clippy `-D warnings` clean.

The split is two questions rather than one: `same_piece` is descriptor, host and map — what the thing
*is* — and `pose_same` is at/yaw/lift/tip plus the resolved y that follows — where it *stands*. A row
that differs only in the second is kept and retargeted. A moved host keeps its riders mounted, because
`MountedOn` names the host's `Entity` and that entity now survives.

## Test diffs — every changed assertion, old → new
Owner grant, 2026-11-02. Each is at least as strong as the claim it replaces.
| test | old | new |
|---|---|---|
| `live_reload::an_edit_respawns_only_what_changed` | `assert_ne!(after["crate_b"], before["crate_b"], "a moved placement is respawned")` + `assert!(get_entity(before).is_err())` | `assert_eq!` on the same entity **and** its `Transform` equals `placed_transform(.., (5.0, 1.0), ..)` |
| `live_reload::a_rider_follows_its_host_and_is_mounted_again` | `assert_ne!` on rider | `assert_eq!` on host and rider, same `MountedOn` claim |
| `live_migrate::a_registered_component_follows_a_piece_that_is_drawn_again` | `assert_ne!`, `Mood` (unregistered) `None` | `assert_eq!`, `Mood` `Some(3)` — nothing respawned, so everything survives |
| `live_migrate::a_rider_drawn_again_with_its_host_carries_its_state_too` | `assert_ne!` on host and rider | `assert_eq!` on both, plus `MountedOn` still naming the same host |
| `live_references` ×3 | `assert_ne!` on the moved piece; `Places` names the new lamp; `Flagged == [anna]` | `assert_eq!`; `Places` names the **same** lamp; `Flagged` **empty** |
| `live_migrate::a_hook_runs_once_per_piece_drawn_again_*` | — | **unchanged**; still proves the hook runs, now for a kind change |

New: `reloading_twice_with_no_change_spawns_nothing_the_second_time` (item 3's guard). Its own doc
records that it passes on the base too and does not discriminate this slice.

## Not done, and why
Item 1 needs #104's society run in the harness; item 2 needs the rename alias for *runtime* state;
item 4 needs an audit that each kind has exactly one hook. #201 stays open on them.

## Review
`reviewer` not spawned. Self-review: (1) *is the split right* — descriptor/host/map versus pose, with
the resolved y on the pose side because it follows from the move; (2) *did I weaken a check* — six
changed assertions are each stronger, and the hook test is untouched; (3) *does the move land where a
spawn would* — both go through `placed_transform`; (4) *could the guard mislead* — its doc says it does
not discriminate; (5) *what did I get wrong* — I first added `moved` to the despawn test, which
despawned the very entity the slice exists to keep, and the suite caught it.

## Release
**`bevy_grimoire v0.4.1`** — https://crates.io/crates/bevy_grimoire/0.4.1
`cargo semver-checks check-release -p bevy_grimoire` said *"no semver update required"* (the one API
change is the addition of `placed_transform`), so this is a PATCH bump. `cargo publish --dry-run` built
the packaged crate, then `cargo publish` uploaded and resolved; tagged `bevy_grimoire-v0.4.1` and pushed.

## Next focus
Release `bevy_grimoire`, then #201 items 1, 2 and 4.
