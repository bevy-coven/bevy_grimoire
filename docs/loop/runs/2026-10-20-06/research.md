# Research — the editor can read a group but not name it

Run: 2026-10-20-06. Feature: **an author renames a composition from the editor, and every map that
stamps it follows.**

---

## The measurement that started this

I drove the real editor over BRP against a copy of Sanctuary's project and tried to **rename a
composition**.

### What the COMPOSE tab offers

The COMPOSE tab lists **sixteen house assemblies** — `town/house_cottage_plaster` (18 members),
`town/house_cottage_brick`, `town/house_cottage_wood`, `town/house_town_house_plaster` (32
members), and so on — with their members, their edge tokens, their faults and their STALE list. It
is a **rich, careful read view**.

**And it cannot change a name.** Measured, every route:

| Route | What happened |
|---|---|
| **The key overlay (`K`)** | The tab's only row is **`Cmd+C` — copy this tab's text**. No rename. |
| **`F2`** | Nothing. The status line did not change. |
| **`F3`** | Nothing. |
| **`N`** | Nothing — `N` is the *map's* rename, and the COMPOSE tab has no map. |
| **Clicking a composition row** | The row took the selection; the status line did not change. |

**There is no route.** The status line's only content is a global one (*"labeler: not
configured"*), so a refused attempt is not even visible.

### What that costs, measured

`compositions.ron` is **5,818 lines and 204 KB** — and it is **hand-written**, because the editor
cannot write it. Every house's eighteen to thirty-eight members were typed by hand.

**And the editor's own capture makes new ones.** `M` on the Map door captures a selection as a
group and commits it through `Project::commit_composition` — so the editor *writes* the file, and
**the write re-serializes the whole thing**, dropping every comment and reformatting 204 KB.

### The shape of the fix, and what it costs

**A composition's id is one line.** Measured in Sanctuary's file:

```
6:            id: "town/house_cottage_plaster",
266:            id: "town/house_cottage_brick",
526:            id: "town/house_cottage_wood",
```

**One line each, at a known indentation.** And `ron_surgery::replace_field(line, key, value)`
rewrites *one field of one line*, keeping the author's padding and the trailing comment.

**And a composition id is referenced in exactly two places:**

| Where | What |
|---|---|
| `compositions.ron`, the composition's own `id:` line | **1** |
| every map's `stamps[].of` | one per stamp |

**The game's own code is safe.** `src/prototype/houses.rs` discovers compositions by **prefix**
(`town/house_*`), not by name — so a rename does not strand the game.

**And the fingerprint is NOT affected — which is worth saying, because it is the obvious fear.**
The composition's own id *is* folded into its fingerprint (`f.str(&comp.id)`, `composition.rs:1108`),
so a renamed composition is a different composition *as a fingerprint*. **But nothing compares
against that fingerprint.** `Stamped::of_fingerprint` is *"what the composition looked like when this
was stamped"* and **no code ever writes it** — it is `None` on every stamp in the shipped file, and
`None` means *"never recorded"*, which is a different fact from stale. The `Stale` badge reads
`Member::of_fingerprint`, which records the **descriptor's** fingerprint, not the composition's id.

**So a rename strands exactly one thing: every map's `Stamped.of`.** Not the fingerprints, not the
members, not the envelope — the name the stamp points at.

### The runner-up, and why it is not this run

**Renaming a kit's namespace** is the same idea with a much larger blast radius. Measured over
Sanctuary's project:

| File | Mentions of `town/` |
|---|---|
| `maps/town.map.ron` | **383** |
| `maps/town_block.map.ron` | **289** |
| `town/library.ron` | **49** |
| `compositions.ron` | **16** |
| `rules.ron` | **7** |
| | **744 mentions, 4 files** |

**And there is no multi-file atomic primitive.** `save_atomic` is per file, and a scout's finding is
that *"a half-finished kit rename leaves a project that fails to open, because `bound_library` will
refuse a namespace mismatch whichever file is written first."*

**That is a separate, larger, and more dangerous feature.** This run carves the **composition
rename**, which is one file's one line plus the stamps that name it — and which the editor's own
COMPOSE tab is *about*.

---

## The literature

### Rename is a refactoring, and the hard part is the references

Feldthaus & Møller, *Semi-automatic rename refactoring for JavaScript* (2013,
[10.1145/2544173.2509520](https://doi.org/10.1145/2544173.2509520)) — the paper that names the
problem this run has:

> "To perform renaming, which is one of the fundamental refactorings, there is often **no practical
> alternative to simple syntactic search-and-replace**."

> "We propose a technique for **semi-automatic** refactoring for JavaScript, with a focus on
> renaming. Unlike traditional refactoring algorithms, semi-automatic refactoring works by **a
> combination of static analysis and interaction with the programmer**."

**"Semi-automatic" is the shape of this feature.** The tool knows the references — it has the whole
project in memory — and the author supplies the intent. **The editor is not a text editor over a
file; it knows what refers to what.** A search-and-replace over `compositions.ron` would rename the
composition's id *and* every `Stamped.of` *and* every `of_fingerprint` — and the last one is a
**number**, not a name, so a text search cannot touch it at all.

### The precondition is the feature

Overbey, *A toolkit for constructing refactoring engines* (2012) — on the **precondition check**,
which is *"the procedure which determines whether or not a refactoring can be performed"*:

> "It identifies preconditions as checking three properties: **input validity, compilability, and
> preservation**."

**A rename has all three**, and this project already has the machinery for two of them:

| precondition | this project's answer |
|---|---|
| **input validity** | `naming::is_id` — *"must start with a lowercase letter, must not end in `_`, and must not contain `__`"* |
| **compilability** | `composition::validate` — the project **refuses to open** a composition that does not resolve |
| **preservation** | the **fingerprint** — `f.str(&comp.id)`, so a renamed composition is *provably* a different one |

**And the editor already refuses a rename it cannot keep.** The Meshes tab's `I` field renames a
*candidate*'s id, and it says:

> *"`{id}` is in the library — renaming it would strand every placement that names it"*

**That refusal is this project's own statement of the rule**, and it is the precedent this feature
follows.

### Referential integrity is the thing being maintained

Kappel & Schrefl, *Local referential integrity* (1992) and Wei, Leck & Link, *Entity Integrity,
Referential Integrity, and Query Optimization with Embedded Uniqueness Constraints* (ICDE 2019,
[10.1109/ICDE.2019.00175](https://doi.org/10.1109/ICDE.2019.00175)) — the database literature's
framing of what a rename is: **an operation that must maintain every reference to the renamed
entity, atomically**.

**And that is the shape of the fix.** The rename is:
1. the composition's own `id:` line, in `compositions.ron`;
2. every `Stamped.of` naming it, in every map;
3. every `Stamped.of_fingerprint`, in every map — **recomputed, not searched**, because the
   fingerprint is a hash of the id and the new id has a new hash.

**Two is the number that matters:** the name, and the reference. And the second is **not** a
string the author can find — it is on a different file, in a different collection, and a text search
over `compositions.ron` finds the name and nothing else. **The tool knows where the references are
because it has the whole project in memory.**

### The fingerprint is not in the way — and the measurement says so

The composition's own fingerprint is folded in at `composition.rs:1108`: `f.str(&comp.id)`. So a
renamed composition is a **different composition** as far as that fingerprint is concerned.

**And nothing compares against it.** The obvious fear — *"the rename will leave every map STALE"* —
is **false, and the reason is worth recording**: `Stamped::of_fingerprint` is *"what the composition
looked like when this was stamped"*, and **no code writes it**. It is `None` on every stamp in the
shipped file, and `None` means *"never recorded"* — *"a group that has never been measured is not a
group that drifted"*. The `Stale` badge reads `Member::of_fingerprint`, which records the
**descriptor's** fingerprint.

**So the rename's whole job is the name and the reference.** The group's members, its envelope, its
note and its member fingerprints are **untouched** — the author changed what it is *called*, not
what it *is*. **Which is the whole feature in one sentence: a rename that leaves the maps pointing
at the old name is a rename that stranded them.**

---

## The candidates, ranked

### 1. **Rename a composition, and every map that stamps it follows.**

**Measured cost:** one line in one file, plus the stamps that name it. The editor can already do
the hard part — it knows the references and it can recompute the hash — and the COMPOSE tab is
already the surface where an author thinks about a group.

**Why it beats the runner-up:** the runner-up (renaming a kit's namespace) is **744 mentions across
4 files with no multi-file atomicity**, and a half-done rename **leaves a project that will not
open**. This one is bounded, it is one file plus its stamps, and **the editor's own tab is where it
belongs**.

### 2. Renaming a kit's namespace.

744 mentions, four files, and no atomic primitive. **A separate run, with the atomicity designed
first.** It is also the one that would need the most care and is the easiest to get wrong.

### 3. The other four missing operations — project rename, project delete, bash creation, policy
patch add.

Real, and each is a new on-disk writer with its own re-parse-before-save discipline. The policy
patch is the hardest: a `Patch` has a multi-line `because` and a nested `Descriptor`, so
`LineDoc::append` — the one-line-record tool — is not a direct fit.

### 4. `fill.rs` has no test at all.

1,058 lines, the one untested module in the mapper, and the module that lays the bulk of a level.
Worth doing, and not a feature an author feels.

### 5. `vlm.rs:1436-1442` — `probe` returns `Ready` without connecting.

A LAN endpoint that is down is reported ready. Needs a test that calls `probe`, not `is_near`.

---

## What great looks like

Acceptance criteria, phrased so a reviewer could fail the implementation against them.

1. **The COMPOSE tab can rename the composition it is showing.** A key opens a name field, the
   author types, `Enter` keeps it and `Esc` leaves it alone — **the same shape as the Meshes tab's
   `I` field**, because it is the same verb.
2. **The name is forced as it is typed** — snake_case, lowercase, no `__`, no trailing `_` — so an
   illegal id is never reachable rather than merely rejected at the end.
3. **A rename that would collide is refused, by name.** Two compositions cannot share an id, and the
   refusal says *which* name it collided with.
4. **Every map that stamps the composition follows.** The stamp's `of` names the new id. **A rename
   that leaves the maps pointing at the old name is a rename that stranded them** — and the map
   would refuse to open, because a stamp naming a composition that does not exist is a dangling
   reference.
5. **The file's comments survive.** `compositions.ron` is hand-authored and 204 KB; a rename must
   not reformat it. The mechanism is the one that already exists: `ron_surgery`, editing the one
   line and writing the file around it.
6. **A refusal, not a degradation.** A rename that cannot be completed — a collision, a map that
   cannot be written — changes **nothing**: the old name stands, the old file stands, and the reason
   is on screen.
7. **The composition's members are untouched.** A rename changes a name and nothing else: the
   members, the envelope and the note are the same group.
8. **The game is not stranded.** The game's own code discovers compositions by prefix, so a rename
   needs no change on its side — and this is checked, not assumed.

## Gaps

The literature does not settle three things, and this project must decide them on its own.

- **Whether a rename should be refused when a map cannot be written.** A rename that updates the
  composition and leaves one map behind is a *partial* rename, and a partial rename is a project
  that will not open. **The safe answer is to refuse the whole thing** — but that means a
  read-only map blocks a rename, which is a refusal over a refusal.
- **Whether the stamp's `of_fingerprint` should be written at all.** It is `None` on every stamp in
  the shipped file, and `None` means *"never recorded"*. A rename could leave it alone, or it could
  be the moment to record it — which would make a *later* member change visible as STALE. **The
  measurement says the first is what the code does today, and the second is a new rule**.
- **Where the verb lives.** The COMPOSE tab is the only tab that shows a composition, so the verb
  belongs there — but the *name field* is a control, and the editor's census counts controls. So the
  rename needs a home that says what it is, and the answer is not free.
