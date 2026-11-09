# Plan — one safe path for renaming a map, and a chooser that shows the right thing

Run: 2026-10-20-07. Feature: **the editor's story about a named thing matches the disk — a rename
either happens completely or is refused, and a panel never describes the wrong object.**

---

## The decision

**One feature, in four parts, each independently testable and each a defect measured in the
previous run's own project.**

1. **A rename never replaces a map and never leaves a duplicate.** The Map door's `N` and the
   chooser's `NAME` field become **one operation**.
2. **The refusal is the kernel's, not a check-then-act.** A no-replace primitive, so the target
   cannot appear between the check and the write.
3. **`Escape` leaves no draft**, so the chooser's panel describes the highlighted map.
4. **Clicking a row's value opens that row's field**, and a label and its value read as two things.

**Why these four together and not one of them.** They are the same defect seen four ways: **the
editor's account of a named thing and the disk's account of it come apart.** A rename that
replaces a map is that, at the level of the file. A panel that says "NEW MAP" over a selected
`town` is that, at the level of the screen. A click that opens ORIGIN when you clicked NAME is
that, at the level of the pointer. **And the brief's phrase for "most usable" is the one they
attack: an author gets a real project built faster and *trusts the result*.** A tool that can
destroy a map without saying so, or show a panel about the wrong map, is not trusted — and
everything else the editor does inherits that.

**Why not the runner-up.** The log's top item is **renaming a kit's namespace** — 744 mentions
across 4 files, with no multi-file atomicity. That is a *larger* feature, not a *more usable* one:
it happens **once per project**, and the load-time refusals (`library.rs:111-130`,
`kits.rs:478-486`) already stop a half-done rename from opening. **This run's defects happen on
every map, on every rename, and one of them destroys work.** A feature that fires once a project is
not worth a day; a feature that fires on every save is.

**Why not the policy patch add** (the scout's pick for developer impact). It is real: Sanctuary's
`kit/project.ron` carries **18 hand-written patch records** with the same `because` text, and the
append primitive (`LineDoc::append`) and the remove (`policy::remove_patch`) both exist. But **it
adds a new authoring surface**, and this run found that the editor's *existing* surfaces already
lie. **Adding a surface to a tool that misreports is not the most usable next thing.**

---

## The shape, in detail

### 1. One rename, in one place

**The rename becomes a core operation**, beside the composition rename the last run added:

| file | what changes |
|---|---|
| `crates/bevy_emerge_core/src/ron_surgery.rs` | **`save_atomic_no_replace`** — write to a tmp file, then move it into place **without replacing an existing target** |
| `crates/bevy_emerge_mapper/src/project.rs` | **`OpenMap::save` refuses a rename onto an existing map**, and **removes the old file** after the new one lands |
| `crates/bevy_emerge_mapper/src/chooser.rs` | the chooser's `NAME` field **reaches the same door** |

**The old file's removal is the part that is currently missing**, and it is the part that makes the
rename *a rename*. A rename that leaves the old file behind is a copy; the project's own map loader
already refuses the result:

```rust
// crates/bevy_emerge_mapper/src/project.rs:774
"{} calls itself `{}`. A map's name IS its file — rename the file to …"
```

**And the removal must not be a side effect of a keystroke.** The editor's comment says why the
current code does not remove it: *"deleting it here would destroy a file on a keystroke."* **The
removal belongs to the save**, where the new file has already landed and the rename is a fact.

**So the sequence is: validate → write the new file (no replace) → remove the old file → report.**
And each step's failure leaves the project in a state the editor can describe:
- the new file is written and the old one is not removed → the rename happened, and the editor
  **reports** the stale file by path, exactly as the chooser's route already does
  (`chooser.rs:5892-5897`);
- the new file cannot be written → **nothing changed**, and the reason is on screen.

### 2. The refusal is the kernel's

**`save_atomic` today ends in `fs::rename`, which replaces.** POSIX specifies that, and specifies
the fix as a different call: macOS's `renamex_np` with `RENAME_EXCL` fails with `EEXIST` and
**leaves both names untouched**.

**So the core grows `save_atomic_no_replace`**, and it is the core's because **the core is where
`save_atomic` already lives** and because the rule is the same everywhere a file is written.

**And `ENOTSUP` is a refusal, not a fallback.** A filesystem without the primitive gets
`ENOTSUP`, and the operation is **refused** — the research's criterion 10, and Lampson's *"Make
actions atomic or restartable"*. **No weaker path**, because a weaker path is the defect.

**The dependency question.** `renamex_np` is a libc call, and `bevy_emerge_core` has **no libc
dependency** — and `tests/engine_free.rs` fails on any `bevy*` dependency, not on libc, but the
crate's own rule is that it is the engine-free half. **`libc` is not a `bevy` dependency**, so it
is allowed; but the crate has no `unsafe` today, and adding it is a decision worth making
deliberately.

**The boring alternative, and why it is worse.** `fs::hard_link` + `fs::remove_file` is atomic
enough on POSIX and needs no libc — but it is two calls, so a crash between them leaves **two**
files, which is the duplicate defect again. **`renamex_np` is one call, and one call is the whole
point.**

### 3. The chooser: `Escape` leaves no draft, and the field you click is the field you get

**The `Escape` fix is one line, in the field handler.** The name field's `Escape` must clear the
draft it belongs to, because it is the only key that reaches that handler:

```rust
// crates/bevy_emerge_mapper/src/chooser.rs:5705
Key::Escape => {
    chooser.raw.clear();
    chooser.problem = None;
    chooser.editing = False;
    chooser.swallowed = true;
    // **And the draft goes with the field.** A `New::Map` left behind keeps the panel's
    // `(Some(New::Map(d)), _)` arm winning, so the panel describes a map that does not exist.
    chooser.creating = None;
    return;
}
```

**And the chord handler's `Escape` stays** — it is the one that clears a draft when **no field** is
open, which is the other half of *"Escape unwinds one layer at a time"*.

**The click fix is one line, in the click handler.** A settings row's click must set the field it
names:

```rust
// crates/bevy_emerge_mapper/src/chooser.rs:5598
RowPane::Settings => {
    chooser.focus = Focus::Settings;
    // **And the field the row names.** The mark is drawn on the row whose field matches
    // `chooser.field`, so a click that moved only the focus opened whatever field the arrows
    // had left open — clicking NAME opened ORIGIN.
    chooser.field = settings_field_for(row.index);
}
```

**Which requires the settings rows to carry their field**, so the click knows which one it is. The
rows are built in `settings_rows` with `Field::Name`, `Field::Bounds`, `Field::Origin`,
`Field::Note` — in that order — so the index into that list is the field.

**And the value must be read, not just the field set.** A click on a row's *value* should open that
field; a click on the row's *label* should move the cursor. **One click opens the field** is the
chooser's existing double-click convention (`on_row_click` acts on `click.count >= 2`), and the
settings rows are **not** in that list — the field is opened by the arrows and by `Enter`. **So the
click's job here is to move the cursor to the field, and the second click to open it.** The
simplest honest rule: **a click on a settings row moves the cursor to that row's field**, and the
author presses `Enter` or types to edit it — which is what the arrows already do.

### 4. The label and the value read as two things

**Measured: the gap between `new work lands here` and its value is 5 px; the other rows in the same
panel have 133 px.** The mechanism is that `row_label` uses `min_width` (a floor) and `info_row()`'s
`column_gap` is a **flex** gap — so a label *longer* than its column pushes the value right with no
gap at all.

**The fix is in `chrome::row_label`, and it is a floor on the gap rather than on the width:** the
label's box gets `min_width` **plus** the gap, so the value never starts at the label's last glyph.

**The measurement decides where the fix goes**, and the research says to measure rather than
assume: if `row_label` is the only place that can produce a zero gap, the fix is in `row_label`,
and **every panel that holds a phrase is fixed at once** — which is the design system's own
argument, *"one answer per panel, from its widest label"*.

### 5. The game's callsites, and what the editor says about them

**A map's name is a string in the game's code**, not a file the game discovers:

- `sanctuary/src/prototype/town_map.rs:35` — `pub const MAP: &str = "town";`
- `sanctuary/src/prototype/emerge_data.rs:28` — `const MAPS: &[&str] = &["town_block", "town"];`

**So a rename strands the game**, and the editor is the only place that can say so. **The rename's
receipt must name it.** This is the same argument as the composition rename's — *"the game's own
code discovers compositions by prefix, so a rename needs no change on its side — and this is
checked, not assumed."* **For a map it is the opposite: the game names the map exactly, so a rename
does need a change on its side, and the editor says so.**

**The consumer callsites this change breaks: none.** The rename's *behaviour* changes (the old file
is now removed, and a taken name is refused), but the game's API is untouched — it reads maps by
name from a list it owns. The change that *does* need a consumer-side note is the one an author
would make: renaming `town` in the editor means editing `town_map.rs:35` and `emerge_data.rs:28`.
**That is noted in `AGENTS_CHAT.md`,** and it is the owner's change to make, not the editor's.

---

## Touched modules

| file | what changes |
|---|---|
| `crates/bevy_emerge_core/src/ron_surgery.rs` | **`save_atomic_no_replace`** — the no-replace primitive, and its refusal |
| `crates/bevy_emerge_core/Cargo.toml` | **`libc`**, if the primitive is libc (see §2) |
| `crates/bevy_emerge_mapper/src/project.rs` | **`OpenMap::save`** — refuse a taken name, remove the old file, report a failed removal |
| `crates/bevy_emerge_mapper/src/chooser.rs` | the field's `Escape` clears the draft; a settings click sets the field; the `NAME` field reaches the same door |
| `crates/bevy_emerge_mapper/src/chrome.rs` | **`row_label`** — the gap survives a long label |
| `crates/bevy_emerge_mapper/src/editor.rs` | the Map door's `N` reaches the same door as the chooser's |

**`bevy_emerge_core` stays engine-free.** `libc` is not a `bevy` dependency, so `engine_free.rs` is
unaffected — **and the ratchet is checked either way.**

**The schema does not change.** A map's `name` field is a `String` with no format rule, and the
file's name is derived from it. **No version moves, nothing is migrated** — the files are renamed,
which is the strongest form of "no migration" there is.

## Tests

1. **A rename to a taken name is refused, and the other map's bytes are unchanged.** A checksum
   before and after. **This is the test that fails today.**
2. **A successful rename removes the old file.** One map, one file, one name — and the new file's
   own `name:` field agrees with its filename.
3. **A rename that cannot remove the old file is reported, and the new file stands.** The editor
   says which file is stale and where.
4. **The refusal is the primitive's, not a check's.** A test that creates the target **between** the
   check and the write must still be refused.
5. **`ENOTSUP` is a refusal**, and both names are unchanged. No weaker path.
6. **`Escape` from a name field leaves no draft.** The panel describes the highlighted map.
7. **A settings click sets the field.** Clicking NAME opens NAME; clicking ORIGIN opens ORIGIN.
8. **A name field refuses a number, and a triple field refuses a name** — each refusal names the
   field.
9. **The label/value gap is at least the panel's own gap**, for the longest label. **A measurement,
   not a mock** — the same pixel measurement the review made, as a test.
10. **The map door's `N` and the chooser's `NAME` field are one operation** — the same result, the
    same refusal, the same files on disk.
11. **Round trip.** Rename, then rename back: the file is **byte-identical** to where it started.
    **A metamorphic test** — the shape the last run's composition rename used.

## Risk

| risk | how it is answered |
|---|---|
| **The rename destroys a map.** | The no-replace primitive. The refusal is the kernel's, so a target that appears late is still refused. |
| **A rename leaves a duplicate.** | The old file is removed by the save, after the new one lands — and a failed removal is **reported**, not discarded. |
| **The core grows `unsafe` or a dependency.** | `libc` is not a `bevy` dependency and `engine_free.rs` still passes. The `unsafe` is **one `extern "C"` declaration**, and it is **the only way to get the primitive** — the boring alternative is two calls, which is the defect. |
| **The chooser's `Escape` change breaks the draft's own `Escape`.** | The chord handler's `Escape` stays, and it is the one that clears a draft when no field is open. **A test pins both.** |
| **The settings click change breaks the double-click convention.** | The click moves the **cursor**; the second click opens the field — the same `click.count >= 2` rule the chooser already uses. |
| **The `row_label` change moves every panel's columns.** | The change is a **floor on the gap**, not on the width, so a short label's column does not move. **A measurement pins it.** |
| **The game is stranded by a rename.** | The receipt **names the consequence**, and `AGENTS_CHAT.md` records the two callsites. |

## Not in this run

- **Renaming a kit's namespace.** 744 mentions across 4 files, and no multi-file atomicity. It wants
  the atomicity designed before the verb — and this run's `save_atomic_no_replace` is **one piece of
  that atomicity**, and it is the piece the other four files would need.
- **The other four missing editor operations** — project rename, project delete, bash creation,
  policy patch add.
- **Composition delete** — this run's composition rename with the opposite direction, and the maps
  that stamp it would have to lose their stamps.
- **`fill.rs` has no test at all** — 1,058 lines, the one untested module in the mapper.
- **`vlm.rs:1436-1442`** — `probe` returns `Ready` for any non-loopback host **without connecting**.
