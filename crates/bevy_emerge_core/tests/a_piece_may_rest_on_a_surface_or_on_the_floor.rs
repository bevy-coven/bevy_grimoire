//! **A piece that may be placed on a support surface *or* on the floor** — issue #1.
//!
//! # Why the schema needed it
//!
//! A `Descriptor` carries exactly one `mount: Option<Mount>`. Sanctuary's kit has *dressing sets* —
//! a smith's tools, an ingot stack, a horseshoe pile, a bundle of rods — and the game's owner decided
//! each may go **either on a bench or directly on the floor**, chosen per placement. A single mount
//! cannot say so: an `OnSurface { class: "support" }` piece can never stand on the floor, and an
//! `OnFloor` piece can never sit on a bench.
//!
//! The only way around it in data was a second descriptor with the same mesh and the other mount,
//! which doubles the library entries and splits one piece's labels across two ids.
//!
//! # What this file pins
//!
//! 1. `Mount::OnSurfaceOrFloor { class }` parses from RON and round-trips.
//! 2. Dropped over a piece offering the class, it is **seated on it** — the host's height plus the
//!    piece's own offset, with `Placed::on` naming the host.
//! 3. Dropped over bare floor, it stands **at the map's floor** with `on: None` — not refused.
//! 4. `resolve_y` gives the same answer as `placement_at`, so the editor's ghost and the game's
//!    spawner cannot disagree about where it went.
//! 5. What each placement records is what survives a save and a load: the map round-trips and
//!    re-resolves to the same heights.
//! 6. A plain `OnSurface` piece is still refused over bare floor — the new variant must not turn the
//!    strict mount into the permissive one.
//! 7. `Vocabularies::validate_library` treats the class as **needed** even though the piece can stand
//!    alone, so a library that offers no such surface fails at load rather than at the first drag.
//! 8. A seated piece whose host has gone is still an error — not a silent fall to the floor.
//! 9. Existing single-`mount:` libraries load and place exactly as before.

use bevy_emerge_core::descriptor::{Align, Descriptor, Extent, Mount, Offers};
use bevy_emerge_core::library::{LIBRARY_VERSION, Library};
use bevy_emerge_core::map::{Map, Placed};
use bevy_emerge_core::stack::{placement_at, resolve_y};
use bevy_emerge_core::vocab::Vocabularies;

/// The project's surface vocabulary, by the names this file uses. Written as RON so the parse is
/// exercised, and so a token typo here fails the same way a real project's would.
fn vocab() -> Vocabularies {
    Vocabularies::parse(
        r#"(
    kind: (tokens: [( name: "prop", note: "a thing" ), ( name: "bench", note: "a workbench" )]),
    effects: (tokens: [( name: "inert", note: "does nothing" )]),
    look: (tokens: [( name: "plain", note: "unremarkable" )]),
    surfaces: (tokens: [( name: "support", note: "any support top" )]),
    capabilities: (tokens: []),
    edge: (tokens: []),
    slot: (tokens: []),
)"#,
    )
    .unwrap_or_else(|e| panic!("the test vocabulary does not parse: {e}"))
}

/// A bench: floor-standing, 0.9 m tall, offering a `support` top.
fn bench() -> Descriptor {
    Descriptor {
        id: "bench".into(),
        extent: Extent {
            footprint: Some((1.8, 0.8)),
            height: Some(0.9),
        },
        offers: Offers {
            surfaces: vec!["support".into()],
            faces: Vec::new(),
            sockets: Vec::new(),
        },
        mount: Some(Mount::OnFloor),
        kind: vec!["bench".into()],
        effects: vec!["inert".into()],
        look: vec!["plain".into()],
        ..Descriptor::default()
    }
}

/// The dressing set: may go on a `support` top **or** on the floor. This is the whole point.
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
        kind: vec!["prop".into()],
        effects: vec!["inert".into()],
        look: vec!["plain".into()],
        // A real dressing set sits a centimetre into whatever it rests on; `datum` applies this on
        // top of the layer's own answer, so the seating assertions below include it on both paths.
        align: Align {
            y_offset: Some(0.01),
            ..Align::default()
        },
        ..Descriptor::default()
    }
}

/// A lamp that may **only** rest on a surface — the strict sibling, to prove the new variant did not
/// loosen it.
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

fn map(placements: Vec<Placed>) -> Map {
    Map {
        name: "multi_mount".into(),
        placements,
        ..Map::default()
    }
}

fn at(id: &str, descriptor: &str, at: (f32, f32), on: Option<&str>) -> Placed {
    Placed {
        id: id.into(),
        descriptor: descriptor.into(),
        at,
        on: on.map(str::to_owned),
        ..Placed::default()
    }
}

/// **The variant parses from RON and survives a round-trip.** Without this the feature cannot be
/// authored at all — the file is how a library is written.
#[test]
fn the_two_way_mount_is_authorable_in_a_library_file() {
    let d = tools();
    let text = ron::ser::to_string_pretty(
        &Descriptor {
            mount: d.mount.clone(),
            ..Descriptor::default()
        },
        ron::ser::PrettyConfig::default(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        text.contains("OnSurfaceOrFloor"),
        "the mount did not serialize by name: {text}"
    );

    let back: Descriptor = ron::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(back.mount, d.mount, "the round-trip changed the mount");
}

/// **Dropped over a bench, it is seated on it.** The host's own height is the answer, plus the
/// piece's own offset, and `on` names the bench — which is what makes the placement survive a reload.
#[test]
fn dropped_on_a_bench_it_rests_on_the_bench() {
    let l = library();
    let m = map(vec![at("b1", "bench", (0.0, 0.0), None)]);
    let ys = resolve_y(&m, &l).unwrap_or_else(|e| panic!("{e}"));

    let (y, host) = placement_at(&m, &l, &ys, &tools(), (0.0, 0.0))
        .unwrap_or_else(|e| panic!("a two-way piece must seat on a bench: {e}"));
    assert_eq!(
        host.map(|h| h.id.as_str()),
        Some("b1"),
        "the bench is under it, so it must be the host"
    );
    assert!(
        (y - (0.9 + 0.01)).abs() < 1e-5,
        "expected the bench's 0.9 m top plus the piece's 0.01 m offset, got {y}"
    );
}

/// **Dropped over bare floor, it stands on the floor** — and is not refused, which is the half the
/// single `OnSurface` could never do.
#[test]
fn dropped_on_bare_floor_it_stands_on_the_floor() {
    let l = library();
    let m = map(vec![at("b1", "bench", (20.0, 0.0), None)]);
    let ys = resolve_y(&m, &l).unwrap_or_else(|e| panic!("{e}"));

    let far = (0.0, 0.0);
    let (y, host) = placement_at(&m, &l, &ys, &tools(), far)
        .unwrap_or_else(|e| panic!("a two-way piece must stand on bare floor: {e}"));
    assert!(host.is_none(), "nothing is under it, so there is no host");
    assert!(
        (y - 0.01).abs() < 1e-5,
        "expected the map's floor plus the piece's 0.01 m offset, got {y}"
    );
}

/// **The two placements resolve to the heights they were seated at**, and the record is what decides
/// — not a re-derivation that could land the same piece on the floor after a reload.
#[test]
fn the_placement_record_decides_which_of_the_two_it_took() {
    let l = library();
    let m = map(vec![
        at("b1", "bench", (0.0, 0.0), None),
        at("seated", "tools", (0.0, 0.0), Some("b1")),
        at("onfloor", "tools", (10.0, 10.0), None),
    ]);
    let ys = resolve_y(&m, &l).unwrap_or_else(|e| panic!("{e}"));
    assert!((ys[1] - 0.91).abs() < 1e-5, "seated: {}", ys[1]);
    assert!((ys[2] - 0.01).abs() < 1e-5, "on the floor: {}", ys[2]);
}

/// **A save and a load reproduce both seatings.** The acceptance criterion, and the one a game
/// depends on: the map file is the only thing the consumer reads.
#[test]
fn both_seatings_survive_a_save_and_a_load() {
    let l = library();
    let m = map(vec![
        at("b1", "bench", (0.0, 0.0), None),
        at("seated", "tools", (0.0, 0.0), Some("b1")),
        at("onfloor", "tools", (10.0, 10.0), None),
    ]);
    let before = resolve_y(&m, &l).unwrap_or_else(|e| panic!("{e}"));

    let text = ron::ser::to_string_pretty(&m, ron::ser::PrettyConfig::default())
        .unwrap_or_else(|e| panic!("{e}"));
    let back: Map = ron::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
    back.validate().unwrap_or_else(|e| panic!("{e}"));
    let after = resolve_y(&back, &l).unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(before, after, "the reloaded map resolves differently");
    assert_eq!(back.placements[1].on.as_deref(), Some("b1"));
    assert_eq!(back.placements[2].on, None);
}

/// **The strict mount is still strict.** A lamp that may only rest on a surface is refused over bare
/// floor; a two-way piece beside it is not. This is the pair, and the reason `needs_surface` and
/// `may_take_surface` are two functions.
#[test]
fn a_surface_only_piece_is_still_refused_on_bare_floor() {
    let l = library();
    let m = map(vec![at("b1", "bench", (20.0, 0.0), None)]);
    let ys = resolve_y(&m, &l).unwrap_or_else(|e| panic!("{e}"));

    let err = placement_at(&m, &l, &ys, &lamp(), (0.0, 0.0))
        .err()
        .unwrap_or_else(|| panic!("a surface-only piece found a host on bare floor"));
    assert!(err.contains("support"), "the refusal names the class: {err}");
}

/// **A seated piece whose host has gone is an error**, not a silent drop to the floor — the edge case
/// the issue names, and the same answer a plain `OnSurface` gives.
#[test]
fn a_seated_piece_whose_host_is_gone_is_refused() {
    let l = library();
    let m = map(vec![at("seated", "tools", (0.0, 0.0), Some("b1"))]);
    let err = resolve_y(&m, &l).err().unwrap_or_default();
    assert!(
        err.contains("does not exist"),
        "expected the missing-host refusal, got: {err}"
    );
}

/// **The class is needed even though the piece can stand alone.** A library where nothing offers
/// `support` must fail at load: otherwise the piece silently always takes the floor and nobody learns
/// the bench side was never reachable.
#[test]
fn the_surface_class_is_needed_from_both_sides() {
    let v = vocab();
    let mut no_offers = bench();
    no_offers.offers.surfaces.clear();
    let err = v
        .validate_library(&[no_offers, tools()])
        .err()
        .unwrap_or_else(|| panic!("a two-way piece with no offerer in the library loaded clean"));
    assert!(err.contains("support"), "{err}");

    // And with the bench present it validates.
    v.validate_library(&[bench(), tools()])
        .unwrap_or_else(|e| panic!("{e}"));
}

/// **An existing single-`mount:` library loads and behaves exactly as before.** The regression the
/// issue calls out: nothing about this feature may change a piece that did not ask for it.
#[test]
fn a_single_mount_library_is_unchanged() {
    let v = vocab();
    let l = library();
    v.validate_library(&l.descriptors)
        .unwrap_or_else(|e| panic!("{e}"));

    // The strict surface piece still seats on the bench and still refuses bare floor, in one test.
    let m = map(vec![at("b1", "bench", (0.0, 0.0), None)]);
    let ys = resolve_y(&m, &l).unwrap_or_else(|e| panic!("{e}"));
    let (y, host) = placement_at(&m, &l, &ys, &lamp(), (0.0, 0.0))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(host.map(|h| h.id.as_str()), Some("b1"));
    assert!((y - 0.9).abs() < 1e-5, "{y}");

    // A floor piece is untouched by any of it.
    let floor_piece = bench();
    let (y, host) = placement_at(&m, &l, &ys, &floor_piece, (0.0, 0.0))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(host.is_none());
    assert!((y - 0.0).abs() < 1e-5, "{y}");
}
