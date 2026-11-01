//! **Do the tiles the author placed agree with the tokens the author declared?**
//!
//! [`Subgrid::edge`](crate::descriptor::SubCell::edge) is what a tile presents to whatever sits
//! beside it. This module is the one thing that reads it: a comparison, [`seam`], and one caller,
//! [`faults`], which walks a finished map and reports every abutting pair whose facing tokens
//! disagree.
//!
//! # Why it reports rather than generates
//!
//! `crate::grammar` already does tile adjacency, and its module doc argues it should be the only way:
//! *"inventing an adjacency schema would mean asking an author to write down a grammar before they
//! are allowed to draw one."* That argument stands, so this does not become a second solver. It
//! answers a different question — **not** "what may go here", which the learned grammar answers from
//! the map, but "does what you drew match what you declared", which nothing could answer before.
//!
//! The two are not rivals, and Karth & Smith 2017 §"Patterns" names them as the algorithm's own two
//! modes: *"In the **simple tiled** version of the algorithm, the patterns are specified as explicit
//! tile constraint relationships. In the overlapping version, the constraints are **inferred from the
//! source image**."* `grammar.rs` is the inferred half. `edge` is the explicit half — and whether it
//! should also feed the solver's support table, rather than only checking it, is `FVS-Q-10`.
//!
//! Tokens are intent; the map is behaviour. A fault is the two disagreeing, and naming which face and
//! which tokens is the whole product. PCG Book ch11 §11.2.3 puts it plainly: a designer *"may become
//! frustrated or confused if the computer consistently acts as though it is not following the model
//! that the human designer has in her head"*. Reporting is also the only one of that chapter's three
//! conflict responses — error, arbitrary pick, or offer alternatives — that satisfies this project's
//! rule that a program fails loudly rather than repairing input behind the author's back.
//!
//! # Matching is equality
//!
//! Merrell & Manocha 2009, *Constraint-Based Model Synthesis* §4.3 — the paper WaveFunctionCollapse
//! descends from — defines adjacency as *"evaluating two states in opposite directions and checking
//! if their evaluations are identical"*. Equality of the facing descriptor, not a compatibility
//! table: a table is a second artifact that can drift from the tokens it is about.
//!
//! An unlabelled cell equals only another unlabelled cell. That is not a wildcard, and the difference
//! matters: a wildcard would make unauthored data permissive and authored data strict, which is one
//! function with two behaviours. Equality gives the useful property for free — with every lattice
//! empty, every face is all-`None`, every pair matches, and `faults` returns nothing. The feature is
//! inert until it is authored, **without a branch anywhere that makes it inert**.
//!
//! # This does not move `snapshot_hash`
//!
//! No RNG, no placement change, no write. It reads a map and returns a list.

use crate::descriptor::Subgrid;
use crate::library::Library;
use crate::map::Map;
use crate::placement::ir::Dir;
use crate::wfc::{E, N, S, W};

/// How close a yaw must be to a quarter turn to count as one, in degrees.
///
/// Yaws are authored in 15° steps (`bevy_cauldron`'s `YAW_STEP`) and stored as `f32`, so a value that
/// went through a serialize/parse round trip can be a hair off. A thousandth of a degree is far below
/// anything an author can express and far above `f32`'s error at these magnitudes.
pub(crate) const SQUARE_EPSILON: f32 = 1e-3;

/// How far inside a cell boundary a piece's edge may land and still count as on it, as a fraction of
/// a cell.
///
/// A 3 m piece centred on the grid has edges that are a hair under or over a whole number of cells
/// depending on where it sits, and a seam that disappears because two `f32`s rounded opposite ways is
/// the kind of fault an author cannot see the cause of. A thousandth of a cell is half a millimetre
/// at the shipped size — far below anything authored, far above `f32`'s error here.
pub(crate) const EDGE_EPSILON: f32 = 1e-3;

/// **Which quarter turn this yaw is**, or a refusal naming the piece and the angle.
///
/// A lattice face only exists at multiples of 90°. At 45° a tile presents a corner to its neighbour
/// and there is no column of cells to compare — so this refuses rather than rounding, because
/// rounding would silently check the wrong face and report a fault the author cannot see the cause
/// of. The editor's turn step is 15°, so off-square yaws are reachable and this refusal will fire.
pub fn quarter_turns(who: &str, yaw_deg: f32) -> Result<u8, String> {
    if !yaw_deg.is_finite() {
        return Err(format!("`{who}` has a yaw of {yaw_deg}, which is not an angle"));
    }
    let from_square = yaw_deg.rem_euclid(90.0);
    let off = from_square.min(90.0 - from_square);
    if off > SQUARE_EPSILON {
        return Err(format!(
            "`{who}` sits at yaw {yaw_deg}; a lattice face is only defined at multiples of 90, so \
             its edge tokens cannot be matched against a neighbour"
        ));
    }
    Ok((yaw_deg / 90.0).round().rem_euclid(4.0) as u8)
}

/// **The edge tokens on one face, in lattice order.**
///
/// `dir` is [`crate::wfc`]'s edge index, and its world meaning is `grammar::learn`'s step table:
/// `N` is −Z, `E` is +X, `S` is +Z, `W` is −X.
///
/// The order is `y` outer, then the axis that is not `dir`'s. A cell with no `edge` contributes
/// `None`, which is a token in its own right; see the module note.
///
/// **This reads a face; it does not compare two.** [`seam`] is the comparison, and it does not go
/// through this — two pieces of different sizes share only part of a face, so reading each in full and
/// checking them element for element is the thing that was wrong. Kept because "what does this face
/// present" is a question worth being able to ask on its own.
pub fn face(g: &Subgrid, dir: Dir, div: (u32, u32, u32)) -> Vec<Option<&str>> {
    let (dx, dy, dz) = div;
    let mut out = Vec::new();
    // A degenerate lattice has no face. `Subgrid::validate` refuses one in a file; this is reached
    // from a map, so it answers rather than panicking.
    if dx == 0 || dy == 0 || dz == 0 {
        return out;
    }
    let token = |at: (u32, u32, u32)| g.at(at).and_then(|c| c.edge.as_deref());
    for y in 0..dy {
        match dir {
            E => (0..dz).for_each(|z| out.push(token((dx - 1, y, z)))),
            W => (0..dz).for_each(|z| out.push(token((0, y, z)))),
            N => (0..dx).for_each(|x| out.push(token((x, y, 0)))),
            S => (0..dx).for_each(|x| out.push(token((x, y, dz - 1)))),
            // `Dir` is a `usize` alias, so the compiler cannot prove the four are exhaustive. A
            // direction outside N/E/S/W has no face rather than a made-up one.
            _ => return Vec::new(),
        }
    }
    out
}

/// **What a geometrically-derived edge token is called when the cell is filled.**
///
/// The pair below are the whole vocabulary a derivation invents, and they are deliberately about
/// *space* rather than about meaning: `SubCell::edge`'s own note says these are matched by equality,
/// so a token has to mean exactly one thing or the seam reports a fault nobody can account for.
/// "There is mesh against this face here" is that kind of thing; "this is a wall" is not.
pub const EDGE_SOLID: &str = "solid";
/// The other half of [`EDGE_SOLID`] — nothing of the mesh reaches this boundary cell.
///
/// **Stated rather than left as `None`.** A missing token already means something here: [`face`]
/// hands back `None` for a cell with no `edge`, and `None` matches only `None`. Deriving *both*
/// halves is what makes a derived lattice say what it means instead of leaning on an absence that
/// a hand-authored piece uses for "not decided yet".
pub const EDGE_OPEN: &str = "open";

/// **Read a piece's boundary occupancy as edge tokens** — sockets inferred from the mesh rather than
/// typed a cell at a time.
///
/// # Why this is possible at all
///
/// `SubCell::solid` is already rasterised from the GLB by `import::occupancy`, and its own note
/// records that **nothing reads it to decide anything** — a ruling from FVS-Q-9, which measured it
/// against `stack::covers` and closed *no*. It was left as *"the author's confirmation that the
/// lattice lines up with the mesh"*. This is the thing it turns out to be exactly right for: the
/// question a socket answers is "what does this face present", and a rasterised boundary answers it
/// without anybody typing.
///
/// Karth & Smith 2019 (`10.1145/3337722.3341845`) record the same move in Bad North, which *"learns
/// which modules can be adjacent by comparing the vertices at the edges of the 3D model and looking
/// for matching profiles in the shared plane"*. This compares the rasterised profile instead of the
/// vertices, because a lattice is what this schema already has and `adjacency` already matches faces
/// cell for cell. Sandhu et al. 2019 name the stake: configuring a constraint system *"can be even
/// more difficult than creating assets"*.
///
/// # What it returns, and what it does not do
///
/// One `(cell, token)` per **boundary** cell — the same cells [`face`] reads, unioned over N/E/S/W —
/// in ascending cell order, deduplicated. A corner cell belongs to two faces and appears once.
///
/// **It proposes; it does not write.** Both tokens have to be declared in the project's `edge` axis
/// before either can be saved, and `Vocabularies::masks` refuses an undeclared one by name. That
/// refusal is the point rather than an obstacle: `vocab.rs` calls a closed axis *"the honest reading
/// of 'this project has not decided what its tiles present'"*, and a derivation that quietly widened
/// the vocabulary would be deciding that on the author's behalf.
///
/// Interior cells are untouched, so a piece can carry derived boundaries and hand-authored insides.
pub fn derive_edges(g: &Subgrid, div: (u32, u32, u32)) -> Vec<((u32, u32, u32), &'static str)> {
    let (dx, dy, dz) = div;
    // A degenerate lattice has no boundary, on the same rule [`face`] follows: answer rather than
    // panic, because this is reached from a measured mesh and not only from a validated file.
    if dx == 0 || dy == 0 || dz == 0 {
        return Vec::new();
    }
    let mut out: Vec<(u32, u32, u32)> = Vec::new();
    for y in 0..dy {
        // The four faces, exactly as `face` indexes them. A corner is in two of these lists and must
        // not be proposed twice — hence the sort-and-dedup below rather than four appends.
        for z in 0..dz {
            out.push((dx - 1, y, z));
            out.push((0, y, z));
        }
        for x in 0..dx {
            out.push((x, y, 0));
            out.push((x, y, dz - 1));
        }
    }
    out.sort_unstable();
    out.dedup();
    out.into_iter()
        .map(|at| {
            let solid = g.at(at).is_some_and(|c| c.solid);
            (at, if solid { EDGE_SOLID } else { EDGE_OPEN })
        })
        .collect()
}

/// **A piece's world extent**, and the lattice divisions that span it.
///
/// The two are quoted together because neither means anything alone: a cell index is only a position
/// once you know which span it divides.
#[derive(Clone, Copy, Debug)]
pub struct Placed3 {
    /// World `(min, max)` per axis, after yaw.
    pub x: (f32, f32),
    pub y: (f32, f32),
    pub z: (f32, f32),
    /// Divisions per axis, matching those world spans — so already turned, if the piece is.
    pub div: (u32, u32, u32),
}

/// Which cell of `n` divisions over `[lo, hi]` contains `v`.
///
/// Clamped rather than refused: callers sample the centre of a step inside the overlap, so a value
/// lands outside only through floating point, and the honest answer there is the edge cell.
pub(crate) fn index(v: f32, (lo, hi): (f32, f32), n: u32) -> u32 {
    if n == 0 || hi <= lo {
        return 0;
    }
    (((v - lo) / (hi - lo) * n as f32) as i64).clamp(0, n as i64 - 1) as u32
}

type SeamTokens<'a> = (Vec<Option<&'a str>>, Vec<Option<&'a str>>);

/// **The tokens either side of a seam presents to the other, over the part they actually share.**
///
/// # Why not whole faces
///
/// Comparing whole faces element for element — which this did until 2026-08-05 — is only meaningful
/// when the two pieces are
/// the same size. The shipped kits say otherwise in two independent places:
///
/// * A 2.40 m wall meets a **2.00 m doorway** with a 0.40 m header above it. Whole faces are five rows
///   against four, so the seam was refused even when every token on it read `wall` — and the header,
///   which supplies the fifth row, was never compared to the doorway at all because it shares its
///   `(x, z)` cells rather than abutting them.
/// * `site_greybox`'s **`wall_corner` is twice as wide** as its own wall. Not an architectural stack,
///   just two pieces of one family being different widths.
///
/// Both are the same defect: equality of whole faces asks "are these pieces the same shape" when the
/// question is "do they agree where they touch". This answers the second.
///
/// # How
///
/// The seam is a rectangle: the **lateral** overlap along the axis the seam runs on, and the
/// **vertical** overlap of the two pieces' world heights. It is sampled at the centre of each subunit
/// step and each sample is mapped into both pieces' own lattices, so pieces divided differently — or
/// offset from each other — are still read at the same physical places, in the same order.
///
/// `None` when they share no rectangle: no lateral overlap, or one piece entirely above the other.
/// That is not a seam and there is nothing to compare.
pub fn seam<'a>(
    a: &'a Subgrid,
    a_at: Placed3,
    b: &'a Subgrid,
    b_at: Placed3,
    dir: Dir,
    subunit: f32,
) -> Option<SeamTokens<'a>> {
    if !(subunit.is_finite() && subunit > 0.0) {
        return None;
    }
    // The vertical overlap is shared by every seam direction.
    let y = (a_at.y.0.max(b_at.y.0), a_at.y.1.min(b_at.y.1));
    if y.1 <= y.0 {
        return None;
    }
    // The lateral axis is whichever one the seam runs along, and the face index on the other is
    // fixed by the direction: entering from the East reads `a`'s last x column and `b`'s first.
    let lateral_is_z = dir == E || dir == W;
    let (a_lat, b_lat) = if lateral_is_z {
        (a_at.z, b_at.z)
    } else {
        (a_at.x, b_at.x)
    };
    let lat = (a_lat.0.max(b_lat.0), a_lat.1.min(b_lat.1));
    if lat.1 <= lat.0 {
        return None;
    }

    let last = |n: u32| n.saturating_sub(1);
    let (a_face, b_face) = match dir {
        E => (last(a_at.div.0), 0),
        W => (0, last(b_at.div.0)),
        N => (0, last(b_at.div.2)),
        S => (last(a_at.div.2), 0),
        _ => return None,
    };

    // Steps, not divisions: the two pieces may divide this stretch differently, so the sampling rate
    // is the project's own subunit and both are read at the same physical places.
    let steps = |span: (f32, f32)| (((span.1 - span.0) / subunit).round() as u32).max(1);
    let (n_lat, n_y) = (steps(lat), steps(y));

    let mut left = Vec::with_capacity((n_lat * n_y) as usize);
    let mut right = Vec::with_capacity((n_lat * n_y) as usize);
    for iy in 0..n_y {
        let wy = y.0 + (iy as f32 + 0.5) * (y.1 - y.0) / n_y as f32;
        for il in 0..n_lat {
            let wl = lat.0 + (il as f32 + 0.5) * (lat.1 - lat.0) / n_lat as f32;
            let cell = |at: Placed3, face: u32| -> (u32, u32, u32) {
                let ay = index(wy, at.y, at.div.1);
                if lateral_is_z {
                    (face, ay, index(wl, at.z, at.div.2))
                } else {
                    (index(wl, at.x, at.div.0), ay, face)
                }
            };
            left.push(a.at(cell(a_at, a_face)).and_then(|c| c.edge.as_deref()));
            right.push(b.at(cell(b_at, b_face)).and_then(|c| c.edge.as_deref()));
        }
    }
    Some((left, right))
}

/// One disagreement between two placed tiles.
#[derive(Clone, Debug, PartialEq)]
pub struct Fault {
    /// The placement whose face is named by [`Self::dir`].
    pub a: String,
    /// The placement on `a`'s `dir` side.
    pub b: String,
    /// N, E, S or W, as [`crate::wfc`] numbers them.
    pub dir: Dir,
    /// What `a` presents on that face.
    pub a_face: Vec<Option<String>>,
    /// What `b` presents back.
    pub b_face: Vec<Option<String>>,
    /// The whole thing in a sentence, for a status line.
    pub message: String,
}

/// N, E, S, W as an author reads them.
///
/// Public because a report that names a fault's direction must spell it the one way this crate
/// spells it: a second copy in the editor is a second spelling, and the two drift the first time
/// one of them gains a word.
pub fn dir_name(dir: Dir) -> &'static str {
    match dir {
        N => "N",
        E => "E",
        S => "S",
        W => "W",
        _ => "?",
    }
}

/// **Every abutting pair whose edge tokens disagree.**
///
/// # Neighbours are decided in world metres, on no grid at all
///
/// This took a grid step (`cell`) until 2026-08-05 and quantised every placement into an integer
/// cell rectangle before asking which pieces touched. That one parameter was doing three
/// incompatible jobs and was wrong at all three:
///
/// * **It was not aligned to anything.** The rectangle was anchored at `map.floor_rect()`'s
///   `min_x`/`min_z` — that is `-bounds/2`, which is a multiple of the step only when the map's
///   size happens to be an even multiple of it. Set a map 33 m wide and every span shifts half a
///   cell, `ax1 == bx0` is never true, and **no seam in the map pairs at all** — reported as `ok`.
/// * **It erased every piece smaller than the step.** The editor passed 1.0 m while placement snaps
///   on [`crate::grid::SNAP`] = 0.5 m, so two touching half-metre pieces landed in one cell, shared
///   one span, and were never compared — silently exempting most of the furniture from the only
///   check that reads their edge tokens.
/// * **It set the sampling rate.** [`seam`] was sampled at `cell / divisions`, but a lattice cell is
///   `SNAP / divisions` — so the editor read every *other* row of a face and called it a match.
///
/// So the grid is gone. Two pieces are neighbours when a face of one **is** a face of the other in
/// world coordinates (within [`EDGE_EPSILON`]) and they overlap on the perpendicular axis, which is
/// the same exact-world-extent question [`seam`] already answers for the tokens themselves. One
/// path, one set of coordinates.
///
/// Pieces that *overlap* rather than meet are not a seam and are not compared. That is deliberate:
/// [`crate::grid::snap_span`] rounds a 0.55 m bench down to a 0.5 m cell precisely so set dressing
/// tiles without gaps, and accepts a 5 cm overlap to do it. Architecture — the thing edge tokens are
/// authored on — is sized in exact multiples and meets exactly.
///
/// # What it reads
///
/// The **authored** `Descriptor`, patched by the placement. `Placed::patch` replaces a lattice
/// wholesale rather than merging it (`Descriptor::patched`), so a placement that carries one is
/// standing on a different lattice than the library entry says — reading the library entry would
/// validate a tile that is not on the map. `Resolved` is no use here either: it has no `subgrid`.
///
/// # Order
///
/// Sorted on `(a, b, dir)`, which is a **total** key because `Map::validate` requires placement ids
/// to be unique — the project's determinism rule, and the reason this cannot depend on the order
/// `map.placements` happens to be in.
///
/// # Refusals are faults too
///
/// A yaw that is not a quarter turn produces a fault carrying that sentence rather than an `Err` for
/// the whole map. One unturnable piece must not blind the author to the other twelve, and the message
/// says exactly which piece and which angle.
pub fn faults(map: &Map, library: &Library, divisions: u32) -> Vec<Fault> {
    let mut out: Vec<Fault> = Vec::new();

    // **Nothing declared, nothing to check.** The module note calls this feature inert until it is
    // authored; this is where that stops being a property of the answer and becomes a property of
    // the cost. Until an edge token exists anywhere, the whole pass is one scan of the library
    // instead of a quadratic walk of the map that clones a descriptor per placement — and the editor
    // reruns it on every placement, so a 1,400-piece flood fill was paying for it repeatedly.
    //
    // Patches are checked too: `Placed::patch` may carry a lattice the library never had.
    let declared = |g: &Option<Subgrid>| {
        g.as_ref()
            .is_some_and(|g| g.cells.iter().any(|c| c.edge.is_some()))
    };
    let any_declared = library.descriptors.iter().any(|d| declared(&d.subgrid))
        || map
            .placements
            .iter()
            .any(|p| p.patch.as_ref().is_some_and(|d| declared(&d.subgrid)));
    if !any_declared {
        return out;
    }
    // **One lattice cell, in metres.** `descriptor::divisions` is `grid::cells(span) * divisions`,
    // and `grid::cells` counts [`crate::grid::SNAP`]-sized units — so a cell is `SNAP / divisions`
    // across, and that, not the caller's idea of a grid step, is the rate at which a face has to be
    // sampled to read every row of it exactly once.
    let subunit = crate::grid::SNAP / divisions.max(1) as f32;

    // A piece whose descriptor is missing from the library is the map's problem, not this check's;
    // `Map::validate` is where that is caught.
    /// **A placement resolved to world coordinates, with its yaw already answered for.**
    ///
    /// The yaw is squared **once**, here. It used to be squared twice — the footprint swapped in this
    /// loop and the cells rotated in the pair loop below, from two separate `quarter_turns` calls on
    /// the same angle. The second call handled its `Err`. The first folded it into a `_` arm shared
    /// with the even-quarter case, so the two calls could disagree about whether a piece was
    /// turnable at all. Storing the one answer is what makes that impossible rather than unlikely.
    struct Tile<'a> {
        id: &'a str,
        /// World `(min, max)` per axis — what decides both who touches whom and where.
        x: (f32, f32),
        y: (f32, f32),
        z: (f32, f32),
        /// **The lattice as it stands, or why this piece has no readable face.**
        ///
        /// `Ok` carries the cells rotated *and* the divisions swapped: a face read off one without
        /// the other is a face of the wrong length. Both are derived from this placement's own
        /// patched extent, so a placement that overrides its size is compared on the lattice it
        /// actually stands on.
        turned: Result<(Subgrid, (u32, u32, u32)), String>,
        /// Whether this tile says anything about its edges at all.
        declares: bool,
    }
    // **Every placement's base Y.** The seam comparison needs a vertical overlap, and a piece's Y is
    // not in its `Placed`: it comes from its `mount` through `stack::datum`, which for anything
    // resting on a surface needs that surface resolved first. `resolve_y` is the one function that
    // does it, so it is the one used here.
    //
    // A map whose heights cannot be resolved is `Map::validate`'s problem and `placement_at`'s to
    // report — the same call this function already makes about a descriptor missing from the library.
    // Reporting it twice under a name that hides its cause helps nobody.
    let Ok(ys) = crate::stack::resolve_y(map, library) else {
        return out;
    };

    let mut placed: Vec<Tile> = Vec::new();
    for (i, p) in map.placements.iter().enumerate() {
        let Some(base) = library.get(&p.descriptor) else {
            continue;
        };
        // Borrowed unless a patch forces a merge. The descriptor carries several string vectors and
        // this runs per placement, per recheck; the lattice below is sparse and cheap to clone.
        let patched;
        let authored: &crate::descriptor::Descriptor = match &p.patch {
            Some(patch) => {
                patched = base.patched_with(patch);
                &patched
            }
            None => base,
        };
        // A piece with no derivable lattice has no face to check. That is not a fault of the map —
        // a missing footprint is `Descriptor::resolve`'s to report, and reporting it here too would
        // put the same problem in front of the author twice under a name that hides its cause.
        let Ok(div) = crate::descriptor::divisions(authored, divisions) else {
            continue;
        };
        let grid = authored.subgrid.clone().unwrap_or_default();
        let declares = grid.cells.iter().any(|c| c.edge.is_some());
        // `divisions` above already refused a piece with no footprint, naming it; there is no second
        // opinion to form here and no default that would be honest.
        //
        // **The placed footprint, not the measured one** — the same call `divisions` just made. These
        // two describe the same box and a seam is read off both, so a scale applied to one and not the
        // other would sample a lattice at coordinates outside the piece it belongs to.
        let Some((fw, fd)) = crate::descriptor::placed_footprint(authored) else {
            continue;
        };
        // **The turn, squared once.** Both halves together: `rotated` takes the divisions the cells
        // were authored on, so it reads `div` before `rotate_div` replaces it.
        let turn = quarter_turns(&p.id, p.yaw);
        let turned = match &turn {
            Ok(q) => Ok((grid.rotated(*q, div), crate::descriptor::rotate_div(div, *q))),
            Err(message) => Err(message.clone()),
        };
        // The footprint as it stands. The arms are spelled out because collapsing two of them is
        // precisely what went wrong here: an off-square yaw has **no** swap that means anything, and
        // sharing an arm with "an even quarter turn changes nothing" made those two cases
        // indistinguishable to the reader and to the next person to edit it.
        let (fw, fd) = match &turn {
            Ok(q) if q % 2 == 1 => (fd, fw),
            Ok(_) => (fw, fd),
            // Its unturned footprint, used for one purpose: letting `side` find whoever stands next
            // to it, so the refusal below reaches an author who has declared something nearby.
            // Nothing reads this piece's lattice — `turned` is an `Err`, so the seam is skipped and
            // the refusal is the only thing the pair can produce.
            Err(_) => (fw, fd),
        };
        let base = ys.get(i).copied().unwrap_or(map.origin.1);
        // The height as it stands, which is the same rule `divisions` uses — see
        // `descriptor::placed_height` on why the scale and the stretch both belong in it.
        let h = crate::descriptor::placed_height(authored).unwrap_or(0.0);
        placed.push(Tile {
            id: p.id.as_str(),
            x: (p.at.0 - fw * 0.5, p.at.0 + fw * 0.5),
            y: (base, base + h),
            z: (p.at.1 - fd * 0.5, p.at.1 + fd * 0.5),
            declares,
            turned,
        });
    }

    // **A pair where neither tile declares an edge is not checked at all.**
    //
    // This is the scope of the question, not a shortcut: the check asks whether what was drawn
    // matches what was *declared*, so where nothing is declared there is nothing to match. It is also
    // what keeps the feature quiet on a map that has never used it — `break_room` has an armchair at
    // yaw 240, and squaring that yaw is only anybody's problem once the armchair has a token on it.
    // Which side of `a` does `b` sit on, if they share a face at all?
    //
    // Two rectangles abut when one's far edge is the other's near edge AND their spans overlap on
    // the other axis — touching at a corner is not a seam, because no column of cells faces another.
    // The world meaning of the directions is `crate::wfc`'s: N is -Z, E is +X, S is +Z, W is -X.
    let side = |a: &Tile, b: &Tile| -> Option<Dir> {
        // One piece's far edge IS the other's near edge, to within the float slop of having got
        // there by adding half a footprint to a centre.
        let meets = |p: f32, q: f32| (p - q).abs() <= EDGE_EPSILON;
        // A strict overlap on the perpendicular axis. Touching at a corner is not a seam, because no
        // column of cells faces another — so a zero-width overlap has to fail, which is what the
        // epsilon on the near side is for.
        let over = |a: (f32, f32), b: (f32, f32)| a.0.max(b.0) + EDGE_EPSILON < a.1.min(b.1);
        if meets(a.x.1, b.x.0) && over(a.z, b.z) {
            Some(E)
        } else if meets(b.x.1, a.x.0) && over(a.z, b.z) {
            Some(W)
        } else if meets(a.z.1, b.z.0) && over(a.x, b.x) {
            Some(S)
        } else if meets(b.z.1, a.z.0) && over(a.x, b.x) {
            Some(N)
        } else {
            None
        }
    };

    let mut yaw_reported: Vec<&str> = Vec::new();
    for a in &placed {
        for b in &placed {
            {
                let Some(dir) = side(a, b) else {
                    continue;
                };
                if !a.declares && !b.declares {
                    continue;
                }
                // **One seam, one fault.** Every adjacent pair is walked twice — once from each side —
                // and reporting both made a single disagreement read as "2 faults", which is the
                // panel telling the author to look for a second problem that is not there. `a < b`
                // picks one of the two orderings; it is total because ids are unique, so which one is
                // not left to the order `map.placements` happens to be in.
                if a.id >= b.id {
                    continue;
                }
                // Both yaws have to square before there is a face to read. Reported once per piece,
                // because the same tilted piece has up to four neighbours and four copies of one
                // sentence is not four pieces of information — and reported *here*, on reaching a
                // pair, rather than when the placement was read: an armchair at 240° with nothing
                // declared anywhere near it is nobody's problem, and saying so would bury the faults
                // that are.
                for t in [a, b] {
                    if let Err(message) = &t.turned
                        && !yaw_reported.contains(&t.id)
                    {
                        yaw_reported.push(t.id);
                        out.push(Fault {
                            a: t.id.to_owned(),
                            b: String::new(),
                            dir: N,
                            a_face: Vec::new(),
                            b_face: Vec::new(),
                            message: message.clone(),
                        });
                    }
                }
                // **The part they share, not the whole face.** See [`seam`] for why, and for the two
                // places in the shipped kits that forced the change.
                let (Ok((a_grid, a_div)), Ok((b_grid, b_div))) = (&a.turned, &b.turned) else {
                    continue;
                };
                let a_at = Placed3 { x: a.x, y: a.y, z: a.z, div: *a_div };
                let b_at = Placed3 { x: b.x, y: b.y, z: b.z, div: *b_div };
                let Some((a_face, b_face)) = seam(a_grid, a_at, b_grid, b_at, dir, subunit) else {
                    // No rectangle in common: the cell rects touch but the pieces do not overlap in Y,
                    // so there is no seam here to be right or wrong about.
                    continue;
                };
                if a_face == b_face {
                    continue;
                }
                out.push(Fault {
                    a: a.id.to_owned(),
                    b: b.id.to_owned(),
                    dir,
                    a_face: a_face.iter().map(|t| t.map(str::to_owned)).collect(),
                    b_face: b_face.iter().map(|t| t.map(str::to_owned)).collect(),
                    message: format!(
                        "`{}` face {} is {} but `{}` presents {}",
                        a.id,
                        dir_name(dir),
                        say(&a_face),
                        b.id,
                        say(&b_face)
                    ),
                });
            }
        }
    }

    // A total key: ids are unique (`Map::validate`), so no two faults tie on all three.
    out.sort_by(|l, r| (&l.a, &l.b, l.dir).cmp(&(&r.a, &r.b, r.dir)));
    out
}

/// One face of a kit piece that no piece of the kit can meet.
#[derive(Clone, Debug, PartialEq)]
pub struct DeadEnd {
    /// The descriptor whose face this is, unturned.
    pub piece: String,
    /// N, E, S or W in the piece's own frame, as [`crate::wfc`] numbers them.
    pub dir: Dir,
    /// What the face presents, in [`face`]'s order.
    pub face: Vec<Option<String>>,
    /// The piece that comes nearest to meeting it (#145), or `None` when no piece shares a stretch
    /// of the face carrying one of its declared tokens.
    pub closest: Option<Closest>,
    /// The whole thing in a sentence, for a status line.
    pub message: String,
}

/// **The piece that comes nearest to closing a dead end**, and the cells that keep it from it.
///
/// Of every trial [`dead_ends`] makes, the one whose seam disagrees with the face in the fewest
/// cells; ties go to the piece id, then the turn, then the placement along the face (near end,
/// centre, far end). So an author reads "one token off, and which one" instead of setting every
/// piece against the face by hand.
#[derive(Clone, Debug, PartialEq)]
pub struct Closest {
    /// The candidate's descriptor id.
    pub piece: String,
    /// Its quarter turns, 0..4, as placed against the face.
    pub turn: u8,
    /// Each seam sample where the two disagree, in [`seam`]'s order.
    pub misses: Vec<Miss>,
}

/// One cell of a seam where the candidate presents something other than the face.
#[derive(Clone, Debug, PartialEq)]
pub struct Miss {
    /// The sample's index along the seam, `y` outer as [`seam`] reads it.
    pub at: usize,
    /// What the dead end's face presents there.
    pub want: Option<String>,
    /// What the candidate presents there.
    pub found: Option<String>,
}

/// **Every declared face in a kit that no piece of the kit can meet** — a kit stress test.
///
/// Shipped modular kits are proved by building with them: loop a hallway back, cap every opening. A
/// socket nothing in the kit presents back is a dead end, and until this existed it was found by
/// placing pieces and reading [`faults`] one seam at a time. This asks the same question of the
/// whole kit at once, **through [`faults`] itself**: for each face of each piece that declares at
/// least one token, every piece of the library (itself included) is set against it at every quarter
/// turn, aligned to the face's near end, its centre and its far end, on a two-piece map. One path,
/// so a face this calls met is a face the map check accepts.
///
/// A face counts as met when some candidate agrees with it over a stretch carrying at least one of
/// its declared tokens. That is deliberately lenient: a long face may be closed by several pieces,
/// and a check that demanded one piece cover it whole would report kits that work. What it reports
/// is therefore certain: **no piece agrees with any declared part of this face.**
///
/// A face of only unlabelled cells declares nothing and is not tested; a kit with no tokens at all
/// reports nothing. Pieces without a footprint, a height or a lattice are skipped, as [`faults`]
/// skips them. Sorted on `(piece, dir)`.
pub fn dead_ends(library: &Library, divisions: u32) -> Vec<DeadEnd> {
    use crate::map::Placed;

    // A piece usable as either side of a trial seam, with its unturned footprint and divisions.
    type Usable<'a> = (&'a crate::descriptor::Descriptor, (f32, f32), (u32, u32, u32));
    let usable: Vec<Usable> = library
        .descriptors
        .iter()
        .filter_map(|d| {
            let fp = crate::descriptor::placed_footprint(d)?;
            let h = crate::descriptor::placed_height(d)?;
            if h <= 0.0 {
                return None;
            }
            let div = crate::descriptor::divisions(d, divisions).ok()?;
            Some((d, fp, div))
        })
        .collect();

    let mut out = Vec::new();
    for (a, (aw, ad), a_div) in &usable {
        let a_id = a.id.as_str();
        let Some(a_grid) = &a.subgrid else {
            continue;
        };
        for dir in [N, E, S, W] {
            let a_face = face(a_grid, dir, *a_div);
            if a_face.iter().all(Option::is_none) {
                continue;
            }
            let mut met = false;
            // (misses, piece, turn, slide, the misses themselves): the smallest is the closest.
            let mut best: Option<(usize, &str, u8, usize, Vec<Miss>)> = None;
            'trials: for (b, (bw, bd), _) in &usable {
                let b_id = b.id.as_str();
                for q in 0..4u8 {
                    // The candidate's footprint as turned.
                    let (bw, bd) = if q % 2 == 1 { (*bd, *bw) } else { (*bw, *bd) };
                    // Where its centre goes so it touches `a`'s `dir` face, and the lateral span it
                    // slides along: z for an E/W seam, x for N/S.
                    let (normal, half_lat, b_half_lat) = match dir {
                        E => ((aw + bw) * 0.5, ad * 0.5, bd * 0.5),
                        W => (-(aw + bw) * 0.5, ad * 0.5, bd * 0.5),
                        S => ((ad + bd) * 0.5, aw * 0.5, bw * 0.5),
                        _ => (-(ad + bd) * 0.5, aw * 0.5, bw * 0.5),
                    };
                    let near = -half_lat + b_half_lat;
                    let far = half_lat - b_half_lat;
                    for (slide, lat) in [near, 0.0, far].into_iter().enumerate() {
                        let at = if dir == E || dir == W {
                            (normal, lat)
                        } else {
                            (lat, normal)
                        };
                        let map = Map {
                            placements: vec![
                                Placed {
                                    id: "a".to_owned(),
                                    descriptor: a_id.to_owned(),
                                    ..Placed::default()
                                },
                                Placed {
                                    id: "b".to_owned(),
                                    descriptor: b_id.to_owned(),
                                    at,
                                    yaw: f32::from(q) * 90.0,
                                    ..Placed::default()
                                },
                            ],
                            ..Map::default()
                        };
                        let Some((a_side, b_side)) = trial_seam(&map, library, divisions, dir) else {
                            continue;
                        };
                        if !a_side.iter().any(Option::is_some) {
                            continue;
                        }
                        if faults(&map, library, divisions).is_empty() {
                            met = true;
                            break 'trials;
                        }
                        let misses: Vec<Miss> = a_side
                            .iter()
                            .zip(&b_side)
                            .enumerate()
                            .filter(|(_, (w, f))| w != f)
                            .map(|(at, (w, f))| Miss { at, want: w.clone(), found: f.clone() })
                            .collect();
                        let key = (misses.len(), b_id, q, slide);
                        if best.as_ref().is_none_or(|(n, p, t, s, _)| key < (*n, *p, *t, *s)) {
                            best = Some((key.0, b_id, q, slide, misses));
                        }
                    }
                }
            }
            if !met {
                let closest = best.map(|(_, piece, turn, _, misses)| Closest {
                    piece: piece.to_owned(),
                    turn,
                    misses,
                });
                let near = match &closest {
                    Some(c) => format!(
                        "; closest is `{}` turned {}°, off in {} cell{}: {}",
                        c.piece,
                        u32::from(c.turn) * 90,
                        c.misses.len(),
                        if c.misses.len() == 1 { "" } else { "s" },
                        c.misses
                            .iter()
                            .map(|m| format!(
                                "wants {} finds {}",
                                m.want.as_deref().unwrap_or("-"),
                                m.found.as_deref().unwrap_or("-")
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    None => String::new(),
                };
                out.push(DeadEnd {
                    piece: a_id.to_owned(),
                    dir,
                    face: a_face.iter().map(|t| t.map(str::to_owned)).collect(),
                    closest,
                    message: format!(
                        "`{a_id}` face {} is {} and no piece in the kit meets it at any turn{near}",
                        dir_name(dir),
                        say(&a_face)
                    ),
                });
            }
        }
    }
    out.sort_by(|l, r| (&l.piece, l.dir).cmp(&(&r.piece, r.dir)));
    out
}

/// **The smallest closed loop a kit can build** — the other half of the kit stress test.
///
/// [`dead_ends`] answers "can every opening be capped". Shipped modular kits are also proved by
/// looping a hallway back on itself, and a kit whose every face has a partner can still fail that:
/// straights only, or corners whose sockets disagree once turned. This answers it the way an author
/// would, by building: four pieces of one turned footprint around a point (north-west, north-east,
/// south-west, south-east), each at a quarter turn, where **every one of the four inner seams is
/// declared** (the near side presents at least one token) **and [`faults`] finds nothing** on it. A
/// declared inner seam on two adjacent sides of every piece is what makes it a turn, so a ring of
/// straights is not a ring.
///
/// Each pair is tried on a two-piece map through [`faults`] itself, as [`dead_ends`] does, and the
/// returned [`Ring::map`] is the four-piece map, so a caller can check it the same way. `None` means
/// the kit cannot close a loop. Deterministic: pieces in id order, turns ascending, north-west first.
/// Pieces without a lattice, a footprint, a height or divisions are skipped, as [`faults`] skips them.
pub fn ring(library: &Library, divisions: u32) -> Option<Ring> {
    let (cells, (w, d)) = block(library, divisions, 2, 2)?;
    let corners = [("nw", (0.0, 0.0)), ("ne", (w, 0.0)), ("sw", (0.0, d)), ("se", (w, d))];
    let map = Map {
        placements: corners
            .iter()
            .zip(&cells)
            .map(|(&(id, at), (piece, q))| block_placed(id, piece, *q, at))
            .collect(),
        ..Map::default()
    };
    Some(Ring {
        pieces: cells.try_into().ok()?,
        map,
    })
}

/// **The smallest room with a middle a kit can build** — the kit stress test after [`ring`] (#162).
///
/// Shipped modular kits are also proved by opening a hall out into a room: a run two and more pieces
/// wide, with a piece in the middle that meets a neighbour on every side (a pillared hall, an open
/// floor). A corridor kit of straights and corners passes [`dead_ends`] and [`ring`] and still never
/// makes one, so every room an author tries ends in patch-up pieces. This builds it: nine pieces of
/// one turned footprint in three rows of three, each at a quarter turn, where **every one of the
/// twelve inner seams is declared and [`faults`] finds nothing** on it. So the middle piece meets a
/// neighbour on all four sides, each edge piece on three and each corner on two. What the outer faces
/// present is not asked, as [`ring`] does not ask it.
///
/// `None` means the kit cannot open into a room. Deterministic on [`ring`]'s rule: pieces in id
/// order, turns ascending, north-west first, row by row. Pieces without a lattice, a footprint, a
/// height or divisions are skipped, as [`faults`] skips them.
pub fn room(library: &Library, divisions: u32) -> Option<Room> {
    const IDS: [[&str; 3]; 3] = [["nw", "n", "ne"], ["w", "middle", "e"], ["sw", "s", "se"]];
    let (cells, (w, d)) = block(library, divisions, 3, 3)?;
    let steps = [0.0, 1.0, 2.0];
    let mut placements = Vec::with_capacity(9);
    for ((ids, row), z) in IDS.iter().zip(cells.chunks(3)).zip(steps) {
        for ((id, (piece, q)), x) in ids.iter().zip(row).zip(steps) {
            placements.push(block_placed(id, piece, *q, (x * w, z * d)));
        }
    }
    let [nw, n, ne, west, middle, east, sw, s, se]: [(String, u8); 9] = cells.try_into().ok()?;
    Some(Room {
        pieces: [[nw, n, ne], [west, middle, east], [sw, s, se]],
        map: Map {
            placements,
            ..Map::default()
        },
    })
}

/// A placement of `piece` at `q` quarter turns centred on `at` ([`Placed::at`](crate::map::Placed::at)
/// is the centre), for a trial map.
fn block_placed(id: &str, piece: &str, q: u8, at: (f32, f32)) -> crate::map::Placed {
    crate::map::Placed {
        id: id.to_owned(),
        descriptor: piece.to_owned(),
        at,
        yaw: f32::from(q) * 90.0,
        ..crate::map::Placed::default()
    }
}

/// Each cell's `(descriptor id, quarter turns)`, row by row, and the turned footprint they share.
type Block = (Vec<(String, u8)>, (f32, f32));

/// **The first `rows` × `cols` block of one turned footprint whose every inner seam is declared**
/// (the near side presents at least one token) **and passes [`faults`]**: the search [`ring`] and
/// [`room`] share. Each cell's `(descriptor id, quarter turns)`, row by row from the north-west, and
/// the turned footprint every cell shares.
///
/// Each pair is tried on a two-piece map through [`faults`] itself, as [`dead_ends`] does, and
/// remembered. Backtracking, cell by cell in row order, over each cell's candidates in option order:
/// a candidate must meet the cell to its west across their east seam and the cell to its north
/// across their south seam. Two prunings keep a kit of many pieces from trying every arrangement of
/// the first cells before finding that the middle has no piece: each cell's candidates are first cut
/// to those with a partner in every neighbouring cell's candidates (arc consistency, repeated until
/// nothing changes), and a placed piece must leave its east neighbour a candidate. Both drop only
/// options that can stand in no block, so the first block found is the same as without them.
fn block(library: &Library, divisions: u32, rows: usize, cols: usize) -> Option<Block> {
    use std::collections::HashMap;

    // (descriptor id, quarter turns, turned footprint) for every usable piece at every turn.
    let mut opts: Vec<(&str, u8, (f32, f32))> = Vec::new();
    let mut ids: Vec<&crate::descriptor::Descriptor> = library
        .descriptors
        .iter()
        .filter(|d| d.subgrid.is_some())
        .filter(|d| crate::descriptor::divisions(d, divisions).is_ok())
        .filter(|d| crate::descriptor::placed_height(d).is_some_and(|h| h > 0.0))
        .collect();
    ids.sort_by(|l, r| l.id.cmp(&r.id));
    for d in ids {
        let Some((w, dp)) = crate::descriptor::placed_footprint(d) else {
            continue;
        };
        for q in 0..4u8 {
            opts.push((d.id.as_str(), q, if q % 2 == 1 { (dp, w) } else { (w, dp) }));
        }
    }
    let same = |l: (f32, f32), r: (f32, f32)| (l.0 - r.0).abs() < 1e-4 && (l.1 - r.1).abs() < 1e-4;

    // Does option `b`, set against option `a`'s `dir` face (E or S), meet it on a declared seam?
    let mut memo: HashMap<(usize, usize, Dir), bool> = HashMap::new();
    let mut meets = |a: usize, b: usize, dir: Dir| -> bool {
        *memo.entry((a, b, dir)).or_insert_with(|| {
            let ((ap, aq, (w, d)), (bp, bq, _)) = (opts[a], opts[b]);
            let at = if dir == E { (w, 0.0) } else { (0.0, d) };
            let map = Map {
                placements: vec![
                    block_placed("a", ap, aq, (0.0, 0.0)),
                    block_placed("b", bp, bq, at),
                ],
                ..Map::default()
            };
            trial_seam(&map, library, divisions, dir)
                .is_some_and(|(near, _)| near.iter().any(Option::is_some))
                && faults(&map, library, divisions).is_empty()
        })
    };

    // Cut each cell's candidates to those with a partner in every neighbour's, until nothing changes.
    // False when a cell is left with none.
    fn consistent(
        cand: &mut [Vec<usize>],
        cols: usize,
        meets: &mut dyn FnMut(usize, usize, Dir) -> bool,
    ) -> bool {
        loop {
            let mut changed = false;
            for i in 0..cand.len() {
                let east = (i + 1 < cand.len() && !(i + 1).is_multiple_of(cols)).then_some((i + 1, E));
                let south = (i + cols < cand.len()).then_some((i + cols, S));
                for (j, dir) in east.into_iter().chain(south) {
                    let (near, far) = (cand[i].clone(), cand[j].clone());
                    let before = (near.len(), far.len());
                    cand[i].retain(|&x| far.iter().any(|&y| meets(x, y, dir)));
                    cand[j].retain(|&y| near.iter().any(|&x| meets(x, y, dir)));
                    changed |= before != (cand[i].len(), cand[j].len());
                }
            }
            if cand.iter().any(Vec::is_empty) {
                return false;
            }
            if !changed {
                return true;
            }
        }
    }

    // Does `o` at cell `i` meet what is already placed west and north of it?
    fn fits(
        cells: &[usize],
        i: usize,
        o: usize,
        cols: usize,
        meets: &mut dyn FnMut(usize, usize, Dir) -> bool,
    ) -> bool {
        (i.is_multiple_of(cols) || meets(cells[i - 1], o, E))
            && (i < cols || meets(cells[i - cols], o, S))
    }

    // Fill `cells` from its length to one per cell of `cand`; true once full.
    fn fill(
        cells: &mut Vec<usize>,
        cand: &[Vec<usize>],
        cols: usize,
        meets: &mut dyn FnMut(usize, usize, Dir) -> bool,
    ) -> bool {
        let i = cells.len();
        if i == cand.len() {
            return true;
        }
        for &o in &cand[i] {
            if !fits(cells, i, o, cols, meets) {
                continue;
            }
            cells.push(o);
            let next = i + 1;
            let east_open = next == cand.len()
                || next.is_multiple_of(cols)
                || cand[next].iter().any(|&y| fits(cells, next, y, cols, meets));
            if east_open && fill(cells, cand, cols, meets) {
                return true;
            }
            cells.pop();
        }
        false
    }

    // Each footprint's pruned candidates, per cell (`None`: some cell has none), worked out once on
    // first use.
    type Candidates = Option<Vec<Vec<usize>>>;
    let mut pruned: Vec<((f32, f32), Candidates)> = Vec::new();
    for first in 0..opts.len() {
        let fp = opts[first].2;
        let k = match pruned.iter().position(|(f, _)| same(*f, fp)) {
            Some(k) => k,
            None => {
                let fits: Vec<usize> = (0..opts.len()).filter(|&o| same(opts[o].2, fp)).collect();
                let mut cand = vec![fits; rows * cols];
                let ok = consistent(&mut cand, cols, &mut meets);
                pruned.push((fp, ok.then_some(cand)));
                pruned.len() - 1
            }
        };
        let Some(cand) = &pruned[k].1 else {
            continue;
        };
        if !cand[0].contains(&first) {
            continue;
        }
        let mut cells = vec![first];
        if fill(&mut cells, cand, cols, &mut meets) {
            let picked = cells
                .iter()
                .map(|&o| (opts[o].0.to_owned(), opts[o].1))
                .collect();
            return Some((picked, fp));
        }
    }
    None
}

/// A closed loop of four pieces, as [`ring`] found it.
#[derive(Clone, Debug, PartialEq)]
pub struct Ring {
    /// `(descriptor id, quarter turns)` for the north-west, north-east, south-west and south-east
    /// piece, in that order.
    pub pieces: [(String, u8); 4],
    /// The four pieces placed edge to edge around the origin's south-east corner, placement ids
    /// `nw`, `ne`, `sw`, `se`: the map [`faults`] passes.
    pub map: Map,
}

/// A room of nine pieces with a middle, as [`room`] found it.
#[derive(Clone, Debug, PartialEq)]
pub struct Room {
    /// `(descriptor id, quarter turns)` for each piece, rows from north to south, each row from west
    /// to east: `pieces[1][1]` is the middle.
    pub pieces: [[(String, u8); 3]; 3],
    /// The nine pieces placed edge to edge, placement ids `nw`, `n`, `ne`, `w`, `middle`, `e`, `sw`,
    /// `s`, `se`, one footprint apart: the map [`faults`] passes.
    pub map: Map,
}

/// Both sides of a seam, owned.
type OwnedSeam = (Vec<Option<String>>, Vec<Option<String>>);

/// The seam of a two-piece trial map across `a`'s `dir` face: what `a` presents there and what `b`
/// presents back, in [`seam`]'s order. `None` when either piece cannot be read or they share no seam.
fn trial_seam(map: &Map, library: &Library, divisions: u32, dir: Dir) -> Option<OwnedSeam> {
    let (pa, pb) = (map.placements.first()?, map.placements.get(1)?);
    let tile = |p: &crate::map::Placed| -> Option<(Subgrid, Placed3)> {
        let d = library.get(&p.descriptor)?;
        let div = crate::descriptor::divisions(d, divisions).ok()?;
        let (fw, fd) = crate::descriptor::placed_footprint(d)?;
        let q = quarter_turns(&p.id, p.yaw).ok()?;
        let (fw, fd) = if q % 2 == 1 { (fd, fw) } else { (fw, fd) };
        let h = crate::descriptor::placed_height(d)?;
        let grid = d.subgrid.clone().unwrap_or_default().rotated(q, div);
        Some((
            grid,
            Placed3 {
                x: (p.at.0 - fw * 0.5, p.at.0 + fw * 0.5),
                y: (0.0, h),
                z: (p.at.1 - fd * 0.5, p.at.1 + fd * 0.5),
                div: crate::descriptor::rotate_div(div, q),
            },
        ))
    };
    let ((ag, a_at), (bg, b_at)) = (tile(pa)?, tile(pb)?);
    let subunit = crate::grid::SNAP / divisions.max(1) as f32;
    let (a_side, b_side) = seam(&ag, a_at, &bg, b_at, dir, subunit)?;
    let own = |s: Vec<Option<&str>>| s.into_iter().map(|t| t.map(str::to_owned)).collect();
    Some((own(a_side), own(b_side)))
}

/// A face, in words. `-` is an unlabelled cell — which is a token, not an absence.
fn say(face: &[Option<&str>]) -> String {
    if face.is_empty() {
        return "no face".to_owned();
    }
    let parts: Vec<&str> = face.iter().map(|t| t.unwrap_or("-")).collect();
    format!("[{}]", parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::{Descriptor, SubCell};
    use crate::map::{Map, Placed};

    fn grid(cells: &[((u32, u32, u32), &str)]) -> Subgrid {
        Subgrid {
            cells: cells
                .iter()
                .map(|(at, token)| SubCell {
                    at: *at,
                    edge: Some((*token).to_owned()),
                    ..SubCell::default()
                })
                .collect(),
        }
    }

    fn solid_grid(cells: &[(u32, u32, u32)]) -> Subgrid {
        Subgrid {
            cells: cells
                .iter()
                .map(|at| SubCell {
                    at: *at,
                    solid: true,
                    ..SubCell::default()
                })
                .collect(),
        }
    }

    /// **A derived face says what the mesh presents, cell for cell.**
    ///
    /// The wall case, which is the one the site kit is made of: a slab filling the near row of a
    /// 3 x 1 x 3 lattice. Its N face is all mesh and its S face is all air, and a derivation that
    /// could not tell those apart would be proposing a token that means nothing.
    #[test]
    fn a_derived_edge_reads_the_boundary_the_mesh_actually_fills() {
        let div = (3, 1, 3);
        let wall = solid_grid(&[(0, 0, 0), (1, 0, 0), (2, 0, 0)]);
        let derived = derive_edges(&wall, div);

        let token_at = |x, y, z| {
            derived
                .iter()
                .find(|(at, _)| *at == (x, y, z))
                .map(|(_, t)| *t)
        };
        // The N face (z = 0) is the filled row.
        assert_eq!(token_at(0, 0, 0), Some(EDGE_SOLID));
        assert_eq!(token_at(1, 0, 0), Some(EDGE_SOLID));
        assert_eq!(token_at(2, 0, 0), Some(EDGE_SOLID));
        // The S face (z = 2) is air.
        assert_eq!(token_at(0, 0, 2), Some(EDGE_OPEN));
        assert_eq!(token_at(2, 0, 2), Some(EDGE_OPEN));

        // **The middle of the lattice is not a boundary and is not proposed**, so a derivation can
        // sit on top of hand-authored interior cells without touching them.
        assert_eq!(token_at(1, 0, 1), None, "the interior cell must not be proposed");
    }

    /// **Every boundary cell once, in a stable order** — a corner is on two faces and must not be
    /// proposed twice, or applying the result writes one cell then writes it again.
    #[test]
    fn a_derivation_proposes_each_boundary_cell_exactly_once() {
        let div = (3, 2, 3);
        let derived = derive_edges(&solid_grid(&[]), div);

        let cells: Vec<(u32, u32, u32)> = derived.iter().map(|(at, _)| *at).collect();
        let mut sorted = cells.clone();
        sorted.sort_unstable();
        assert_eq!(cells, sorted, "ascending, so two machines agree");
        let mut deduped = sorted.clone();
        deduped.dedup();
        assert_eq!(cells, deduped, "a corner belongs to two faces and appears once");

        // 3x3 per layer is 9 cells, of which 1 is interior — 8 boundary, twice for two layers.
        assert_eq!(cells.len(), 16, "every boundary cell of both layers: {cells:?}");
        // An empty lattice is all air, so every proposal is the open token — never `None`, which is
        // what a hand-authored "not decided yet" looks like.
        assert!(derived.iter().all(|(_, t)| *t == EDGE_OPEN));
    }

    /// A lattice with no cells on an axis has no boundary, and answers rather than panicking — the
    /// same rule [`face`] follows, and for the same reason: this is reached from a measured mesh.
    #[test]
    fn a_degenerate_lattice_has_nothing_to_derive() {
        assert!(derive_edges(&solid_grid(&[]), (0, 1, 1)).is_empty());
        assert!(derive_edges(&solid_grid(&[]), (1, 1, 0)).is_empty());
    }

    /// **The derived tokens round-trip through `face`**, which is the whole point: a derivation the
    /// matcher cannot read is a derivation that decides nothing.
    #[test]
    fn what_is_derived_is_what_the_matcher_reads() {
        let div = (2, 1, 2);
        let mut g = solid_grid(&[(0, 0, 0), (1, 0, 0)]);
        for (at, token) in derive_edges(&g, div) {
            match g.cells.iter_mut().find(|c| c.at == at) {
                Some(c) => c.edge = Some(token.to_owned()),
                None => g.cells.push(SubCell {
                    at,
                    edge: Some(token.to_owned()),
                    ..SubCell::default()
                }),
            }
        }
        assert_eq!(
            face(&g, N, div),
            vec![Some(EDGE_SOLID), Some(EDGE_SOLID)],
            "the filled row reads as solid from the north"
        );
        assert_eq!(
            face(&g, S, div),
            vec![Some(EDGE_OPEN), Some(EDGE_OPEN)],
            "and the empty row reads as open from the south"
        );
    }

    // **The inert property, and it must hold without a branch.** Every shipped descriptor has an
    // empty lattice, so every face is all-`None`, so everything matches everything. If this ever


    // Faces of different lengths cannot agree, rather than agreeing on a prefix.
    //
    // Divisions are derived from size now, so this is the differently-**sized** case: a piece one
    // cell deep beside one three cells deep. Their faces are genuinely different shapes and saying

    /// Four quarter turns put every cell back where it started.
    #[test]
    fn rotating_four_times_is_the_identity() {
        let div = (3, 2, 4);
        let g = grid(&[((0, 0, 0), "a"), ((2, 1, 3), "b"), ((1, 0, 2), "c")]);
        let mut turned = g.clone();
        let mut turned_div = div;
        for _ in 0..4 {
            turned = turned.rotated(1, turned_div);
            turned_div = crate::descriptor::rotate_div(turned_div, 1);
        }
        assert_eq!(turned_div, div);
        let mut want = g.cells.clone();
        let mut got = turned.cells.clone();
        want.sort_by_key(|c| c.at);
        got.sort_by_key(|c| c.at);
        assert_eq!(got, want);
    }

    /// **A quarter turn carries a face round with it.** The +X face becomes the −Z face, because a
    /// positive yaw turns +X toward −Z — the project's one forward convention.
    #[test]
    fn a_quarter_turn_moves_the_east_face_to_north() {
        let div = (2, 1, 3);
        let g = grid(&[((1, 0, 0), "seam"), ((1, 0, 1), "seam"), ((1, 0, 2), "seam")]);
        let east = face(&g, E, div);
        assert_eq!(east, vec![Some("seam"), Some("seam"), Some("seam")]);
        let turned = g.rotated(1, div);
        let turned_div = crate::descriptor::rotate_div(div, 1);
        assert_eq!(turned_div, (3, 1, 2), "a quarter turn swaps the x and z divisions");
        assert_eq!(face(&turned, N, turned_div), east, "the east face is now the north face");
    }

    /// Off-square yaws are refused by name rather than rounded to the nearest face.
    #[test]
    fn a_yaw_that_is_not_a_quarter_turn_is_refused_with_the_angle() {
        assert_eq!(quarter_turns("a", 0.0), Ok(0));
        assert_eq!(quarter_turns("a", 90.0), Ok(1));
        assert_eq!(quarter_turns("a", 270.0), Ok(3));
        assert_eq!(quarter_turns("a", -90.0), Ok(3));
        let e = quarter_turns("chair_a", 15.0).unwrap_err();
        assert!(e.contains("chair_a") && e.contains("15"), "{e}");
    }

    fn map_with(placements: Vec<Placed>) -> Map {
        Map {
            placements,
            ..Map::default()
        }
    }

    /// Every entry is one authoring cell on a side, so at `divisions: 1` its lattice is 1x1x1 and
    /// each face is a single token — the smallest shape that still has faces to compare.
    fn library_with(entries: &[(&str, Subgrid)]) -> Library {
        Library {
            descriptors: entries
                .iter()
                .map(|(id, g)| Descriptor {
                    id: (*id).to_owned(),
                    extent: crate::descriptor::Extent {
                        footprint: Some((crate::grid::SNAP, crate::grid::SNAP)),
                        height: Some(crate::grid::SNAP),
                    },
                    subgrid: Some(g.clone()),
                    ..Descriptor::default()
                })
                .collect(),
            ..Library::default()
        }
    }

    /// The shipped divisions-per-tile, so a test lattice is the size its `extent` says.
    const ONE_PER_TILE: u32 = 1;

    fn placed(id: &str, descriptor: &str, at: (f32, f32), yaw: f32) -> Placed {
        Placed {
            id: id.to_owned(),
            descriptor: descriptor.to_owned(),
            at,
            yaw,
            ..Placed::default()
        }
    }

    /// A library whose entries are `w` x `d` metres on the floor, so a test can place pieces that
    /// are bigger than one cell — which is what the centre-cell pairing could not handle.
    fn sized_library(entries: &[(&str, (f32, f32), Subgrid)]) -> Library {
        Library {
            descriptors: entries
                .iter()
                .map(|(id, fp, g)| Descriptor {
                    id: (*id).to_owned(),
                    extent: crate::descriptor::Extent {
                        footprint: Some(*fp),
                        height: Some(crate::grid::SNAP),
                    },
                    subgrid: Some(g.clone()),
                    ..Descriptor::default()
                })
                .collect(),
            ..Library::default()
        }
    }

    /// **A map nobody has authored tokens for costs one scan of the library.**
    ///
    /// The editor rechecks on every placement, so the quadratic walk was being paid repeatedly for
    /// an answer that is empty by construction until an edge token exists. Pinned by behaviour
    /// rather than by timing: a map far too large to walk quadratically still answers instantly, and
    /// would take visible seconds if the early-out were removed.
    #[test]
    fn an_unauthored_map_short_circuits_however_large_it_is() {
        let lib = sized_library(&[("crate_a", (0.5, 0.5), Subgrid::default())]);
        let many: Vec<Placed> = (0..4000)
            .map(|i| {
                placed(
                    &format!("p{i}"),
                    "crate_a",
                    ((i % 64) as f32 * 0.5, (i / 64) as f32 * 0.5),
                    0.0,
                )
            })
            .collect();
        assert_eq!(
            faults(&map_with(many), &lib, ONE_PER_TILE),
            Vec::new()
        );
    }

    /// And the early-out cannot hide a fault: one authored token anywhere turns the check back on.
    #[test]
    fn a_single_declared_token_re_enables_the_whole_check() {
        let lib = sized_library(&[
            ("wall", (0.5, 0.5), grid(&[((0, 0, 0), "stone")])),
            ("gap", (0.5, 0.5), Subgrid::default()),
        ]);
        let map = map_with(vec![
            placed("g1", "gap", (0.25, 0.25), 0.0),
            placed("w1", "wall", (0.75, 0.25), 0.0),
        ]);
        assert_eq!(faults(&map, &lib, ONE_PER_TILE).len(), 1);
    }

    /// **The defect this pairing was rewritten for.** Two 3 m walls side by side share a seam, but
    /// their centres are six cells apart — so pairing by the cell a placement's `at` fell in never
    /// compared them, and a wall seam is the thing edge tokens exist to check.
    #[test]
    fn two_wide_pieces_side_by_side_are_compared() {
        let lib = sized_library(&[
            // The seam under test is the wall's EAST face, which is its last x cell.
            ("wall", (3.0, 0.5), grid(&[((5, 0, 0), "stone")])),
            ("gap", (3.0, 0.5), Subgrid::default()),
        ]);
        // Centres 3 m apart on a 0.5 m grid: touching, six cells between the centres.
        let map = map_with(vec![
            placed("w1", "wall", (1.5, 0.25), 0.0),
            placed("g1", "gap", (4.5, 0.25), 0.0),
        ]);
        let found = faults(&map, &lib, ONE_PER_TILE);
        assert_eq!(found.len(), 1, "the seam between two touching walls: {found:#?}");
        assert_eq!((found[0].a.as_str(), found[0].b.as_str()), ("g1", "w1"));
    }

    /// And the other half: pieces whose centre cells are neighbours but which do not touch are no
    /// longer compared. A gap of a whole cell is a gap.
    #[test]
    fn pieces_with_a_gap_between_them_are_not_compared() {
        let lib = sized_library(&[
            ("wall", (0.5, 0.5), grid(&[((0, 0, 0), "stone")])),
            ("gap", (0.5, 0.5), Subgrid::default()),
        ]);
        let map = map_with(vec![
            placed("w1", "wall", (0.25, 0.25), 0.0),
            // One empty cell between them.
            placed("g1", "gap", (1.25, 0.25), 0.0),
        ]);
        assert_eq!(faults(&map, &lib, ONE_PER_TILE), Vec::new());
    }

    /// **The answer cannot depend on how big the map is.**
    ///
    /// Neighbours were decided on an integer cell grid anchored at `floor_rect`'s `min_x` — that is
    /// `-bounds/2`, a multiple of the cell size only when the map's width happens to be an even
    /// multiple of it. At 33 m every span shifted half a cell, `ax1 == bx0` was never true, and **no
    /// seam in the map paired at all** — reported to the author as `ok`. Two identical maps that
    /// differ only in `bounds` must give the identical answer, which is what makes it a property of
    /// the pieces rather than of the paperwork around them.
    #[test]
    fn an_odd_map_bound_does_not_unpair_every_seam() {
        let lib = library_with(&[
            ("wall", grid(&[((0, 0, 0), "stone")])),
            ("gap", Subgrid::default()),
        ]);
        let touching = vec![
            placed("w1", "wall", (0.5, 0.5), 0.0),
            placed("g1", "gap", (1.0, 0.5), 0.0),
        ];
        // Every odd metre, and a fractional one: none of these is a whole number of 0.5 m cells away
        // from its own half-width, which is the shape of the bug.
        for width in [33.0, 7.0, 1.0, 12.5] {
            let map = Map {
                bounds: (width, 4.0, width),
                placements: touching.clone(),
                ..Map::default()
            };
            let found = faults(&map, &lib, ONE_PER_TILE);
            assert_eq!(
                found.len(),
                1,
                "a {width} m map must find the same one seam: {found:#?}"
            );
        }
    }

    /// **A face is sampled at the lattice's own rate, not the caller's.**
    ///
    /// `descriptor::divisions` is `grid::cells(span) * divisions` and `grid::cells` counts
    /// [`crate::grid::SNAP`] units, so one cell is `SNAP / divisions` metres. [`seam`] used to be
    /// handed `cell / divisions` from the same parameter that set the neighbour grid — 1.0 m from the
    /// editor — so a 2.40 m wall's five rows were read at **two** places and the three rows between
    /// them were never compared. This puts the disagreement on one of the rows that sampling missed.
    #[test]
    fn every_row_of_a_face_is_read_exactly_once() {
        let rows = |odd: &str| {
            let mut cells: Vec<_> = (0..5)
                .map(|y| SubCell {
                    at: (0, y, 0),
                    edge: Some("wall".to_owned()),
                    ..SubCell::default()
                })
                .collect();
            // Row 2. At the old 1.0 m sampling rate a 2.40 m face was read at y = 0.6 and y = 1.8,
            // which land in rows 1 and 3 — so this row, and rows 0 and 4, went unread.
            cells[2].edge = Some(odd.to_owned());
            Subgrid { cells }
        };
        let wall = |id: &str, g: Subgrid| Descriptor {
            id: id.to_owned(),
            extent: crate::descriptor::Extent {
                footprint: Some((crate::grid::SNAP, crate::grid::SNAP)),
                height: Some(2.4),
            },
            subgrid: Some(g),
            ..Descriptor::default()
        };
        let lib = Library {
            descriptors: vec![wall("plain", rows("wall")), wall("odd", rows("glass"))],
            ..Library::default()
        };
        let map = map_with(vec![
            placed("a", "plain", (0.5, 0.5), 0.0),
            placed("b", "odd", (1.0, 0.5), 0.0),
        ]);
        let found = faults(&map, &lib, ONE_PER_TILE);
        assert_eq!(found.len(), 1, "row 2 disagrees: {found:#?}");
        assert_eq!(found[0].a_face.len(), 5, "five rows, five samples: {found:#?}");
        assert!(found[0].message.contains("glass"), "{}", found[0].message);
    }

    /// **A corner is not a seam.** Two pieces meeting only at a point present no column of cells to
    /// each other, so there is nothing to compare and nothing to report.
    #[test]
    fn pieces_touching_only_at_a_corner_are_not_compared() {
        let lib = sized_library(&[
            ("wall", (0.5, 0.5), grid(&[((0, 0, 0), "stone")])),
            ("gap", (0.5, 0.5), Subgrid::default()),
        ]);
        let map = map_with(vec![
            placed("w1", "wall", (0.25, 0.25), 0.0),
            placed("g1", "gap", (0.75, 0.75), 0.0),
        ]);
        assert_eq!(faults(&map, &lib, ONE_PER_TILE), Vec::new());
    }

    /// A turned piece abuts on its turned footprint — a 3 m wall at 90 degrees runs along Z, and the
    /// neighbour it touches is the one beside it *then*, not the one beside it before the turn.
    #[test]
    fn a_turned_piece_abuts_on_the_footprint_it_actually_has() {
        let lib = sized_library(&[
            ("wall", (3.0, 0.5), grid(&[((0, 0, 0), "stone")])),
            ("gap", (0.5, 0.5), Subgrid::default()),
        ]);
        // Turned a quarter, the wall is 0.5 wide and 3 deep, running from z = 0 to z = 3. The token was
        // authored at x = 0 and `rotated` carries `(x, y, z)` to `(z, y, dx - 1 - x)`, so it lands at
        // z = 5 of 6 — the LAST half-metre of the run. That is where a neighbour has to be to meet it.
        let map = map_with(vec![
            placed("w1", "wall", (0.25, 1.5), 90.0),
            // Beside its long side. Reachable at all only if the turn was accounted for in the
            // pairing, and a fault only if the token moved with it.
            placed("g1", "gap", (0.75, 2.75), 0.0),
        ]);
        let found = faults(&map, &lib, ONE_PER_TILE);
        assert_eq!(found.len(), 1, "the turned wall's long face: {found:#?}");

        // **The same neighbour at the other end of the run is not a fault**, because the token is not
        // there. Under whole-face equality it was one — six cells against one, refused on length
        // alone — and that is exactly the difference `seam` makes: it asks what is *at* the seam, not
        // how big the two faces are.
        let elsewhere = map_with(vec![
            placed("w1", "wall", (0.25, 1.5), 90.0),
            placed("g1", "gap", (0.75, 0.25), 0.0),
        ]);
        assert_eq!(faults(&elsewhere, &lib, ONE_PER_TILE), Vec::new());
    }

    /// A map of unauthored tiles reports nothing — the inert property, end to end.
    #[test]
    fn a_map_of_unauthored_tiles_has_no_faults() {
        let lib = library_with(&[("crate_a", Subgrid::default())]);
        let map = map_with(vec![
            placed("one", "crate_a", (0.5, 0.5), 0.0),
            placed("two", "crate_a", (1.0, 0.5), 0.0),
        ]);
        assert_eq!(faults(&map, &lib, ONE_PER_TILE), Vec::new());
    }

    /// One mismatched pair is reported once per direction it is wrong in — here `one`'s E face and
    /// `two`'s W face, which is one ordered pair each way.
    #[test]
    fn a_mismatched_pair_is_reported_and_names_both_faces() {
        let lib = library_with(&[
            ("wall", grid(&[((0, 0, 0), "stone")])),
            ("gap", Subgrid::default()),
        ]);
        let map = map_with(vec![
            placed("w1", "wall", (0.5, 0.5), 0.0),
            placed("g1", "gap", (1.0, 0.5), 0.0),
        ]);
        let found = faults(&map, &lib, ONE_PER_TILE);
        assert_eq!(found.len(), 1, "one seam is one fault, not two: {found:#?}");
        assert!(found[0].message.contains("stone"), "{}", found[0].message);
        // `g1` sorts before `w1`, so that is the ordering reported — and the sentence still names
        // both pieces and both faces, so nothing is lost by picking one.
        assert_eq!((found[0].a.as_str(), found[0].b.as_str()), ("g1", "w1"));
    }

    /// **Order is stable and does not depend on placement order.** The determinism rule: the key is
    /// total, so shuffling the input cannot reorder the output.
    #[test]
    fn fault_order_does_not_depend_on_placement_order() {
        let lib = library_with(&[
            ("wall", grid(&[((0, 0, 0), "stone")])),
            ("gap", Subgrid::default()),
        ]);
        let forward = faults(
            &map_with(vec![
                placed("aaa", "wall", (0.5, 0.5), 0.0),
                placed("bbb", "gap", (1.0, 0.5), 0.0),
            ]),
            &lib,
            ONE_PER_TILE,
        );
        let backward = faults(
            &map_with(vec![
                placed("bbb", "gap", (1.0, 0.5), 0.0),
                placed("aaa", "wall", (0.5, 0.5), 0.0),
            ]),
            &lib,
            ONE_PER_TILE,
        );
        assert_eq!(forward, backward);
    }

    /// An off-square placement becomes a fault that names it, and does not stop the rest of the map
    /// being checked. Reported once, not once per neighbour.
    #[test]
    fn an_off_square_yaw_is_a_fault_not_a_dead_check() {
        let lib = library_with(&[("wall", grid(&[((0, 0, 0), "stone")]))]);
        let map = map_with(vec![
            placed("tilted", "wall", (0.5, 0.5), 15.0),
            placed("square", "wall", (1.0, 0.5), 0.0),
        ]);
        let found = faults(&map, &lib, ONE_PER_TILE);
        assert_eq!(
            found.iter().filter(|f| f.a == "tilted" && f.b.is_empty()).count(),
            1,
            "one sentence per tilted piece, not one per neighbour: {found:#?}"
        );
        assert!(found.iter().any(|f| f.message.contains("tilted")));
    }

    /// **A tile that declares nothing is not asked to square its yaw.** `break_room` ships an armchair
    /// at yaw 240 and no edge tokens anywhere; before this rule the editor opened reporting a fault
    /// about a feature nobody had used yet.
    #[test]
    fn an_undeclared_tile_at_an_odd_yaw_is_not_a_fault() {
        let lib = library_with(&[("armchair", Subgrid::default()), ("table", Subgrid::default())]);
        let map = map_with(vec![
            placed("chair", "armchair", (0.5, 0.5), 240.0),
            placed("table", "table", (1.0, 0.5), 0.0),
        ]);
        assert_eq!(faults(&map, &lib, ONE_PER_TILE), Vec::new());
    }

    /// But once its neighbour declares one, the odd yaw is exactly what stops the check, and it says
    /// so.
    #[test]
    fn an_odd_yaw_matters_as_soon_as_a_neighbour_declares_an_edge() {
        let lib = library_with(&[
            ("armchair", Subgrid::default()),
            ("wall", grid(&[((0, 0, 0), "stone")])),
        ]);
        let map = map_with(vec![
            placed("chair", "armchair", (0.5, 0.5), 240.0),
            placed("w1", "wall", (1.0, 0.5), 0.0),
        ]);
        let found = faults(&map, &lib, ONE_PER_TILE);
        assert!(
            found.iter().any(|f| f.a == "chair" && f.message.contains("240")),
            "{found:#?}"
        );
    }
}

/// **The seam rule, and the two cases in the shipped kits that decided it.**
#[cfg(test)]
mod seam_tests {
    use super::*;
    use crate::descriptor::{Descriptor, Extent, SubCell};
    use crate::map::Placed;

    /// `wall` on every cell of a lattice, which is how the kits are authored.
    fn all_wall(div: (u32, u32, u32)) -> Subgrid {
        let (dx, dy, dz) = div;
        Subgrid {
            cells: (0..dx)
                .flat_map(|x| (0..dy).flat_map(move |y| (0..dz).map(move |z| (x, y, z))))
                .map(|at| SubCell {
                    at,
                    edge: Some("wall".into()),
                    ..SubCell::default()
                })
                .collect(),
        }
    }

    fn piece(id: &str, w: f32, h: f32, d: f32, div: (u32, u32, u32)) -> Descriptor {
        Descriptor {
            id: id.into(),
            extent: Extent {
                footprint: Some((w, d)),
                height: Some(h),
            },
            mount: Some(crate::descriptor::Mount::OnFloor),
            subgrid: Some(all_wall(div)),
            ..Descriptor::default()
        }
    }

    fn map_of(ps: Vec<Placed>) -> Map {
        Map {
            name: "t".into(),
            placements: ps,
            ..Map::default()
        }
    }

    fn at(id: &str, d: &str, x: f32, z: f32) -> Placed {
        Placed {
            id: id.into(),
            descriptor: d.into(),
            at: (x, z),
            yaw: 0.0,
            ..Placed::default()
        }
    }

    /// A wall's run-face against a doorway's, the shipped shapes: 2.40 m against 2.01 m, so five rows
    /// against four. **Whole-face equality refused this**; the overlap agrees, because every cell on
    /// the part they share says `wall` on both sides.
    #[test]
    fn a_wall_and_a_shorter_doorway_agree_over_the_rows_they_share() {
        let lib = Library {
            descriptors: vec![
                piece("wall", 0.1, 2.40, 1.0, (1, 5, 2)),
                piece("door", 0.1, 2.01, 1.0, (1, 4, 2)),
            ],
            ..Library::default()
        };
        let map = map_of(vec![at("w1", "wall", 0.25, 0.5), at("d1", "door", 0.25, 1.5)]);
        let found = faults(&map, &lib, 1);
        assert!(found.is_empty(), "{found:#?}");
    }

    /// And a header, which only exists between 2.00 m and 2.40 m, agrees with the wall over exactly
    /// that band. **This pair was never compared at all** under the old pairing: a header sitting above
    /// a doorway shares its `(x, z)` cells, so `side` saw overlap rather than abutment. Here it is
    /// beside the wall, which is the seam it really has.
    #[test]
    fn a_header_agrees_with_the_wall_over_the_band_it_occupies() {
        let mut header = piece("header", 0.1, 0.40, 1.0, (1, 1, 2));
        // A header hangs at wall height rather than standing on the floor.
        header.mount = Some(crate::descriptor::Mount::OnWall { height: 2.0 });
        let lib = Library {
            descriptors: vec![piece("wall", 0.1, 2.40, 1.0, (1, 5, 2)), header],
            ..Library::default()
        };
        let map = map_of(vec![at("w1", "wall", 0.25, 0.5), at("h1", "header", 0.25, 1.5)]);
        let found = faults(&map, &lib, 1);
        assert!(found.is_empty(), "{found:#?}");
    }

    /// **A disagreement inside the overlap is still a fault.** The whole point is to compare *less*,
    /// not to compare nothing — a permissive rule that reported nothing would be worse than the strict
    /// one it replaced.
    #[test]
    fn a_different_token_on_the_shared_rows_is_still_reported() {
        let mut door = piece("door", 0.1, 2.01, 1.0, (1, 4, 2));
        door.subgrid = Some(Subgrid {
            cells: (0..4)
                .flat_map(|y| [(0, y, 0), (0, y, 1)])
                .map(|at| SubCell {
                    at,
                    edge: Some("glass".into()),
                    ..SubCell::default()
                })
                .collect(),
        });
        let lib = Library {
            descriptors: vec![piece("wall", 0.1, 2.40, 1.0, (1, 5, 2)), door],
            ..Library::default()
        };
        let map = map_of(vec![at("w1", "wall", 0.25, 0.5), at("d1", "door", 0.25, 1.5)]);
        let found = faults(&map, &lib, 1);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert!(found[0].message.contains("glass"), "{}", found[0].message);
    }

    /// The greybox case: two pieces of one family at different **widths**, meeting along a run. Not an
    /// architectural stack — just a corner that is two cells wide against a wall that is one — and the
    /// overlap is the metre they actually share.
    #[test]
    fn a_wider_corner_agrees_with_its_wall_over_the_width_they_share() {
        let lib = Library {
            descriptors: vec![
                piece("wall", 0.5, 2.40, 1.0, (1, 5, 2)),
                piece("corner", 1.0, 2.40, 1.0, (2, 5, 2)),
            ],
            ..Library::default()
        };
        // Wall spans x [0, 0.5]; corner spans x [0, 1.0]. They meet on z, overlapping on the wall's
        // half-metre of width.
        let map = map_of(vec![at("w1", "wall", 0.25, 0.5), at("c1", "corner", 0.5, 1.5)]);
        let found = faults(&map, &lib, 1);
        assert!(found.is_empty(), "{found:#?}");
    }

    /// **No vertical overlap is not a seam.** A piece floating entirely above another shares a cell
    /// footprint and no surface, so there is nothing to be right or wrong about — and reporting one
    /// would be inventing a relationship the geometry does not have.
    #[test]
    fn pieces_that_do_not_overlap_in_height_are_not_a_seam() {
        let mut high = piece("high", 0.1, 0.40, 1.0, (1, 1, 2));
        high.mount = Some(crate::descriptor::Mount::OnWall { height: 3.0 });
        high.subgrid = Some(Subgrid {
            cells: vec![SubCell {
                at: (0, 0, 0),
                edge: Some("glass".into()),
                ..SubCell::default()
            }],
        });
        let lib = Library {
            descriptors: vec![piece("wall", 0.1, 2.40, 1.0, (1, 5, 2)), high],
            ..Library::default()
        };
        // Beside the wall on the floor plan, but 3.0 m up where the wall stops at 2.40.
        let map = map_of(vec![at("w1", "wall", 0.25, 0.5), at("h1", "high", 0.25, 1.5)]);
        assert_eq!(faults(&map, &lib, 1), Vec::new());
    }
}
