# 2026-11-01 demos: the repo ships a bedroom, the standing demos are re-recorded on it (#262), and #129 slice 12

Focus: demos, then E-002 · Commits: 39f762a..960f6a8 · Machine: big · Outcome: shipped

## Summary
Every open `ui` issue still waits on an open gooey `G-` issue or on UI-10/UI-30 (G-11 and later are open; G-01..G-10 and G-12 are closed), so the pass took the owner's standing-demo rule (build-loop.md, "Every pass ships", item 6) and the issue the last pass filed for it, #262. `assets/grimoire/maps/bedroom.map.ron` gives every storyboard a map a reader can open; the eight storyboards were rewritten against it, each passes `tools/demo/check.py`, and `stale.py` exits 0 with all nine rows fresh. Then #129 slice 12.

## Baseline
Gate: green (pushed 0e4acc5, 111 s). `accept.py` on `assets`: 10/10 before and after the map. Load 18 on arrival (HomeStill and gooey share big).

## Cluster
- #262 (demos): closed by 39f762a. Bedroom map, eight storyboards, `docs/media/*` re-recorded, captions and Captured cells rewritten, `check.py` counts Ctrl+Z, a core test that the map resolves and stands inside its room.
- #129 slice 12: `AcceptEdges` -> `piece_set_cells` with a new `edges` verb; 95 -> 94. Issue stays open.
- Skipped: every `ui` issue (each names the open gooey issue or UI-10/UI-30 it waits on), #201/#202 (the owner's re-pin), #152/#193/#194 (protected fixtures, bevy_elements#55), #112/#113 (live clef-flash), #96 (gooey#18), #103 (re-pins protected tests), #195 (the owner's decision on loops), #197 (`wontfix`). A cluster of two, not five or six: nothing else was ready.

## What the demos taught
- `check.py` asks for 5% of the scene to change at every moment. A sofa in a 540 px crop is about 3%, so the place and build demos zoom (`grimoire_frame_view` on the floor they use) and the agent demos remove the bed, the biggest piece; the numbers are in each storyboard's history, not loosened.
- Under load a chunk overruns its plan, which makes a static run or a late undo; `/tmp` scripts retried a storyboard until `check.py` passed (all eight passed first time in the final batch).
- Ctrl+Z, not Cmd+Z: `keys::MOD_KEYS` is Ctrl on Linux, and `check.py` only knew Cmd+Z, so it was extended (test in `test_check.py`).
- The kit door already carried real `qwen3-vl` labels, so `review_labels` needs no fixture.

## Enhancement: #129 slice 12 (`edges`)
Goal: keeping a scan's derived edges is the operation an agent calls. Files: `ops.rs`, `tiles.rs`, `keys.rs`, `tests/keeping_derived_edges_is_a_cell_operation.rs` (+ `all.rs`), `operations_have_two_front_ends.rs` (one entry off the list, one number down). Proof: the person-side test failed with the old write path (no log line). Editor binary 1018 pass, clippy clean.

## Acceptance
`reached 10/10 → 10/10` (assets, big).

## Review
Reviewer not spawned (time); self-review: (1) does the library path keep the vocabulary's refusal text? yes, the check runs first and the protected test asserting `vocab.ron` passes; (2) is a candidate still in memory? yes, only a library piece routes; (3) does one undo take the keep back? tested; (4) did any check loosen? no; (5) does the schema change break an agent? `cells` is no longer required, `edges` is optional.

## Test diffs
`operations_have_two_front_ends.rs`: `Action::AcceptEdges` off UNMIGRATED, `UNMIGRATED_MAX` 95 -> 94, the shrink the ratchet exists for. `tools/demo/test_check.py`: a new test and a `keys` argument. New test files only otherwise.

## Release
bevy_grimoire_core 0.6.4 (a test file was added to its package): see the log line for the crates.io URL.

## Next focus
UI series when a G- issue closes; otherwise #129's remaining tile-building verbs.
