# Research — the editor's story about a named thing must match the disk

Run: 2026-10-20-07. Feature: **one safe path for renaming a map, and a chooser that never shows a
fact about the wrong thing.**

---

## The measurement that started this

I drove the real editor over BRP against a copy of Sanctuary's project and tried to **rename a map**.

**There are two renames, and they disagree.**

| route | what it does | measured |
|---|---|---|
| **The chooser's MAP INFO `NAME` field** | refuses an existing target, writes the new file, removes the old | correct, but see below |
| **The Map door's `N`** ("rename map") | changes the name in memory; the path is re-derived on the next save | **unsafe** |

### What the Map door's `N` actually does

`OpenMap::save` derives the path from the map's **name**, not from where the file was:

```rust
// crates/bevy_emerge_mapper/src/project.rs:880
self.map_path = project.maps_dir.join(naming::map_file_name(&self.map.name));
```

and `ron_surgery::save_atomic` ends in a plain `fs::rename`:

```rust
// crates/bevy_emerge_core/src/ron_surgery.rs:735
std::fs::rename(&tmp, path).map_err(|e| format!("ron: {}: {e}", path.display()))?;
```

**POSIX specifies that `rename()` replaces an existing target.** From IEEE Std 1003.1-2024
([The Open Group](https://pubs.opengroup.org/onlinepubs/9799919799/functions/rename.html)):

> "if the directory entry named by *new* exists, it shall be removed and *old* renamed to *new*."

**So two things follow, and both are defects:**

1. **A rename to a name another map already has destroys that map.** The other map's file is
   replaced by this one's content, and nothing says so.
2. **A rename to a fresh name leaves a duplicate.** The old file is never removed — the editor's own
   comment says so: *"the file follows the name on the next save, and the old one stays where it is
   — deleting it here would destroy a file on a keystroke."* So the project ends up with **two files
   answering to one map**, and the stale one *"refuses to open with `<path>` calls itself `<name>`"* —
   a message about a schema mismatch, for a rename that half-happened.

**And the two routes disagree about what a rename is.** The chooser's own route
(`chooser.rs:5879-5897`) gets both right: it refuses an existing target **and** removes the old
file, and it *reports* a failed removal rather than discarding it. So the editor has **two answers to
"what does renaming a map do", and the wrong one is on the door the author is standing on.**

### The same failure, in the chooser's own panel

**The chooser's MAP INFO panel describes a map that does not exist, after one `Escape`.**

Reproduced at the keyboard:

1. Press `N` on the MAPS column. The draft opens: the panel reads **NEW MAP IN kit / NAME (needs a
   name) / BOUNDS 10 x 4 x 10**, and the name field is open.
2. Press **`Escape` once**. The field closes. **The panel still reads "NEW MAP IN kit"** — and the
   MAPS list highlights `town`, 727 pieces.
3. Move the arrows. **The panel does not follow.** It still describes the draft.
4. Press `Escape` a second time. **The panel snaps back** to `MAP INFO / NAME town / BOUNDS 300 x 60
   x 1120 / NOTE Sanctuary's whole t… / bash every kit` — and now it follows.

**The mechanism, from the code:**

- The name field's `Escape` (`chooser.rs:5705-5711`) sets `editing = false` and `swallowed = true`,
  and **does not clear `chooser.creating`**.
- The *only* other place that clears it (`chooser.rs:6161-6164`) is the chord handler — and
  `swallowed` is precisely what stops it from reading the same press.
- So a stale `New::Map` draft survives, and `map_rows`' match arm:

  ```rust
  // crates/bevy_emerge_mapper/src/chooser.rs:4457
  let (header, name, bounds, origin, note, bash) = match (&self.creating, self.current_map()) {
      (Some(New::Map(d)), _) => ( /* the new-map form */ ),
      …
      (None, Some(m)) => match &m.summary { /* the map's own facts */ },
  ```

  **the draft arm comes first and wins**, so the panel shows the new-map form for every map.

**So one `Escape` too few leaves a panel whose every fact is about an object that does not exist,
while the list beside it highlights a real one.** Nothing on screen says the panel is stale.

### And the field you click is not the field you get

**A third defect, in the same panel, and it is the sharpest.**

Clicking the **`NAME` row's value** — the word `town` — opens the **`ORIGIN` row's** field. Measured:
the `•` mark lands on ORIGIN, and the typed text goes there:

> `0 x 0 x 0town_block_` is not three numbers — type them like `32 4 32`

**The mechanism:** the settings rows are built with a `Field` each (`Name`, `Bounds`, `Origin`,
`Note`), and the `•` mark is drawn on the row whose field matches `chooser.field`. But the click
handler for a settings row only moves the **focus**:

```rust
// crates/bevy_emerge_mapper/src/chooser.rs:5598
RowPane::Settings => chooser.focus = Focus::Settings,
```

**`chooser.field` is never set**, so it stays wherever it was — and the arrow keys, not the mouse,
decide which field you are in. **An author who clicks the name of a map and types gets a number
parse error about the origin.** The field is drawn wrong, and it accepts the wrong input.

### A fourth, smaller one: the label and the value read as one phrase

`KIT INFO`'s row `new work lands here` / `no` renders with a **5 px** gap, so it reads as one
phrase. The other rows in the same panel have a **133 px** gap. Measured on the capture, glyph run
to glyph run.

`chrome::row_label` uses `min_width: Val::Px(width)` — a **floor** — and `info_row()`'s
`column_gap` is a **flex** gap, so a label *longer* than its column pushes the value right with no
gap at all. `new work lands here` plus the mark is 21 glyphs = 126 px, over `COL_WIDE` (96). The
doc's claim that a long label *"pushes its own value right instead of painting over it"* is true, and
the result still reads as one phrase.

---

## The literature

### A rename must not be able to replace a file

**POSIX `rename()`** ([IEEE Std 1003.1-2024](https://pubs.opengroup.org/onlinepubs/9799919799/functions/rename.html))
— the specification that explains the defect, and names the fix:

> "if the directory entry named by *new* exists, it shall be removed and *old* renamed to *new*."

> "If the *rename*() function fails for any reason other than [EIO], any file named by *new* shall be
> unaffected."

> Rationale: *"specifies behavior when the *new* parameter names a file that already exists. That
> specification requires that the action of the function be atomic."*

**The no-replace primitive exists.** macOS's `rename(2)` documents `renamex_np` with `RENAME_EXCL`:

> "[EEXIST] flags has RENAME_EXCL set but new already exists."

**So the refusal can be the kernel's, not a check-then-act race.** A pre-check followed by a rename
is two calls, and POSIX gives atomicity to the single call only.

**Lampson, *Hints for computer system design*** (SOSP 1983,
[10.1145/800217.806614](https://doi.org/10.1145/800217.806614)) — the principle behind "one path":

> "Make actions atomic or restartable."

**Schlichting & Schneider, *Fail-stop processors*** (ACM TOCS 1983) — the refusal argument, in one
sentence:

> "Such a processor automatically halts in response to any internal failure and does so before the
> effects of that failure become visible."

**A rename that half-happened is exactly a failure whose effects became visible.** The old file
gone and the new one not written is the state a fail-stop never reaches.

**Pillai et al., *All file systems are not created equal*** (OSDI 2014,
[10.5555/2685048.2685082](https://doi.org/10.5555/2685048.2685082)) — on why a two-step update
protocol is where correctness is lost:

> "applications use complex update protocols to persist state, and that the correctness of these
> protocols is highly dependent on subtle behaviors of the underlying file system, which we term
> persistence properties."

**Santry et al., *Deciding when to forget in the Elephant file system*** (SOSP 1999,
[10.1145/319151.319159](https://doi.org/10.1145/319151.319159)) — the file system's own view of what
an overwrite costs:

> "the file system to protect valuable data from accidental delete or overwrite."

### A panel that shows the wrong object is a gulf of evaluation

**Hutchins, Hollan & Norman, *Direct Manipulation Interfaces*** (HCI 1(4), 1985,
[10.1207/s15327051hci0104_2](https://doi.org/10.1207/s15327051hci0104_2)) — the *gulf of evaluation*
is the distance between the system's state and what the person can tell from the display. **A MAP
INFO panel reading "NEW MAP IN kit" while the list highlights `town` is that gulf, in full.**

**Norman, *Design rules based on analyses of human error*** (CACM 26, 1983,
[10.1145/2163.358092](https://doi.org/10.1145/2163.358092)) — the slip/mistake distinction, which
classifies the Escape defect exactly:

> "Slips are defined to be situations in which the user's intention was proper, but the results did
> not conform to that intention."

**The author's intention was to leave the field. The system kept a draft.** That is a slip with a
feedback failure — and the feedback is wrong too, because the panel then describes the wrong object.

**Gutwin & Greenberg, *A Descriptive Framework of Workspace Awareness*** (CSCW 2002,
[10.1023/a:1021271517844](https://doi.org/10.1023/a:1021271517844)) — *"the up-to-the-moment
understanding of another person's interaction with a shared workspace."* A detail pane bound to a
stale object fails that, and the *up-to-the-moment* is the part that matters.

### Undo, and the cost of asking twice

**Archer, Conway & Schneider, *User Recovery and Reversal in Interactive Systems*** (ACM TOPLAS 8,
1984, [10.1145/357233.357234](https://doi.org/10.1145/357233.357234)) — the founding statement:

> "Interactive systems, such as editors and program development environments, should explicitly
> support facilities that permit a user to reverse the effects of past actions and to restore an
> object to a prior state."

**Shneiderman, *Direct Manipulation: A Step Beyond Programming Languages*** (IEEE Computer 16(8),
1983, [10.1109/MC.1983.1654471](https://doi.org/10.1109/MC.1983.1654471)) — the reason undo is
preferred to a prompt:

> "Permit easy reversal of actions. This feature relieves anxiety, since the user knows that errors
> can be undone."

**Apple HIG, Alerts** ([developer.apple.com](https://developer.apple.com/design/human-interface-guidelines/alerts))
— when a prompt is and is not owed:

> "Avoid displaying alerts for common, undoable actions, even when they're destructive."

> "when people take an uncommon destructive action that they can't undo, it's important to display an
> alert in case they initiated the action accidentally."

**And the habituation literature** (*Tuning Out Security Warnings*, MISQ 2018,
[10.25300/misq/2018/14124](https://doi.org/10.25300/misq/2018/14124); *Polymorphic Warnings*, CHI
2015, [10.1145/2702123.2702322](https://doi.org/10.1145/2702123.2702322)) — warnings lose their power
when they are shown for things that are fine. **So a rename that is safe must not ask; a rename that
is not safe must ask once, and must be rare.**

**Nielsen, Heuristic 5 (Error Prevention)** ([nngroup.com](https://www.nngroup.com/articles/ten-usability-heuristics/)):

> "Either eliminate error-prone conditions, or check for them and present users with a confirmation
> option before they commit to the action."

**So: eliminate it, where it can be eliminated.**

---

## What great looks like

Acceptance criteria, phrased so a reviewer could fail the implementation against them.

1. **One path for renaming a map.** The Map door's `N` and the chooser's `NAME` field are **one
   operation**, with one answer. Not two implementations that agree today.
2. **A rename never replaces a map.** Renaming a map to a name another map already has is **refused,
   by name**, and the other map's bytes are **unchanged** — byte for byte.
3. **A rename never leaves a duplicate.** After a successful rename, the old file **does not
   exist**. One map, one file, one name — on disk and in the file's own `name:` field.
4. **The refusal is the kernel's, not a check-then-act.** The no-replace primitive, so the target
   cannot appear between the check and the write.
5. **A refusal changes nothing.** Both names stand, and the reason is on screen.
6. **Escape leaves no draft.** After one `Escape` from a name field, the chooser's panel describes
   **the highlighted map, or nothing** — never "NEW MAP".
7. **Every fact in a panel is about the highlighted object.** Clicking a row's value opens **that
   row's** field — NAME opens NAME, ORIGIN opens ORIGIN — and the `•` mark is on the field that is
   open.
8. **A field accepts what that field is.** A name field refuses a number, a triple field refuses a
   name, and the refusal names the field.
9. **A label and its value read as two things.** The gap between a label and its value is **at least
   the panel's own gap**, for the longest label in the panel.
10. **The game's callsites are listed.** A map's name is a **string in the game's code**
    (`town_map.rs:35` `MAP = "town"`, `emerge_data.rs:28` `MAPS = ["town_block", "town"]`), so a
    rename **strands the game** and must say so.

## Gaps

The literature settles three things and leaves two to this project.

- **Whether a rename should ask before it acts.** The HIG's split is clear — *safe: do not ask;
  irreversible: ask* — and the answer here follows from the fix: **if the rename is made safe, it
  does not ask.** The prompt is not designed; it is **removed**.
- **What "the panel describes the highlighted map" means when nothing is highlighted.** The choice is
  between an empty panel and a panel that keeps the last selection. The evidence favours **empty**:
  the panel's facts are read from the selection, and a selection of nothing has no facts.
- **Whether the label/value gap is a design-system change or a local one.** `COL_WIDE` is shared by
  every panel that holds a phrase, so the fix is either in `row_label` (a gap that survives a long
  label) or in the panels that hold phrases. **The measurement decides**: if the defect is in
  `row_label`, it is in `row_label`.
