//! **A running game re-reads a data file of its own when it changes, and says what became of the edit**
//! (#263, H5b: `game_data` leaves the restart list).
//!
//! A game keeps tuning numbers in RON files of its own types, and lists them in `data.ron`. The game
//! registers a reader for one with [`GrimoireLiveApp::reload_data_file`]: when the file changes and
//! reads, the reader runs and the game's value is replaced; when it does not read, or a leaf is out of
//! its registered bounds, the old value stays and the game's verdict (`.live/game.ron`) names the file
//! and the leaf. The verdict keeps what the project's own reload said, and the project's reload keeps
//! what the data said. Headless, like `live_verdicts.rs`.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_grimoire::live::{GrimoireLiveApp, GrimoireLivePlugin};
use bevy_grimoire::{GrimoirePlugin, GrimoireWorld};
use bevy_grimoire_core::data_files;
use bevy_grimoire_core::descriptor::{Descriptor, Extent};
use bevy_grimoire_core::kits::{Bind, KITS_VERSION, Kits, Lattice};
use bevy_grimoire_core::library::{LIBRARY_VERSION, Library};
use bevy_grimoire_core::live::{KindVerdict, Outcome, Verdict};
use bevy_grimoire_core::map::{MAP_VERSION, Map, Placed};
use bevy_grimoire_core::project_data::ProjectData;
use bevy_grimoire_core::composition::{COMPOSITIONS_VERSION, Compositions};
use bevy_grimoire_core::rules::RULES_VERSION;

/// The game's data lives beside the project's directory, not in it (a game's `assets/data` next to its
/// `assets/grimoire`), so the project's own watch does not also reload the project when it changes.
const FILE: &str = "../data/mob.ron";
const REGISTRY: &str = "(version: 1, files: [(id: \"tuning/mob\", path: \"../data/mob.ron\", bounds: {\"speed\": (min: 0.0, max: 10.0)})])";

/// The game's own value, read from its own file.
#[derive(Resource, Debug, PartialEq)]
struct Speed(f32);

fn mob(speed: &str) -> String {
    format!("(speed: {speed}, name: \"orc\")")
}

/// **The game's reader**: its own parse of its own file, `Err` when it does not read. It needs a `name`
/// as well as a `speed`, which the registry does not bound, so it can refuse what the registry takes.
fn apply(world: &mut World, text: &str) -> Result<(), String> {
    let leaves = data_files::leaves(text)?;
    leaves.iter().find(|(path, _)| path == "name").ok_or("the file has no name")?;
    let speed = leaves.iter().find(|(path, _)| path == "speed").ok_or("the file has no speed")?;
    let speed: f32 = speed.1.parse().map_err(|e| format!("speed is not a number: {e}"))?;
    world.insert_resource(Speed(speed));
    Ok(())
}

fn piece(id: &str) -> Descriptor {
    Descriptor {
        id: id.to_owned(),
        extent: Extent { footprint: Some((1.0, 1.0)), height: Some(1.0) },
        ..Descriptor::default()
    }
}

fn map(placements: Vec<Placed>) -> Map {
    Map { version: MAP_VERSION, name: "yard".to_owned(), bounds: (100.0, 10.0, 100.0), placements, ..Map::default() }
}

fn row(id: &str, at: (f32, f32)) -> Placed {
    Placed { id: id.to_owned(), descriptor: "kit/crate".to_owned(), at, ..Placed::default() }
}

struct Project {
    grimoire: PathBuf,
}

impl Project {
    fn new(tag: &str) -> Project {
        let dir = std::env::temp_dir().join(format!("grimoire_live_data_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let grimoire = dir.join("assets/grimoire");
        std::fs::create_dir_all(grimoire.join("kit")).unwrap();
        std::fs::create_dir_all(grimoire.join("maps")).unwrap();
        std::fs::create_dir_all(grimoire.join("../data")).unwrap();
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
        p.write_map(&map(vec![row("crate_a", (1.0, 1.0))]));
        p.write("data.ron", REGISTRY);
        p.write(FILE, &mob("1.0"));
        p
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
        .insert_resource(world)
        // The game loaded its own file when it started.
        .insert_resource(Speed(1.0));
    app.reload_data_file("tuning/mob", apply);
    // The first frames take the baseline of what is on disk; nothing is decided about it.
    for _ in 0..3 {
        app.update();
    }
    app
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn speed(app: &App) -> f32 {
    app.world().resource::<Speed>().0
}

fn entry<'a>(v: &'a Verdict, file: &str) -> &'a KindVerdict {
    v.kinds.iter().find(|k| k.file == file).unwrap_or_else(|| panic!("no verdict for `{file}` in {:?}", v.kinds))
}

#[test]
fn an_edit_of_a_registered_data_file_reaches_the_running_game_and_is_said_applied_once() {
    let project = Project::new("applied");
    let mut app = app(&project);
    assert!(Verdict::read(&project.grimoire).unwrap().is_none(), "what the game started with is no decision");

    project.write(FILE, &mob("2.5"));
    settle(&mut app, 8);
    assert_eq!(speed(&app), 2.5, "the running game holds the edit with no restart");
    let v = project.verdict();
    assert_eq!(v.seq, 1, "one edit is one verdict");
    assert!(v.ok, "{}", v.message);
    let k = entry(&v, FILE);
    assert_eq!((k.kind.as_str(), k.outcome, k.field.as_deref()), ("game_data", Outcome::Applied, None), "{k:?}");

    // However many frames pass, the same bytes are not decided again.
    settle(&mut app, 12);
    assert_eq!(project.verdict().seq, 1, "an unchanged file is decided once");
    assert_eq!(speed(&app), 2.5);
}

#[test]
fn a_data_file_that_does_not_read_keeps_the_last_good_value_and_names_the_leaf() {
    let project = Project::new("refused");
    let mut app = app(&project);

    // A leaf outside its registered bounds: refused at the leaf, with the range.
    project.write(FILE, &mob("99.0"));
    settle(&mut app, 8);
    assert_eq!(speed(&app), 1.0, "a refused edit changes nothing");
    let v = project.verdict();
    assert!(!v.ok);
    let k = entry(&v, FILE);
    assert_eq!((k.kind.as_str(), k.outcome, k.field.as_deref()), ("game_data", Outcome::Refused, Some("speed")), "{k:?}");
    assert!(k.reason.contains("10"), "the refusal says the range: {k:?}");

    // A file cut off: refused, and still the last good value.
    project.write(FILE, "(speed: 3.0, na");
    settle(&mut app, 8);
    assert_eq!(speed(&app), 1.0);
    let v = project.verdict();
    assert_eq!(entry(&v, FILE).outcome, Outcome::Refused);
    assert_eq!(v.seq, 2, "each distinct bad edit is one decision");

    // An extra leaf is not a refusal: the registry bounds only what it names, and the reader takes it.
    project.write(FILE, "(speed: 3.0, name: \"orc\", extra: 1)");
    settle(&mut app, 8);
    assert_eq!(speed(&app), 3.0, "a file the registry and the reader both take is applied");

    // The next good write recovers.
    project.write(FILE, &mob("4.0"));
    settle(&mut app, 8);
    assert_eq!(speed(&app), 4.0);
    let v = project.verdict();
    assert!(v.ok, "{}", v.message);
    assert_eq!(entry(&v, FILE).outcome, Outcome::Applied);
}

#[test]
fn a_reader_that_refuses_a_file_the_registry_accepts_keeps_the_value_and_says_why() {
    let project = Project::new("reader_refuses");
    let mut app = app(&project);
    // The registry bounds `speed` only, so a file with no `name` reads; the game's own reader refuses it.
    project.write(FILE, "(speed: 2.0)");
    settle(&mut app, 8);
    assert_eq!(speed(&app), 1.0, "the reader refused, so the value is the last good one");
    let v = project.verdict();
    assert!(!v.ok);
    let k = entry(&v, FILE);
    assert_eq!((k.kind.as_str(), k.outcome), ("game_data", Outcome::Refused), "{k:?}");
    assert!(k.reason.contains("no name"), "the reader's reason is the verdict's: {k:?}");

    // A leaf that is not a number is the registry's refusal, at its path.
    project.write(FILE, "(speed: \"fast\", name: \"orc\")");
    settle(&mut app, 8);
    assert_eq!(speed(&app), 1.0);
    let v = project.verdict();
    let k = entry(&v, FILE);
    assert_eq!((k.outcome, k.field.as_deref()), (Outcome::Refused, Some("speed")), "{k:?}");
}

#[test]
fn the_data_verdict_keeps_what_the_project_said_and_the_projects_keeps_what_the_data_said() {
    let project = Project::new("both");
    let mut app = app(&project);

    // The project reloads: its files are in the verdict, and the data file is not yet.
    project.write_map(&map(vec![row("crate_a", (1.0, 1.0)), row("crate_b", (3.0, 3.0))]));
    settle(&mut app, 8);
    let v = project.verdict();
    assert_eq!(v.seq, 1);
    assert_eq!(entry(&v, "maps/yard.map.ron").outcome, Outcome::Applied);

    // The data file changes: its entry joins the project's in the next verdict.
    project.write(FILE, &mob("2.0"));
    settle(&mut app, 8);
    let v = project.verdict();
    assert_eq!(v.seq, 2);
    assert_eq!(entry(&v, "maps/yard.map.ron").outcome, Outcome::Applied, "the project's entries stay");
    assert_eq!(entry(&v, FILE).outcome, Outcome::Applied);

    // The project reloads again: the data file's entry is carried, not dropped.
    project.write_map(&map(vec![row("crate_a", (1.0, 1.0))]));
    settle(&mut app, 8);
    let v = project.verdict();
    assert_eq!(v.seq, 3);
    assert_eq!(entry(&v, FILE).outcome, Outcome::Applied, "a project reload does not forget what the data said");
}
