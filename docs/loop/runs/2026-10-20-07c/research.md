# Research — the stage's labels must not collide

Run: 2026-10-20-07c. Feature: **the COMPOSE tab's stage labels are legible, attached, and honest
about what they elide.**

---

## The measurement that started this

I drove the real editor over BRP against a copy of Sanctuary's project and opened the **COMPOSE
tab** — the tab where a composition is edited.

**Its stage draws the groups as miniatures on a diagonal strip, and each miniature is labelled with
its group's full id. The labels are drawn on top of each other.**

### What the capture shows

The COMPOSE tab, at full frame, with `town/house_cottage_plaster` selected:

> the stage carries **`town/house_cottage_plaster`** (amber), **`town/house_cottage_brick`** and
> **`town/house_cottage_wood`** — **all three drawn at the same place**, so the three strings lie
> across each other and none of them is readable.

**Measured**, the labels' ink as one run:

| | the focal group | the ink |
|---|---|---|
| the cottage group | `town/house_cottage_plaster` | **x 1653..2466 — 813 px**, with no clean boundary between the three labels |
| the hall group | `town/house_hall_wood` | **x 1239..2157 — 918 px**, with **four** labels in the pile |

**At 2.5×** the focal group's label is drawn *through* its neighbour's, and the two are
indistinguishable: `hall_wood` (amber) over `town/pub_extras` (grey).

### The mechanism, and it is exactly one thing

**A label is the group's full id, centred on the group's projected screen position.**

```rust
// crates/bevy_emerge_mapper/src/compose.rs:1886
Text::new(if armed { format!("{} *", c.id) } else { c.id.clone() }),
```

```rust
// crates/bevy_emerge_mapper/src/compose.rs:1942
cam.world_to_viewport(cam_tf, COMPOSE_STAGE + Vec3::new(slot.at.0, 0.0, slot.at.1))
```

**And the groups stand close together.** The strip runs diagonally — `STRIP` is
`(1/√2, −1/√2)` — and the slots are spaced by the *focal group's own reach* plus one `SNAP`
(`compose.rs:1403`). So two neighbouring groups are **a few tens of centimetres apart in world
space**, which projects to **a few tens of pixels apart on screen** — while each label is
**22 to 25 characters wide**.

**So: a label is centred on its group, a neighbour's label is centred on its group, and the two
centres are closer than half the sum of the two label widths.** Every one of them is drawn anyway.

### Who it hits

**An author editing a composition** — the tab that is the editor's reason for existing on a kit.
**Every time they open it**, the stage is illegible. And the *focal* group — the one being edited,
the one the author came to look at — is the one drawn **in the middle of the pile**.

---

## The literature

### The failure is a known one, and the centroid approach is the one that causes it

**Bell, Feiner & Höllerer (2001), *View Management for Virtual and Augmented Reality*, UIST.**
[10.1145/502348.502363](https://doi.org/10.1145/502348.502363) — **the closest prior work to a
3-D viewport labelling problem**, and it names the stage's approach as the one that fails:

> "…approaches that **position each label at the projected centroid of the object… result in
> overlaps and mislabelings** because they do not take into account hidden parts of the buildings or
> conflicts in label placement."

**And its answer is allocation-or-withhold, plus a leader:**

> "If the label is allocated within this space, it will neither be occluded by nor occlude any other
> object. **If no such space can be found, then no label is allocated.**"

> "less important objects can be **elided** if adding them will violate other constraints" (§3.5)

> "Since external labels are potentially ambiguous, we also generate a **leader-line arrow** from the
> label to the interior of its object."

**So the literature's answer has three parts: allocate only into free space, elide by a stated
priority, and attach what is left unambiguously.**

**Christensen, Marks & Shieber (1995), *An Empirical Study of Algorithms for Point-Feature Label
Placement*, ACM TOG.** [10.1145/212332.212334](https://doi.org/10.1145/212332.212334) — the
empirical anchor:

> "A major factor affecting the clarity of graphical displays that include text labels is the
> **degree to which labels obscure display features (including other labels) as a result of spatial
> overlap**."

**And it names the three-way trade the design has to make** — overlap, deletion, and displacement
are the alternatives, and *"the basic PFLP problem and most interesting variants of it are
NP-hard"*, so **no exact optimum is available** and a fast heuristic is the right answer.

**Azuma & Furmanski (2003), *Evaluating Label Placement for Augmented Reality View Management*,
ISMAR.** [10.1109/ismar.2003.1240689](https://doi.org/10.1109/ismar.2003.1240689) — **the
measurement that settles what to optimise**:

> "human subjects were able to **read labels fastest with the algorithms that most quickly prevented
> overlap, even if placement wasn't ideal**."

**So: speed of deconfliction beats optimality.** A fast, always-correct "these two collide, so one
of them moves" beats a perfect solver that is slow.

**Peterson, Axholt, Cooper & Ellis (2009), *Detailed Information on the Effect of Label Layout Design
in Augmented Reality*, 3DUI.** [10.1109/3dui.2009.4811215](https://doi.org/10.1109/3dui.2009.4811215)
— **the failure that is not about legibility**:

> "significant performance degradation when the 2D view plane methods produced **potentially
> confusing spatial correlations between labels and the markers they designate**."

**A label that is readable but sits beside the wrong group is a failure of *interpretation*.** The
stage's labels have exactly that problem: the pile means the reader cannot say which id belongs to
which miniature.

**Luboschik, Schumann & Cords (2008), *Particle-based labeling*, IEEE TVCG.**
[10.1109/tvcg.2008.152](https://doi.org/10.1109/tvcg.2008.152) — the property to state as a
criterion:

> "a placed label should neither occlude other labels nor visual representatives… that communicate
> crucial information."

**Been, Daiches & Yap (2006), *Dynamic Map Labeling*, IEEE TVCG.**
[10.1109/TVCG.2006.136](https://doi.org/10.1109/TVCG.2006.136) — **priority-ordered dynamic
placement**, which is the mechanism: *"The simple version is based on label priorities."*

**Stein & Décoret (2008), *Dynamic label placement for improved interactive exploration*, NPAR.**
[10.1145/1377980.1377986](https://doi.org/10.1145/1377980.1377986) — **the order matters**:

> "greedy label placement algorithms do not pay particular attention to the order of placement and…
> **successive labels get progressively more difficult to place**."

**So the order of placement decides which label loses — and it must be a stated order, not whatever
the loop happens to do.**

### Industry's answer is the same one, in one line

**Google Maps Platform, Marker Collision Management**
([developers.google.com](https://developers.google.com/maps/documentation/javascript/examples/marker-collision-management))
— priority tiers `OPTIONAL_AND_HIDES_LOWER_PRIORITY` and `REQUIRED_AND_HIDES_OPTIONAL`:
**hide-by-priority is standard practice**, and the *required* tier is the one that is never hidden.

### Long identifiers, and the part that distinguishes them

**Lawrie, Morrell, Feild & Binkley (2007), *Effective Identifier Names for Comprehension and
Memory*, ISSE.** [10.1007/s11334-007-0031-2](https://doi.org/10.1007/s11334-007-0031-2) — the
bearing on *this project's* ids, which share a prefix:

> "identifier names may **'crowd-out' needed information**."

> "**long names can be misread when they differ little from each other.**"

**And this project's ids differ little from each other** — `town/house_cottage_plaster`,
`town/house_cottage_brick`, `town/house_cottage_wood`. **They share `town/house_cottage_`, and the
part that tells them apart is the tail.** So a head-truncation would make three labels identical —
which is why the industry guidance (Vercel's *middle truncate*, Pixso's *"if three documents share
a long prefix, adding an ellipsis to the end can make all three look identical"*) keeps the tail.

**Hofmeister, Siegmund & Holt (2017), *Shorter Identifier Names Take Longer to Comprehend*, SANER.**
[10.1109/SANER.2017.7884623](https://doi.org/10.1109/SANER.2017.7884623) — the counterweight, and
it matters: *"words lead to, on average, 19% faster comprehension speed compared to letters and
abbreviations"*. **So abbreviating an id is not free** — and the elision has to be a *spatial*
one, not a *lexical* one, wherever it can be.

### What the author should see, and what is one action away

**Nielsen (2006), *Progressive Disclosure*, NN/g**
([nngroup.com](https://www.nngroup.com/articles/progressive-disclosure/)) — the two rules that
constrain the design:

> "Initially, show users **only a few** of the most important options."

> "**It must be obvious how users progress** from the primary to the secondary disclosure levels."

**Shneiderman (1996), *The Eyes Have It*, VL.**
[10.1109/VL.1996.545307](https://doi.org/10.1109/VL.1996.545307) — *"Overview first, zoom and
filter, then details-on-demand."*

**Furnas (2006), *A Fisheye Follow-up*, CHI.**
[10.1145/1124772.1124921](https://doi.org/10.1145/1124772.1124921) — **the marker rule**, and it is
the one the stage most needs: **an elision must be visible.** *"Explicit elision markers ('…')"*.

**Hutchins, Hollan & Norman (1985), *Direct Manipulation Interfaces*, HCI 1(4).**
[10.1207/s15327051hci0104_2](https://doi.org/10.1207/s15327051hci0104_2) — **the object of
interest**:

> "The **gulf of evaluation** is bridged by making the output displays present a good conceptual
> model of the system that is readily perceived, interpreted, and evaluated."

**And the object of interest is the group being edited** — which is the one the pile hides.

**Cockburn, Karlson & Bederson (2008), *A Review of Overview+Detail, Zooming, and Focus+Context
Interfaces*, ACM Computing Surveys 41(1).**
[10.1145/1456650.1456652](https://doi.org/10.1145/1456650.1456652) — the cue-based family, which
is the one this fits: techniques that *"selectively highlight or suppress items"*, and the note
that the cue is the information scent.

**Kouřil et al. (2018), *Labels on Levels: Labeling of Multi-Level 3D Objects*, IEEE TVCG.**
[10.1109/tvcg.2018.2864491](https://doi.org/10.1109/tvcg.2018.2864491) — **the strongest
precedent for grouping rather than labelling each instance**: *"labeling only representative
instances for each type of an object."*

**Arezoumand et al. (2025), arXiv:2507.00198** — the measurement that grouping is faster:
grouped labels took **14.0 s versus 16.5 s per item** (p<0.05), at equal accuracy.

---

## What the editor already has, and the design's shape

**The panel beside the stage already lists every group's full id, its member count and its
envelope, and highlights the one being edited.** So the stage's label is a **second presentation of
a fact the panel already states** — and it is the one that is broken.

**That is the design's opening.** The literature's three answers map onto three different amounts of
work, and the panel decides which:

| the design | what the stage shows | the cost |
|---|---|---|
| **(a) withhold** | **only the focal group's label**; the neighbours are unlabelled | the cheapest, and the panel beside it already names every group |
| **(b) elide** | the focal group's label in full, and a **short** form for the neighbours, with a visible marker | the middle, and it needs a short form that keeps the discriminating tail |
| **(c) displace** | every label, moved into free space, with **leader lines** | the most, and it needs a leader |

**And the acceptance criteria a reviewer could fail an implementation against**, from the research:

1. **No two drawn labels overlap at any reachable camera pose.** Measured in a headless run, not by
   eye. (Bell 2001; Azuma 2003.)
2. **Every visible label is fully legible** — its ink does not intersect another label's ink. A
   withheld label is **absent, not partly drawn**. (Bell 2001; Luboschik 2008.)
3. **Every label is attached to its group** — a leader, or an unambiguous proximity check, at every
   pose. **A label that sits nearer a neighbour's group fails.** (Bell 2001; Peterson 2009.)
4. **Elision is deterministic and follows a stated priority**, and the priority is documented. (Bell
   2001 §3.5; Google Maps tiers.)
5. **Each group's full id is reachable in one action**, including one whose label is elided or
   truncated. (Shneiderman 1996; Vercel.)
6. **A truncated form keeps the discriminating tail** — two siblings sharing a prefix must remain
   distinct on screen. (Pixso; Lawrie 2007.)
7. **Any truncation, elision or withholding shows a visible marker.** Silent removal fails. (Furnas
   2006.)
8. **The label for the object under the cursor or active selection is never suppressed by a
   neighbour.** (Hutchins et al. 1985; Bell 2001.)
9. **No stale id is drawn after a rename, a move or a reorder.** (Parasuraman & Riley 1997; Lee &
   See 2004.)
10. **A label that is withheld is reported in the channel the author watches.** [the editor's own
    rule, from the last run]

## Gaps

The literature settles the *shape* and leaves three things to this project.

- **Whether the stage's labels should be elided at all, given the panel.** The panel lists every id
  in full, so the stage's label is redundant *for the list's own sake* — but the stage's label is
  what ties a name to a **miniature in space**, and the panel's row is what ties a name to a
  **composition**. **Those are different jobs, and only one of them is broken.**
- **Whether a neighbour's label is worth eliding at all, or whether only the focal group should be
  labelled.** The panel is one keypress away (`O`/`P` steps the carousel), so *the neighbour's
  label may be worth nothing at all* — and Bell's rule is *"no label is allocated"* when there is
  no free space, not *"a worse label is allocated"*.
- **How a group is identified when its label is elided.** The miniature is a **shape**, and two
  groups can share one — so shape is not an identifier, and the panel's row is the only thing that
  is. **Which means the elided label's absence must be *obvious*, not merely invisible** (Furnas's
  elision marker).
