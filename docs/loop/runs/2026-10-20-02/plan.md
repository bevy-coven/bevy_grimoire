# Plan — the run of a thing

Run: 2026-10-20-02. Feature: **an author draws a run of posts — a fence, a wall, a kerb — with one
gesture, at a pitch of their choosing, with the corners handled.**

## The decision

**A new placement mode, "run": arm a piece, drag a line, and the piece repeats along it at its own
pitch, turning at the corners.**

Not a new gesture — the **same** drag the fill box already uses (press, drag, release) with a
different arithmetic. The run's arithmetic lives in `bevy_emerge_core`, engine-free, so the editor
and any future generator can lay the same run.

### Why this and not the alternatives

- **Not "make the fill smarter".** The fill is a *grid*: one piece, one yaw, one pitch, over a
  region. A run is a *path*: a direction that changes, a pitch that is the piece's, and posts that
  turn. Teaching the fill to do that makes the fill do two jobs, and the fill is already the
  simplest thing in the editor.
- **Not "let the author place 282 fences by hand".** That is what the town map did, and it is the
  friction this run exists to remove.
- **Not "a rectangle primitive" (drag a box, get a ring).** The graveyard's south edge has a
  **3-post gap**, and a ring primitive cannot express a gap. The research's gap between the
  literature and this project is settled in `research.md`: the gap is a **parameter of the run**, and
  a closed ring is *four runs* with a corner rule — which is how a fence is actually built.

### What it costs

The Map tab's `Context::Map` is **at its twelve-row ceiling** (`no_context_carries_more_than_a_learnable_vocabulary`
enforces it). So the run must be a **mode**, not a new key: a key that *arms* the run rather than
a key that *is* the run.

But the mode needs a way in and a way out. The editor already has a precedent for exactly this
shape — `Tool::Place` / `Tool::Remove` / `Tool::Move` / `Tool::Clone`, entered by a key and left by
a click — and the census's rule about modes is Raskin's: *a mode you can forget* is the problem, so
a mode must be **drawn**. The run is drawn twice over: the mode is visible in the inspector, and
the posts it would lay are on screen as a ghost while the drag is in progress.

**The key budget: one row.** `Tool::Run` joins the `B` family — `B` is already "move / Shift: clone
/ M: keep as a composition", so the run is the **fourth** verb on that key. But the census says
`every_action_has_exactly_one_binding`, and `B` already has three. So the run takes a key of its
own from the Map context, and something gives one up.

## The shape of the change

### `bevy_emerge_core::run` — the arithmetic, engine-free

New module. A run is a pure function of a few named values:

```rust
/// **One piece, repeated along a path.** The smallest reusable thing the editor had no word for.
pub struct Run {
    /// The piece to repeat. Its own footprint is the default pitch.
    pub descriptor: String,
    /// Metres between posts. Defaults to the piece's own footprint along the run's axis.
    pub pitch: Option<f32>,
    /// Where the run starts, in map space.
    pub from: (f32, f32),
    /// Where it ends.
    pub to: (f32, f32),
    /// **The turns.** Each is a point on the path where the direction changes.
    pub corners: Vec<Corner>,
    /// **The opening** — how many posts are missing, and where along the run.
    pub gaps: Vec<Gap>,
}

/// **One post on the run.**
pub struct Post {
    pub at: (f32, f32),
    pub yaw: f32,
}

/// **The run's posts**, and the refusals that stop it.
pub fn lay(run: &Run, library: &Library) -> Result<Vec<Post>, String>;
```

**The rules, each from a measurement:**

1. **The pitch is the piece's.** The run asks the library for the descriptor's footprint and uses
   the extent **along the run's direction**. For the fence (2.064 × 0.12 m) a run along X gets a
   2.064 m pitch; the same piece run along Z gets 0.12 m — which is wrong, and is **refused**,
   because a fence's long dimension is the one that spans the gap. The run refuses a direction
   whose extent is not the piece's long one, naming the piece and the two numbers.

   *Why not the lattice.* The project's lattice is 1.000 / 0.333 / 0.111 m (measured), and a fill
   steps by `ceil(span / pitch) * pitch`, giving 3.000 / 2.333 / 2.111 for a 2.064 m fence — **none
   of them 2.032 m**, the closest 8 cm out. A run's pitch is the *piece's* business, and the run
   never lands on the project's snap rungs. This is the same discipline `fill::Lattice` already
   states for the fill: a fill needed *its own* lattice, and a run needs one too.

2. **The corners turn the piece.** A `Corner` is a point on the path and the yaw after it. The post
   at the corner carries the *new* yaw, and the run's pitch is measured from it. The kit has **one**
   fence descriptor and the corner is that piece at yaw 90 — so the corner rule is a *rule*, not a
   second descriptor, and the run needs no new asset.

3. **A gap is a run parameter.** A `Gap` is a count of missing posts and the index they would have
   occupied. The graveyard's gap is 3 posts, and it is *part of the shape*.

4. **A refusal, not a degradation.** A run that cannot be laid refuses, by name: a piece with no
   measured footprint, a pitch that is not finite or not positive, a span shorter than one pitch, a
   corner that is not on the path, a gap that would eat more posts than the run has.

**The direction rule.** A run is a polyline: `from` → `corner[0]` → `corner[1]` → … → `to`. Each
segment is straight, and each segment's posts are laid along it. The posts at a corner belong to
the segment that *ends* there, and carry the corner's yaw.

**The `library` dependency is one call** — the descriptor's footprint — so the module stays
engine-free and testable with a hand-built `Library`.

### `bevy_emerge_core::map` — a run's posts are ordinary placements

**No schema change.** A run's posts are `Placed` rows like any other, so:
- the game needs no change (the previous iteration's loader already expands stamps; a run's posts
  are plain rows);
- the inspector's count already includes them;
- the map file gains rows and nothing else.

The one thing the run needs is a **stable id per post**, so a run is one undoable act and its
posts are addressable: `<descriptor>@<n>` like every other minted row, minted by the same counter.

### `bevy_emerge_mapper` — the mode, the ghost, the key

- **`Tool::Run`** joins the tool enum. It is a *mode* like `Tool::Move` and `Tool::Clone`, entered
  by a key and left by a click — and it is **drawn**: the inspector names the armed tool, and the
  posts it would lay are a ghost while the drag is in progress.
- **The gesture is the fill's gesture**: press, drag, release, with the same `CLICK_EPS` threshold
  that tells a click from a drag, and the same `Alt` modifier that means "place one, exactly
  here" (so `Alt`-dragging does not start a run).
- **The ghost** is the run's real posts, drawn where they would land. **The first capture showed
  only a line**, and the review caught it: a line is the *instruction* to lay, not the landing, and
  the editor's own rule is that a preview must show what will happen. So the ghost stands the
  posts, translucent, through the `Ghost` marker the other previews use.
- **The key: `Cmd+B`**, the fourth verb on the `B` family — "move / Shift: clone / Cmd: run / M:
  keep as a composition". The family **costs no badge row** (they share a `does`), but the
  vocabulary ratchet counts *bindings*, so `Context::Map` goes 24 → **25**. That is a deliberate
  purchase, and the pin says so with the reason.

### `bevy_emerge_mapper` — the filter takes the keyboard

**`Cmd+F`**, on every tab. The filter box was `Context::Tiles`, so the Map tab's PLACE palette and
the Rigs tab's rig list had **no keyboard route to their own filter at all** — and a box the
author cannot reach by keyboard is a box they will miss, and a click that misses it falls through
to the map and **edits the level**.

`Cmd+F` is the platform's own "find" chord and is free on every tab, which is what makes one
**Global** row the right shape: the tab decides *which* box, and the chord is the same everywhere.
`Context::Global` goes 16 → **17**, also deliberate and recorded in the pin.

And one more fix the review forced: **`filter::keys` now drains the message stream when no box is
focused.** The box opens in `Phase::Act` and the text handler runs in `Phase::Text`, so the
opening keystroke was still in the stream and landed in the box as its first character — the
`xseam` bug, in the one field that did not drain.

**Why the filter is in this iteration and not another.** The run's *pitch* is the piece's, but
the author still has to *find* the piece; and 90 keystrokes to find a piece is the tax on every
run an author draws. It is the same task, so it belongs in the same iteration.

## Touched modules

| Crate | Module | Change |
|---|---|---|
| `bevy_emerge_core` | `run.rs` (new) | `Run`, `Post`, `Corner`, `Gap`, `lay` — the arithmetic, engine-free |
| `bevy_emerge_core` | `lib.rs` | the module is declared |
| `bevy_emerge_core` | `descriptor.rs` | **none** — `placed_footprint` already answers the run's question |
| `bevy_emerge_mapper` | `editor.rs` | `Tool::Run`; `drive_run`; `drive_run_ghost`; `run_between`; `run_for_test`; the undo entry |
| `bevy_emerge_mapper` | `keys.rs` | **one row**: `Cmd+B` on the `B` family, and `Cmd+F` as the filter's chord |
| `bevy_emerge_mapper` | `tiles.rs` | `FocusFilter` focuses the current tab's pane |
| `bevy_emerge_mapper` | `filter.rs` | `keys` drains the stream when no box is focused — the opening keystroke must not become the first character |
| `bevy_emerge_mapper` | `screen.rs` | `RunDrag` and `RunGhostDrag` join the ownership table |
| Sanctuary (consumer) | — | **none.** A run's posts are ordinary placements. |

## Tests

**`bevy_emerge_core/tests/a_run_is_laid_in_the_piece_s_own_pitch.rs`** — the arithmetic:

1. a run of a 2.064 m fence places its posts **2.064 m apart**, and the spacing is **not** one of
   the project's snap rungs (1.000 / 0.333 / 0.111) — the measurement that proved the fill cannot
   do it;
2. the posts' positions are the anchor plus `n × pitch` **to the millimetre**;
3. a run with a corner turns the piece: the corner post's yaw differs from its neighbours' by 90°,
   and **it is the same descriptor** — the kit has one fence piece;
4. a run with a gap of 3 leaves 3 posts missing, and the posts around it are at the right indices;
5. a run whose direction is the piece's *short* axis **refuses**, naming the piece and the two
   numbers;
6. a run of a piece with no measured footprint **refuses**;
7. a run with a non-finite pitch **refuses**;
8. a run shorter than one pitch **refuses**;
9. **a round trip**: the posts a run lays, written as `Placed` rows and read back, are the same
   rows — and the map's own validation accepts them.

**`bevy_emerge_mapper/tests/`** — the editor's half:

10. the ghost shows the posts the commit will lay — the same rows, in the same order, at the same
    places (the rule the stamp ghost already follows);
11. the commit lays the ghost's posts;
12. **undo takes the whole run in one act**, and redo puts it back — the fill's rule;
13. the run's rows are in `map.placements` (not in a second list), so the inspector's count includes
    them and the game gets them — the previous iteration's contract, not broken;
14. **`F` focuses the Map tab's filter**, and the text lands in the box — the measured friction.

**The census's own ratchets** must still pass: `no_context_carries_more_than_a_learnable_vocabulary`
(the Map context does not grow — the run shares `B`'s row), `every_action_has_exactly_one_binding`,
`every_key_has_a_home`, and `tests/census_is_the_one_counter.rs`.

## Schema impact

**None.** A run's posts are `Placed` rows; `MAP_VERSION` stays 6; nothing on disk changes shape.

## Risks

- **The run's pitch vs the piece's footprint.** A run lays the piece's *centre* every `pitch`
  metres, so a piece whose footprint is longer than the pitch overlaps its neighbour. That is
  correct for a fence (the posts share a rail) and wrong for a crate. The run's pitch is a
  parameter for exactly this reason, and the default — the piece's own footprint — is the answer
  that is right for an unseen piece.
- **The run is a mode.** Raskin's objection is a mode you can *forget*; the answer is that it is
  drawn (the inspector names it, the ghost shows it) and left by a click.
- **The `B` family grows.** Four verbs on one key is the most the census's `rows()` can collapse
  legibly. If a fifth is ever needed, the family is the thing that gives way, not the ceiling.
