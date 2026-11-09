//! **The stage's labels must not collide.**
//!
//! The COMPOSE tab's stage draws each group as a miniature with its **full id** centred on it.
//! The groups stand a few tens of centimetres apart on a **diagonal** strip, so the labels — 22 to
//! 25 characters wide — **lie across each other**.
//!
//! Measured on a real capture of Sanctuary's own project: **813 px** of continuous ink where three
//! labels should be, and **918 px** where four should be. At 2.5× the focal group's name is drawn
//! *through* its neighbour's, and the two are indistinguishable.
//!
//! # What the literature says
//!
//! **Bell, Feiner & Höllerer (2001)**, *View Management for Virtual and Augmented Reality*, UIST
//! ([10.1145/502348.502363](https://doi.org/10.1145/502348.502363)) — the closest prior work to
//! a 3-D viewport labelling problem, and it names the stage's approach as the one that fails:
//!
//! > "…approaches that **position each label at the projected centroid of the object… result in
//! > overlaps and mislabelings** because they do not take into account hidden parts of the
//! > buildings or conflicts in label placement."
//!
//! And its answer is allocation-or-withhold, plus a leader:
//!
//! > "If the label is allocated within this space, it will neither be occluded by nor occlude any
//! > other object. **If no such space can be found, then no label is allocated.**"
//!
//! **Azuma & Furmanski (2003)**, *Evaluating Label Placement for Augmented Reality View Management*,
//! ISMAR ([10.1109/ismar.2003.1240689](https://doi.org/10.1109/ismar.2003.1240689)) — the
//! measurement that settles what to optimise:
//!
//! > "human subjects were able to **read labels fastest with the algorithms that most quickly
//! > prevented overlap, even if placement wasn't ideal**."
//!
//! **So: speed of deconfliction beats optimality.** A fast, always-correct "these two collide, so
//! one of them is withheld" is the right answer — and not a solver, because
//! **Christensen, Marks & Shieber (1995)**, *An Empirical Study of Algorithms for Point-Feature
//! Label Placement*, ACM TOG
//! ([10.1145/212332.212334](https://doi.org/10.1145/212332.212334)) show that *"the basic PFLP
//! problem and most interesting variants of it are NP-hard"*.
//!
//! **Stein & Décoret (2008)**, *Dynamic label placement for improved interactive exploration*,
//! NPAR ([10.1145/1377980.1377986](https://doi.org/10.1145/1377980.1377986)) — the order decides
//! which label loses, so it must be stated:
//!
//! > "greedy label placement algorithms do not pay particular attention to the order of placement
//! > and… **successive labels get progressively more difficult to place**."
//!
//! # The design
//!
//! **One order, and it is the editor's own:** the **focal group first** — it is the object of
//! interest, and **it is never withheld** — and then the neighbours **by distance**. A neighbour
//! whose label would collide with one already placed is **withheld**, and leaves a **marker**, so
//! the group is visibly there with its name one action away (Furnas 2006, on explicit elision
//! markers; Shneiderman 1996, on details-on-demand).

use crate::chrome::BODY_CHAR_W;

/// **Where a label would go, and how wide it is.**
///
/// **The label's own box, in logical pixels.** The width is the string's length times the advance
/// of one glyph at the size it is set — the same measurement the drawing uses, so the collision test
/// and the drawn text cannot disagree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelBox {
    /// The label's left edge, in logical viewport pixels.
    pub left: f32,
    /// The label's top edge, in logical viewport pixels.
    pub top: f32,
    /// The label's width, in logical pixels.
    pub width: f32,
    /// The label's height, in logical pixels.
    pub height: f32,
    /// **How much room it has to be pushed down before it would sit on another.** The row of
    /// labels is a column of them, so the vertical space is what the deconfliction spends.
    pub height_allowance: f32,
}

impl LabelBox {
    /// **Does this box overlap `other`?** Two boxes overlap when they overlap on **both** axes.
    ///
    /// **A little slack, on the horizontal axis only, because the drawn text is not the box.** The
    /// string is centred in its box and the box carries a glyph's worth of padding, so two boxes
    /// that merely touch would have their last and first glyphs a hair apart. **The slack is one
    /// advance**, so the test is conservative: it refuses a placement that would *nearly* collide.
    ///
    /// **The vertical axis has no slack**, and that is not an oversight — it is what lets a label
    /// sit *below* another one. The labels are a row under their groups, and the deconfliction
    /// spends the vertical space: a neighbour that cannot fit beside the focal group's label goes
    /// below it. **A vertical slack would make that impossible**, because a box pushed down by its
    /// own height plus a gap would still be "overlapping" the one it cleared.
    pub fn overlaps(&self, other: &LabelBox) -> bool {
        let slack = BODY_CHAR_W;
        let x = self.left < other.left + other.width + slack
            && other.left < self.left + self.width + slack;
        let y = self.top < other.top + other.height && other.top < self.top + self.height;
        x && y
    }

    /// **The box moved down by `by` pixels.**
    pub fn below(&self, by: f32) -> LabelBox {
        LabelBox {
            top: self.top + by,
            ..*self
        }
    }
}

/// **Which labels are drawn, and which are withheld — and why the order is what it is.
///
/// **The focal group first.** It is the group being edited, the object of interest, and **the only
/// label that is never withheld** — the author came to look at *that* group, and a stage that hid
/// its name would be hiding the answer to the question the author asked.
///
/// **Then the neighbours by distance**, nearest first, so the neighbour the author is most likely to
/// want is the one that survives. **Stein & Décoret's point**: successive labels get progressively
/// harder to place, so the order decides the loser and the order has to be stated rather than
/// incidental.
///
/// **A withheld label is reported, not dropped** — `withheld` is the count, and the caller draws a
/// marker for each. Furnas (2006) on explicit elision markers: a thing that is present but unnamed
/// must say that it is unnamed.
pub fn deconflict(
    boxes: &[(usize, LabelBox)],
    focal: usize,
) -> (Vec<(usize, LabelBox)>, Vec<usize>) {
    // --- The order: the focal group, then the neighbours by distance from it.
    //
    // --- **The order is over the boxes' own entries, not over a range.** The caller passes the
    // --- label's index, and the boxes are `(label index, box)` pairs whose *order* is the order
    // --- the labels were spawned in — which is not the label's index. **Sorting a range of
    // --- positions and then looking them up by label index mixes the two**, and the focal group
    // --- ends up ranked by its position rather than by its identity.
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    let box_of = |i: usize| boxes.iter().find(|(ix, _)| *ix == i).map(|(_, b)| *b);
    let focal_left = box_of(focal).map(|b| b.left).unwrap_or(0.0);
    order.sort_by(|a, b| {
        // --- The focal group first, then the neighbours by distance. `f32` is not `Ord`, so the
        // --- comparison is written out rather than through a sort key.
        let rank = |i: &usize| if boxes[*i].0 == focal { 0u8 } else { 1u8 };
        let dist = |i: &usize| {
            if boxes[*i].0 == focal {
                0.0
            } else {
                (boxes[*i].1.left - focal_left).abs()
            }
        };
        rank(a)
            .cmp(&rank(b))
            .then_with(|| {
                dist(a)
                    .partial_cmp(&dist(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| boxes[*a].0.cmp(&boxes[*b].0))
    });

    // --- **The drawn labels, with the position they are drawn at** — which is not where they
    // --- asked to be, because a neighbour that collided was pushed down. The caller needs the
    // --- pushed position, and so does a test that wants to check the drawn labels do not overlap.
    let mut drawn: Vec<(usize, LabelBox)> = Vec::with_capacity(boxes.len());
    let mut withheld: Vec<usize> = Vec::new();
    let mut placed: Vec<LabelBox> = Vec::with_capacity(boxes.len());

    for slot in order {
        // --- **The box at this position, and the label's own index.** The order is over positions
        // --- in `boxes`, and the label's index is what the caller knows the label by.
        let Some((label, b)) = boxes.get(slot).map(|(ix, b)| (*ix, *b)) else {
            continue;
        };
        // --- **The focal group's label never moves and is never withheld.** It is the object of
        // --- interest, and it is where the author is looking.
        if label == focal {
            drawn.push((label, b));
            placed.push(b);
            continue;
        }
        // --- **A neighbour: placed if it fits, dropped if it does not.**
        //
        // --- **The push is downwards only, and it is ONE step.** The labels are a row under their
        // --- groups, so a neighbour that cannot fit beside the focal group's label goes *below* it
        // --- rather than over anything.
        //
        // --- **And one step only, because a label that has travelled far from its group is
        // --- ambiguous.** Peterson et al. (2009) measured that "potentially confusing spatial
        // --- correlations between labels and the markers they designate" degrade judgement, and a
        // --- cascade would put the third label 36 px below its own group — further from it than
        // --- the neighbour it is naming. **So a label that cannot be placed one step down is
        // --- withheld**, and the author sees a marker saying the name is one action away.
        let mut candidate = b;
        if placed.iter().any(|p| candidate.overlaps(p)) {
            // --- **One step down from where the label wanted to be** — its own height plus the
            // --- allowance. **Not below the lowest label already placed**, which would cascade:
            // --- the third label would end up 36 px below its own group, further from it than
            // --- the neighbour it is naming, and Peterson et al. (2009) measured that a label
            // --- ambiguously placed is worse than one that is absent.
            let below = candidate.below(candidate.height + candidate.height_allowance);
            // --- **And it must clear *every* label already placed** — or the pushed label lands
            // --- on a third one.
            if placed.iter().all(|p| !below.overlaps(p)) {
                candidate = below;
            }
        }
        if placed.iter().any(|p| candidate.overlaps(p)) {
            withheld.push(label);
        } else {
            drawn.push((label, candidate));
            placed.push(candidate);
        }
    }

    (drawn, withheld)
}
