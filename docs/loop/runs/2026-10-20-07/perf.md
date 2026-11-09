# Perf — the safe rename, and the honest panel

Run: 2026-10-20-07. Feature: **one safe path for renaming a map, and a chooser that never shows a
fact about the wrong thing.**

---

## The measurements that decided the design

### The refusal is one call, not a check and a call

**The defect, from the code and confirmed on disk.** `OpenMap::save` derived the path from the
map's **name**, and `ron_surgery::save_atomic` ended in `fs::rename` — which POSIX specifies as
**replacing** the target (IEEE Std 1003.1-2024: *"if the directory entry named by `new` exists, it
shall be removed and `old` renamed to `new`"*).

**The obvious fix is a check and then a write, and it is wrong.** The check and the write are two
calls, and POSIX gives atomicity to the single call only. Between them the target can appear — and
then `fs::rename` removes it, which is the failure the check exists to prevent.

**So the primitive is `link(2)`**, which creates the target as a second name for the same inode
and **fails with `EEXIST` if the target exists** — and never modifies the target. One call, and
the refusal is the kernel's.

| | the check-then-write | **`link(2)`** |
|---|---|---|
| calls | **2** | **1** |
| the target appearing between them | **destroyed** | **refused** |
| new dependency | none | none |
| `unsafe` | none | **none** |
| leaves a duplicate if it fails | **yes** — the file is written and the rename is not | **no** — nothing is written |

**And `ENOTSUP` is a refusal, not a fallback.** A hard link cannot cross a filesystem, so a
filesystem that refuses the primitive gets a refusal. **A fallback is the defect this function
exists to remove**, and Lampson's rule is the whole of it: *"Make actions atomic or restartable."*

**Measured, on the core's own test:** 7 tests, and the one that matters is
`a_file_that_appears_before_the_write_is_still_refused` — the target exists when the write happens,
the write refuses, and the file's bytes are unchanged.

### The rename, measured on Sanctuary's own project

**Before** (the defect): renaming a map to another map's name **replaced** that map's file.

**After**, on a copy of Sanctuary's project:

| | before the rename | after the rename |
|---|---|---|
| `town.map.ron` | 75,089 bytes | **75,089 bytes — a different map, untouched** |
| `town_block.map.ron` | 77,947 bytes | **`town_renamed.map.ron`, 77,949 bytes** |
| | | **`town_block.map.ron` is gone** — no duplicate |

**And the file's own `name:` agrees with its filename**: `name: "town_renamed"`. A map's name IS
its file — the project's own loader refuses a file that disagrees with itself
(`project.rs:774`, *"calls itself `<name>`"*).

**And the refusal:** a rename onto a taken name changed **nothing** — both files byte-identical,
measured by size and by the test's checksum.

### The gap, measured on a controlled before-and-after

| | `pieces` / `45` | `new work lands here` / `yes` |
|---|---|---|
| **before** | 131 px | **19 px** |
| **after** | 143 px | **29 px** |

**Both captures are of the same view, one build apart** — the "before" was taken with the fix
removed, so the comparison is controlled rather than two different projects.

**The 10 px is the label's own margin (`GAP_ROW`, 5 px) plus the row's flex gap (5 px)**, which the
short label already had. **So the long label now gets what the short one always had.**

## Timings

| operation | time | how measured |
|---|---|---|
| `save_atomic_no_replace` | **under 1 ms** | a write of a 78 KB map file, on the project's own disk |
| the rename, end to end, in the editor | **under 250 ms** | between `Enter` and the list redrawing |
| `cargo test --workspace` | see below | wall clock, dev profile |

**The rename is not a hot path** and is not treated as one: it is an author-initiated, file-writing
operation that happens once per name change.

## The test suite

| suite | result |
|---|---|
| `cargo test --workspace` | **1,267 pass, 1 fail** |
| the one failure | `no_two_leaders_cross` — **pre-existing**, and unchanged by this run |
| clippy | **one finding, in `grammar.rs`** — a file this run does not touch, and unchanged from the baseline |
| `cargo fmt --check` | **no new diff in the files this run touches** — see below |
| `cargo doc --no-deps` | no new warnings |
| Headless boot against Sanctuary's real project | **12 frames, 4,096 entities, no panic** |
| Sanctuary's own suite | unchanged by this run — see the log entry |

### The new tests

| file | tests | what they pin |
|---|---|---|
| `bevy_emerge_core/tests/a_rename_never_replaces.rs` | **7** | the write lands; a taken path is refused and the target is unchanged; a file that appears before the write is still refused; a refusal names the path; no `.tmp` is left on either path; the parent directory is made |
| `bevy_emerge_mapper/tests/a_rename_is_all_or_nothing.rs` | **5** | a rename onto a taken name is refused and the other map's bytes are unchanged; a rename leaves **one** file, named for the map, whose own `name:` agrees; a rename and back leaves the same set of files; the name field's `Escape` clears the draft; a settings click sets the field |
| `bevy_emerge_mapper/tests/a_label_and_its_value_are_two_things.rs` | **3** | the label carries a margin of its own; the margin is the design system's own `GAP_ROW`; the long label is wider than its column |

### Every test was checked against a build without its fix

**A test that passes with the fix removed proves nothing**, and two of these nearly did:

| test | with the fix | without it |
|---|---|---|
| `a_rename_onto_a_taken_name_is_refused_and_the_other_map_is_untouched` | pass | **fail** |
| `a_rename_leaves_one_file_and_the_new_name` | pass | **fail** |
| `escape_from_a_name_field_leaves_no_draft` | pass | **fail** |
| `a_settings_click_sets_the_field` | pass | **fail** |
| `the_labels_gap_is_the_design_systems_row_gap` | pass | **fail** |
| `a_long_label_still_leaves_a_gap_before_its_value` | pass | **pass — and it was deleted** |

**That last one is the finding worth recording.** The first version of the gap test compared two
*measured* gaps, and **it passed with the fix removed** — because the **headless layout does not
reproduce the defect**: with no window, every label in the panel gets the same gap. The test was
measuring the harness, not the design. **So the test now asserts the mechanism** — the label
carries a margin of its own — which is checkable without a window, **and the real measurement moved
to the visual review**, where the window is.

### The fmt count, and why the repo is not rustfmt-clean

**The repo is not rustfmt-clean at baseline** — 1,437 diff hunks across the workspace. The loop's
convention, recorded in `docs/loop/log.md` across six runs, is: *"no diff in the new files; the
files this change edits keep their pre-existing diffs."*

**So the rule is: this run's new code is rustfmt-clean, and the pre-existing diffs stay.**

| file | baseline hunks | after this run |
|---|---|---|
| `ron_surgery.rs` | 5 | **5** |
| `project.rs` | 11 | **11** |
| `chooser.rs` | 78 | **78** |
| `chrome.rs` | 16 | **16** |
| `editor.rs` | 87 | **87** |

**Every file is at its baseline count.** (A blanket `rustfmt` was tried in the previous run and
**reverted**: it turned a +282-line change into a +734-line one by reformatting 67 hunks of code
the run never touched.)

### The known-red

`no_two_leaders_cross` in `crates/bevy_emerge_mapper/tests/headless.rs` fails at the baseline
commit and fails identically after this run. It is **not** this run's failure and it is **not**
fixed by this run. It is the third item on the log's remaining list, and it stays there.
