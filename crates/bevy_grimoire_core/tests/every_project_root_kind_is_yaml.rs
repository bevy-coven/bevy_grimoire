//! **Every project-root kind is YAML, read by the one reader and named `.yaml`** (GD-03, #365).
//!
//! The cutover's second batch: the files that sit at `assets/grimoire/` on their own — `kits.yaml`,
//! `vocab.yaml`, `compositions.yaml`, `rules.yaml`, `standards.yaml`, `repetition.yaml`, `rigs.yaml` and
//! `family.yaml` — plus the two at the game's root, `data.yaml` in a kit and the destructive pair. Each was
//! read by `ron::from_str` and written by `ron::ser::to_string_pretty`; each reads YAML and writes through
//! [`bevy_grimoire_core::yaml`] now, and each kind's path in `reload::KINDS` ends `.yaml`.
//!
//! **This is the half the reload table cannot prove.** A table row is a string, and a string is right in a
//! table and wrong in the reader: the row can say `kits.yaml` while `Kits::parse` still calls `ron`. So every
//! assertion here writes the kind's YAML through the type's own writer, reads it back through the type's own
//! reader, and then hands the *same text* to the RON reader — which must refuse it, because a type that
//! still took RON would take both and there would be two paths where the cutover wants one.
//!
//! The value proved in each case is not `Default::default()`: a round trip of an empty list proves only that
//! `[]` survives. Each holds a field the writer has to spell (a tuple, an `Option`, an enum) and a list with
//! something in it.

use bevy_grimoire_core::composition::Compositions;
use bevy_grimoire_core::destructible::{Bake, Marks, DESTRUCTIBLE_VERSION};
use bevy_grimoire_core::family::Family;
use bevy_grimoire_core::kits::Kits;
use bevy_grimoire_core::library::Library;
use bevy_grimoire_core::policy::Policy;
use bevy_grimoire_core::repetition::RepetitionRules;
use bevy_grimoire_core::rigs::Rigs;
use bevy_grimoire_core::rules::Rules;
use bevy_grimoire_core::standards::Standards;
use bevy_grimoire_core::vocab::Vocabularies;
use bevy_grimoire_core::yaml;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// **One kind proved**: the path its pattern names, and the value a person's file holds.
fn cases() -> Vec<(&'static str, String)> {
    // The shipped project is the value for every kind it carries, read through the type the editor reads it
    // with today — so the content is the real one rather than a fixture built to pass.
    let grimoire = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/grimoire");
    let shipped = |rel: &str| {
        std::fs::read_to_string(grimoire.join(rel))
            .unwrap_or_else(|e| panic!("{}: {e}", grimoire.join(rel).display()))
    };
    vec![
        ("kits.yaml", as_yaml::<Kits>(&shipped("kits.yaml"))),
        ("vocab.yaml", as_yaml::<Vocabularies>(&shipped("vocab.yaml"))),
        ("compositions.yaml", as_yaml::<Compositions>(&shipped("compositions.yaml"))),
        ("rigs.yaml", as_yaml::<Rigs>(&shipped("rigs.yaml"))),
        ("site/library.yaml", as_yaml::<Library>(&shipped("site/library.yaml"))),
        ("site/project.yaml", as_yaml::<Policy>(&shipped("site/project.yaml"))),
        // These the shipped project has no sample of, so each is a value with a field that has to be spelled.
        ("rules.yaml", rules()),
        ("standards.yaml", standards()),
        ("repetition.yaml", repetition()),
        ("family.yaml", family()),
        ("site/data.yaml", data()),
        ("destructible.yaml", marks()),
        ("destructible/site__cart.bake.yaml", bake()),
    ]
}

/// **The shipped file, read and written again through the one reader and writer**, so what the test checks
/// is this build's own output for the value the project holds — not the bytes on disk, which a person may
/// have edited.
fn as_yaml<T: DeserializeOwned + Serialize>(yaml_text: &str) -> String {
    let value: T =
        yaml::from_str(yaml_text, "a shipped file").unwrap_or_else(|p| panic!("the shipped file does not read: {}", p.line()));
    yaml::to_string(&value).unwrap_or_else(|p| panic!("{}", p.line()))
}

fn rules() -> String {
    // `Rules` has no `Default` — a rule set is authored, not empty — so the value is one block, which is
    // also the smallest thing that makes the writer spell a tuple (`size: (2, 2)`) and an `Option`.
    let value = Rules {
        version: bevy_grimoire_core::rules::RULES_VERSION,
        blocks: vec![bevy_grimoire_core::rules::BlockRules {
            example: "yard".to_owned(),
            regions: vec!["town/road".to_owned()],
            cell: 1.0,
            size: (2, 2),
            border: Some("town/wall".to_owned()),
            no_empty: true,
            weights: Vec::new(),
            footprints: Vec::new(),
            attempts: 8,
            seed: 7,
        }],
    };
    yaml::to_string(&value).unwrap_or_else(|p| panic!("{}", p.line()))
}

fn standards() -> String {
    yaml::to_string(&Standards::default()).unwrap_or_else(|p| panic!("{}", p.line()))
}

fn repetition() -> String {
    yaml::to_string(&RepetitionRules::default()).unwrap_or_else(|p| panic!("{}", p.line()))
}

fn family() -> String {
    // A crate named, because a list of none round-trips as `[]` and proves only that an empty sequence
    // survives — which is the one thing every writer already does.
    let value = Family { version: bevy_grimoire_core::family::FAMILY_VERSION, crates: vec!["bevy_elements".to_owned()] };
    yaml::to_string(&value).unwrap_or_else(|p| panic!("{}", p.line()))
}

fn data() -> String {
    // **A registry is written by the editor, and `DataFiles` is built by the type's own reader** (it holds
    // the folder its paths hang from, which a value cannot invent). So the value is `parse`d from the
    // spelling, and `parse` reads YAML from this commit on — the assertion that it does is the one below.
    let text = format!(
        "version: {}\nfiles:\n  - id: tuning/mob\n    path: ../data/mob.yaml\n",
        bevy_grimoire_core::data_files::REGISTRY_VERSION
    );
    let value = bevy_grimoire_core::data_files::DataFiles::parse(&text)
        .unwrap_or_else(|e| panic!("a registry written as YAML does not read: {e}"));
    yaml::to_string(&value).unwrap_or_else(|p| panic!("{}", p.line()))
}

fn marks() -> String {
    let value = Marks { version: DESTRUCTIBLE_VERSION, pieces: vec!["site/cart".to_owned()] };
    yaml::to_string(&value).unwrap_or_else(|p| panic!("{}", p.line()))
}

fn bake() -> String {
    let value = Bake {
        version: DESTRUCTIBLE_VERSION,
        key: 7,
        chunks: vec![bevy_grimoire_core::destructible::Chunk {
            points: vec![[0.0, 0.0, 0.0]],
            faces: vec![vec![0, 0, 0]],
        }],
    };
    yaml::to_string(&value).unwrap_or_else(|p| panic!("{}", p.line()))
}

/// **The kind of a project-relative path**, as the reload table answers it: a `Base::Grimoire` row hangs from
/// `assets/grimoire`, a `Base::Root` one from the game root.
fn kind_at(path: &str) -> &'static bevy_grimoire_core::reload::FileKind {
    use bevy_grimoire_core::reload::{Base, kind_of};
    let under = path.strip_prefix("assets/grimoire/").unwrap_or(path);
    kind_of(Base::Grimoire, under)
        .or_else(|| kind_of(Base::Root, path))
        .unwrap_or_else(|| panic!("`{path}` is no kind's path in reload::KINDS"))
}

#[test]
fn every_project_root_kind_is_named_dot_yaml_by_the_reload_table() {
    for (path, _) in cases() {
        let kind = kind_at(path);
        assert!(
            kind.pattern.ends_with(".yaml"),
            "`{}`'s pattern is `{}`, and the cutover names every project file `.yaml`",
            kind.id,
            kind.pattern
        );
    }
}

#[test]
fn every_project_root_kind_reads_the_yaml_its_writer_writes_and_refuses_the_ron_it_no_longer_reads() {
    for (path, text) in cases() {
        let kind = kind_at(path);
        // **A file this build writes is a file it reads**, through the kind's own reader — which is what
        // `reload_check` hands the editor's watcher, the game's reload and an agent's `grimoire_state`.
        let verdict = bevy_grimoire_core::reload_check::check(kind, path, &text);
        assert!(
            matches!(verdict, bevy_grimoire_core::reload_check::Check::Valid),
            "`{}` did not read its own writer's YAML at `{path}`: {verdict:?}\n--- text ---\n{text}",
            kind.id
        );
        // **One spelling.** The type's RON reader no longer exists, so the same text must not read as RON:
        // a reader that took both would be the second path the cutover exists to remove.
        assert!(
            ron::from_str::<ron::Value>(&text).is_err(),
            "`{}`'s YAML reads as RON too, so the cutover left a second spelling:\n{text}",
            kind.id
        );
    }
}

#[test]
fn a_project_holding_any_root_kind_as_ron_is_refused_naming_that_file() {
    // The refusal is per kind and names the file, so it cannot be satisfied by one kind moving: each file
    // below is written alone, and each must be the one the refusal names.
    for (path, _) in cases() {
        let root = std::env::temp_dir().join(format!(
            "grimoire-root-refusal-{}-{}",
            path.replace(['/', '.'], "-"),
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        // **A kind's base decides where its file lives**: a `Base::Root` kind (`family.yaml`,
        // `destructible.yaml`) hangs from the project root, every other from `assets/grimoire`.
        let kind = kind_at(path);
        let base = match kind.base {
            bevy_grimoire_core::reload::Base::Root => root.clone(),
            bevy_grimoire_core::reload::Base::Grimoire => root.join("assets/grimoire"),
        };
        let ron = base.join(path.trim_end_matches(".yaml").to_owned() + ".ron");
        std::fs::create_dir_all(ron.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(&ron, "(version: 1)").unwrap_or_else(|e| panic!("{e}"));

        let problem = bevy_grimoire_core::migrate::leftover(&root, bevy_grimoire_core::migrate::COMMAND)
            .unwrap_or_else(|p| panic!("{}", p.line()))
            .unwrap_or_else(|| panic!("`{path}` as RON is a file nothing reads, so an open must refuse it"));
        // **The refusal names the file it met**, which is the `.ron` spelling, as the project sees it.
        let ron = path.trim_end_matches(".yaml").to_owned() + ".ron";
        let named = match kind.base {
            bevy_grimoire_core::reload::Base::Root => ron.clone(),
            bevy_grimoire_core::reload::Base::Grimoire => {
                format!("{}/{ron}", bevy_grimoire_core::migrate::GRIMOIRE_DIR)
            }
        };
        assert!(problem.line().contains(&named), "the refusal names the file it met (`{named}`): {}", problem.line());
        assert!(
            problem.line().contains(bevy_grimoire_core::migrate::COMMAND),
            "the refusal names the command that fixes it: {}",
            problem.line()
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
