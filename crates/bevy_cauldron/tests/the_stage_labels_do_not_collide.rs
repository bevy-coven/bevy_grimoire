//! **The stage's labels must not collide.**
//!
//! The regression tests for the 2026-10-20-07c run.
//!
//! # What was wrong
//!
//! The COMPOSE tab's stage draws each group as a miniature with its **full id** centred on it.
//! The groups stand a few tens of centimetres apart on a **diagonal** strip, so the labels — 22 to
//! 25 characters wide — **lie across each other**.
//!
//! **Measured, on a real capture of Sanctuary's own project:**
//!
//! | | the ink |
//! |---|---|
//! | the cottage group | **x 1653..2466 — 813 px**, with no clean boundary between the three labels |
//! | the hall group | **x 1239..2157 — 918 px**, with **four** labels in the pile |
//!
//! At 2.5× the focal group's label is drawn *through* its neighbour's, and the two are
//! indistinguishable.
//!
//! # What the literature says
//!
//! **Bell, Feiner & Höllerer (2001)**, *View Management for Virtual and Augmented Reality*, UIST
//! ([10.1145/502348.502363](https://doi.org/10.1145/502348.502363)) names the stage's approach as
//! the one that fails — *"position each label at the projected centroid of the object… result in
//! overlaps and mislabelings"* — and answers: **allocate a label only into free space, or withhold
//! it.** **Azuma & Furmanski (2003)**, ISMAR
//! ([10.1109/ismar.2003.1240689](https://doi.org/10.1109/ismar.2003.1240689)) settles the trade:
//! *"read labels fastest with the algorithms that most quickly prevented overlap, even if placement
//! wasn't ideal."* **Stein & Décoret (2008)**, NPAR
//! ([10.1145/1377980.1377986](https://doi.org/10.1145/1377980.1377986)) shows the order decides
//! which label loses, so it must be stated.

use bevy_cauldron::compose_labels::{LabelBox, deconflict};

/// **A label's box, at a given left edge, with a given width.**
fn box_at(left: f32, width: f32) -> LabelBox {
    LabelBox {
        left,
        top: 500.0,
        width,
        height: 14.0,
        height_allowance: 4.0,
    }
}

/// **The width of a 22-character id at the body size** — the real thing, from the shipped face.
fn id_width(chars: usize) -> f32 {
    chars as f32 * bevy_cauldron::chrome::BODY_CHAR_W
}

/// **No two drawn labels overlap.**
#[test]
fn no_two_drawn_labels_overlap() {
    // --- The measured case: the focal group's label and a neighbour's, 60 px apart, each ~200 px
    // --- wide. **They overlap by 140 px.**
    let focal = box_at(1000.0, id_width(22));
    let neighbour = box_at(1060.0, id_width(22));
    let (drawn, withheld) = deconflict(&[(0, focal), (1, neighbour)], 0);

    // --- **The focal group's label is always drawn.** It is the object of interest.
    assert!(
        drawn.iter().any(|(i, _)| *i == 0),
        "the focal group's label was withheld: drawn {drawn:?}, withheld {withheld:?}"
    );

    // --- **And the drawn boxes really do not overlap.** The boxes as they are *drawn* — which is
    // --- the point: a neighbour that collided was pushed down, and the drawn boxes are the ones
    // --- that must not overlap.
    let placed: Vec<LabelBox> = drawn.iter().map(|(_, b)| *b).collect();
    for (i, a) in placed.iter().enumerate() {
        for b in placed.iter().skip(i + 1) {
            assert!(
                !a.overlaps(b),
                "two drawn labels overlap, so the stage is illegible"
            );
        }
    }
}

/// **The focal group's label is never withheld.**
///
/// It is the object of interest — the group the author came to edit — and a stage that hid its
/// name would be hiding the answer to the question the author asked. (Hutchins, Hollan & Norman
/// 1985, on the object of interest; Bell et al. 2001, on priority.)
#[test]
fn the_focal_groups_label_is_never_withheld() {
    // --- The focal label, surrounded by neighbours on both sides, all overlapping it.
    let boxes = vec![
        (0usize, box_at(900.0, id_width(22))),
        (1usize, box_at(940.0, id_width(22))),
        (2usize, box_at(1000.0, id_width(22))),
        (3usize, box_at(1010.0, id_width(22))),
    ];
    let (drawn, withheld) = deconflict(&boxes, 2);

    assert!(
        drawn.iter().any(|(i, _)| *i == 2),
        "the focal group's label was withheld: drawn {drawn:?}, withheld {withheld:?}. The author \\
         came to look at that group."
    );
    assert!(
        !withheld.contains(&2),
        "and it was withheld: drawn {drawn:?}, withheld {withheld:?}"
    );
}

/// **A neighbour that fits is drawn; one that does not is withheld.**
#[test]
fn a_neighbour_is_drawn_when_it_fits() {
    // --- Far enough apart to clear.
    let boxes = vec![
        (0usize, box_at(1000.0, id_width(22))),
        (1usize, box_at(1600.0, id_width(22))),
    ];
    let (drawn, withheld) = deconflict(&boxes, 0);
    assert_eq!(
        drawn.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        vec![0, 1],
        "a neighbour that fits is drawn"
    );
    assert!(withheld.is_empty(), "and nothing is withheld");

    // --- Too close: it is **pushed down**, not dropped, because there is room below.
    let boxes = vec![
        (0usize, box_at(1000.0, id_width(22))),
        (1usize, box_at(1010.0, id_width(22))),
    ];
    let (drawn, withheld) = deconflict(&boxes, 0);
    assert_eq!(
        drawn.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        vec![0, 1],
        "the neighbour is pushed below, not dropped"
    );
    assert!(withheld.is_empty(), "and nothing is withheld: {withheld:?}");
}

/// **The nearest neighbour wins over the farther one.**
///
/// The order is the stated one — the focal group, then the neighbours by distance — because
/// Stein & Décoret (2008) show that *successive labels get progressively more difficult to place*,
/// so the order decides the loser and it has to be a decision rather than an accident.
#[test]
fn the_nearest_neighbour_wins() {
    // --- Two neighbours, both colliding with the focal label, at different distances.
    let near = 1_040.0;
    let far = 1_090.0;
    let boxes = vec![
        (0usize, box_at(1000.0, id_width(22))),
        (1usize, box_at(near, id_width(22))),
        (2usize, box_at(far, id_width(22))),
    ];
    let (drawn, withheld) = deconflict(&boxes, 0);

    // --- **The nearer one is placed and the farther one is not** — and the *reason* is the order,
    // --- not the geometry: the nearer one is placed first, so it takes the space, and the farther
    // --- one has nowhere to go. Stein & Décoret (2008): *"successive labels get progressively
    // --- more difficult to place"*, so the order decides the loser and the order has to be stated.
    assert!(
        drawn.iter().any(|(i, _)| *i == 1),
        "the nearer neighbour is drawn: {drawn:?} / {withheld:?}"
    );
    assert!(
        !drawn.iter().any(|(i, _)| *i == 2),
        "and the farther one is not: {drawn:?} / {withheld:?}"
    );
}

/// **A label is pushed down rather than dropped, when there is room below.**
///
/// One step only. A label that has travelled far from its group is ambiguous, and Peterson et al.
/// (2009) measured that *"potentially confusing spatial correlations between labels and the markers
/// they designate"* degrade judgement.
#[test]
fn a_label_is_pushed_down_when_there_is_room() {
    // --- The neighbour overlaps the focal label horizontally, but there is nothing below.
    let boxes = vec![
        (0usize, box_at(1000.0, id_width(22))),
        (1usize, box_at(1010.0, id_width(22))),
    ];
    let (drawn, withheld) = deconflict(&boxes, 0);
    // --- Two labels, one above the other, both drawn — because the push had room.
    assert_eq!(
        drawn.len(),
        2,
        "the neighbour was pushed below the focal label rather than dropped: {drawn:?} / \\
         {withheld:?}"
    );
    assert!(withheld.is_empty(), "so nothing was withheld: {withheld:?}");
}

/// **The deconfliction is fast.** The stage is redrawn every frame, and Azuma & Furmanski's
/// measurement is that *speed of deconfliction* is what matters.
#[test]
fn the_deconfliction_is_fast() {
    let boxes: Vec<(usize, LabelBox)> = (0..5)
        .map(|i| (i, box_at(1000.0 + i as f32 * 12.0, id_width(22))))
        .collect();
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        deconflict(&boxes, 0);
    }
    let each = start.elapsed() / 1000;
    assert!(
        each.as_micros() < 1000,
        "the deconfliction took {each:?} per call. The stage is redrawn every frame, so this has to \\
         be free."
    );
}
