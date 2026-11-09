//! **Every type a project holds round-trips through the one YAML writer** (GD-01, #363).
//!
//! **The values are today's fixtures, read through RON.** Where the shipped project has a file, that is
//! it; where it has none, the file a release wrote and `tests/old_data.rs` freezes is: those hold real
//! content — a `standards.ron` with a rule in it, a `relations.ron` with a relation, a destructive bake
//! with a chunk — where `Default::default()` holds empty lists and would prove only that an `[]`
//! round-trips. Each is written by `bevy_grimoire_core::yaml::to_string` and read back by
//! `yaml::from_str`, and the value must come back equal: a round trip that loses a field, an empty list
//! or a float's last digit is a file the editor writes and cannot read.
//!
//! **Not here: the kinds whose type lives above this crate** (`Forest`, `People`, `Scripts`, a
//! `Checks`, a `TreeParams`, the editor's `EditorSettings`), because this crate cannot name the type to
//! read the fixture as. Their round trip is GD-03's (#365), in the crate that owns them.
//!
//! The written form is checked too, because the style is what a person reads and what the surgery
//! (#364) keeps: two spaces per level, block style, the type's field order, a trailing newline, and no
//! flow braces.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::composition::Compositions;
use bevy_grimoire_core::data_files::DataFiles;
use bevy_grimoire_core::destructible::{Bake, Marks};
use bevy_grimoire_core::family::Family;
use bevy_grimoire_core::heightfield::Heightfield;
use bevy_grimoire_core::kits::Kits;
use bevy_grimoire_core::library::Library;
use bevy_grimoire_core::map::{MAP_VERSION, Map};
use bevy_grimoire_core::note_agent::AgentSettings;
use bevy_grimoire_core::notes::Notes;
use bevy_grimoire_core::plan::Plan;
use bevy_grimoire_core::play::{PlayConfig};
use bevy_grimoire_core::policy::Policy;
use bevy_grimoire_core::references::References;
use bevy_grimoire_core::relations::Relations;
use bevy_grimoire_core::repetition::RepetitionRules;
use bevy_grimoire_core::rules::Rules;
use bevy_grimoire_core::rigs::Rigs;
use bevy_grimoire_core::standards::Standards;
use bevy_grimoire_core::vocab::Vocabularies;
use bevy_grimoire_core::volume::Volume;
use bevy_grimoire_core::water::{WaterField, WaterPlan};
use bevy_grimoire_core::yaml;
use serde::Serialize;
use serde::de::DeserializeOwned;

fn grimoire() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/grimoire")
}

/// A shipped file, read as the value it holds through the RON reader it is written in today.
fn shipped<T: DeserializeOwned>(rel: &str) -> T {
    let path = grimoire().join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    ron::from_str(&text).unwrap_or_else(|e| panic!("{} does not read as RON: {e}", path.display()))
}

/// **A file a release wrote and `tests/old_data.rs` freezes**, read as the value it holds: the fixture a
/// kind the shipped project has no sample of is proved against, so the value has content and not only
/// empty lists.
fn frozen<T: DeserializeOwned>(rel: &str) -> T {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/old_data").join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    ron::from_str(&text).unwrap_or_else(|e| panic!("{} does not read as RON: {e}", path.display()))
}

/// The values a project holds, each with the file its kind is written at.
fn project() -> Vec<(&'static str, String)> {
    vec![
        ("kits.yaml", round_trip::<Kits>("kits.yaml", shipped("kits.ron"))),
        ("site/library.yaml", round_trip::<Library>("site/library.yaml", shipped("site/library.ron"))),
        ("site/project.yaml", round_trip::<Policy>("site/project.yaml", shipped("site/project.ron"))),
        ("vocab.yaml", round_trip::<Vocabularies>("vocab.yaml", shipped("vocab.ron"))),
        ("compositions.yaml", round_trip::<Compositions>("compositions.yaml", shipped("compositions.ron"))),
        ("rigs.yaml", round_trip::<Rigs>("rigs.yaml", shipped("rigs.ron"))),
        ("rules.yaml", round_trip::<Rules>("rules.yaml", ron::from_str(&format!("(version: {}, blocks: [])", bevy_grimoire_core::rules::RULES_VERSION)).expect("the rules file this build writes"))),
        ("maps/bedroom.map.yaml", round_trip::<Map>("maps/bedroom.map.yaml", shipped("maps/bedroom.map.ron"))),
        // Kinds the shipped project has no sample of: a file a release wrote, frozen in tests/old_data.
        ("standards.yaml", round_trip::<Standards>("standards.yaml", frozen("core-0.3.0/standards.ron"))),
        ("repetition.yaml", round_trip::<RepetitionRules>("repetition.yaml", frozen("core-0.3.0/repetition.ron"))),
        ("maps/yard.notes.yaml", round_trip::<Notes>("maps/yard.notes.yaml", frozen("core-0.3.0/notes.ron"))),
        ("maps/yard.plan.yaml", round_trip::<Plan>("maps/yard.plan.yaml", frozen("core-0.3.0/plan.ron"))),
        ("maps/yard.references.yaml", round_trip::<References>("maps/yard.references.yaml", frozen("core-0.3.0/references.ron"))),
        ("maps/yard.relations.yaml", round_trip::<Relations>("maps/yard.relations.yaml", frozen("core-0.3.0/relations.ron"))),
        ("site/terrain/yard.yaml", round_trip::<Heightfield>("site/terrain/yard.yaml", frozen("core-0.3.0/terrain.ron"))),
        ("site/water/yard.yaml", round_trip::<WaterPlan>("site/water/yard.yaml", frozen("core-0.3.0/water_plan.ron"))),
        ("site/water/yard.field.yaml", round_trip::<WaterField>("site/water/yard.field.yaml", frozen("core-0.3.0/water_field.ron"))),
        ("site/volume/yard.yaml", round_trip::<Volume>("site/volume/yard.yaml", frozen("core-0.3.0/volume.ron"))),
        ("site/data.yaml", round_trip::<DataFiles>("site/data.yaml", frozen("core-0.3.0/data_registry.ron"))),
        ("family.yaml", round_trip::<Family>("family.yaml", frozen("core-0.3.0/family_list.ron"))),
        ("agent.yaml", round_trip::<AgentSettings>("agent.yaml", frozen("core-0.3.0/agent.ron"))),
        ("play.yaml", round_trip::<PlayConfig>("play.yaml", frozen("core-0.3.0/play.ron"))),
        ("destructible.yaml", round_trip::<Marks>("destructible.yaml", frozen("editor-3f4084c/destructible.ron"))),
        (
            "destructible/site__cart.bake.yaml",
            round_trip::<Bake>("destructible/site__cart.bake.yaml", frozen("editor-3f4084c/site__cart.bake.ron")),
        ),
        (
            "maps/yard.map.yaml",
            round_trip::<Map>("maps/yard.map.yaml", Map { version: MAP_VERSION, name: "yard".to_owned(), bounds: (10.0, 4.0, 10.0), ..Map::default() }),
        ),
    ]
}

/// Write `value` as `file` with the one writer, read it back, and answer the text it wrote.
fn round_trip<T>(file: &str, value: T) -> String
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let text = yaml::to_string(&value).unwrap_or_else(|e| panic!("{file}: {e}"));
    let back: T = yaml::from_str(&text, file).unwrap_or_else(|e| panic!("{file}: {}", e.line()));
    assert_eq!(back, value, "{file} did not round-trip through YAML:\n{text}");
    text
}

#[test]
fn every_type_a_project_holds_round_trips_through_the_one_writer() {
    let files = project();
    assert!(files.len() >= 20, "the list has lost files: {}", files.len());
    for (file, text) in &files {
        assert!(text.ends_with('\n'), "{file} does not end in a newline:\n{text}");
        assert!(!text.ends_with("\n\n"), "{file} ends in a blank line:\n{text}");
    }
}

#[test]
fn written_files_come_out_in_one_style() {
    for (file, text) in project() {
        // Block style, not flow: a project file is read by a person, one field a line. The one
        // exception is an EMPTY collection, which a block cannot write and which is `[]` or `{}`.
        for (n, line) in text.lines().enumerate() {
            let flow = line.split_once(": ").map_or(line, |(_, value)| value).trim();
            assert!(
                flow.is_empty() || matches!(flow, "[]" | "{}") || (!line.contains('{') && !line.contains('}')),
                "{file}:{} is flow style: {line}",
                n + 1
            );
            assert!(!line.contains('\t'), "{file}:{} holds a tab: {line}", n + 1);
            let indent = line.len() - line.trim_start().len();
            assert_eq!(indent % 2, 0, "{file}:{} is indented {indent} spaces, not two per level: {line}", n + 1);
        }
    }
}

#[test]
fn the_writer_keeps_the_types_field_order() {
    // A map's `version` and `name` are its first two fields, in that order, and the writer does not
    // sort them: the file reads the way the type is declared, so a diff of one field is one field.
    let map = yaml::to_string(&Map { version: MAP_VERSION, name: "yard".to_owned(), ..Map::default() }).unwrap();
    let first = map.lines().next().unwrap_or_default();
    assert_eq!(first, format!("version: {MAP_VERSION}"), "{map}");
    assert!(map.lines().nth(1).is_some_and(|l| l == "name: yard"), "{map}");
}

#[test]
fn a_string_is_quoted_only_when_yaml_needs_it() {
    let quote = |s: &str| {
        let doc = Map { version: MAP_VERSION, name: s.to_owned(), ..Map::default() };
        yaml::to_string(&doc).unwrap()
    };
    let yard = quote("yard");
    assert!(yard.contains("name: yard\n"), "a plain word is not quoted:\n{yard}");
    let ambiguous = quote("no");
    assert!(ambiguous.contains("name: no\n"), "a string YAML 1.2 reads as a word is not quoted for the sake of YAML 1.1:\n{ambiguous}");
    let needs = quote("a: b");
    assert!(needs.contains("name: 'a: b'\n"), "a string with YAML syntax in it is quoted:\n{needs}");
    let empty = quote("");
    assert!(empty.contains("name: ''\n"), "an empty string says so:\n{empty}");
}

#[test]
fn a_value_that_is_not_yaml_at_all_is_refused_with_the_line() {
    let problem = yaml::from_str::<Map>("version: 9\nname: [unclosed\n", "maps/yard.map.yaml")
        .expect_err("an unclosed flow sequence does not read");
    assert!(problem.line().contains("maps/yard.map.yaml"), "{}", problem.line());
    assert!(problem.why.as_deref().is_some_and(|w| w.contains("line")), "a refusal says where: {}", problem.line());
    assert!(problem.fix.is_some(), "{}", problem.line());
}
