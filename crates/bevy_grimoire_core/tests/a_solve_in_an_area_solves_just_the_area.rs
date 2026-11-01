//! **A solve in an area solves the area, and the ring around it only narrows its edge** (#261, the
//! findings #192's first slice left).
//!
//! Slice 1 collapsed the area *and* the ring of cells around it, with the ring pinned to what stands
//! there. That went wrong two ways: two ring neighbours that the grammar does not allow side by side
//! refused an area that could have been filled, and an enclosure wish counted the ring's cells as
//! part of the region, so a 3 x 3 area could never reach more than 9 of 25 cells, 0.36.

use bevy_grimoire_core::composition::{Body, Composition, Envelope, Member};
use bevy_grimoire_core::descriptor::{Descriptor, Extent, SubCell, Subgrid};
use bevy_grimoire_core::grammar::{self, Area, Composed, Grammar, Prototype, Wishes};
use bevy_grimoire_core::library::Library;
use bevy_grimoire_core::map::{Map, Placed};

fn ids() -> impl FnMut() -> String {
    let mut n = 0;
    move || {
        n += 1;
        format!("gen@{n}")
    }
}

// ── two ring neighbours that disagree ────────────────────────────────────────────────────────────

const EMPTY: usize = 0;
const A: usize = 1;
const C: usize = 2;

/// **A three-prototype grammar written out**, so the rule that matters is the one on the page:
/// `a` may stand beside empty and `c` but never beside another `a`; `c` may stand beside anything.
fn grammar_where_a_never_meets_a() -> Grammar {
    let piece = |descriptor: &str| Prototype::Piece {
        descriptor: descriptor.to_owned(),
        yaw: 0.0,
    };
    let beside: [u32; 3] = [
        // empty: empty, a, c
        1 << EMPTY | 1 << A | 1 << C,
        // a: empty, c
        1 << EMPTY | 1 << C,
        // c: empty, a, c
        1 << EMPTY | 1 << A | 1 << C,
    ];
    Grammar {
        prototypes: vec![Prototype::Empty, piece("a"), piece("c")],
        weights: vec![1.0, 1.0, 1.0],
        support: [beside.to_vec(), beside.to_vec(), beside.to_vec(), beside.to_vec()],
    }
}

/// The centre of cell `(x, z)` on a 6 m map whose floor runs from −3 to 3.
fn centre(x: usize, z: usize) -> (f32, f32) {
    (x as f32 - 2.5, z as f32 - 2.5)
}

/// A 6 x 6 map with an `a` at each of the cells named.
fn map_with_a_at(cells: &[(usize, usize)]) -> Map {
    Map {
        name: "ring".into(),
        bounds: (6.0, 3.0, 6.0),
        placements: cells
            .iter()
            .map(|&(x, z)| Placed {
                id: format!("a{x}_{z}"),
                descriptor: "a".to_owned(),
                at: centre(x, z),
                ..Placed::default()
            })
            .collect(),
        ..Map::default()
    }
}

/// **The two `a`s above the area cannot stand together, and the area can still be filled.** The old
/// solve held the pair in its grid, found the conflict between them and refused the area. A solve of
/// just the area asks each edge cell only what may stand below its own neighbour.
#[test]
fn two_ring_neighbours_that_disagree_do_not_refuse_an_area_that_can_be_filled() {
    let g = grammar_where_a_never_meets_a();
    let map = map_with_a_at(&[(2, 1), (3, 1)]);
    // Cells (2, 2) and (3, 2), the two directly below the pair.
    let area = Area { from: centre(2, 2), to: centre(3, 2) };

    let solved = grammar::solve_in(&map, &g, 1.0, area, 4, ids())
        .unwrap_or_else(|e| panic!("an area under two clashing neighbours must still solve: {e}"));

    assert_eq!(solved.free_cells, 2, "both cells of the area are laid");
    for p in &solved.placements {
        assert_eq!(
            p.descriptor, "c",
            "`{}` at {:?}: only `c` may stand below an `a`, so an `a` there joins nothing",
            p.id, p.at
        );
    }
    assert_eq!(
        (solved.width, solved.height),
        (4, 3),
        "the grid still reads as the area and its ring: columns 1 to 4, rows 1 to 3"
    );
}

/// The ring still decides what the edge may be: an area whose only way out is the piece the ring
/// forbids is refused, rather than laid against it.
#[test]
fn the_ring_still_narrows_the_edge_it_touches() {
    let g = grammar_where_a_never_meets_a();
    // A one-cell area directly below an `a` and directly above another, holding an owned `a`: it joins
    // neither neighbour.
    let mut map = map_with_a_at(&[(2, 1), (2, 3)]);
    map.placements.push(Placed {
        id: "owned".into(),
        descriptor: "a".to_owned(),
        at: centre(2, 2),
        owned: true,
        ..Placed::default()
    });
    let area = Area { from: centre(2, 2), to: centre(2, 2) };
    let err = grammar::solve_in(&map, &g, 1.0, area, 1, ids())
        .expect_err("an owned `a` between two `a`s joins neither");
    assert!(err.contains("no arrangement"), "the refusal says what happened: {err}");
}

// ── an enclosure wish over the area alone ────────────────────────────────────────────────────────

/// A kit of a floor, a corner, a wall and a doorway, each a one-metre tile.
fn kit() -> Composed {
    let plate = |id: &str| Descriptor {
        id: id.to_owned(),
        extent: Extent { footprint: Some((1.0, 1.0)), height: Some(0.1) },
        ..Default::default()
    };
    let solid = |mut d: Descriptor| {
        let div = bevy_grimoire_core::descriptor::divisions(&d, 1).expect("measured");
        let mut cells = Vec::new();
        for x in 0..div.0 {
            for y in 0..div.1 {
                for z in 0..div.2 {
                    cells.push(SubCell {
                        at: (x, y, z),
                        solid: true,
                        edge: Some("wall".to_owned()),
                    });
                }
            }
        }
        d.subgrid = Some(Subgrid { cells });
        d
    };
    let wall = solid(Descriptor {
        id: "wall".to_owned(),
        extent: Extent { footprint: Some((0.1, 1.0)), height: Some(2.4) },
        ..Default::default()
    });
    let header = solid(Descriptor {
        id: "header".to_owned(),
        extent: Extent { footprint: Some((0.1, 1.0)), height: Some(0.4) },
        ..Default::default()
    });
    let library = Library {
        version: bevy_grimoire_core::library::LIBRARY_VERSION,
        note: None,
        descriptors: vec![plate("floor"), wall, header],
    };
    let member = |id: &str, at: (f32, f32)| Member {
        id: id.to_owned(),
        body: Body::Descriptor { id: id.to_owned(), tip: (0, 0), on: None, patch: None },
        at,
        yaw: 0.0,
        lift: 0.0,
        paint: 0,
        of_fingerprint: None,
        note: None,
    };
    let tile = |id: &str, members: Vec<Member>| Composition {
        id: id.to_owned(),
        envelope: Envelope::Bounded { size: (1.0, 2.4, 1.0) },
        members,
        locations: Vec::new(),
        note: None,
    };
    let north = || Member { at: (0.0, -0.45), yaw: 90.0, ..member("wall", (0.0, -0.45)) };
    let west = || Member { id: "wall_2".to_owned(), at: (-0.45, 0.0), ..member("wall", (-0.45, 0.0)) };
    let lintel = || Member {
        id: "header".to_owned(),
        at: (0.0, -0.45),
        yaw: 90.0,
        lift: 2.0,
        ..member("header", (0.0, -0.45))
    };
    let comps = vec![
        tile("site/tile_corner_nw", vec![member("floor", (0.0, 0.0)), north(), west()]),
        tile("site/tile_doorway_n", vec![member("floor", (0.0, 0.0)), lintel()]),
        tile("site/tile_floor", vec![member("floor", (0.0, 0.0))]),
        tile("site/tile_wall_n", vec![member("floor", (0.0, 0.0)), north()]),
    ];
    grammar::from_compositions(&comps, &library, 1, 1.0, bevy_grimoire_core::composition::agrees)
        .expect("the fixture kit learns")
}

/// **A 3 x 3 area meets an enclosure wish above 0.36 when the kit allows it.** Nine of the 25 cells
/// the old window held is 0.36; the area is nine cells, and a room of four corners, four walls and a
/// floor encloses all of them.
#[test]
fn a_three_by_three_area_meets_an_enclosure_wish_above_the_old_ceiling() {
    let Composed { grammar: g, faces, .. } = kit();
    let map = Map {
        name: "open".into(),
        bounds: (13.0, 3.0, 13.0),
        ..Map::default()
    };
    // Nine cells in the middle of a 13 m floor from −6.5 to 6.5, whose cell centres are whole metres.
    let area = Area { from: (-1.0, -1.0), to: (1.0, 1.0) };
    let want = 0.6;
    let wishes = Wishes { enclosure: want, enclosure_weight: None };

    let solved = grammar::solve_constrained_in(&map, &g, &faces, &wishes, 1.0, area, 7, ids())
        .unwrap_or_else(|e| panic!("a hard wish of {want} over nine cells is reachable: {e}"));

    assert_eq!(solved.free_cells, 9);
    assert_eq!(solved.unmet, 0, "a hard wish is met or refused");
    assert_eq!(solved.stamps.len(), 9, "nine tiles are laid, and no more");
    assert!(solved.placements.is_empty());
}
