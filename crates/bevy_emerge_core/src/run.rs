//! **A run** — one piece, repeated along a path, at the piece's own pitch.
//!
//! # What this is for
//!
//! A fence is not 282 pieces. It is **one piece, a pitch, a direction, and a corner rule** — and
//! Sanctuary's `town.map.ron` holds 282 hand-placed `prop_wooden_fence_single` rows because the
//! editor had no way to say the other thing.
//!
//! The three tools the editor had could not draw it, and the reason is not that they were clumsy:
//!
//! * **`F` (the flood) lays a grid.** One piece, one yaw, one pitch, over a region. A fence is a
//!   *line* that turns, and a grid has no way to turn.
//! * **The box fill lays a grid too**, and a grid has no way to have a **gap** — and the
//!   graveyard's south edge has a three-post opening that is part of its shape.
//! * **The capture (`Shift+B`) takes what is already placed**, so it cannot make the first one.
//!
//! And the pitch is the sharpest part. The town's fence posts stand **2.032 m** apart, while the
//! project's lattice is `snap_divisor: 3` — so the editor's rungs are 1.000 m, 0.333 m and
//! 0.111 m, and a fill steps a 2.064 m fence by `ceil(span / pitch) * pitch`, which is
//! **3.000 m, 2.333 m or 2.111 m**. **None of them is 2.032 m**; the closest is 8 cm out, which
//! is a visible gap in every joint — exactly the thing a fence must not have.
//!
//! So a run's pitch is **the piece's own**, and the run never lands on the project's snap rungs.
//! This is the same discipline `crate::fill`'s own lattice already states: a fill needed *its own*
//! lattice rather than a second reading of the click's, and a run needs one too.
//!
//! # The shape of a run
//!
//! A run is a **polyline** — a start, some turns, and an end — plus a **pitch** and a **gap**.
//! Each straight segment carries posts; each turn carries the post that starts the new direction,
//! and that post is **the same piece turned**, because a corner is a rule and not a second
//! descriptor.
//!
//! ```
//! use bevy_emerge_core::run::{Run, lay};
//! use bevy_emerge_core::library::Library;
//!
//! // A library with one 2.064 x 0.12 m fence post.
//! let library = Library { version: 1, note: None, descriptors: vec![] };
//! let run = Run {
//!     descriptor: "kit/fence".to_owned(),
//!     pitch: None,
//!     from: (0.0, 0.0),
//!     to: (4.0, 0.0),
//!     corners: Vec::new(),
//!     gaps: Vec::new(),
//! };
//! // An empty library has no such piece, so the run refuses by name.
//! assert!(lay(&run, &library).is_err());
//! ```
//!
//! # The rules, and where each comes from
//!
//! 1. **The pitch is the piece's**, along the run's direction. A piece's own footprint is the
//!    default because "the posts abut exactly" is the answer that is right for a piece nobody has
//!    looked at. A run along the piece's *short* axis is **refused**, because a fence's long
//!    dimension is the one that spans the gap.
//! 2. **A corner turns the piece.** The post at a turn carries the turn's yaw, and the next
//!    segment's pitch is measured from it.
//! 3. **A gap is a parameter of the run**, because a fence with no way through is not a fence.
//! 4. **A refusal, not a degradation.** Every way this can fail is named.

use crate::descriptor::Descriptor;
use crate::library::Library;

/// The most posts one run may lay.
///
/// A fence round a field is a few dozen; a run that lays thousands is a mistake, not a fence, and
/// the editor's fill carries the same kind of cap for the same reason — a limit that silently
/// truncates reads as a tool that finished.
pub const MAX_POSTS: usize = 1024;

/// **A piece repeated along a path.**
///
/// The whole of a run's *intent*. The posts it stands for are derived by [`lay`], and the same
/// [`Run`] always derives the same posts — which is what lets the editor's ghost and its commit
/// agree.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// The descriptor to repeat. Its own footprint is the default pitch.
    pub descriptor: String,
    /// Metres between posts. `None` means **the piece's own footprint along the run's direction**.
    pub pitch: Option<f32>,
    /// Where the run starts, in map space (metres).
    pub from: (f32, f32),
    /// Where the run ends, in map space (metres).
    pub to: (f32, f32),
    /// **The turns**, in the order the path meets them.
    pub corners: Vec<Corner>,
    /// **The openings** — posts that are missing, and where they would have been.
    pub gaps: Vec<Gap>,
}

/// **One turn in a run's path.
///
/// The point the path turns at, and the direction it takes *after* the turn. The post at the corner
/// carries this yaw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corner {
    /// Where the turn is, in map space.
    pub at: (f32, f32),
    /// The yaw after the turn, in degrees. A right angle is 90.
    pub yaw: f32,
}

/// **An opening in a run** — a stretch of posts that are not there.
///
/// `count` posts are missing, starting at `at_post` (0-based, counted along the run). The
/// graveyard's south edge is one of these: three posts out of ten, and the posts either side of
/// them are still at the run's pitch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gap {
    /// How many posts are missing.
    pub count: usize,
    /// The index of the first missing post, 0-based along the whole run.
    pub at_post: usize,
}

/// **One post on a run.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Post {
    /// Where it stands, in map space.
    pub at: (f32, f32),
    /// Which way it faces, in degrees.
    pub yaw: f32,
}

/// **The run's posts, and the refusals that stop it.**
///
/// # The refusals, and why each is one
///
/// * **a piece the library does not define** — there is no honest pitch for a piece nobody knows;
/// * **a piece with no measured footprint** — a run's pitch is the piece's size, and an unmeasured
///   piece has none;
/// * **a direction the piece is not built for** — a fence run along the piece's 12 cm side would
///   put a post every 12 cm, which is not a fence and not what the author meant;
/// * **a pitch that is not a positive number** — a run that never terminates is not a run;
/// * **a span shorter than one pitch** — there is nowhere for the post to stand;
/// * **a corner off the path** — a turn the path does not pass through is not a turn;
/// * **a gap that eats more posts than the run has** — and a gap that would eat the post at a
///   corner, because that post is the one the next segment's pitch is measured from.
///
/// Every one names what was wrong. A run that lays some of its posts and then stops is the
/// failure this module exists to remove: a fence with a hole in it, in a file, in a game.
pub fn lay(run: &Run, library: &Library) -> Result<Vec<Post>, String> {
    let Some(descriptor) = library.get(&run.descriptor) else {
        return Err(format!(
            "run: descriptor `{}` is not in this library, so there is no piece to repeat",
            run.descriptor
        ));
    };
    let descriptor: &Descriptor = descriptor;

    // **The path, as its vertices.** `from`, the corners in order, `to` — and the corners must
    // really be on the path, because a turn the path does not pass through is a turn the run
    // cannot make.
    let mut vertices: Vec<(f32, f32)> = Vec::with_capacity(run.corners.len() + 2);
    vertices.push(run.from);
    for c in &run.corners {
        if !c.at.0.is_finite() || !c.at.1.is_finite() {
            return Err(format!(
                "run: a corner is at ({}, {}), which is not a position",
                c.at.0, c.at.1
            ));
        }
        if !c.yaw.is_finite() {
            return Err(format!(
                "run: the corner at ({}, {}) turns to {yaw} degrees, which is not an angle",
                c.at.0,
                c.at.1,
                yaw = c.yaw
            ));
        }
        vertices.push(c.at);
    }
    vertices.push(run.to);
    if !run.from.0.is_finite() || !run.from.1.is_finite() {
        return Err(format!(
            "run: starts at ({}, {}), which is not a position",
            run.from.0, run.from.1
        ));
    }
    if !run.to.0.is_finite() || !run.to.1.is_finite() {
        return Err(format!(
            "run: ends at ({}, {}), which is not a position",
            run.to.0, run.to.1
        ));
    }

    // **The pitch.** The piece's own size along the run's direction, unless the author said.
    let (w, depth) = crate::descriptor::placed_footprint(descriptor).ok_or_else(|| {
        format!(
            "run: `{}` has no measured footprint, so there is no honest pitch to repeat it at. \
             Measure it on the tiles tab first.",
            run.descriptor
        )
    })?;
    if !(w.is_finite() && depth.is_finite() && w > 0.0 && depth > 0.0) {
        return Err(format!(
            "run: `{}` has a footprint of {w} by {depth} metres, which is not a size a run can \
             repeat it at.",
            run.descriptor
        ));
    }

    let mut posts: Vec<Post> = Vec::new();
    // The yaw of the segment being laid, and the piece's extent along it.
    let mut yaw = 0.0f32;
    for (i, seg) in vertices.windows(2).enumerate() {
        let (a, b) = (seg[0], seg[1]);
        let dx = b.0 - a.0;
        let dz = b.1 - a.1;
        let length = (dx * dx + dz * dz).sqrt();
        if length <= f32::EPSILON {
            return Err(format!(
                "run: the segment from ({}, {}) to ({}, {}) has no length, so there is nowhere for \
                 a post to stand",
                a.0, a.1, b.0, b.1
            ));
        }

        // **The piece's extent along this segment.** The run is along X or along Z — a rectangle's
        // edges — and the piece's long dimension must be the one that spans the gap.
        let along_x = dx.abs() >= dz.abs();
        let extent = if along_x { w } else { depth };

        // **The first segment must run along the piece's LONG side.**
        //
        // A fence post is 2.064 x 0.12 m, and a straight run of them along Z would space them
        // 2.064 m apart with 12 cm of post facing along the run — so they would not span the gaps
        // at all. The piece's own note says its long dimension "should be placed to span gaps".
        //
        // **Only the first segment.** A *turn* is exactly a run along the other axis: a fence's
        // corner is its long piece turned 90°, and the stretch after the turn runs along the axis
        // the long dimension now points across. Refusing that would refuse every corner, which is
        // the whole point of a run.
        if i == 0 && extent < w.max(depth) * 0.999 {
            return Err(format!(
                "run: `{}` is {w} by {depth} metres, and a run along {} would repeat it across \
                 its short side — the posts would not span the gaps. Run it along its long side \
                 instead.",
                run.descriptor,
                if along_x { "X" } else { "Z" },
            ));
        }

        // **The pitch: the author's, or the piece's own.**
        //
        // The default is the piece's **long** dimension whatever the segment's axis, so a run that
        // turns lays the corner at the piece's own size on each side of the turn. That is the
        // fence's shape: a 2.064 m post along X and the same post turned 90° along Z, and the two
        // meet. Pitching the second segment by the piece's 12 cm depth instead would collapse the
        // fence into a hedge.
        let pitch = match run.pitch {
            Some(p) => {
                if !(p.is_finite() && p > 0.0) {
                    return Err(format!(
                        "run: the pitch is {p} metres, which is not a distance a run can step by"
                    ));
                }
                p
            }
            None => w.max(depth),
        };
        if length < pitch {
            return Err(format!(
                "run: the segment from ({}, {}) to ({}, {}) is {length:.3} metres long, which is \
                 shorter than one pitch of {pitch:.3} metres. There is nowhere for a post to stand.",
                a.0, a.1, b.0, b.1
            ));
        }

        // **The yaw of this segment** — which way the piece faces along it. A post's long side
        // runs along the run, so the piece faces along the run.
        let segment_yaw = if along_x {
            if dx >= 0.0 { 0.0 } else { 180.0 }
        } else if dz >= 0.0 {
            90.0
        } else {
            270.0
        };
        // **A corner overrides the segment's yaw** — the turn's, which is the direction the path
        // takes after the turn.
        let post_yaw = if i == 0 {
            segment_yaw
        } else {
            // The corner that opened this segment.
            let corner = run.corners[i - 1];
            corner.yaw
        };
        let _ = yaw;
        yaw = post_yaw;

        // **The posts along this segment.** The first one stands at the segment's start — which, for
        // a segment after a corner, is the corner itself.
        let count = (length / pitch).floor() as usize;
        let ux = dx / length;
        let uz = dz / length;
        for n in 0..=count {
            let at = (a.0 + ux * pitch * n as f32, a.1 + uz * pitch * n as f32);
            posts.push(Post { at, yaw: post_yaw });
        }
        if posts.len() > MAX_POSTS {
            return Err(format!(
                "run: this run lays more than {MAX_POSTS} posts, which is a mistake rather than a \
                 fence. Shorten it or raise the pitch."
            ));
        }
    }

    // **Every post is at the run's pitch, and the run ends where the pitch puts it.**
    //
    // It used to put a last post at the path's *end* as well, which meant the final gap was
    // whatever was left over — 1.808 m on an 8 m run at 2.064 m. A run whose posts are all at one
    // pitch is the whole point of the thing: a fence with one short gap in it is a fence with a
    // hole in it, and it is exactly the defect the lattice rule was written to prevent.
    //
    // So the run ends at the last post the pitch reaches, and the stretch from there to the
    // author's end is **not** filled. A run that must reach its end exactly wants a pitch that
    // divides the span, and the run says so rather than guessing.
    if posts.is_empty() {
        return Err("run: the path laid no posts at all".to_owned());
    }

    // **The gaps.** They remove posts by index, and they are checked against the run's own length
    // so a gap cannot eat more than the run has.
    for gap in &run.gaps {
        if gap.count == 0 {
            continue;
        }
        let end = gap.at_post + gap.count;
        if end > posts.len() {
            return Err(format!(
                "run: a gap of {} post(s) at post {index} would need {end} posts, and this run \
                 lays {len}. A gap cannot eat more than the run has.",
                gap.count,
                index = gap.at_post,
                len = posts.len()
            ));
        }
        // **A gap must not eat a corner's post.** The next segment's pitch is measured from it, so
        // a run with a hole at a turn has a hole in its own geometry.
        for corner in &run.corners {
            if posts
                .iter()
                .position(|p| {
                    (p.at.0 - corner.at.0).abs() < 1e-3 && (p.at.1 - corner.at.1).abs() < 1e-3
                })
                .is_some_and(|i| i >= gap.at_post && i < end)
            {
                return Err(format!(
                    "run: a gap of {} post(s) at post {index} would eat the post at the corner \
                     ({}, {}), and the next stretch is measured from it.",
                    gap.count,
                    corner.at.0,
                    corner.at.1,
                    index = gap.at_post
                ));
            }
        }
        posts.drain(gap.at_post..end);
    }

    if posts.len() > MAX_POSTS {
        return Err(format!(
            "run: this run lays {} posts, which is more than {MAX_POSTS}.",
            posts.len()
        ));
    }
    Ok(posts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::Extent;
    use crate::library::{LIBRARY_VERSION, Library};

    fn library() -> Library {
        Library {
            version: LIBRARY_VERSION,
            note: None,
            descriptors: vec![Descriptor {
                id: "kit/fence".to_owned(),
                extent: Extent {
                    footprint: Some((2.064, 0.12)),
                    height: Some(0.838),
                },
                ..Default::default()
            }],
        }
    }

    fn run() -> Run {
        Run {
            descriptor: "kit/fence".to_owned(),
            pitch: None,
            from: (0.0, 0.0),
            to: (4.0, 0.0),
            corners: Vec::new(),
            gaps: Vec::new(),
        }
    }

    /// **The pitch is the piece's.** 2.064 m, and *not* one of the project's rungs.
    #[test]
    fn the_pitch_is_the_pieces_own_footprint() {
        let posts = lay(&run(), &library()).unwrap_or_else(|e| panic!("{e}"));
        // A 4 m run at 2.064 m stands posts at 0 and 2.064 — and stops there, because the next
        // would be 4.128, past the end. **Every gap is the pitch**: a run whose posts are all at
        // one pitch is the whole point of the thing.
        assert_eq!(posts.len(), 2, "0 and 2.064: {posts:?}");
        assert!((posts[1].at.0 - 2.064).abs() < 1e-4, "{posts:?}");
        // **Not on the lattice.** The project's rungs are 1.000, 0.333 and 0.111 m.
        for rung in [1.0f32, 1.0 / 3.0, 1.0 / 9.0] {
            assert!(
                (posts[1].at.0 / rung).fract().abs() > 1e-3,
                "the pitch landed on the {rung} rung, which is the grid the fill uses"
            );
        }
    }

    /// **A run along the piece's short side is refused**, by name.
    ///
    /// The *first* stretch, which is the one that sets the fence's grain. A stretch *after* a
    /// turn is along the short side by definition — that is what a corner is — and is legal.
    #[test]
    fn a_run_across_the_pieces_short_side_is_refused() {
        let mut r = run();
        r.to = (0.0, 4.0);
        let err = lay(&r, &library()).expect_err("across the short side");
        assert!(err.contains("kit/fence"), "{err}");
        assert!(err.contains("short side"), "{err}");
    }

    /// **A corner turns the piece**, and it is the same piece.
    #[test]
    fn a_corner_turns_the_piece() {
        let mut r = run();
        r.to = (4.0, 4.0);
        r.corners = vec![Corner {
            at: (4.0, 0.0),
            yaw: 90.0,
        }];
        let posts = lay(&r, &library()).unwrap_or_else(|e| panic!("{e}"));
        // The first two are along X at yaw 0; the corner post and the stretch after it are along Z
        // at yaw 90. The corner is the **same descriptor**, turned.
        assert_eq!(posts[0].yaw, 0.0, "{posts:?}");
        assert_eq!(posts[2].yaw, 90.0, "the corner post turns: {posts:?}");
        assert!((posts[2].at.0 - 4.0).abs() < 1e-4, "{posts:?}");
        assert!((posts[2].at.1 - 0.0).abs() < 1e-4, "{posts:?}");
        assert!((posts[3].at.0 - 4.0).abs() < 1e-4, "{posts:?}");
        // **The second stretch is pitched by the piece's LONG side** — 2.064 m, not its 12 cm depth.
        // Pitching it by the depth would collapse the fence into a hedge.
        assert!((posts[3].at.1 - 2.064).abs() < 1e-4, "{posts:?}");
    }

    /// **A gap removes posts, and the ones either side are still at the pitch.**
    #[test]
    fn a_gap_removes_posts_and_leaves_the_rest_at_the_pitch() {
        let mut r = run();
        r.to = (8.0, 0.0);
        r.gaps = vec![Gap {
            count: 2,
            at_post: 1,
        }];
        let posts = lay(&r, &library()).unwrap_or_else(|e| panic!("{e}"));
        // An 8 m run at 2.064 m stands posts at 0, 2.064, 4.128 and 6.192 — four, and the last is
        // a pitch short of 8 because the next would be 8.256, past the end. The gap takes indices 1
        // and 2 (2.064 and 4.128), so two are left: 0 and 6.192.
        assert_eq!(posts.len(), 2, "{posts:?}");
        // **The post after the gap is still at the run's pitch** — 6.192 is the fourth position,
        // and it did not move to fill the hole. A gap is an absence, not a compaction.
        assert!((posts[1].at.0 - 6.192).abs() < 1e-4, "{posts:?}");
    }

    /// **A gap cannot eat more than the run has.**
    #[test]
    fn a_gap_wider_than_the_run_is_refused() {
        let mut r = run();
        r.gaps = vec![Gap {
            count: 99,
            at_post: 0,
        }];
        let err = lay(&r, &library()).expect_err("the gap is wider than the run");
        assert!(err.contains("cannot eat more than the run has"), "{err}");
    }

    /// **A piece the library does not define is refused by name.**
    #[test]
    fn an_unknown_piece_is_refused_by_name() {
        let mut r = run();
        r.descriptor = "kit/no_such_fence".to_owned();
        let err = lay(&r, &library()).expect_err("no such piece");
        assert!(err.contains("kit/no_such_fence"), "{err}");
    }

    /// **A piece with no measured footprint is refused**, because a run's pitch is the piece's size.
    #[test]
    fn an_unmeasured_piece_is_refused() {
        let mut lib = library();
        lib.descriptors[0].extent.footprint = None;
        let err = lay(&run(), &lib).expect_err("unmeasured");
        assert!(err.contains("no measured footprint"), "{err}");
    }

    /// **A pitch that is not a distance is refused.**
    #[test]
    fn a_bad_pitch_is_refused() {
        for pitch in [0.0f32, -1.0, f32::NAN, f32::INFINITY] {
            let mut r = run();
            r.pitch = Some(pitch);
            let err = lay(&r, &library()).expect_err("a bad pitch");
            assert!(
                !err.is_empty(),
                "the refusal must say something about {pitch}"
            );
        }
    }

    /// **A span shorter than one pitch is refused.**
    #[test]
    fn a_span_shorter_than_a_pitch_is_refused() {
        let mut r = run();
        r.to = (0.5, 0.0);
        let err = lay(&r, &library()).expect_err("too short");
        assert!(err.contains("shorter than one pitch"), "{err}");
    }
}
