//! **A running game refuses a bad file of any kind it reads, keeps what it had, and says which file and
//! which field** (#200, H3). Headless, like `live_reload.rs`: footprint-only descriptors draw as marker
//! slabs.
//!
//! For each file the game reads (`kits.ron`, a kit's `library.ron` and `project.ron`, `vocab.ron`,
//! `rules.ron`, `compositions.ron`, a map) three bad edits: corrupt, a field of the wrong type, a
//! required field missing. Each leaves every entity and the `GrimoireWorld` as they were and writes one
//! verdict whose per-file entry for that file is refused, with the kind, the path, the field and a
//! reason, while every other file's entry stays applied. The next good write recovers.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_grimoire::live::{GrimoireLivePlugin, ProjectReloaded};
use bevy_grimoire::{GrimoirePlugin, GrimoireWorld, Placement};
use bevy_grimoire_core::composition::{COMPOSITIONS_VERSION, Compositions};
use bevy_grimoire_core::descriptor::{Descriptor, Extent};
use bevy_grimoire_core::kits::{Bind, KITS_VERSION, Kits, Lattice};
use bevy_grimoire_core::library::{LIBRARY_VERSION, Library};
use bevy_grimoire_core::live::{KindVerdict, Outcome, Verdict};
use bevy_grimoire_core::map::{MAP_VERSION, Map, Placed};
use bevy_grimoire_core::project_data::ProjectData;
use bevy_grimoire_core::rules::RULES_VERSION;

fn piece(id: &str) -> Descriptor {
    Descriptor {
        id: id.to_owned(),
        extent: Extent { footprint: Some((1.0, 1.0)), height: Some(1.0) },
        ..Descriptor::default()
    }
}

fn row(id: &str, descriptor: &str, at: (f32, f32)) -> Placed {
    Placed { id: id.to_owned(), descriptor: descriptor.to_owned(), at, ..Placed::default() }
}

fn map(placements: Vec<Placed>) -> Map {
    Map { version: MAP_VERSION, name: "yard".to_owned(), bounds: (100.0, 10.0, 100.0), placements, ..Map::default() }
}

struct Project {
    grimoire: PathBuf,
}

impl Project {
    fn new(tag: &str) -> Project {
        let dir = std::env::temp_dir().join(format!("grimoire_live_verdicts_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let grimoire = dir.join("assets/grimoire");
        std::fs::create_dir_all(grimoire.join("kit")).unwrap();
        std::fs::create_dir_all(grimoire.join("maps")).unwrap();
        let p = Project { grimoire };
        let kits = Kits {
            version: KITS_VERSION,
            note: None,
            lattice: Lattice::default(),
            bind: vec![Bind { namespace: "kit".to_owned(), dir: "kit".to_owned() }],
            bash: Vec::new(),
            authoring: Some("kit".to_owned()),
        };
        p.write("kits.ron", &kits.to_ron().unwrap());
        std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/grimoire/vocab.ron"), p.grimoire.join("vocab.ron"))
            .unwrap();
        p.write("kit/project.ron", "(\n    version: 2,\n    note: None,\n    patches: [],\n)");
        let library = Library { version: LIBRARY_VERSION, note: None, descriptors: vec![piece("kit/crate")] };
        p.write("kit/library.ron", &library.to_ron().unwrap());
        p.write("rules.ron", &format!("(\n    version: {RULES_VERSION},\n    blocks: [],\n)"));
        p.write("compositions.ron", &Compositions { version: COMPOSITIONS_VERSION, ..Compositions::default() }.to_ron().unwrap());
        p.write_map(&map(vec![row("crate_a", "kit/crate", (1.0, 1.0))]));
        p
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.grimoire.join(rel)).unwrap()
    }

    /// Writes with a modification time later than anything written before, so a fast test is never
    /// defeated by a coarse filesystem clock.
    fn write(&self, rel: &str, text: &str) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static TICK: AtomicU64 = AtomicU64::new(1);
        let path = self.grimoire.join(rel);
        std::fs::write(&path, text).unwrap();
        let t = SystemTime::now() + Duration::from_secs(TICK.fetch_add(10, Ordering::SeqCst));
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(t).unwrap();
    }

    fn write_map(&self, m: &Map) {
        self.write("maps/yard.map.ron", &m.to_ron().unwrap());
    }

    fn verdict(&self) -> Verdict {
        Verdict::read(&self.grimoire).unwrap().expect("the game wrote a verdict")
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

/// Steps frames until the watcher has had time to notice and settle; how many reloads landed.
fn settle(app: &mut App, frames: usize) -> usize {
    let mut reloads = 0;
    for _ in 0..frames {
        app.update();
        reloads += app.world().resource::<Messages<ProjectReloaded>>().iter_current_update_messages().count();
    }
    reloads
}

/// What the running game holds: how many placements and descriptors its world has.
fn held(app: &App) -> (usize, usize) {
    let w = app.world().resource::<GrimoireWorld>();
    (w.map.placements.len(), w.library.descriptors.len())
}

fn entry<'a>(v: &'a Verdict, file: &str) -> &'a KindVerdict {
    v.kinds.iter().find(|k| k.file == file).unwrap_or_else(|| panic!("no verdict for `{file}` in {:?}", v.kinds))
}

/// **The files the game reads**: path, the kind it is, and the root field each bad edit damages.
const FILES: &[(&str, &str, &str)] = &[
    ("kits.ron", "kits", "version"),
    ("kit/library.ron", "library", "version"),
    ("kit/project.ron", "policy", "version"),
    ("vocab.ron", "vocab", "kind"),
    ("rules.ron", "rules", "version"),
    ("compositions.ron", "compositions", "version"),
    ("maps/yard.map.ron", "map", "version"),
];

fn bad(project: &Project, rel: &str, field: &str, case: &str) -> String {
    let good = project.read(rel);
    match (case, field) {
        ("corrupt", _) => good[..good.len() / 2].to_owned(),
        ("wrong type", "version") => good.replacen("version:", "version: \"wrong\", was:", 1),
        ("missing field", "version") => good.lines().filter(|l| !l.trim_start().starts_with("version:")).collect::<Vec<_>>().join("\n"),
        ("wrong type", _) => format!("({field}: 7)"),
        ("missing field", _) => "()".to_owned(),
        _ => unreachable!(),
    }
}

#[test]
fn a_bad_file_of_each_kind_is_refused_named_and_changes_nothing() {
    let project = Project::new("each");
    let mut app = app(&project);
    let before = (entities(&mut app), held(&app));
    let mut seq = 0;

    for (rel, kind, field) in FILES {
        for case in ["corrupt", "wrong type", "missing field"] {
            let good = project.read(rel);
            project.write(rel, &bad(&project, rel, field, case));
            let reloads = settle(&mut app, 8);
            let what = format!("{case} in `{rel}`");

            assert_eq!(reloads, 0, "{what}: a refused reload writes no ProjectReloaded");
            assert_eq!((entities(&mut app), held(&app)), before, "{what}: the running state must not change");
            let v = project.verdict();
            seq += 1;
            assert_eq!(v.seq, seq, "{what}: one edit, one verdict");
            assert!(!v.ok, "{what}: {}", v.message);
            let k = entry(&v, rel);
            assert_eq!((k.kind.as_str(), k.outcome), (*kind, Outcome::Refused), "{what}: {k:?}");
            assert!(!k.reason.is_empty(), "{what}: refused with no reason");
            if case != "corrupt" {
                assert_eq!(k.field.as_deref(), Some(*field), "{what}: {}", k.reason);
            }
            for other in v.kinds.iter().filter(|o| o.file != *rel) {
                assert_eq!(other.outcome, Outcome::Applied, "{what}: `{}` is fine and must say so: {other:?}", other.file);
            }

            // Recovery: the good text comes back, and one verdict says every file is applied.
            project.write(rel, &good);
            assert_eq!(settle(&mut app, 8), 1, "{what}: the good file reloads once");
            seq += 1;
            let v = project.verdict();
            assert_eq!(v.seq, seq);
            assert!(v.ok && v.kinds.iter().all(|k| k.outcome == Outcome::Applied), "{what}: {:?}", v.kinds);
            assert_eq!(entities(&mut app), before.0, "{what}: an unchanged placement keeps its entity");
        }
    }
    let v = project.verdict();
    assert_eq!(v.kinds.len(), FILES.len(), "every file the game reads has an entry: {:?}", v.kinds);
}

#[test]
fn one_edit_is_one_verdict_however_many_frames_pass() {
    let project = Project::new("once");
    let mut app = app(&project);

    project.write("maps/yard.map.ron", "this is not a map");
    settle(&mut app, 8);
    let first = project.verdict();
    assert_eq!(first.seq, 1);
    settle(&mut app, 60);
    let later = project.verdict();
    assert_eq!(later, first, "sixty more frames with no edit write nothing");

    project.write_map(&map(vec![row("crate_a", "kit/crate", (1.0, 1.0)), row("crate_b", "kit/crate", (3.0, 1.0))]));
    assert_eq!(settle(&mut app, 60), 1);
    let v = project.verdict();
    assert_eq!(v.seq, 2, "the good edit is the second verdict");
    assert!(v.ok);
}

#[test]
fn a_refusal_about_how_files_fit_together_goes_to_the_file_it_names() {
    let project = Project::new("fit");
    let mut app = app(&project);
    let before = (entities(&mut app), held(&app));

    // The map parses, and places a descriptor no library has.
    project.write_map(&map(vec![row("ghost", "kit/missing", (1.0, 1.0))]));
    assert_eq!(settle(&mut app, 8), 0);
    assert_eq!((entities(&mut app), held(&app)), before);
    let v = project.verdict();
    assert!(!v.ok);
    let k = entry(&v, "maps/yard.map.ron");
    assert_eq!((k.kind.as_str(), k.outcome), ("map", Outcome::Refused));
    assert!(k.reason.contains("kit/missing"), "{}", k.reason);
    assert!(v.kinds.iter().filter(|o| o.file != "maps/yard.map.ron").all(|o| o.outcome == Outcome::Applied), "{:?}", v.kinds);
}

#[test]
fn an_error_no_file_owns_is_a_project_entry_beside_the_files_that_are_wrong() {
    let project = Project::new("project");
    let mut app = app(&project);
    let before = (entities(&mut app), held(&app));

    // The library reads on its own, and names a token no vocabulary has: the project does not fit
    // together, and no single file's path starts the error. The map is corrupt on top of that.
    let mut odd = piece("kit/crate");
    odd.kind = vec!["no-such-token".to_owned()];
    let library = Library { version: LIBRARY_VERSION, note: None, descriptors: vec![odd] };
    project.write("kit/library.ron", &library.to_ron().unwrap());
    project.write("maps/yard.map.ron", "this is not a map");
    assert_eq!(settle(&mut app, 8), 0);
    assert_eq!((entities(&mut app), held(&app)), before);

    let v = project.verdict();
    assert!(!v.ok);
    let fit = entry(&v, ".");
    assert_eq!((fit.kind.as_str(), fit.outcome), ("project", Outcome::Refused));
    assert!(fit.reason.contains("no-such-token"), "{}", fit.reason);
    assert_eq!(entry(&v, "maps/yard.map.ron").outcome, Outcome::Refused, "the corrupt map is named too, not hidden by the other error");
    assert_eq!(entry(&v, "kit/library.ron").outcome, Outcome::Applied, "the library reads fine by itself");
}
