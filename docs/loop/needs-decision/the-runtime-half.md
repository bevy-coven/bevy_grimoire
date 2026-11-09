# The runtime half: retire `bevy_emerge`, or let the game adopt it

**Status:** open — owner's call. Found 2026-10-20 by the technical review.

## The problem

`bevy_emerge` is 1,035 lines: `EmergeWorld`, `EmergePlugin`, `spawn_descriptor`, `SmartObjects` /
`Offer`, `MountedOn` / `Supporting`, `Paint` / `PaintedMaterials` / `apply_paint`.

Outside its own `lib.rs` and `examples/spawn_headless.rs`, **none of it has a caller.** The editor
calls `spawn_descriptor` directly. Sanctuary does not depend on the crate at all — it reads
`bevy_emerge_core` and spawns its own town through `src/prototype/town_map.rs` and
`src/prototype/town.rs`.

The crate header claims: *"the editor spawns pieces through `bevy_emerge::spawn_descriptor` rather
than its own copy, so a map cannot look one way here and another in the game."* That is true
**editor-vs-runtime-crate** and false **editor-vs-game**, which is the failure the claim exists to
prevent. The game currently has two spawn paths.

Supporting facts, all verified:

- `spawn_world` is O(n²) in its `by_id` lookup for `MountedOn`.
- There is no reload or despawn path in the crate. Re-inserting `EmergeWorld` would spawn a second
  copy. Sanctuary has its own watcher (`emerge_data.rs`) and its own despawn.
- Asset-load failure is unhandled: `assets.load` is async, and a missing `.glb` is silent.
- The crate's own test suite has no App-level test of `EmergePlugin` spawning placements, and no
  test of `tip_seat`, `tip_quat`, `apply_paint` or the `MountedOn` linkage.
- `SmartObjects` / `Offer` / `MountedOn` / `Paint` / `apply_paint` have **no** callers outside the
  crate. They are a second, parallel model of interaction that nothing uses.

## The options

### A. Let the game adopt it

Sanctuary loads `EmergeWorld` and lets `EmergePlugin` spawn the map; the editor already calls
`spawn_descriptor`, so both halves share one spawner and the claim becomes true.

- **Cost:** the game inherits the crate's O(n²) mount lookup, its silent asset failure, and its
  missing reload path — or those get fixed first. The game also has to give up its own spawn
  helpers, which are more capable than `spawn_descriptor` in the ways Sanctuary needs (its
  `town.rs` helpers batch, tint and light pieces).
- **Gains:** one spawner, which is the whole point. The editor's preview and the game's town stop
  being two implementations.

### B. Retire it

Delete `bevy_emerge`, keep `spawn_descriptor` where it is needed (or move it into the editor), and
let the game keep its own spawn path. The claim in the crate header stops being a claim.

- **Cost:** the editor and the game still have two spawn paths, and "a map cannot look one way
  here and another in the game" stays true in the negative. `SmartObjects` and the `MountedOn`
  relationship have nowhere to live, so anything built on them is blocked.
- **Gains:** 1,035 lines and a false promise go away. The crate set becomes core + editor +
  anim + three leaves, which is a clearer story.

### C. Finish it, then decide

Give the crate what a real consumer needs — a reload path, asset-load failure handling, a map
lookup that is not O(n²) — and an App-level test of the spawn path. Then decide A or B on the
strength of a crate that works.

- **Cost:** the most work of the three, and it may be wasted if the answer turns out to be B.
- **Gains:** the decision afterwards is on a working crate, and the work is useful under A.

## Recommendation

**C, then A** — with a hard deadline, because the current state is the worst of both worlds: a
crate that promises a shared spawner while the game has its own.

The single cheapest step that moves toward A: make the game spawn at least one *class* of piece
through `spawn_descriptor` and prove the two paths agree, then widen. If the game cannot use it
without giving up something it needs (batching, tinting, lighting), the answer is B, and the
crate goes.

**Owner decisions needed:**

1. Is the game's own spawn path a feature (it batches, tints and lights) or an accident?
2. If A, is the game's `TownPiece` despawn/reload machinery replaced by the crate's, or does the
   crate grow one?
3. Do `SmartObjects` and `MountedOn` have a future? If not, they go with the crate under B.

## What the loop must not do

Do not grow this crate "while the feature is nearby", and do not delete it either. Both are this
decision. Land the next feature in the parts that are not in dispute — the core's schema, the
editor's authoring surfaces, the VLM pipeline — and let the decision stand.
