//! **Runtime state keeps its identity across a reload** (#201). A game hangs state on a placement's
//! `Entity` (a booking, an animation phase, a camera target), so a reload that draws a piece again
//! throws that state away even when the edit did not change the piece. These tests count what a
//! reload draws: an edit to one piece must not draw another, and a rename is not a change.
//!
//! Headless: footprint-only descriptors draw as marker slabs, so no glTF and no GPU is needed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_grimoire::live::GrimoireLivePlugin;
use bevy_grimoire::{GrimoirePlugin, GrimoireWorld, MountedOn, OfDescriptor, Placement};
use bevy_grimoire_core::descriptor::{Descriptor, Extent};
use bevy_grimoire_core::kits::{Bind, KITS_VERSION, Kits, Lattice};
use bevy_grimoire_core::library::{LIBRARY_VERSION, Library};
use bevy_grimoire_core::map::{MAP_VERSION, Map, Placed};
use bevy_grimoire_core::project_data::ProjectData;

/// State a game hung on a placement's entity.
#[derive(Component, Debug, PartialEq)]
struct Booked(u32);

/// How many placement entities were drawn since the last reset.
#[derive(Resource, Default)]
struct Spawns(usize);

fn piece(id: &str, footprint: (f32, f32)) -> Descriptor {
    Descriptor {
        id: id.to_owned(),
        extent: Extent { footprint: Some(footprint), height: Some(1.0) },
        ..Descriptor::default()
    }
}

fn row(id: &str, descriptor: &str, at: (f32, f32)) -> Placed {
    Placed { id: id.to_owned(), descriptor: descriptor.to_owned(), at, ..Placed::default() }
}

fn map(placements: Vec<Placed>) -> Map {
    Map {
        version: MAP_VERSION,
        name: "yard".to_owned(),
        bounds: (100.0, 10.0, 100.0),
        placements,
        ..Map::default()
    }
}

struct Project {
    grimoire: PathBuf,
}

impl Project {
    fn new(tag: &str) -> Project {
        let dir = std::env::temp_dir().join(format!("grimoire_live_identity_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let grimoire = dir.join("assets/grimoire");
        std::fs::create_dir_all(grimoire.join("kit")).unwrap();
        std::fs::create_dir_all(grimoire.join("maps")).unwrap();
        let kits = Kits {
            version: KITS_VERSION,
            note: None,
            lattice: Lattice::default(),
            bind: vec![Bind { namespace: "kit".to_owned(), dir: "kit".to_owned() }],
            bash: Vec::new(),
            authoring: Some("kit".to_owned()),
        };
        std::fs::write(grimoire.join("kits.ron"), kits.to_ron().unwrap()).unwrap();
        std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/grimoire/vocab.ron"), grimoire.join("vocab.ron"))
            .unwrap();
        std::fs::write(grimoire.join("kit/project.ron"), "(\n    version: 2,\n    note: None,\n    patches: [],\n)").unwrap();
        let library = Library {
            version: LIBRARY_VERSION,
            note: None,
            descriptors: vec![piece("kit/crate", (1.0, 1.0)), piece("kit/table", (2.0, 1.0))],
        };
        std::fs::write(grimoire.join("kit/library.ron"), library.to_ron().unwrap()).unwrap();
        Project { grimoire }
    }

    /// Writes with a modification time later than anything written before, so a fast test is
    /// never defeated by a coarse filesystem clock.
    fn write_map(&self, m: &Map) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static TICK: AtomicU64 = AtomicU64::new(1);
        let path = self.grimoire.join("maps/yard.map.ron");
        std::fs::write(&path, m.to_ron().unwrap()).unwrap();
        let t = SystemTime::now() + Duration::from_secs(TICK.fetch_add(10, Ordering::SeqCst));
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(t).unwrap();
    }
}

fn app(project: &Project) -> App {
    let data = ProjectData::load(&project.grimoire, &["yard"]).unwrap_or_else(|e| panic!("{e}"));
    let world = GrimoireWorld::with_compositions(
        data.bound.library.clone(),
        data.maps["yard"].clone(),
        data.vocab.clone(),
        &[],
        data.kits.lattice,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .add_plugins(GrimoirePlugin)
        .add_plugins(GrimoireLivePlugin { dir: project.grimoire.clone(), maps: vec!["yard".to_owned()] })
        .init_resource::<Spawns>()
        .add_observer(|_: On<Add<Placement>>, mut spawns: ResMut<Spawns>| spawns.0 += 1)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(600)))
        .insert_resource(world);
    app.update();
    app.world_mut().resource_mut::<Spawns>().0 = 0;
    app
}

fn entities(app: &mut App) -> HashMap<String, Entity> {
    app.world_mut().query::<(Entity, &Placement)>().iter(app.world()).map(|(e, p)| (p.0.clone(), e)).collect()
}

/// Steps frames until the watcher has had time to notice and settle, and returns how many
/// placement entities those frames drew.
fn settle(app: &mut App) -> usize {
    for _ in 0..8 {
        app.update();
    }
    std::mem::take(&mut app.world_mut().resource_mut::<Spawns>().0)
}

#[test]
fn a_renamed_placement_keeps_its_entity_and_what_was_hung_on_it() {
    let project = Project::new("rename");
    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("crate_b", "kit/crate", (4.0, 1.0)),
    ]));
    let mut app = app(&project);
    let before = entities(&mut app);
    app.world_mut().entity_mut(before["crate_b"]).insert(Booked(7));

    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("crate_beta", "kit/crate", (4.0, 1.0)),
    ]));
    let drawn = settle(&mut app);

    let after = entities(&mut app);
    assert_eq!(after.len(), 2, "{after:?}");
    assert!(!after.contains_key("crate_b"), "the old name is gone: {after:?}");
    assert_eq!(after["crate_beta"], before["crate_b"], "the rename follows the same Entity");
    assert_eq!(app.world().get::<Booked>(after["crate_beta"]), Some(&Booked(7)), "what a game hung on it survives");
    assert_eq!(after["crate_a"], before["crate_a"]);
    assert_eq!(drawn, 0, "a rename draws nothing");
    assert_eq!(app.world().resource::<GrimoireWorld>().map.placements[1].id, "crate_beta");
}

#[test]
fn a_rename_that_also_moves_is_a_new_piece() {
    let project = Project::new("rename_and_move");
    project.write_map(&map(vec![row("crate_a", "kit/crate", (1.0, 1.0)), row("crate_b", "kit/crate", (4.0, 1.0))]));
    let mut app = app(&project);
    let before = entities(&mut app);

    project.write_map(&map(vec![row("crate_a", "kit/crate", (1.0, 1.0)), row("crate_beta", "kit/crate", (6.0, 1.0))]));
    let drawn = settle(&mut app);

    let after = entities(&mut app);
    assert_ne!(after["crate_beta"], before["crate_b"], "an edit that changes what the piece is draws it again");
    assert!(app.world().get_entity(before["crate_b"]).is_err());
    assert_eq!(after["crate_a"], before["crate_a"]);
    assert_eq!(drawn, 1, "only the renamed and moved piece");
}

#[test]
fn what_rests_on_a_renamed_host_stays_mounted_on_it() {
    let project = Project::new("rename_host");
    let mut lamp = row("lamp", "kit/crate", (5.0, 1.0));
    lamp.on = Some("table_a".to_owned());
    project.write_map(&map(vec![row("table_a", "kit/table", (5.0, 1.0)), lamp.clone()]));
    let mut app = app(&project);
    let before = entities(&mut app);
    app.world_mut().entity_mut(before["lamp"]).insert(Booked(3));

    lamp.on = Some("table_b".to_owned());
    project.write_map(&map(vec![row("table_b", "kit/table", (5.0, 1.0)), lamp]));
    let drawn = settle(&mut app);

    let after = entities(&mut app);
    assert_eq!(after["table_b"], before["table_a"], "the host follows its rename");
    assert_eq!(after["lamp"], before["lamp"], "the rider's row names the new host and nothing else changed");
    assert_eq!(app.world().get::<Booked>(after["lamp"]), Some(&Booked(3)));
    assert_eq!(app.world().get::<MountedOn>(after["lamp"]).map(|m| m.0), Some(after["table_b"]));
    assert_eq!(drawn, 0);
}

#[test]
fn a_reload_with_no_change_draws_nothing_twice() {
    let project = Project::new("no_change");
    let rows = vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("table_a", "kit/table", (5.0, 1.0)),
        row("crate_b", "kit/crate", (9.0, 1.0)),
    ];
    project.write_map(&map(rows.clone()));
    let mut app = app(&project);
    let before = entities(&mut app);
    for _ in 0..2 {
        // The same text, written again: the file's modification time moves, its content does not.
        project.write_map(&map(rows.clone()));
        assert_eq!(settle(&mut app), 0, "nothing is drawn for an edit that changed nothing");
        assert_eq!(entities(&mut app), before, "every entity stays");
    }
}

#[test]
fn changing_one_pieces_kind_draws_that_piece_only() {
    let project = Project::new("kind");
    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("crate_b", "kit/crate", (4.0, 1.0)),
        row("crate_c", "kit/crate", (8.0, 1.0)),
    ]));
    let mut app = app(&project);
    let before = entities(&mut app);

    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("crate_b", "kit/table", (4.0, 1.0)),
        row("crate_c", "kit/crate", (8.0, 1.0)),
    ]));
    let drawn = settle(&mut app);

    let after = entities(&mut app);
    assert_ne!(after["crate_b"], before["crate_b"], "the piece whose kind changed is drawn again");
    assert_eq!(app.world().get::<OfDescriptor>(after["crate_b"]).map(|d| d.0.as_str()), Some("kit/table"));
    assert_eq!(after["crate_a"], before["crate_a"]);
    assert_eq!(after["crate_c"], before["crate_c"]);
    assert_eq!(drawn, 1);
}
