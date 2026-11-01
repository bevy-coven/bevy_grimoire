//! **A two-way piece is seated by every door, not only by a click** — issue #1's review gaps.
//!
//! # Why this file exists
//!
//! `Mount::OnSurfaceOrFloor` decides its layer per placement. The click path asked
//! `stack::placement_at` and got that right; two other doors decided a host their own way and did
//! not. Both were found by an independent review of the diff, and both break the issue's second
//! acceptance criterion on a path outside it:
//!
//! 1. **`build::host_for` read `editor::mount_class`** — the *strict* mount. It returned `None` for a
//!    two-way piece, so a member dropped against a bench in the Build tab saved `on: None`, and
//!    `rebind_hosts` reset a hand-authored `on: Some(sibling)` on the next edit in that tab. Stamped,
//!    the piece landed at floor height **inside** the bench.
//! 2. **`stamp_set` refused a paste when `host_under` found nothing.** A strict `OnSurface` piece
//!    should be refused there — but a two-way piece is legal on open floor, and a click at the same
//!    spot accepts it. A paste and a click disagreeing about one piece at one spot is two answers to
//!    one question.
//!
//! These are behavioural: `host_for` is a plain function, and the paste is `stamp_set`'s own branch,
//! so neither needs a window.

use bevy_grimoire_core::composition::{Body, Member};
use bevy_grimoire_core::descriptor::{Descriptor, Extent, Mount, Offers};
use bevy_grimoire_core::library::{LIBRARY_VERSION, Library};
use bevy_grimoire_core::stack::{
    host_under, needs_surface, placement_at, resolve_y,
};
use bevy_cauldron::build::host_for;
use bevy_grimoire_core::map::{Map, Placed};

/// The bench, as the Build tab sees it: floor-standing, offering a `support` top 0.9 m up.
fn bench() -> Descriptor {
    Descriptor {
        id: "bench".into(),
        extent: Extent {
            footprint: Some((1.8, 0.9)),
            height: Some(0.9),
        },
        offers: Offers {
            surfaces: vec!["support".into()],
            faces: Vec::new(),
            sockets: Vec::new(),
        },
        mount: Some(Mount::OnFloor),
        ..Descriptor::default()
    }
}

/// The dressing set: on a `support` top, or on the floor.
fn tools() -> Descriptor {
    Descriptor {
        id: "tools".into(),
        extent: Extent {
            footprint: Some((0.4, 0.3)),
            height: Some(0.15),
        },
        mount: Some(Mount::OnSurfaceOrFloor {
            class: "support".into(),
        }),
        ..Descriptor::default()
    }
}

/// A lamp that may **only** rest on a surface — the control, so the fix did not loosen it.
fn lamp() -> Descriptor {
    Descriptor {
        id: "lamp".into(),
        extent: Extent {
            footprint: Some((0.3, 0.3)),
            height: Some(0.5),
        },
        mount: Some(Mount::OnSurface {
            class: "support".into(),
        }),
        ..Descriptor::default()
    }
}

fn library() -> Library {
    Library {
        version: LIBRARY_VERSION,
        note: None,
        descriptors: vec![bench(), tools(), lamp()],
    }
}

fn member(id: &str, at: (f32, f32)) -> Member {
    Member {
        id: id.into(),
        body: Body::Descriptor {
            id: "bench".into(),
            tip: (0, 0),
            on: None,
            patch: None,
        },
        at,
        yaw: 0.0,
        lift: 0.0,
        paint: 0,
        of_fingerprint: None,
        note: None,
    }
}

fn guest(id: &str, descriptor: &str, at: (f32, f32)) -> Member {
    Member {
        id: id.into(),
        body: Body::Descriptor {
            id: descriptor.into(),
            tip: (0, 0),
            on: None,
            patch: None,
        },
        at,
        yaw: 0.0,
        lift: 0.0,
        paint: 0,
        of_fingerprint: None,
        note: None,
    }
}

// ── Gap 1: the Build tab ──────────────────────────────────────────────────────────────────────────

/// **Dropped against a bench in the Build tab, the piece records the bench as its host.**
///
/// Before the fix this was `Ok(None)`: `host_for` asked `mount_class`, which is the strict question,
/// so a two-way piece was treated as one that needs no host at all.
#[test]
fn the_build_tab_seats_a_two_way_piece_on_the_sibling_it_touches() {
    let l = library();
    let members = vec![member("b1", (0.0, 0.0))];
    let found = host_for(&members, &l, &tools(), None, (0.0, 0.0), (0.4, 0.3))
        .unwrap_or_else(|e| panic!("a two-way piece must not be refused here: {e}"));
    assert_eq!(
        found.as_deref(),
        Some("b1"),
        "the bench is touching and offers `support`, so it is the host"
    );
}

/// **And dropped where nothing offers the class, it is left to stand on the floor** — not refused.
/// The click path already answers this way; the Build tab has to agree.
#[test]
fn the_build_tab_leaves_a_two_way_piece_on_the_floor() {
    let l = library();
    let found = host_for(&[], &l, &tools(), None, (0.0, 0.0), (0.4, 0.3))
        .unwrap_or_else(|e| panic!("a two-way piece on open floor must not be refused: {e}"));
    assert_eq!(found, None, "nothing is under it, so it has no host");
}

/// **The strict piece is still refused** by the Build tab when nothing offers its class. The fix
/// widened the mount that gets seated; it must not widen the one that needs a seat.
#[test]
fn the_build_tab_still_refuses_a_surface_only_piece_with_no_host() {
    let l = library();
    let err = host_for(&[], &l, &lamp(), None, (0.0, 0.0), (0.3, 0.3))
        .err()
        .unwrap_or_else(|| panic!("a surface-only piece with no host must be refused"));
    assert!(err.contains("support"), "the refusal names the class: {err}");
}

/// **`rebind_hosts` keeps a hand-authored seating.** A composition captured with `on: Some("b1")`
/// must not be reset to `None` by the next edit in the Build tab — which is what the old
/// `host_for` did, silently, to every two-way member.
#[test]
fn a_recorded_sibling_seating_survives_a_rebind() {
    let l = library();
    let mut comp = bevy_grimoire_core::composition::Composition {
        members: vec![member("b1", (0.0, 0.0)), guest("t1", "tools", (0.0, 0.0))],
        ..Default::default()
    };
    bevy_cauldron::build::rebind_hosts(&mut comp, &l)
        .unwrap_or_else(|e| panic!("{e}"));
    let Body::Descriptor { on, .. } = &comp.members[1].body else {
        panic!("the guest is a descriptor member");
    };
    assert_eq!(
        on.as_deref(),
        Some("b1"),
        "the seating a person authored must survive a rebind"
    );
}

// ── Gap 2: the paste ──────────────────────────────────────────────────────────────────────────────

/// **A paste on open floor takes the floor.** This is the question `stamp_set` asks, in the one
/// function both callers share — the strict refusal stays, the permissive answer does not.
#[test]
fn a_two_way_piece_placed_on_open_floor_resolves_to_the_floor() {
    let l = library();
    let mut m = Map {
        name: "paste".into(),
        ..Map::default()
    };
    m.placements.push(Placed {
        id: "b1".into(),
        descriptor: "bench".into(),
        at: (20.0, 20.0),
        ..Placed::default()
    });
    let ys = resolve_y(&m, &l).unwrap_or_else(|e| panic!("{e}"));
    let (y, host) = placement_at(&m, &l, &ys, &tools(), (0.0, 0.0))
        .unwrap_or_else(|e| panic!("a paste on open floor must not be refused: {e}"));
    assert!(host.is_none(), "nothing is under it");
    assert!((y - 0.0).abs() < 1e-5, "it stands on the map's floor, got {y}");
    // And the strict piece is still refused at the same spot, which is the branch's other half.
    assert!(needs_surface(&lamp()).is_some());
    assert!(
        placement_at(&m, &l, &ys, &lamp(), (0.0, 0.0)).is_err(),
        "a surface-only piece must still be refused here"
    );
}

/// **A paste over a bench seats on it**, so a two-way piece pasted onto a host is not left inside it.
#[test]
fn a_two_way_piece_pasted_over_a_bench_is_seated_on_it() {
    let l = library();
    let mut m = Map {
        name: "paste".into(),
        ..Map::default()
    };
    m.placements.push(Placed {
        id: "b1".into(),
        descriptor: "bench".into(),
        at: (0.0, 0.0),
        ..Placed::default()
    });
    let ys = resolve_y(&m, &l).unwrap_or_else(|e| panic!("{e}"));
    let found = host_under(&m, &l, &ys, &tools(), (0.0, 0.0));
    assert_eq!(
        found.map(|(p, _)| p.id.as_str()),
        Some("b1"),
        "the bench is under the paste, so it is the host"
    );
}
