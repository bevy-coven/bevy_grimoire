# 2026-11-02 lights: a placement can carry a whole bevy_lights Fire

Focus: family F7, lights and fire (#103) · Commits: ae16a6b · Machine: big · Outcome: shipped (one slice; #103 closed after the push)

## Summary
Every `ui` issue still depends on #224, which depends on #210, whose first acceptance item (`chrome.rs` declares no colour constants) re-pins protected tests (the `chrome::NAME` spellings); nothing in that chain moved since pass 210, so the pass took the one unblocked item on the family list: #103's remaining line, `bevy_lights::Fire`. The earlier passes read it as a swap that re-pins protected tests; it is built additively instead, so `Flicker` text keeps reading exactly as before and no assertion changed.

## Baseline
Gate: green on main at the start (CI run f07b70c: success). Load 3.6, 187 GB free, 15 GB available RAM. `accept.py` not run (nothing it measures changed; the change is behind `--features bevy_lights`).

## Cluster
- #103 F7: slice committed and its last acceptance item was already met (items 1 and 2 by earlier passes, item 3's clip by pass 199), so the issue is closed with this slice's evidence.
- Everything else, skipped as in pass 213: #210/#211/#224 and all `ui` issues behind them (protected `chrome::NAME` spellings, asked on #210 in pass 210, no answer); #235 (deleting protected tests); #129's remainder; #152, #201 (protected edits); #193/#194 (bevy-coven/bevy_elements#55, open); #112/#113 (no decision model); #195 (hosts' settings); #197 (wontfix).

## Enhancement: a placement carries a Fire
Goal: `placement_set_family` with `(kind: Campfire)` puts the crate's own `Fire` on a piece; the preview draws its flame and pooled light; the light map and `light_pool` read its lumens.
Files: `fixture.rs` (new: `Fixture::read` tells a `Fire` from a `Flicker` by the fields only a `Fire` has, because both are `serde(default)` and neither denies unknown fields, so `(kind: Campfir)` used to read as a default `Flicker`; `Fixture::lumens`), `light_view.rs` (hangs a `Fire` under the placement; `FirePreviewPlugin`: `FlamePlugin` + `LightPoolPlugin<MainCamera>` + `FirePlugin`, added by the binary only because `FlamePlugin` needs a render app), `ops_family.rs` (`reads_as`), `ops_light.rs` (`bake`), `ops_pool.rs` (`budget`), `main.rs`; `bevy_lights` pin 15af29a -> ae3b818 (v0.5.0).
Proof: `tests/a_fire_is_a_fixture.rs` (4; all four failed first: a misspelt kind was accepted, no `Fire` was held, a Fire's lumens read as 0 on the light map). Live on big (RTX 3090, Vulkan, Xvfb :99) with `--features bevy_lights,bevy_light_grid,bevy_elements` at 23:00: a Torch and a Brazier burn, the room is lit by the pool; clip `docs/changelog/media/2026-11-02-103-fire.mp4`.
MCP: no operation added; `placement_set_family`'s `value` now takes a `Fire` too.
Measured, not fixed: RON writes `Option` as `Some(..)`, so `lumens: 60.0` is refused ("Expected option") and the refusal says so; a Fire's lumens are in the crate's exposure, like a Flicker's `base`.

## Demos
Nine standing demos listed stale by `tools/demo/stale.py` (a visible change since the last capture; none of them shows a placed light); re-recorded by a subagent, results in the log line.

## Performance
Not measured (`perf.py` needs a real display).

## Acceptance
`accept.py` not run.

## Review
Reviewer not spawned; self-review: (1) does `Flicker` text read as before? yes, `placement_family` and `light_*` tests (26 with the new ones) pass unchanged, and the refusal text keeps "not a bevy_lights Flicker". (2) a value naming `phase` only is a `Flicker`: yes, `phase` is shared. (3) headless harness: holds the `Fire`, no flame (no render app). (4) two `FlickerPlugin`s: `LightViewPlugin` checks `is_plugin_added` first. (5) pool focus: the map camera.

## Test diffs
`tests/all.rs`: one `mod a_fire_is_a_fixture;` line (assumed owner exception, additive). No assertion changed.

## Release
None: only `publish = false` crates changed.

## Next focus
Owner: say how #210's `chrome::NAME` spellings may change, or that UI-99 carries them: 30 `ui` issues wait on it.
