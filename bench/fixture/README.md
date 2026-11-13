# The benchmark's fixture

**A trial copies the repository's own project and lays this tree over the copy.** Nothing here is a
second project: `assets/` is the project, with its real library, real meshes and real map arithmetic, so
a task's numbers are the numbers a person works with rather than numbers a test made up.

This tree holds only what the suite adds:

| File | Why it is here |
|---|---|
| `assets/grimoire/maps/bench_bedroom.map.yaml` | The room the first three tasks open. Three pieces, no overlaps, and enough empty floor to place something in. |
| `assets/grimoire/maps/bench_*.map.yaml` | One small map for each other dev task of the first suite (BM-04, #414): a bedside, a study, a wardrobe, a store room, a floating lamp, a chair inside a desk, a mistake, a garden, a meadow, a yard, a hallway, a note, a locked bed, a loft and a lamp check. Each is its task's whole scene, so a task's starting position is the suite's and moves only when the suite's own file does. |
| `assets/grimoire/terrain/bench_meadow.yaml`, `bench_yard.yaml` | The ground of the two terrain maps: 21 x 21 cells of 1 m from (-10, -10). The meadow is flat; the yard slopes 0.05 m per metre down to its south-west corner, which is the lowest. |
| `assets/grimoire/data.yaml`, `data/game.yaml` | The game's tuning data the data task edits: the registry, and the gate's and the wall's numbers. |

A task of its own (a held-out one, which is never in this repository) brings its map in a `fixture/` directory
beside its `task.yaml`, laid over the trial's copy after this tree and only for that task's trial.

## Why the map is here and not in `assets/grimoire/maps/`

Two reasons, and both are about nobody being surprised later:

- **Tests count the shipped project's maps.** A map added to `assets/grimoire/maps/` moves those counts.
- **A person edits `assets/`.** A task's starting position may not move because someone dressed the
  bedroom, so the suite's own map is the suite's own file.

A task's `map:` field and a check's `files:` field both resolve **here first**, then in the project, so
this is the whole of what the fixture may add.

## Adding a map

Write it as a normal map file, at `assets/grimoire/maps/<name>.map.yaml` under this directory. Keep the
pieces in it clear of each other — a task's grade is about what an agent did, and a room that already
overlaps fails `no_problem` before the agent starts.
