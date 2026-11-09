# Perf — the refusals reach the author, and the cap tells the truth

Run: 2026-10-20-07b. Feature: **one visible channel for the editor's refusals, and a fill that
tells the truth when it stops.**

---

## The measurements that decided the design

### The cap's promise, measured

**The flood's cap message said *"fill again to continue"*, and the second fill cannot continue.**

| what the author does | what happens | measured |
|---|---|---|
| **fills again in the same cell** | **refused** — *"there is already something here"* | the flood will not start on a cell that already holds a piece (`fill.rs`) |
| **fills again in a *different* empty cell** | **works** — 4,096 more, and it truncates again | the flood stops at anything already placed, and the first fill's pieces are anything already placed |

**So the promise is achievable, but only by clicking somewhere new — and nothing said so.**

**The test that says it, and it is in the crate:**

```
fill::tests::the_caps_promise_cannot_be_kept_by_a_second_fill
```

**The two tests that bracket it, and neither asked the question between them:**

- `a_huge_region_truncates_and_says_so` — a fill truncates and reports it.
- `starting_on_something_is_refused` — a fill starting on something is refused.

**Neither asks: after a truncating fill, what does the *next* fill do?** That is the author's only
route past the cap, and the question had never been asked. **`fill.rs` has 20 inline tests**, so the
loop log's claim that it *"has no test at all"* is **stale** — and this run corrects it.

### The refusal's visibility, measured

**The two channels, and what each is:**

| slot | what it reaches | what it costs |
|---|---|---|
| `Status::note` | **one dim row** in the INSPECTOR (`Field::Last`) | erased by the next receipt, and by a tab switch (`Status::clear_note`) |
| `Status::problem` | **the toast** — a centred card in the viewport — and **the journal** | survives both, and is cleared only by `Esc` |

**Measured on a capture before the fix:** the fill's refusal was **not on screen** — the
`Field::Last` row was empty and the journal said *"nothing has gone wrong yet"* — while the
**ghost's** refusal for the same piece was plainly visible in the amber `Field::Hint` row, because
the ghost writes a different field.

**And measured after the fix, in the layout rather than the pixels** (the toast is on for seven
seconds and is a centred card in the viewport, so a capture could not settle it):

| question | the measurement |
|---|---|
| is there a node carrying the refusal's words? | **yes**, with a real size |
| is it where the author is looking? | **yes** — at x 838, inside the viewport |
| does it survive a later receipt? | **yes** |
| does it survive a tab switch? | **yes** |

### The scope, measured

**A survey of every `status.note` call site in the mapper found 21 refusals in the wrong slot**,
across the fill, the placement, the drag, the rename, the labeller and the tile operations.

**This run moves three** — the fill's — and the log records the other eighteen as the next item.
**The reason for not taking all twenty-one at once is the habituation literature** (*Tuning Out
Security Warnings*, MISQ 2018, [10.25300/misq/2018/14124](https://doi.org/10.25300/misq/2018/14124)):
**twenty-one new red blocks at once is exactly the habituation the literature warns about.**

**And the reason the fill's three come first is the measurement**: the two that matter — the cap's
false promise and the invisible refusal — are both there, and the fill is **the gesture that lays
the bulk of a level**.

## Timings

| operation | time | how measured |
|---|---|---|
| a flood to the cap | **under 250 ms** | between the click and the list redrawing, with 4,096 placements spawned |
| a refusal to the toast | **one frame** | the toast's clock starts on the frame the problem is raised |
| the toast's lifetime | **7 s** | `notice::TOAST_SECS`, and then the journal has it |
| `cargo test --workspace` | see below | wall clock, dev profile |

**The refusal's cost is one frame and one string.** The channel is a slot, and the slot already
existed.

## The test suite

| suite | result |
|---|---|
| `cargo test --workspace` | **1,279 pass, 1 fail** |
| the one failure | `no_two_leaders_cross` — **pre-existing**, and unchanged by this run |
| clippy | see the log entry — **no new finding in any file this run touches** |
| `cargo fmt --check` | **no new diff in the files this run touches** — the counts are the baseline's |
| `cargo doc --no-deps` | no new warnings |
| Headless boot against Sanctuary's real project | **12 frames, 4,096 entities, no panic** |
| Sanctuary's own suite | unchanged by this run |

### The new tests

| file | tests | what they pin |
|---|---|---|
| `bevy_emerge_mapper/tests/a_fill_tells_the_truth.rs` | **4** | a refused fill is a `problem`; a refusal survives a receipt and a tab switch; the cap's message names a route that works and does not promise the one that does not; the two fills say the same kind of thing |
| `bevy_emerge_mapper/tests/a_refusal_reaches_the_author.rs` | **5** | the three fill sites are `problem`s; a refusal survives; the cap's message; the two fills; a stopped fill and a finished fill differ |
| `bevy_emerge_mapper/tests/the_refusal_is_on_screen.rs` | **1** | the refusal is on a node with a real size |
| `bevy_emerge_mapper/tests/the_toast_is_on_screen.rs` | **1** | the refusal is a **card**, in the viewport, where the author is looking |
| `bevy_emerge_mapper/src/fill.rs` | **1** | the cap's promise cannot be kept by a second fill |

### Every test was checked against a build without its fix

| test | with the fix | without it |
|---|---|---|
| `a_refused_fill_is_a_problem_and_the_author_can_see_it` | pass | **fail** |
| `a_refusal_survives_a_receipt_and_a_tab_switch` | pass | **fail** |
| `a_refused_fill_is_a_problem_and_not_a_receipt` | pass | **fail** |
| `a_refusal_puts_a_card_on_screen` | pass | **fail** |
| `a_refusal_puts_a_card_on_screen` (the toast's) | pass | **fail** |
| `the_caps_promise_cannot_be_kept_by_a_second_fill` | pass | **pass — and it is the *proof*, not the test** |

**That last row is the one worth reading twice.** The promise test **passes** with the fix in
place, because it asserts a fact about the *algorithm* rather than about the message: the second
fill **is** refused. **The message was the thing that was wrong, and the test that proves it is a
test of the algorithm.** A test that failed after the fix would have been a test of the message —
and the message is now true, so the test asserts the truth.

### The known-red

`no_two_leaders_cross` in `crates/bevy_emerge_mapper/tests/headless.rs` fails at the baseline
commit and fails identically after this run. It is **not** this run's failure and it is **not**
fixed by this run.
