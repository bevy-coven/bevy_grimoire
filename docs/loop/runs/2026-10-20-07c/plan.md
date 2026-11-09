# Plan — the stage's labels must not collide

Run: 2026-10-20-07c. Feature: **the COMPOSE tab's stage labels are legible, attached, and honest
about what they elide.**

---

## The decision

**One change, in one place: the stage's labels stop colliding, and a label that cannot be shown
says so.**

The COMPOSE tab's stage draws each group as a miniature with **its full id** centred on it. The
groups stand a few tens of centimetres apart on a diagonal strip, so the labels — 22 to 25 characters
wide — **lie across each other**. Measured: **813 px and 918 px of continuous ink** where three and
four labels should be, and at 2.5× the focal group's name is drawn *through* its neighbour's.

**The intent is already in the code and already right:** the focal group's label is `ACCENT` at body
size, the neighbours' are `DIM` at hint size — *"so the strip reads as one focal group among
neighbours rather than as five equal things"*. **The implementation defeats that intent by drawing
all of them anyway.**

**So the fix is the intent, made true.**

## The shape, in detail

### 1. The focal group's label is always there, in full, and never moves

**It is the object of interest** — Hutchins et al.'s term, and the group the author came to edit.
**Its label is the one that must be complete**, and it is the one the panel's highlight agrees with.

### 2. A neighbour's label is shown only where it does not collide

**Bell et al. (2001)**: a label is *"allocated within this space"* or **not allocated at all**.

**And Azuma & Furmanski (2003)** settles the trade: *"human subjects were able to read labels
fastest with the algorithms that most quickly prevented overlap, even if placement wasn't ideal."*
**So the rule is a fast, always-correct deconfliction — not a perfect solver.**

**And the priority is stated, because Stein & Décoret (2008) show the order decides which label
loses:** *"successive labels get progressively more difficult to place."* The order is **the focal
group first, then the neighbours by distance** — and the focal group is the one that never loses.

### 3. A label that is elided leaves a marker

**Furnas (2006)**: an elision is **visible** — *"Explicit elision markers ('…')"*. **And the
editor's own rule, from the last run: a refusal is reported in the channel the author watches.**

**So a neighbour whose label is withheld leaves a small mark on the stage** — the group is still
there, and the author can see that its name is one action away rather than gone.

### 4. The full id is one action away, for every group

**Shneiderman (1996)**: *"Overview first, zoom and filter, then details-on-demand."*

**The editor already has the route: the panel lists every group's full id, and `O`/`P` step the
carousel.** So the elided group's name is already one keypress away — **and the marker says so.**

### 5. A truncated form keeps the discriminating tail — if any truncation happens at all

**Lawrie et al. (2007)**: *"long names can be misread when they differ little from each other"* —
and this project's ids differ little: `town/house_cottage_plaster`, `town/house_cottage_brick`,
`town/house_cottage_wood` **share `town/house_cottage_`**, and the tail is what tells them apart.

**So the design keeps the tail, never the head** — and **the head is the namespace, which the
panel's own grouping already carries.**

**And Hofmeister et al. (2017) is the reason not to abbreviate the words** — *"words lead to, on
average, 19% faster comprehension speed compared to letters and abbreviations"* — so **a truncated
form drops a *segment*, not a word.**

## Touched modules

| file | what changes |
|---|---|
| `crates/bevy_emerge_mapper/src/compose.rs` | **`place_labels`** — the deconfliction, the priority, the elision marker; and **`rebuild_labels`** — the marker node, and the truncated form |
| `crates/bevy_emerge_mapper/src/chrome.rs` | **nothing** — the tokens (`COL_LABEL`, `BODY_CHAR_W`, `text::HINT`) are already there |

**`bevy_emerge_core` is untouched.** The core knows nothing about labels. **No schema change** — a
label is not a file. **No new verb** — `BINDINGS` does not change, and the rule is *"a new verb adds
one row to `BINDINGS`, nowhere else"*, so a change that adds no verb adds no row.

**`engine_free.rs` is unaffected** — nothing in the core changed.

## Tests

1. **No two drawn labels overlap, at any reachable camera pose.** **Measured in a headless run** —
   the labels' layout bounds, not a screenshot. (Azuma 2003; Bell 2001.)
2. **The focal group's label is never withheld.** The object of interest is never suppressed by a
   neighbour. (Hutchins et al. 1985.)
3. **A label that is elided leaves a visible marker.** Silent removal fails. (Furnas 2006.)
4. **A label that is shown is attached to its group** — it does not sit nearer a neighbour's
   miniature. (Bell 2001; Peterson 2009.)
5. **The elision order is the stated one** — the focal group first, then the neighbours by
   distance, and the *nearest* neighbour wins over the farther one. (Stein & Décoret 2008; Been et
   al. 2006.)
6. **A truncated form keeps the discriminating tail**, and two siblings that share a prefix stay
   distinct on screen. (Pixso; Lawrie 2007.)
7. **The full id is reachable in one action** for a group whose label is elided — the panel lists it.
   (Shneiderman 1996.)
8. **A label that is elided is reported** in the channel the author watches. (the editor's rule)

## Risk

| risk | how it is answered |
|---|---|
| **The stage gets quieter, and the author loses the neighbours' names.** | **The panel beside it lists every group's full id, and the marker says the name is one key away.** The stage's job is to tie a name to a *shape in space*; the panel's is to tie a name to a *composition*. **Only the stage's is broken.** |
| **The deconfliction is slow** — the stage is redrawn every frame. | **Azuma & Furmanski: speed of deconfliction beats optimality.** The rule is O(labels²) over **five** labels — and the carousel is `WINGS: i32 = 2`, so **at most five**. |
| **A label moves as the camera moves, and the author loses track of it.** | **The focal group's label does not move** — it is the object of interest and it is where the author is looking. The neighbours' are secondary, and the panel's highlight is the fallback. (Tatzgern et al. 2014, on frame-coherent layout, is the literature's warning.) |
| **The truncated form is not the author's id.** | **The truncation drops a *segment*, not a word** (Hofmeister et al. 2017), **keeps the tail** (Lawrie et al. 2007), and **the full id is one action away** (Shneiderman 1996). |
| **The elision marker is clutter.** | **One small mark per elided group, and at most two** — the carousel's wings are two. |

## Not in this run

- **The other twelve to eighteen mis-slotted refusals** — measured, each one line, and the log says
  where they are. **The mechanism is proven** and the habituation literature says the batch comes
  after the first three have been seen in place. **This is not that.**
- **A resumable flood with a real cursor** — AIP-158's shape. The flood's behaviour is already
  correct; the *cursor* is the new mechanism.
- **Renaming a kit's namespace** — 744 mentions across 4 files, and no multi-file atomicity.
- **The 282 hand-placed fences** — content, and the owner's.
