//! **A map holds a run, not the rows it lays** (#115, slice 1: data, `expanded()`, validation, version).
//!
//! A fence of hundreds of posts is one line. The map stores the run (descriptor, path, pitch, gaps)
//! and `Map::expanded` lays it, so moving an end is one edit and the rows follow.

use bevy_grimoire_core::descriptor::{Descriptor, Extent};
use bevy_grimoire_core::library::{LIBRARY_VERSION, Library};
use bevy_grimoire_core::map::{MAP_VERSION, Map, Placed};
use bevy_grimoire_core::run::{Run, RunPlaced};

const W: f32 = 2.064;

fn library() -> Library {
    Library {
        version: LIBRARY_VERSION,
        note: None,
        descriptors: vec![Descriptor {
            id: "kit/fence".to_owned(),
            extent: Extent { footprint: Some((W, 0.12)), height: Some(0.838) },
            ..Default::default()
        }],
    }
}

fn run(to: f32) -> Run {
    Run {
        descriptor: "kit/fence".to_owned(),
        pitch: None,
        from: (0.0, 0.0),
        to: (to, 0.0),
        corners: Vec::new(),
        gaps: Vec::new(),
    }
}

fn map_with(runs: Vec<RunPlaced>) -> Map {
    Map {
        name: "yard".to_owned(),
        bounds: (100.0, 10.0, 100.0),
        runs,
        ..Map::default()
    }
}

fn placed(id: &str, run: Run) -> RunPlaced {
    RunPlaced { id: id.to_owned(), run, owned: false, owned_because: None }
}

fn ron_of(map: &Map) -> String {
    bevy_grimoire_core::yaml::to_string(map).unwrap()
}

/// Criterion 1: a run of ten posts round-trips and expands to ten rows `<id>/0..9` at the pitch.
#[test]
fn a_run_round_trips_and_expands_to_its_rows() {
    let map = map_with(vec![placed("ring", run(19.0))]);
    let back = Map::parse(&ron_of(&map)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(back, map);
    assert!(back.placements.is_empty(), "the rows are never stored: {:?}", back.placements);

    let out = back.expanded(&[], &library()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(out.placements.len(), 10);
    for (n, row) in out.placements.iter().enumerate() {
        assert_eq!(row.id, format!("ring/{n}"));
        assert_eq!(row.descriptor, "kit/fence");
        assert!((row.at.0 - W * n as f32).abs() < 1e-4, "{row:?}");
    }
    assert_eq!(out.runs, back.runs, "the reference survives the expansion");
}

/// An owned run lays owned rows; a hand row beside it is untouched.
#[test]
fn rows_are_owned_like_their_run() {
    let mut r = placed("ring", run(5.0));
    r.owned = true;
    r.owned_because = Some("the graveyard ring".to_owned());
    let mut map = map_with(vec![r]);
    map.placements.push(Placed {
        id: "well".to_owned(),
        descriptor: "kit/fence".to_owned(),
        ..Placed::default()
    });
    let out = map.expanded(&[], &library()).unwrap();
    let rows: Vec<_> = out.placements.iter().filter(|p| p.id.starts_with("ring/")).collect();
    assert!(!rows.is_empty() && rows.iter().all(|p| p.owned && p.owned_because.is_some()));
    assert!(!out.placements.iter().find(|p| p.id == "well").unwrap().owned);
}

/// Edge cases: a missing piece refuses naming the run; an id colliding with a placement refuses.
#[test]
fn a_run_that_cannot_lay_or_collides_refuses() {
    let mut bad = run(10.0);
    bad.descriptor = "kit/nothing".to_owned();
    let err = map_with(vec![placed("ring", bad)])
        .expanded(&[], &library())
        .expect_err("a missing piece");
    assert!(err.contains("`ring`") && err.contains("kit/nothing"), "{err}");

    let mut m = map_with(vec![placed("fence", run(10.0))]);
    m.placements.push(Placed { id: "fence".to_owned(), descriptor: "kit/fence".to_owned(), ..Placed::default() });
    let err = m.validate().expect_err("an id shared with a placement");
    assert!(err.contains("run `fence`"), "{err}");

    let m = map_with(vec![placed("a", run(10.0)), placed("a", run(10.0))]);
    assert!(m.validate().expect_err("twice").contains("used twice"));
}

/// Criterion 4: a version-7 map (no `runs`) loads and re-saves at the current version, with nothing
/// written for the empty list; a file numbered above this build is refused.
#[test]
fn an_older_map_loads_and_a_newer_one_is_refused() {
    assert_eq!(MAP_VERSION, 9);
    let current = ron_of(&map_with(Vec::new()));
    assert!(!current.contains("runs"), "an empty list is not written: {current}");
    let old = current.replace(&format!("version: {MAP_VERSION}"), "version: 7");
    assert!(old.contains("version: 7"), "{old}");
    let loaded = Map::parse(&old).unwrap_or_else(|e| panic!("a v7 map must load: {e}"));
    assert!(loaded.runs.is_empty());

    let with = ron_of(&map_with(vec![placed("ring", run(19.0))]));
    assert!(with.contains("runs"), "{with}");
    let newer = with.replace(&format!("version: {MAP_VERSION}"), &format!("version: {}", MAP_VERSION + 1));
    let err = Map::parse(&newer).expect_err("a newer map");
    assert!(err.contains("newer tool"), "{err}");
}

fn same_rows(a: &[Placed], b: &[Placed]) {
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b) {
        assert_eq!(x.descriptor, y.descriptor);
        assert!((x.at.0 - y.at.0).abs() < 1e-4 && (x.at.1 - y.at.1).abs() < 1e-4, "{x:?} {y:?}");
        assert_eq!(x.yaw, y.yaw);
    }
}

/// Criterion 3: rows `run::lay` produced (a corner, a gap) convert to a run that expands to them.
#[test]
fn rows_a_run_laid_convert_back_to_the_run() {
    let mut r = run(8.0);
    r.corners = vec![bevy_grimoire_core::run::Corner { at: (8.256, 0.0), yaw: 90.0 }];
    r.to = (8.256, 7.0);
    r.gaps = vec![bevy_grimoire_core::run::Gap { count: 1, at_post: 2 }];
    let lib = library();
    let original = map_with(vec![placed("ring", r)]).expanded(&[], &lib).unwrap();
    let ids: Vec<String> = original.placements.iter().map(|p| p.id.clone()).collect();

    let mut flat = Map { runs: Vec::new(), ..original.clone() };
    flat.placements = original.placements.clone();
    flat.placements.push(Placed { id: "well".into(), descriptor: "kit/fence".into(), ..Placed::default() });
    flat.placements_to_run("fence", &ids, &lib).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(flat.runs.len(), 1);
    assert_eq!(flat.placements.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["well"]);
    let back = flat.expanded(&[], &lib).unwrap();
    let rows: Vec<Placed> = back.placements.into_iter().filter(|p| p.id != "well").collect();
    same_rows(&rows, &original.placements);
}

/// Criterion 3: a row 5 mm off the pitch refuses, names that row, and leaves the map alone.
#[test]
fn a_row_off_the_pitch_does_not_convert() {
    let lib = library();
    let laid = map_with(vec![placed("ring", run(19.0))]).expanded(&[], &lib).unwrap();
    let mut flat = Map { runs: Vec::new(), ..laid.clone() };
    flat.placements[4].at.0 += 0.005;
    let before = flat.clone();
    let ids: Vec<String> = flat.placements.iter().map(|p| p.id.clone()).collect();
    let err = flat.placements_to_run("fence", &ids, &lib).expect_err("a nudged row");
    assert!(err.contains("`ring/4`"), "{err}");
    assert_eq!(flat, before);
}
