# Plan — the stamped group the game never sees

Run: 2026-10-20-01. Feature: **a map's stamps reach the consumer game.**

## The decision

**`ProjectData::load` expands a map's stamps, and hands the consumer a map whose `placements`
include the stamped rows.**

The map *file* still stores a reference. The expansion is a read-time projection, and it happens
in exactly one place — `bevy_emerge_core::composition::expand` — which the editor already calls and
the game will now call too.

### Why this and not the alternatives

- **Not "the game calls `expand` itself".** That leaves the burden on every consumer: the game
  would have to remember to expand before reading `map.placements`, and a consumer that forgets
  gets a map missing rows with no error. The loader is the one door every consumer goes through;
  putting the expansion there makes the wrong thing impossible rather than merely discouraged.
- **Not "the editor bakes the rows into `placements` on save".** That destroys the reason the
  reference exists: editing the group would no longer change the maps that stamped it, and the
  editor's own note on `Map::stamps` says so — *"never written back into `Self::placements`"*.
- **Not "a second list on `Map`".** `map.expanded_placements` would be a second way to name the
  same thing, and every reader would have to pick one. The crate's rule is one path per feature.

### What it costs

`Map::placements` changes meaning for the *consumer*: it becomes "the rows the level has", not
"the rows the author typed". Three things follow, and each is handled:

1. **The editor's count.** `census::of_map` counts `map.placements.len()`. The editor shows
   `N placed`, and the consumer will have `N + expanded`. The count the author reads must be the
   count the game has, or the panel is a lie. `census::of_map` must gain the expansion — which
   means it needs the compositions and the library, so its signature changes.
2. **The editor's save gate.** `OpenMap::save` already expands to check the map loads. It must
   keep doing so, and the check must be the *same* check the consumer's load does.
3. **The game's `check_ids`.** Sanctuary's `town_map::check_ids` walks `map.placements` and
   refuses a descriptor the game does not know. A stamped row's descriptor is a kit piece, so it
   would be refused — **correctly**, because the game genuinely does not know every kit piece's
   descriptor. The game's side must be taught to allow the rows a stamp brings.

## The shape of the change

### `bevy_emerge_core::composition` — the id convention, named once, and a named door

`expand` already exists and is already shared. Two things were added.

**The id convention, stated once.** Every row a stamp brings is named `<stamp>/<member>`, and
that convention was written out in five places in the file — once per site that minted one. It
is now `composition::stamped_id(stamp, member)` and `composition::split_stamped_id(id)`, and
every minting site calls it. A convention is a *fact about ids*, and a fact stated in five
places is five facts.

**A named door for the common case.** `expand` takes the stamps as an argument, because the
editor also expands a *proposal* (a trial map, before a stamp is kept). A consumer does not have
that case:

```rust
pub fn expand_map(map: &Map, compositions: &[Composition], library: &Library)
    -> Result<Expansion, String>
```

It is `expand(map, &map.stamps, compositions, library)` — the one expander, called with the
map's own stamps. No second implementation.

### `bevy_emerge_core::map` — an expanded view of a map, and the answer to "is this row stamped?"

```rust
impl Map {
    /// **The map as a consumer sees it**: the same map, with every stamp's rows in
    /// `placements`. `stamps` is unchanged — the reference survives, so saving the expanded map
    /// writes the reference and not the rows.
    pub fn expanded(&self, compositions: &[Composition], library: &Library)
        -> Result<Map, String>

    /// **The stamp a row belongs to**, and the member within it — `None` for a hand-placed row.
    pub fn stamp_of<'a>(&self, placement_id: &'a str) -> Option<(&'a str, &'a str)>
}
```

**Why `stamp_of` consults the map's own stamps, and not just the id's shape.** A placement id is
an authored string and nothing shape-checks it — `fridge@1` ships, and `mess_a/table` is legal
too. So a `/` in an id does **not** make a row a stamped one. The id's *shape* comes from the
one convention `composition::stamped_id` writes, so the two cannot drift; the *membership* is this
map's own fact. A hand-placed row named `mess_a/table` on a map with no such stamp is a
hand-placed row.

**Locations.** A stamp brings locations with it (`Expansion::locations`), and they are added too
— or the group's affordances vanish. Their `props` name `<stamp>/<member>` ids, so they resolve
against the expanded placements.

### `bevy_emerge_core::project_data` — the loader expands

`load` reads a map, validates it against the library, and inserts it. It now:

1. reads the map,
2. `map.expanded(&compositions, &library)`,
3. validates **the expanded** map (so a stamped row naming a missing descriptor is caught here,
   by name),
4. inserts the expanded map.

**The failure is a refusal, not a degradation.** A map whose stamps do not expand is refused with
the stamp named — the same shape as the other refusals there.

**Schema impact: none.** `Map`'s stored shape is unchanged. `MAP_VERSION` stays at 6. A file
written today reads today. The change is in what the loader *returns*, not in what it accepts.

### `bevy_emerge_core::census` — the count the author reads is the count the game has

`of_map` gains the expansion and a `stamped` count, and returns a `Result`:

```rust
pub fn of_map(map: &Map, compositions: &[Composition], library: &Library)
    -> Result<MapCensus, String>
```

The module's own note said the counter deliberately does not expand, *"because expansion can
legitimately fail (a member with nowhere to rest) and a counter that can fail is a counter every
caller has to branch on"*. **That reasoning was sound for a counter of *references* and wrong for
a counter of *rows*.** `map.stamps.len()` cannot fail; the row count can, and a row count that
quietly omitted a house would be a number nobody could check. So the two are separate numbers and
the one that can fail is a different counter from the one that cannot.

`MapCensus::total()` is the number the consumer will have.

### `bevy_emerge_mapper` — the panel and the gate

- `editor.rs` `Field::Map`: the count becomes the **total** the consumer has, with a breakdown of
  where it came from. A stamp is a reference, so its rows are derived — and a panel saying "727
  placed" beside a game holding 745 is a panel lying.
- `project.rs` `OpenMap::save`: the gate already expanded the stamps to check the map loads. It
  now expands them **the same way the consumer does** — through `Map::expanded` — so the two
  cannot drift. Before this, the gate proved the file was loadable while the game did not load it
  at all: the gate was on the wrong side of the file.

## The consumer (Sanctuary) — its own uncommitted change

`town_map::check_ids` walks `map.placements` and refuses a descriptor the game does not know.
With the loader expanding, the map now carries the stamped rows, and their descriptors are kit
pieces the game legitimately does not know.

The game's side therefore **exempts the rows a group brought** — and the exemption is read from
the map's own stamp list through `Map::stamp_of`, not from the shape of the id. The rows a group
brought are that group's business: a house's wall panels are the house's business, chosen by the
group, not by the town.

The game also does not *place* those rows (`props` skips them): the group places them, with the
group's own style.

## Touched modules

| Crate | Module | Change |
|---|---|---|
| `bevy_emerge_core` | `composition.rs` | `expand_map`; `stamped_id` / `split_stamped_id` — the id convention, named once |
| `bevy_emerge_core` | `map.rs` | `Map::expanded`; `Map::stamp_of`; `MAP_VERSION` unchanged |
| `bevy_emerge_core` | `project_data.rs` | `load` expands each map and refuses a map whose stamps do not expand |
| `bevy_emerge_core` | `census.rs` | `of_map` gains the expansion and a `stamped` count; `MapCensus::total()` |
| `bevy_emerge_mapper` | `editor.rs` | the inspector's count shows the total the consumer has |
| `bevy_emerge_mapper` | `project.rs` | the save gate expands the same way the consumer does; the open log counts the rows too |
| Sanctuary (consumer) | `src/prototype/town_map.rs` | `check_ids` and `props` skip the rows a stamp brought |

## Tests

**`crates/bevy_emerge_core/tests/stamps_reach_the_consumer.rs`** (11 tests) — the contract:

1. the consumer gets the rows a stamp stands for, with the same ids and positions;
2. the editor's expansion and the consumer's expansion are the same rows;
3. a stamp that cannot expand refuses the load, naming the stamp and the group;
4. the reference survives the expansion;
5. the census counts the rows the consumer has;
6. a map with no stamps is unchanged;
7. a `/` in a hand-placed id does not make it a stamped row;
8. a stamped row names its stamp and its member;
9. a map with stamps **and** compositions **and** rules round-trips — the area the 2026-10-20
   review listed as uncovered;
10. expanding an already-expanded map refuses rather than doubling the rows;
11. the loader is in the engine-free crate.

**`crates/bevy_emerge_mapper/tests/the_editor_and_the_game_agree.rs`** (3 tests) — the claim that
spans two crates: the editor's drawn rows and the game's loaded rows are the same rows; the saved
file holds a reference and not the rows; the editor's count and the consumer's count agree.

**Existing tests that had to change** — `census_is_the_one_counter.rs`, because `of_map` now takes
the compositions and the library and returns a `Result`. The ratchet in that file caught one real
mistake during this work: a log line that counted `map.placements.len()` outside the census.

**Sanctuary's own suite** — the game builds and passes after its `check_ids` and `props` change.

## Schema impact

**None on disk.** `Map`'s stored shape is unchanged; `MAP_VERSION` stays 6. The change is in what
the loader *returns*.

## Risks

- **The expanded map is bigger.** A 282-row fence group would be 282 rows in memory per stamp.
  Bounded by `MAX_RESOLVED_MEMBERS` (256) per stamp and by the depth cap.
- **Two readers of `map.placements`.** The editor's `OpenMap` holds the *unexpanded* map (it
  must, to save the reference); the consumer's holds the expanded one. The editor's count uses
  the expansion. One reader, one meaning — audited by the census test.
- **The game sees rows it does not know.** Handled by `check_ids` and `props` skipping the rows a
  stamp brought, using the structured `Map::stamp_of`.
