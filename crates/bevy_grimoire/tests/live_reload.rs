//! **A running game follows its project's files, and only the entities that changed are touched.**
//! Headless: footprint-only descriptors draw as marker slabs, so no glTF and no GPU is needed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_grimoire::live::{GrimoireLivePlugin, ProjectReloaded};
use bevy_grimoire::{GrimoirePlugin, GrimoireWorld, MountedOn, Placement};
use bevy_grimoire_core::descriptor::{Descriptor, Extent};
use bevy_grimoire_core::kits::{Bind, KITS_VERSION, Kits, Lattice};
use bevy_grimoire_core::library::{LIBRARY_VERSION, Library};
use bevy_grimoire_core::live::Verdict;
use bevy_grimoire_core::map::{MAP_VERSION, Map, Placed};
use bevy_grimoire_core::project_data::ProjectData;

fn piece(id: &str, footprint: (f32, f32)) -> Descriptor {
    Descriptor {
        id: id.to_owned(),
        extent: Extent { footprint: Some(footprint), height: Some(1.0) },
        ..Descriptor::default()
    }
}

fn library(table_footprint: (f32, f32)) -> Library {
    Library {
        version: LIBRARY_VERSION,
        note: None,
        descriptors: vec![piece("kit/crate", (1.0, 1.0)), piece("kit/table", table_footprint)],
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
        let dir = std::env::temp_dir().join(format!("grimoire_live_reload_{tag}_{}", std::process::id()));
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
        std::fs::write(grimoire.join("kits.yaml"), kits.to_yaml().unwrap()).unwrap();
        std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/grimoire/vocab.ron"), grimoire.join("vocab.ron"))
            .unwrap();
        std::fs::write(grimoire.join("kit/project.yaml"), "(\n    version: 2,\n    note: None,\n    patches: [],\n)").unwrap();
        let p = Project { grimoire };
        p.write_library(library((2.0, 1.0)));
        p
    }

    /// Writes with a modification time later than anything written before, so a fast test is
    /// never defeated by a coarse filesystem clock.
    fn stamp(&self, path: &Path) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static TICK: AtomicU64 = AtomicU64::new(1);
        let t = SystemTime::now() + Duration::from_secs(TICK.fetch_add(10, Ordering::SeqCst));
        std::fs::File::options().write(true).open(path).unwrap().set_modified(t).unwrap();
    }

    fn write_library(&self, l: Library) {
        let path = self.grimoire.join("kit/library.yaml");
        std::fs::write(&path, l.to_yaml().unwrap()).unwrap();
        self.stamp(&path);
    }

    fn write_map(&self, m: &Map) {
        let path = self.grimoire.join("maps/yard.map.yaml");
        std::fs::write(&path, m.to_yaml().unwrap()).unwrap();
        self.stamp(&path);
    }

    fn write_raw_map(&self, text: &str) {
        let path = self.grimoire.join("maps/yard.map.yaml");
        std::fs::write(&path, text).unwrap();
        self.stamp(&path);
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
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(600)))
        .insert_resource(world);
    app.update();
    app
}

fn entities(app: &mut App) -> HashMap<String, Entity> {
    app.world_mut().query::<(Entity, &Placement)>().iter(app.world()).map(|(e, p)| (p.0.clone(), e)).collect()
}

/// Steps frames until the watcher has had time to notice and settle.
fn settle(app: &mut App) -> Vec<ProjectReloaded> {
    let mut out = Vec::new();
    for _ in 0..8 {
        app.update();
        let messages = app.world().resource::<Messages<ProjectReloaded>>();
        let mut cursor = messages.get_cursor_current();
        let _ = &mut cursor;
        out.extend(messages.iter_current_update_messages().cloned());
    }
    out
}

#[test]
fn an_edit_respawns_only_what_changed() {
    let project = Project::new("edit");
    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("crate_b", "kit/crate", (4.0, 1.0)),
        row("crate_c", "kit/crate", (8.0, 1.0)),
    ]));
    let mut app = app(&project);
    let before = entities(&mut app);
    assert_eq!(before.len(), 3);

    // b moves, c goes, d arrives; a is untouched.
    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("crate_b", "kit/crate", (5.0, 1.0)),
        row("crate_d", "kit/crate", (12.0, 1.0)),
    ]));
    let reloads = settle(&mut app);
    assert_eq!(reloads.len(), 1, "one reload after the write");
    let diff = &reloads[0].diffs["yard"];
    assert_eq!((diff.added.len(), diff.removed.len(), diff.changed.len()), (1, 1, 1));
    assert!(!reloads[0].library_changed);

    let after = entities(&mut app);
    assert_eq!(after.len(), 3);
    assert_eq!(after["crate_a"], before["crate_a"], "an unchanged placement keeps its Entity");
    // **A moved placement keeps its Entity too** (#201 acceptance item 5: move, turn, retint and
    // retune preserve identity; only a change of kind respawns). The old claim here was the
    // opposite — `assert_ne!` and "the old entity is gone" — and it is what made an unrelated edit
    // restart a society run's people and a fire's flicker. What must still be true is that the
    // entity is where the map now says it is, so the transform is asserted rather than the identity.
    assert_eq!(after["crate_b"], before["crate_b"], "a moved placement keeps its Entity");
    // **And it stands where the new map says it does.** Keeping the entity is only half the claim;
    // the other half is that the transform was retargeted, which is the work a respawn used to do.
    let moved = app.world().get::<Transform>(after["crate_b"]).copied().expect("a placed piece has a Transform");
    let world = app.world().resource::<GrimoireWorld>();
    let descriptor = world.entry("kit/crate").expect("the descriptor").0.clone();
    let want = bevy_grimoire::placed_transform(&descriptor, (5.0, 1.0), 0.0, (0, 0), (0.0, 0.0, 0.0), world.y[1]);
    assert_eq!(
        moved.translation, want.translation,
        "a moved piece stands where the new map says it does, with the entity it always had"
    );
    assert!(!after.contains_key("crate_c") && after.contains_key("crate_d"));
    assert_eq!(app.world().resource::<GrimoireWorld>().map.placements.len(), 3);
}

#[test]
fn a_refused_reload_changes_nothing_and_says_so() {
    let project = Project::new("refused");
    project.write_map(&map(vec![row("crate_a", "kit/crate", (1.0, 1.0))]));
    let mut app = app(&project);
    let before = entities(&mut app);

    project.write_raw_map("this is not a map");
    let reloads = settle(&mut app);
    assert!(reloads.is_empty(), "a refused reload writes no ProjectReloaded");
    assert_eq!(entities(&mut app), before, "every entity stays in place");
    let verdict = Verdict::read(&project.grimoire).unwrap().expect("the refusal is written");
    assert!(!verdict.ok, "{}", verdict.message);

    // The next good write recovers, and the unchanged piece still keeps its entity.
    project.write_map(&map(vec![row("crate_a", "kit/crate", (1.0, 1.0)), row("crate_b", "kit/crate", (3.0, 1.0))]));
    let reloads = settle(&mut app);
    assert_eq!(reloads.len(), 1);
    let after = entities(&mut app);
    assert_eq!(after["crate_a"], before["crate_a"]);
    assert!(after.contains_key("crate_b"));
    assert!(Verdict::read(&project.grimoire).unwrap().unwrap().ok);
}

#[test]
fn a_library_edit_respawns_every_placement_of_that_descriptor() {
    let project = Project::new("library");
    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("table_a", "kit/table", (5.0, 1.0)),
        row("table_b", "kit/table", (9.0, 1.0)),
    ]));
    let mut app = app(&project);
    let before = entities(&mut app);

    project.write_library(library((3.0, 1.5)));
    let reloads = settle(&mut app);
    assert_eq!(reloads.len(), 1);
    assert!(reloads[0].library_changed);
    assert!(reloads[0].diffs["yard"].is_empty(), "the map itself did not change");
    let after = entities(&mut app);
    assert_eq!(after["crate_a"], before["crate_a"]);
    assert_ne!(after["table_a"], before["table_a"]);
    assert_ne!(after["table_b"], before["table_b"]);
}

#[test]
fn a_rider_follows_its_host_and_is_mounted_again() {
    let project = Project::new("rider");
    let mut lamp = row("lamp", "kit/crate", (5.0, 1.0));
    lamp.on = Some("table_a".to_owned());
    project.write_map(&map(vec![row("table_a", "kit/table", (5.0, 1.0)), lamp.clone()]));
    let mut app = app(&project);
    let before = entities(&mut app);
    assert_eq!(app.world().get::<MountedOn>(before["lamp"]).map(|m| m.0), Some(before["table_a"]));

    project.write_map(&map(vec![row("table_a", "kit/table", (6.0, 1.0)), lamp]));
    settle(&mut app);
    let after = entities(&mut app);
    // **A moved host keeps its entity, and so does its rider** (#201, item 5), and the rider is
    // still mounted on the same table — the relationship is what has to survive a move, not the
    // despawn that used to carry it.
    assert_eq!(after["table_a"], before["table_a"], "a moved host keeps its Entity");
    assert_eq!(after["lamp"], before["lamp"], "and its rider keeps its own");
    assert_eq!(app.world().get::<MountedOn>(after["lamp"]).map(|m| m.0), Some(after["table_a"]));
}

/// **Reload twice with no change between them: nothing is spawned the second time** (#201, acceptance
/// item 3).
///
/// The first reload is allowed to spawn — the map has just been written, and a piece the old world
/// never drew has no entity to keep. The second reload has nothing to do at all: every placement is in
/// both maps with the same descriptor, the same host and the same pose, so `plan`'s `spawn` set is
/// empty and `spawn_placements` draws nothing. Counting the entities before and after is what makes
/// "nothing respawns" a measurement rather than a hope; a plan that mis-classified one row would show
/// up as a new `Entity` here even though the map on disk never changed.
///
/// **Measured, and it does not discriminate this change**: with the move split reverted this test
/// passes too, because two identical files differ in no row at all and the old `same_but_id` already
/// said so. It is kept as the guard acceptance item 3 asks for — it would catch a future plan that
/// spawned on a timer, a re-read, or a comparison that had grown an always-false arm — and the proof
/// that *this* slice works is `an_edit_respawns_only_what_changed` and
/// `a_rider_follows_its_host_and_is_mounted_again`, both of which fail on the base with
/// `assertion left != right failed: a moved placement is respawned`.
#[test]
fn reloading_twice_with_no_change_spawns_nothing_the_second_time() {
    let project = Project::new("steady");
    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("crate_b", "kit/crate", (4.0, 1.0)),
    ]));
    let mut app = app(&project);
    // The first write is the edit that makes the editor's world match the file.
    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("crate_b", "kit/crate", (5.0, 1.0)),
    ]));
    settle(&mut app);
    let before = entities(&mut app);
    assert_eq!(before.len(), 2);

    // **The same bytes again.** A save that changed nothing must not draw anything again.
    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("crate_b", "kit/crate", (5.0, 1.0)),
    ]));
    settle(&mut app);

    let after = entities(&mut app);
    assert_eq!(after, before, "a reload with nothing changed must keep every Entity, id for id");
    for (id, entity) in &before {
        assert!(
            app.world().get_entity(*entity).is_ok(),
            "`{id}`'s entity was despawned by a reload that had nothing to do"
        );
    }
}
