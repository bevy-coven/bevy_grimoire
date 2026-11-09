# Plan — the composition can be renamed

Run: 2026-10-20-06. Feature: **an author renames a composition from the COMPOSE tab, and every map
that stamps it follows.**

---

## The decision

**Two changes, in this order, each small and each independently testable.**

1. **The core grows one writer**: `composition::rename_composition`, which renames a composition in
   `compositions.ron` **and** in every map that stamps it, **without reformatting either file**.
2. **The COMPOSE tab grows the verb**: a name field, the same shape as the Meshes tab's `I` field,
   with the same refusals.

**And the name of the thing is the whole point:** the editor can *read* a group in rich detail and
*write* a new one, but it cannot change a group's **name** — which is the one edit every author
needs and the one the file's own format makes trivial (one line).

## Why the composition and not the kit

**Measured, the two are not the same size.**

| | a composition rename | a kit namespace rename |
|---|---|---|
| Files touched | **1**, plus the maps that stamp it | **4** |
| Mentions to rewrite | **1** | **744** |
| The id's line | **one line**, at a known indentation | scattered through `library.ron` |
| Atomicity | **one file at a time, and the maps are the author's** | **none** — a half-done rename **leaves a project that will not open** |
| The tool's edge | the editor knows every stamp | the editor must rewrite ids in four formats |

**A kit rename is a different feature with a different risk**, and it wants the atomicity designed
before the verb. This run takes the bounded one, and the bound is real: **one line in one file, plus
the references the tool can enumerate.**

## The shape, in detail

### 1. The core writer

**`bevy_emerge_core::composition::rename_composition`** — a function that takes the *text* of
`compositions.ron`, the *text* of every map that stamps the composition, and the old and new ids,
and returns the new texts.

**It edits, it does not re-serialize.** The mechanism is the one that already exists:

- **`ron_surgery::scan_ron_leaves`** finds every scalar in the file with its path and its span.
- **`ron_surgery::save_atomic`** writes the result.

**Measured, the leaves are addressable.** The probe on Sanctuary's own files:

```
compositions[0].id                                 text="town/house_cottage_plaster"
compositions[0].members[0].id                      text="p00_wall"
compositions[0].members[0].body.id                 text="wall"
stamps[0].of                                       text="town/house_cottage_plaster"
```

**Why the span surgery and not `LineDoc`.** A `Composition` is a **multi-line block** and a `Member`
is **thirteen lines** — `LineDoc`'s records are the lines that start with `(`, so it addresses a
one-line record and **cannot see either**. The span surgery can: the composition's `id` is a leaf at
the path `compositions.[3].id`, and a map's stamp's `of` is a leaf at `stamps.[0].of`.

**The paths are the reason the span surgery exists.** A rename is not *"find the string"* — it is
*"find the leaf at this path"*, and the two are different. `scan_ron_leaves` gives the path, so the
rename says *which* leaf it means and the tool refuses to touch a leaf it did not name.

### And the measurement that says the surgery is worth it

**`compositions.ron` would survive an ordinary rewrite** — the probe round-trips the shipped
204 KB file **byte-identically**, and the crate's own rule is that prose lives in a `note` field.
**A map would not.** The probe round-trips `town.map.ron` from 75,089 bytes to **228,856** — the
writer expands every one-line placement into six lines. That is the editor's **existing** save
behaviour, so it is not a regression; but a *rename* that reformats 229 KB of a hand-authored map
is a large diff for a one-word change, and the surgical edit avoids it.

**So the two files are edited by two different means, and the reason is measured, not aesthetic:**
the composition's file is rewritten (nothing to lose), and the maps are **edited in place** (a
lot to lose, and the map is the author's).

**And the refusal is the design.** The Meshes tab's `I` field already says:

> *"`{id}` is in the library — renaming it would strand every placement that names it"*

**So the rename refuses a name that is already taken, and it refuses a rename that would leave a
stamp pointing at nothing.** Both refusals happen **before** anything is written.

### 2. What the rename touches, exactly

| File | What | How |
|---|---|---|
| `compositions.ron` | the composition's own `id:` line | **one leaf**, at the path `compositions.[i].id` |
| `compositions.ron` | the composition's **members'** ids | **nothing** — a member's id is its own name, not the composition's |
| every map's `stamps[].of` | the reference | **one leaf** per stamp, at the path `stamps.[i].of` |
| every map's `stamps[].of_fingerprint` | — | **nothing** — it is `None` on every stamp, and `None` means *"never recorded"* |
| the `note:` fields | — | **nothing** — a note is prose about the group, and a rename does not rewrite prose |

**And the fingerprint is not in the way.** The composition's own id *is* folded into its fingerprint
(`f.str(&comp.id)`), so a renamed composition is a different composition *to the fingerprint* — **and
nothing compares against that fingerprint.** `Stamped::of_fingerprint` is *"what the composition
looked like when this was stamped"* and **no code writes it**. The `Stale` badge reads
`Member::of_fingerprint`, which records the **descriptor's** fingerprint. **The members' fingerprints
are untouched, because the members are untouched.**

### 3. The verb

**One row in `BINDINGS`.** The rule is *"A new verb adds one row to `BINDINGS` in `keys.rs`, nowhere
else."*

**The key is `I`**, because `I` is already *"type an id"* on the Meshes tab — and the two are **the
same verb**: *"say what this thing is called"*. The Meshes tab's `I` renames a *candidate*'s id;
the COMPOSE tab's `I` renames a *composition*'s id. **One verb, two subjects, two contexts**, and
the census can tell them apart because a tab's context is in the binding.

**`Context::Compose` and `Context::Meshes` never overlap** — a tab overlaps only itself — so the
same key is free in both, and the badge says what each does in its own tab's words.

**The name field's home.** The Meshes tab's field is `ControlId::IdField`; the COMPOSE tab needs its
own, because **two visible nodes claiming one id is a bug** — the ratchet
`every_control_the_census_homes_a_verb_at_is_on_screen` says so, and it is the same ratchet that
caught the picker's duplicate in the last run. So a **new `ControlId`** for the COMPOSE tab's field,
and the badge goes there.

**And the name is forced as it is typed** — `naming::to_snake_case`, the same rule the Meshes tab's
field uses, so an illegal id is never *reachable* rather than merely rejected at the end.

### 4. The refusals

**Four, and each is a refusal rather than a degradation:**

1. **A name that is already taken.** Two compositions cannot share an id, and the refusal says
   *which* name it collided with.
2. **A name that is not a legal id.** Empty, or not snake_case, or `__` in it, or ending in `_`.
3. **A map that cannot be read.** A map that stamps the composition and will not parse is a map the
   rename cannot follow — and the rename **refuses**, because a rename that updates some maps and
   not others leaves a project that will not open.
4. **A map that cannot be written.** Same shape, same reason.

**And the order:** every refusal is checked **before** anything is written. A rename that fails
half-way changes **nothing**: the old name stands, the old file stands, and the reason is on screen.

## Touched modules

| file | what changes |
|---|---|
| `crates/bevy_emerge_core/src/composition.rs` | `rename_composition` — the writer, and the paths it edits |
| `crates/bevy_emerge_core/src/ron_surgery.rs` | only if the span surgery cannot address a leaf inside a nested block — **to be measured first** |
| `crates/bevy_emerge_mapper/src/compose.rs` | the name field, the verb, the refusals |
| `crates/bevy_emerge_mapper/src/keys.rs` | **one row** in `BINDINGS`, and the `ControlId` |
| `crates/bevy_emerge_mapper/src/project.rs` | the call into the core writer, and the map's save |
| `crates/bevy_emerge_core/` | **the schema does not change.** `COMPOSITIONS_VERSION` stays 1, `MAP_VERSION` stays 6 |

**`bevy_emerge_core` stays engine-free** — the writer is pure text in, pure text out, and the crate's
`engine_free.rs` ratchet proves it.

## Schema impact

**None.** A composition's id is a `String` with no format rule in the schema, and a map's stamp
names a composition by that same string. **No file changes shape, no version moves, and nothing is
migrated** — the file is *edited*, which is the strongest form of "no migration" there is.

## Tests

1. **The rename edits the file, and does not reformat it.** A fixture with a hand-written note and
   a `//` comment: after the rename, **the comment is still there, byte for byte**, and the rest of
   the file is unchanged. This is the test that fails if the writer reaches for `to_ron`.
2. **Every map that stamps the composition follows.** A map with two stamps of the composition and
   one of another: after the rename, the two name the new id and the third is untouched.
3. **A map with no stamp is untouched.** Byte-identical.
4. **A name that is already taken is refused**, and **nothing is written** — the file is
   byte-identical.
5. **A rename that would leave a stamp dangling is refused** — the same shape.
6. **A composition that does not exist is refused.**
7. **The members are untouched.** The renamed composition has the same members, the same envelope
   and the same note — only its id changed.
8. **Round trip.** Rename, then parse: the parsed file has the composition under the new id, with
   everything else the same. **A metamorphic test** — rename A→B then B→A gives back the original
   text.
9. **The game is not stranded.** The game's own code discovers compositions by prefix, so a rename
   needs no change on its side. This is checked against Sanctuary's `houses.rs`, not assumed.

## Risk

| risk | how it is answered |
|---|---|
| **A partial rename leaves a project that will not open.** | Every refusal is checked **before** anything is written, and the writer is given **all** the texts and returns **all** the new texts — the caller writes them only if every one succeeded. **The editor writes the composition and the maps together, or writes neither.** |
| **A rename re-formats a 204 KB hand-written file.** | The writer **edits one leaf** and writes the file around it. The test asserts the comment survives byte for byte. |
| **A name collision is accepted.** | Refused by name, before any write. |
| **The span surgery cannot address a leaf inside a nested block.** | **To be measured before the build.** `scan_ron_leaves` gives paths; if the path for a composition's id is not addressable, the answer is a *targeted* edit — the block's own `id:` line, found by its indentation — and the test still pins the comment. |
| **The core stops being engine-free.** | The writer is pure text in, pure text out. `engine_free.rs` proves it. |

## Not in this run

- **Renaming a kit's namespace.** 744 mentions across four files, and no multi-file atomicity — a
  half-done rename **leaves a project that will not open**. It wants the atomicity designed first,
  and it is a separate run.
- **Deleting a composition.** The same writer, with the opposite direction — and the maps that
  stamp it would have to lose their stamps, which is a *deletion* decision rather than a rename.
- **Project rename, project delete, bash creation, policy patch add.** Four more writers, each with
  its own re-parse-before-save discipline. The policy patch is the hardest: a `Patch` has a
  multi-line `because` and a nested `Descriptor`, so `LineDoc::append` — the one-line-record tool — is
  not a direct fit.
