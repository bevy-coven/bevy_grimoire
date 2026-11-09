# Research — the stamped group the game never sees

Run: 2026-10-20-01. Feature: **a reusable group stamped on a map must reach the consumer game.**

## The problem, in one sentence

An author can draw a fence run once, keep it as a named group, and stamp it on the map — the
editor draws the group's rows and the map file holds one reference — but the game that loads that
map **never sees the rows**, so the level the author sees and the level the player walks are
different worlds.

## Sources

| # | Source | What it was used for |
|---|---|---|
| S1 | Shirley, M. et al., *Procedural Content Generation in Games*, ACM Books, 2018 — ch. 1 "Introduction" and ch. 11 "Mixed-Initiative Content Creation" (PDF in the local corpus: `pcgbook-ch01-introduction.pdf`, `pcgbook-ch11-mixed-initiative-content-creation.pdf`) | The rule that a tool's output must be reachable by the machine that consumes it, and the "can the computer explain itself?" question |
| S2 | Mo, K., Guerrero, P., Yi, L., Su, H., Wonka, P., Mitra, N. J., Guibas, L. J., "StructureNet: Hierarchical Graph Networks for 3D Shape Generation", *ACM TOG* 38(6):242, 2019, DOI [10.1145/3355089.3356527] | Why one uniform recursive type beats a ladder of levels, and the encoding invariant that stops the same set having several encodings |
| S3 | Mokhov, S., Mitchell, N., Peyton Jones, S., "Build Systems à la Carte", *Proc. ACM PL 2(ICFP):79*, 2018, DOI [10.1145/3232787.3232789] | Provenance and dirty-checking: a dependent must be told when its input changed, not silently re-read |
| S4 | Spall, S., Mitchell, N., Tobin-Hochstadt, S., "Forward Build Systems, Formally", arXiv:2202.05328, 2022 | The correctness notion for a build that *derives* rows from a source: the derivation must be equivalent to rebuilding everything |
| S5 | Lamb, C., Zacchiroli, S., "Reproducible Builds: Increasing the Integrity of Software Supply Chains", *IEEE Software* 38(6), 2021, DOI [10.1109/MS.2021.3073045] | The trust argument: a build whose output cannot be checked against its source is a build whose output nobody can trust |
| S6 | Raistrick, A. et al., "Infinigen Indoors: Photorealistic Indoor Scenes using Procedural Generation", arXiv:2406.11824, 2024 | Spatial-relation constraints and the *user-controllable parameter* model — a generator the author steers rather than one that overrides |
| S7 | Mo, K. et al., "ShapeAssembly: Learning to Generate Programs for 3D Shape Structure Synthesis", arXiv:2009.08026, 2020 | The "one program, many shapes" idea: a single authored program with parameters standing for a family of related content |
| S8 | Tutenel, N., Smelik, R., Bidarra, M., de Kraker, M., "A Semantic Scene Description Language for Procedural Layout Solving", *AIIDE 2010* | A support is a first-class surface feature; a prop's vertical position falls out of its support's height rather than being guessed |

## Technique

### 1. A derived level must be derivable by the consumer, or the authoring is a lie

S1's mixed-initiative chapter sets the bar a tool has to clear before a designer can trust it:

> "It is difficult for a human and computer to engage in a design collaboration if neither is able
> to explain itself to the other. In particular, a human designer may become frustrated or confused
> if the computer consistently acts as though it is not following the model that the human designer
> has in her head for how the system should work."

and, on expressivity:

> "All content that a human can produce using a mixed-initiative PCG system must be possible for the
> computer to generate on its own."

The tool's counterpart obligation, which is the one this run is about: **all content the tool
displays must be obtainable by the consumer of the file.** An editor that draws 18 rows the game
does not have is a designer "acting as though it is not following the model that the human designer
has in her head" — the model being "the map is the level".

S4 gives the formal shape of the obligation. A build that derives rows from a source must be
*equivalent to rebuilding everything*; the correctness notion is not "the derived rows look right"
but "the consumer can reproduce them from the source it was given". The consumer here is given
`map.placements` and `map.stamps` and reads only the first. So the derivation is not performed by
the consumer, and the level it runs is not the level the file describes.

S5 is the trust argument in its sharpest form — "if we can guarantee that building a given source
tree always generates bit-for-bit identical results, we can establish trust in these artifacts".
The editor's own save gate already half-implements this: `OpenMap::save` refuses to write a map
whose stamps do not expand, with the note *"the editor saying saved while the game says broken is
the exact drift the shared `bevy_emerge_core` validation exists to prevent"*. The gate checks that
the stamps **can** expand. Nothing checks that the game **does** expand them. The gate is on the
wrong side of the file.

### 2. A reference, not a copy — and the encapsulation that keeps it honest

S2 is the reason the map holds a reference rather than the rows:

> "Such object structure can typically be organized into a hierarchy of constituent object parts
> and relationships, represented as a hierarchy of *n*-ary graphs."

StructureNet abandoned GRASS's fixed binary hierarchy and replaced it with **one uniform
recursive type**, because an imposed arity *"can introduce arbitrary ordering in nodes and make the
hierarchy inconsistent"*. The fix is an encoding invariant: the same set must not have several
encodings, or two authors would produce diffs that differ without meaning to.

`bevy_emerge_core::composition` already carries this: `Composition::members` is stored in a
canonical order (sorted by `Member::id`), and parenthood is author-declared through `on`, never
inferred from an ordering heuristic. The module's own note names the risk it guards:

> "one uniform recursive type makes the same set expressible as several different trees. Two
> authors — or one author twice — would then produce diffs that differ without meaning to."

The same chapter carries the discipline that keeps a reference from going stale: **encapsulation**.
A stamp may override a member's *values*; it may not delete a member, re-parent one, or reach
into a nested composition's internals. The `Override` type enforces this in the schema
(`member` is the composition's *own* member id), and `expand` refuses an override naming a member
that does not exist. This is the right shape and must be kept.

### 3. Provenance: the dependent must be told when its input changed

S3's contribution is the one the `of_fingerprint` field already borrows: record what a dependency
looked like, and call the dependent dirty when a recomputation disagrees. `Member::of_fingerprint`
and `Stamped::of_fingerprint` are exactly that — a verifying trace, with `None` meaning *never
recorded*, which is a different fact from *stale*.

S3's other half is the one this run has to add: **a build system's result is a function of its
inputs, and the consumer must be able to run that function.** The stamped rows are a function of
(`compositions.ron`, the stamp's position, the library). The consumer is given the *output* of a
different function — the hand-placed rows — and never the function.

### 4. Parameters, not copies: the fence run is a program, not 282 rows

S7's "one program, many shapes" is the authoring model this feature should have: a fence run is a
short program (a piece and a pitch), and the map holds a few instances of it at different
positions. S6's user-controllable-parameter model is the same idea in a generator: the author
controls the parameters, the tool fills the rest.

The fence run in Sanctuary's town map is 282 rows of one descriptor. Its shape, measured from
`town.map.ron`:

| Group | Rows | Structure |
|---|---|---|
| `field_fence_*` | 240 | Four rectangular field enclosures, each a ring of posts on a 2.032 m / 1.016 m pitch, with the corner posts turned 90° |
| `grave_fence_*` | 42 | One graveyard enclosure, 12 posts along the north edge, 10 along the south, 10 down each side, with a gap in the south |

That is **two** shapes, not 282. The pitch is not the fence's own 2.064 m footprint: the fence
pieces sit on a 1.016 m lattice (half the tile), and the corner pieces are turned 90°. The
enclosures' *long* sides are on a 2.032 m pitch (a whole tile) with a half-tile infill. This is
what a "road and fence tool" produces, and it is what the editor's existing `F` fill and `Shift+B`
capture cannot: the fill lays a grid of *one* piece at *one* pitch with *one* yaw, and the capture
takes a set of pieces that are already placed.

## What great looks like

Acceptance criteria, phrased so a reviewer can fail the implementation against them.

1. **The consumer gets the rows.** A map with one stamp of an 18-member group, loaded through
   `bevy_emerge_core::project_data::load`, yields **18 additional placements** the consumer can
   spawn — the same 18 the editor drew. Measured, not asserted by construction: the test compares
   the editor's expansion with the consumer's.
2. **One expander, still.** `composition::expand` remains the single function that turns a stamp
   into rows. The consumer does not get a second implementation; it gets the *same* one, called
   from the loader.
3. **The rows are derived, not stored.** The map file still holds a reference. Editing the group
   still changes every map that stamped it. The file does not grow by 18 rows per stamp.
4. **A refusal, not a degradation.** If a stamp does not expand, the consumer's load **fails**,
   naming the stamp and the reason. It does not load a map missing rows, and it does not silently
   drop the stamp.
5. **The editor's save gate and the consumer's load agree.** The editor refuses to save a map the
   consumer would refuse to load, and neither refuses a map the other accepts. One rule, two
   callers.
6. **The count the author reads is the count the game has.** The inspector's `N placed` and the
   consumer's placement count are the same number for the same map. A stamp is a reference, so the
   count says what the game will have — not what the editor drew.
7. **Round-trip.** A map with stamps **and** compositions **and** rules, saved by the editor and
   reloaded by the consumer, yields byte-identical placements after a re-save. (This is the test
   area the 2026-10-20 review listed as uncovered.)
8. **Undo takes the stamp back.** Undoing a stamp removes the reference; the rows go with it,
   because they were never stored.

## Gaps

The literature does not settle three things, and this project must decide them on its own.

- **Where does the expansion happen?** S4 says the consumer must be able to reproduce the
  derivation, but says nothing about *whose* copy of the function runs. The options are: expand in
  the loader and hand the game an expanded map; or expand in a separate explicit step the game
  calls. This project already has a strong prior — `bevy_emerge_core` is engine-free and shared,
  and the crate's rule is "one path per feature" — so the decision is: **the loader expands, and
  the expansion is part of loading**, not a separate step a game might forget.
- **Do the expanded rows belong in `map.placements`?** If the loader expands into `map.placements`,
  then `Map::placements` no longer means "what the author placed by hand", and every reader that
  distinguishes the two (the editor's badge that says a stamp is STALE, the game's `check_ids`)
  must be updated. The alternative is a second list — which is the "second way to name the same
  thing" the crate's own rules forbid.
- **Does the editor keep the reference?** If the consumer expands at load, the editor must keep the
  reference too or the two disagree. The editor's whole reason for storing a reference — edit the
  group once, every stamped map follows — survives only if the reference stays the *stored* form
  and the expansion is a *read-time* projection in both places.

The corpus holds no write-up on Unity/Unreal/Godot prefab override semantics or on modern editor
stamping UX (Unreal PCG Framework, Houdini Copy-to-Points). The `paper_search` and `distill_search`
queries for those returned nothing relevant, so the design here rests on the academic sources above
and on the project's own stated rules, not on a survey of shipping tools.
