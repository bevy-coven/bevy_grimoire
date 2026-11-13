# The benchmark's tasks

One directory per task, each holding a `task.yaml`. A task states a prompt, the scene it starts from, a
known answer, and the checks a run must satisfy; `crates/bevy_cauldron_bench` reads it and grades a run
from the files it left, with no model in the loop.

```yaml
id: bed-against-the-north-wall          # the directory's own name
title: The bed against the north wall
split: dev                              # dev | held_out
tags: [place, relations]
map: bench_bedroom                      # a stem under assets/grimoire/maps/
prompt: |
  Put a double bed flat against the north wall of the bedroom, centred on that wall.
setup: []                               # operations applied before the agent starts
solution:                               # a known answer: operations that solve the task
  - op: placement_add
    params: {piece: bed_double, at: [0.0, -1.39], yaw: 0.0}
  - op: grimoire_present                # the hand-back: not an operation, and the only evidence a
    params: {title: …, summary: …}      # refusal or a report left any
checks:                                 # every one must hold on what the run left
  - placement_near_wall: {piece: bed_double, wall: north, max_gap_m: 0.05}
budget: {wall_s: 600, tool_calls: 60}
```

## Every task proves itself, twice, in the push gate

`crates/bevy_cauldron/tests/every_task_is_valid.rs` boots the headless editor on a fresh copy of the
fixture for each task and runs it two ways:

1. **the do-nothing run** — `setup` and nothing else — must leave **at least one check failing**;
2. **the known answer** — `solution` applied through `ops::dispatch` — must leave **every check passing**.

A task that fails either way is a task that measures nothing, and the test names the task, the check and
the value it measured. The second is the *do-nothing loophole* the Agentic Benchmark Checklist names
(`10.48550/arXiv.2507.02825`): a task whose checks already hold on the untouched scene is passed by an
agent that connects and stops.

## The checks, and why there is no free-form one

Each check is a typed variant that refuses unknown fields, and each measures from the **saved files** with
its own arithmetic — never by calling the operation that made the change, because a check that shared its
derivation with its subject would pass on the subject's bug.

| Check | Written as | Asks |
|---|---|---|
| `placement_exists` | `{piece: bed_double, count: 1}` | a placement, or a count of one, exists — by `id` or by `piece` |
| `placement_at` | `{id: bed, yaw_deg: 0, yaw_tolerance_deg: 1}` | its centre, facing or lift is within a stated tolerance |
| `placement_near_wall` | `{piece: bed_double, wall: north, max_gap_m: 0.05}` | its own edge is within `max_gap_m` of a wall of the map's rectangle |
| `placement_relative` | `{piece: lamp, other: {id: desk}, direction: east, max_gap_m: 0.05}` | it is a stated direction from another piece, edge to edge |
| `placement_rests_on` | `{piece: lamp, host: desk@3}` | it names a host, as its `on` field states |
| `unchanged` | `[maps/bench_bedroom.map.yaml]` | the named files are **byte-identical** to the setup state |
| `project_unchanged` | `{}` | the whole project is byte-identical — what a task an agent must refuse needs |
| `no_problem` | `[overlap]` | the map holds no overlap, edge fault, doorway or clearance |
| `oplog_has` | `{op: placement_add, min: 1}` | the log has at least that many applied lines for an operation |
| `oplog_lacks` | `{op: placement_add}` | the log has no applied line for an operation |
| `handback_contains` | `[north wall, centred]` | a `grimoire_present` states given facts — the half a refusal needs |
| `decide` | `{check: bed_centred, want: true, min_confidence: 0.8}` | a calibrated decision the harness was given answers as the task wants |

**A check is a mapping with one key** — its own name — and the value is that variant's arguments. Serde's
own YAML tag form (`!placement_near_wall {…}`) is deliberately not what a task writes: a person writes a
mapping, and a key that is not a check is refused at load **by name with the list of the ones this build
has**.

Compass is the workspace's: `north` is `−Z`, `east` is `+X`, `south` is `+Z`, `west` is `−X`.

**The operation log is left out of the byte comparisons**, because the dispatcher appends a line for
everything it applies — including a task's own `setup`. A check that cares what the log says uses
`oplog_has` or `oplog_lacks`, which read it by op name and outcome.

## A refusal is two checks, never one

A task an agent must refuse — a piece the project does not have, a locked placement — states
`project_unchanged` **and** `handback_contains`. The first stops an agent inventing a piece or placing the
nearest thing; the second stops an agent that connects and says nothing, which would otherwise pass the
first one alone.

## The three shapes a suite must have

A suite is three kinds of work, and every check in the table is exercised by one of them:

- **place something** — `bed-against-the-north-wall`;
- **refuse something the project cannot do** — `refuse-a-piece-the-project-lacks`;
- **report without changing anything** — `count-what-the-bedroom-holds`.

## Adding a task

Make `bench/tasks/<id>/task.yaml`, with `<id>` also the file's `id`. A task naming a field this build does
not read, an operation its registry does not have, or a map or file the fixture and project do not hold is
**refused when the suite loads**, with its path, line and field — never skipped, because a skipped task is
one that silently stops measuring anything.
