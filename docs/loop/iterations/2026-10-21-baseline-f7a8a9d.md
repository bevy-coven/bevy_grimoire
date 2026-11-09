# Loop iteration 2026-10-21 · baseline f7a8a9d

Verdict: **nothing shipped as an enhancement; no bug fixed.** Phase 0 measurements and the gate are recorded below. The documentation of the iteration is the only change in this PR.

## Phase 0 · baseline (HEAD f7a8a9d, merge of PR #7)

Editor: `~/.cargo-target-shared/global/debug/bevy_emerge_mapper . town` on `/tmp/emerge_live/hotproj3` (scratch copy, not in the repo), BRP port 15733, Metal, Apple M5, 2560×1406 window. The editor was closed after measurement.

| Measure | Value | Method |
|---|---|---|
| Frame time | median 16.66 ms (windows 16.64–16.72) | BRP `bevy_debugger/status` frame counter, 5 × 2 s, no capture running; vsync-capped at 60 Hz |
| Still (`emerge_capture_still`) | 121 ms | one call |
| Sequence, 4 fps, 1 s | 4 frames in 988 ms | `emerge_capture_sequence` |
| MCP edit to visible | call 43 ms (applied); first still showing the new Y bound after 150 ms (1 still) | `emerge_apply_operation map_set_settings`, stills until the MAP SIZE Y field changes |
| File edit to visible | first still showing it after 452 ms (3 stills) | write of the map's `bounds` in `town.map.ron`, stills until the MAP SIZE Y field changes |
| Editor warnings | 0 WARN/ERROR lines in the detached run's log | `grep` of the run log |

Measurement note: the first baseline run used a crop (`40,540,320,760`) that missed the MAP SIZE block, which sits at about y=740–890 in the 2560×1406 frame. That run reported "NEVER visible" for both paths. The corrected crop (`40,740,330,890`) produced the numbers above. The raw stills are in `target/loop/2026-10-21-base/` (uncommitted): `still.png`, `mcp-probe.png`, `file-probe.png`, and the before frames. The full-window still (`mcp-probe.png`) was read at full resolution: the panel, the MAP SIZE block (X 300, Y 147, Z 1120), the placements, and the place list all render with no overlap.

Frame time and the edit-to-visible figures are a baseline only. No runtime code changed, so there is no A/B.

## Gate (f7a8a9d)

- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0, 0 errors.
- `cargo test --workspace`: **1386 passed, 0 failed** (exit 0, 43.6 s).

## Release (E-006)

No release. `publish = false` is set on `bevy_devshot`, `bevy_emerge_rng`, and `bevy_emerge_solver`. The repo has no `.github/workflows` directory and no git tags, so there is no release workflow. The publishable crates have never been published. Nothing was published or tagged.

## Phase 1 · sweep (limited)

- Open bugs in `docs/loop/backlog.json`: B-012, B-013, B-014, B-018. None is must-fix. Each has at most one "yes" on the three-question test (B-013 no workaround only; B-012 common only; B-014 and B-018 none), so none is a must-fix. None was fixed in this iteration.
- Parity and the ratchets were not touched; no operation or key was added.
- The hands-on sweep was limited to the Phase 0 stills and the editor log. A full sweep of gestures, windows, and the kit door was not run this iteration.

## Phase 2 · candidates (ranked)

1. **E-014, snip human gesture (developer's request).** The agent half is shipped (`emerge_snip`). The human half needs a note field registered in `editor::Fields`, a windowed `Screenshot` crop inside `bevy_emerge_mapper` (the crop currently lives in `bevy_emerge_mcp/src/snip.rs` and would move to core rather than be copied), and a decision whether a completed snip is a registry operation (it changes no map) or a documented gesture whose MCP front end is `emerge_snip`. A partial version would be a scope reduction, and I did not ship one.
2. **E-021, frame key (Home).** The framing code exists (`view::frame_box`, `editor::resolve_frame`, `apply_frame`). It needs one `keys::Action` and one registry migration in the same commit (`UNMIGRATED_MAX = 128`). It is the nearest candidate, but the migration target is not yet chosen.
3. **E-015, action palette** over `keys::Action::ALL`. Large: the palette must refuse what `keys::Context` refuses.
4. **E-018, undo history list with jump.** Each jump step is a guarded undo and can refuse.

## Decision: ship nothing

The brief's ship-nothing exit applies. The developer's snip request (E-014) cannot be finished in one verified iteration without a reduction, and a partial snip is not shipped as complete. E-021 is the next iteration's candidate, and it needs the migration choice made first. Log entries: E-014 evidence updated in `docs/loop/backlog.json` with the three open decisions.

## Deferred and blocked

- E-014 (human snip gesture): open, decision needed (operation or gesture).
- E-021 (frame key): open, migration target to choose.
- E-015, E-018: open.
- B-012, B-013, B-014, B-018: open, none must-fix.
- E-006: no release, no crate published; tracked as backlog item.
