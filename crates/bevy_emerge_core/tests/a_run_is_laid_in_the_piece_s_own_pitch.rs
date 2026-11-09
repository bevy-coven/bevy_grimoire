//! **A run is laid in the piece's own pitch** — the arithmetic, tested apart from the editor.
//!
//! # The measurement this file exists for
//!
//! Sanctuary's `town.map.ron` holds **282** hand-placed `prop_wooden_fence_single` rows. The
//! posts stand **2.032 m** apart, on a 1.016 m half-tile lattice, and the corners are the **same
//! piece turned 90°** — the kit has no corner descriptor at all.
//!
//! The editor's three placement tools could not draw it, and the reason is arithmetic rather than
//! clumsiness. The project's lattice is `snap_divisor: 3`, so the rungs are 1.000 m, 0.333 m and
//! 0.111 m, and a fill steps by `ceil(span / pitch) * pitch`:
//!
//! | Rung | The fence's step | Error against 2.032 m |
//! |---|---|---|
//! | Tile | 3.000 m | 0.968 m |
//! | Fine | 2.333 m | 0.301 m |
//! | Finer | 2.111 m | 0.079 m |
//!
//! **None of them is 2.032 m**, and the closest is 8 cm out — a visible gap in every joint, which
//! is the one thing a fence must not have. A run's pitch is the **piece's own**, and the run never
//! lands on the project's snap rungs.
//!
//! # What is here
//!
//! The contract: the pitch is the piece's, the corners turn the piece, a gap is an absence rather
//! than a compaction, and every way the run can fail is a refusal that names what was wrong.

use bevy_emerge_core::descriptor::{Descriptor, Extent};
use bevy_emerge_core::library::{LIBRARY_VERSION, Library};
use bevy_emerge_core::map::{MAP_VERSION, Map, Placed};
use bevy_emerge_core::run::{Corner, Gap, Run, lay};

/// The fence post: 2.064 by 0.12 m, 0.838 m tall.
const W: f32 = 2.064;
const D: f32 = 0.12;

fn library() -> Library {
    Library {
        version: LIBRARY_VERSION,
        note: None,
        descriptors: vec![Descriptor {
            id: "kit/fence".to_owned(),
            extent: Extent {
                footprint: Some((W, D)),
                height: Some(0.838),
            },
            ..Default::default()
        }],
    }
}

fn run(from: (f32, f32), to: (f32, f32)) -> Run {
    Run {
        descriptor: "kit/fence".to_owned(),
        pitch: None,
        from,
        to,
        corners: Vec::new(),
        gaps: Vec::new(),
    }
}

/// **The pitch is the piece's own footprint, and it is not on the lattice.**
///
/// The whole reason the tool exists, as one number.
#[test]
fn the_pitch_is_the_pieces_own_footprint_and_not_the_lattices() {
    let posts = lay(&run((0.0, 0.0), (8.0, 0.0)), &library()).unwrap_or_else(|e| panic!("{e}"));

    // 0, 2.064, 4.128 and 6.192 — and the run stops a pitch short of 8, because the next post
    // would be 8.256 and past the end. **Every gap is the pitch.**
    assert_eq!(posts.len(), 4, "{posts:?}");
    for pair in posts.windows(2) {
        let step = pair[1].at.0 - pair[0].at.0;
        assert!(
            (step - W).abs() < 1e-4,
            "the posts are {step} apart and the piece is {W} wide"
        );
    }

    // **The project's rungs.** `snap_divisor: 3` and `TILE: 1.0`.
    for rung in [1.0f32, 1.0 / 3.0, 1.0 / 9.0] {
        assert!(
            (posts[1].at.0 / rung).fract().abs() > 1e-3,
            "the pitch landed on the {rung} rung — the grid the fill uses"
        );
    }
}

/// **The posts are the anchor plus `n × pitch`, to the millimetre.**
#[test]
fn the_posts_are_the_anchor_plus_n_times_the_pitch() {
    let posts =
        lay(&run((3.5, -7.25), (11.5, -7.25)), &library()).unwrap_or_else(|e| panic!("{e}"));
    for (n, post) in posts.iter().enumerate() {
        let want = 3.5 + W * n as f32;
        assert!(
            (post.at.0 - want).abs() < 1e-4,
            "post {n} is at {} and the pitch says {want}: {posts:?}",
            post.at.0
        );
        assert!(
            (post.at.1 + 7.25).abs() < 1e-4,
            "the row is off its line: {posts:?}"
        );
    }
}

/// **A corner turns the piece** — the same descriptor, at a new angle.
#[test]
fn a_corner_turns_the_piece() {
    let mut r = run((0.0, 0.0), (4.0, 4.0));
    r.corners = vec![Corner {
        at: (4.0, 0.0),
        yaw: 90.0,
    }];
    let posts = lay(&r, &library()).unwrap_or_else(|e| panic!("{e}"));

    // The stretch before the corner is along X; the corner and the stretch after it are along Z.
    let along_x = posts.iter().filter(|p| p.yaw == 0.0).count();
    let along_z = posts.iter().filter(|p| p.yaw == 90.0).count();
    assert!(along_x >= 2, "the first stretch runs along X: {posts:?}");
    assert!(along_z >= 2, "the second stretch runs along Z: {posts:?}");

    // **The corner post is at the turn**, and carries the turn's yaw.
    let corner = posts
        .iter()
        .find(|p| (p.at.0 - 4.0).abs() < 1e-4 && (p.at.1).abs() < 1e-4)
        .expect("a post at the corner");
    assert_eq!(corner.yaw, 90.0, "the corner post turns: {posts:?}");

    // **And the second stretch is pitched by the piece's LONG side** — 2.064 m, not its 12 cm
    // depth. Pitching it by the depth would collapse the fence into a hedge.
    let after = posts
        .iter()
        .find(|p| (p.at.0 - 4.0).abs() < 1e-4 && p.at.1 > 0.1)
        .expect("a post after the corner");
    assert!(
        (after.at.1 - W).abs() < 1e-4,
        "the second stretch is pitched by the wrong number: {posts:?}"
    );
}

/// **A gap is an absence, not a compaction.** The posts either side keep their places.
#[test]
fn a_gap_is_an_absence_and_not_a_compaction() {
    let mut r = run((0.0, 0.0), (8.0, 0.0));
    r.gaps = vec![Gap {
        count: 2,
        at_post: 1,
    }];
    let posts = lay(&r, &library()).unwrap_or_else(|e| panic!("{e}"));

    // Four posts without the gap; two are gone, so two.
    assert_eq!(posts.len(), 2, "{posts:?}");
    // **The post after the gap did not slide forward to fill the hole** — it is still at the
    // run's pitch, 6.192 m along.
    assert!(
        (posts[1].at.0 - 6.192).abs() < 1e-4,
        "the post after the gap moved: {posts:?}"
    );
}

/// **Every failure is a refusal, and it names what was wrong.**
///
/// A run that lays some of its posts and then stops is the failure this module exists to remove:
/// a fence with a hole in it, in a file, in a game.
#[test]
fn every_failure_is_a_refusal_that_names_what_was_wrong() {
    let lib = library();

    // A piece the library does not define.
    let mut r = run((0.0, 0.0), (4.0, 0.0));
    r.descriptor = "kit/nope".to_owned();
    let err = lay(&r, &lib).expect_err("no such piece");
    assert!(err.contains("kit/nope"), "{err}");

    // A piece with no measured footprint.
    let mut unmeasured = lib.clone();
    unmeasured.descriptors[0].extent.footprint = None;
    let err = lay(&run((0.0, 0.0), (4.0, 0.0)), &unmeasured).expect_err("unmeasured");
    assert!(err.contains("no measured footprint"), "{err}");

    // A first stretch along the piece's short side.
    let err = lay(&run((0.0, 0.0), (0.0, 4.0)), &lib).expect_err("across the short side");
    assert!(err.contains("short side"), "{err}");

    // A pitch that is not a distance.
    for pitch in [0.0f32, -1.0, f32::NAN, f32::INFINITY] {
        let mut r = run((0.0, 0.0), (4.0, 0.0));
        r.pitch = Some(pitch);
        assert!(lay(&r, &lib).is_err(), "a pitch of {pitch} must be refused");
    }

    // A span shorter than one pitch.
    let err = lay(&run((0.0, 0.0), (0.5, 0.0)), &lib).expect_err("too short");
    assert!(err.contains("shorter than one pitch"), "{err}");

    // A gap wider than the run.
    let mut r = run((0.0, 0.0), (4.0, 0.0));
    r.gaps = vec![Gap {
        count: 99,
        at_post: 0,
    }];
    let err = lay(&r, &lib).expect_err("the gap is wider than the run");
    assert!(err.contains("cannot eat more than the run has"), "{err}");

    // **And a refused run lays nothing at all** — never a partial fence.
    let mut r = run((0.0, 0.0), (0.0, 4.0));
    r.pitch = Some(0.5);
    assert!(
        lay(&r, &lib).is_err(),
        "a run across the short side is refused"
    );
}

/// **The run's posts are ordinary rows, and a map accepts them.**
///
/// The round trip that matters: the posts a run lays become `Placed` rows, and the map's own
/// validation takes them. So the file gains rows and nothing else, and the game's loader — which
/// reads `map.placements` — has them.
#[test]
fn the_runs_posts_are_ordinary_rows_a_map_accepts() {
    let lib = library();
    let posts = lay(&run((0.0, 0.0), (8.0, 0.0)), &lib).unwrap_or_else(|e| panic!("{e}"));

    let rows: Vec<Placed> = posts
        .iter()
        .enumerate()
        .map(|(n, p)| Placed {
            id: format!("fence@{n}"),
            descriptor: "kit/fence".to_owned(),
            at: p.at,
            yaw: p.yaw,
            ..Placed::default()
        })
        .collect();

    let map = Map {
        version: MAP_VERSION,
        name: "yard".to_owned(),
        origin: (0.0, 0.0, 0.0),
        bounds: (100.0, 10.0, 100.0),
        bash: None,
        placements: rows.clone(),
        ..Map::default()
    };
    map.validate()
        .unwrap_or_else(|e| panic!("the map must accept a run's rows: {e}"));

    // **And it round-trips through the file** — the same rows, the same places.
    let text = ron::ser::to_string_pretty(&map, ron::ser::PrettyConfig::default())
        .unwrap_or_else(|e| panic!("{e}"));
    let back = Map::parse(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(back.placements, rows);
    assert_eq!(back.placements.len(), 4);
}

/// **The same run always lays the same posts.**
///
/// The property the editor's ghost depends on: a preview computed from the same run must be the
/// rows the commit lays, and two runs of the editor must agree.
#[test]
fn the_same_run_always_lays_the_same_posts() {
    let lib = library();
    let r = run((1.0, 2.0), (9.0, 2.0));
    let first = lay(&r, &lib).unwrap_or_else(|e| panic!("{e}"));
    let second = lay(&r, &lib).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        first, second,
        "the same run laid two different sets of posts"
    );
}
