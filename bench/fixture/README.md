# The benchmark's fixture

**A trial copies the repository's own project and lays this tree over the copy.** Nothing here is a
second project: `assets/` is the project, with its real library, real meshes and real map arithmetic, so
a task's numbers are the numbers a person works with rather than numbers a test made up.

This tree holds only what the suite adds. Today that is one map:

| File | Why it is here |
|---|---|
| `assets/grimoire/maps/bench_bedroom.map.yaml` | The room every task opens. Three pieces, no overlaps, and enough empty floor to place something in. |

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
