//! **One JSON Schema per file kind, generated from its type** (GD-04, #366).
//!
//! An outside editor gets completion and hover from a schema, so each kind's schema is *generated from
//! the Rust type the reader reads*: a schema written by hand could describe a file no reader accepts, and
//! the two would drift silently.
//!
//! The test that matters is the **validator's**, not a comparison against the type: every kind's fixture
//! is checked against its generated schema, and a copy of that fixture with one field wrong is checked
//! to fail — at the same field path GD-01's `Problem` names. A schema that a wrong file passes is a
//! schema an editor will offer wrong completions from, and only a validator can tell.
//!
//! The rest is what an outside editor needs to be useful: a field's doc comment becomes its
//! `description` (which is what hover shows), and a field that names another file's value carries
//! `x-dark-factory-ref: <kind>`, so navigation can follow it.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::schema;
use serde_json::Value;

fn grimoire() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/grimoire")
}

fn frozen(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/old_data").join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// **A frozen file as YAML**, through the migration — the one conversion that has been proved lossless
/// (`tests/a_project_migrates_from_ron_to_yaml.rs`). A frozen file is never edited; it is read as it was
/// written and converted here.
fn frozen_yaml(rel: &str, project_name: &str) -> String {
    let (_, text) = bevy_grimoire_migrate::written_as_yaml(
        project_name,
        &frozen(rel),
        bevy_grimoire_migrate::COMMAND,
    )
    .unwrap_or_else(|p| panic!("{}", p.line()));
    text
}

fn shipped(rel: &str) -> String {
    let path = grimoire().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// **Every kind whose type this crate owns, and its fixture as YAML.**
///
/// The fixture is this crate's own writer over a value read from RON, so it is exactly the value the
/// round trip proves (`tests/yaml_round_trips_every_project_type.rs`), in the format the schema
/// describes.
fn kinds() -> Vec<(&'static str, String)> {
    vec![
        ("kits", value_of::<bevy_grimoire_core::kits::Kits>(&shipped("kits.yaml"))),
        ("library", value_of::<bevy_grimoire_core::library::Library>(&shipped("site/library.yaml"))),
        ("policy", value_of::<bevy_grimoire_core::policy::Policy>(&shipped("site/project.yaml"))),
        ("vocab", value_of::<bevy_grimoire_core::vocab::Vocabularies>(&shipped("vocab.yaml"))),
        ("compositions", value_of::<bevy_grimoire_core::composition::Compositions>(&shipped("compositions.yaml"))),
        ("rigs", value_of::<bevy_grimoire_core::rigs::Rigs>(&shipped("rigs.yaml"))),
        ("rules", value_of::<bevy_grimoire_core::rules::Rules>(&frozen_yaml("core-0.3.0/rules.ron", "rules.ron"))),
        ("map", value_of::<bevy_grimoire_core::map::Map>(&shipped("maps/bedroom.map.yaml"))),
        ("standards", value_of::<bevy_grimoire_core::standards::Standards>(&frozen_yaml("core-0.3.0/standards.ron", "standards.ron"))),
        ("repetition", value_of::<bevy_grimoire_core::repetition::RepetitionRules>(&frozen_yaml("core-0.3.0/repetition.ron", "repetition.ron"))),
        ("notes", value_of::<bevy_grimoire_core::notes::Notes>(&frozen_yaml("core-0.3.0/notes.ron", "maps/yard.notes.ron"))),
        ("plan", value_of::<bevy_grimoire_core::plan::Plan>(&frozen_yaml("core-0.3.0/plan.ron", "maps/yard.plan.ron"))),
        ("board", value_of::<bevy_grimoire_core::board::Board>(&frozen_yaml("core-0.3.0/references.ron", "maps/yard.references.ron"))),
        ("relations", value_of::<bevy_grimoire_core::relations::Relations>(&frozen_yaml("core-0.3.0/relations.ron", "maps/yard.relations.ron"))),
        ("terrain", value_of::<bevy_grimoire_core::heightfield::Heightfield>(&frozen_yaml("core-0.3.0/terrain.ron", "site/terrain/yard.ron"))),
        ("water_plan", value_of::<bevy_grimoire_core::water::WaterPlan>(&frozen_yaml("core-0.3.0/water_plan.ron", "site/water/yard.ron"))),
        ("water_field", value_of::<bevy_grimoire_core::water::WaterField>(&frozen_yaml("core-0.3.0/water_field.ron", "site/water/yard.field.ron"))),
        ("volume", value_of::<bevy_grimoire_core::volume::Volume>(&frozen_yaml("core-0.3.0/volume.ron", "site/volume/yard.ron"))),
        ("data_registry", value_of::<bevy_grimoire_core::data_files::DataFiles>(&frozen_yaml("core-0.3.0/data_registry.ron", "site/data.ron"))),
        ("family_list", value_of::<bevy_grimoire_core::family::Family>(&frozen_yaml("core-0.3.0/family_list.ron", "family.ron"))),
        ("agent", value_of::<bevy_grimoire_core::note_agent::AgentSettings>(&frozen_yaml("core-0.3.0/agent.ron", "agent.ron"))),
        ("play", value_of::<bevy_grimoire_core::play::PlayConfig>(&frozen_yaml("core-0.3.0/play.ron", "play.ron"))),
        ("destructible_marks", value_of::<bevy_grimoire_core::destructible::Marks>(&frozen_yaml("editor-3f4084c/destructible.ron", "destructible.ron"))),
        ("destructible_bake", value_of::<bevy_grimoire_core::destructible::Bake>(&frozen_yaml("editor-3f4084c/site__cart.bake.ron", "destructible/site__cart.bake.ron"))),
    ]
}

/// **A fixture read by its type and written again by the one writer**, so what the schema is asked about is
/// this build's own output for the value the project holds — the same document the editor writes.
fn value_of<T>(yaml: &str) -> String
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let value: T = bevy_grimoire_core::yaml::from_str(yaml, "a fixture")
        .unwrap_or_else(|p| panic!("the fixture does not read: {}", p.line()));
    bevy_grimoire_core::yaml::to_string(&value).unwrap_or_else(|p| panic!("{}", p.line()))
}

/// **The fixture as the JSON an outside editor sees**, read by the same plain YAML-to-JSON reader an
/// editor's extension uses — **never through the Rust type**, which would hand back the value the schema
/// is checked against and make the check agree with itself. A kind whose fixture carries a tag is in
/// [`TAGGED`] and never reaches this.
fn as_json(_kind: &str, text: &str) -> Value {
    serde_yaml_ng::from_str::<Value>(text).unwrap_or_else(|e| panic!("the fixture is not plain YAML-to-JSON: {e}"))
}

#[test]
fn every_kind_has_a_schema_generated_from_its_type() {
    let all = schema::all();
    assert_eq!(all.len(), schema::KINDS.len(), "every kind gets a schema");
    for kind in schema::KINDS {
        let generated = schema::for_kind(kind).unwrap_or_else(|e| panic!("{kind}: {}", e.line()));
        assert!(generated["properties"]["version"].is_object(), "{kind}: a versioned kind's schema has a `version` the file states");
    }
}

#[test]
fn every_kinds_fixture_validates_and_a_wrong_field_does_not() {
    for (kind, text) in kinds().into_iter().filter(|(kind, _)| !TAGGED.iter().any(|(id, _)| id == kind)) {
        let generated = schema::for_kind(kind).unwrap_or_else(|e| panic!("{kind}: {}", e.line()));
        let validator = jsonschema::validator_for(&generated).unwrap_or_else(|e| panic!("{kind}: its schema is not a JSON Schema: {e}"));
        let fixture = as_json(kind, &text);
        assert!(
            validator.is_valid(&fixture),
            "{kind}: its own fixture does not validate against its schema: {:?}",
            validator.iter_errors(&fixture).map(|e| (e.instance_path().to_string(), e.to_string())).collect::<Vec<_>>()
        );

        // One field wrong. The path the validator names is the same path GD-01's `Problem` names,
        // because both read the same file.
        let mut doc = as_json(kind, &text);
        let (leaf, path) = first_leaf_refusing_a_string(&generated, &mut doc)
            .unwrap_or_else(|| panic!("{kind}: its fixture has no leaf the schema says is not a string"));
        *leaf = Value::from("not the kind this field is");
        // The validator names a JSON Pointer (`/placements/0/turn_deg`); the refusal names the same field
        // in the file's own spelling. Both are this path, so the pointer is built from it.
        let pointer = format!("/{}", path.replace('.', "/").replace(['[', ']'], "/").replace("//", "/").trim_end_matches('/'));
        let errors: Vec<String> = validator.iter_errors(&doc).map(|e| e.instance_path().to_string()).collect();
        assert!(
            errors.iter().any(|named| named == &pointer),
            "{kind}: a wrong value at `{path}` ({pointer}) was not refused there; the validator named {errors:?}, so the schema accepts what the reader refuses"
        );
    }
}

/// **A leaf of a JSON document whose schema does not accept a string**, and its path in the file's own
/// spelling (`placements[0].turn_deg`).
///
/// The wrong value has to be one the field *cannot* hold: writing a string into a `String` field is what
/// the schema is supposed to accept, so such a leaf is skipped.
///
/// A `$ref` is followed through the schema's own `$defs`, which is where `schemars` puts a nested type.
fn first_leaf_refusing_a_string<'a>(root_schema: &Value, value: &'a mut Value) -> Option<(&'a mut Value, String)> {
    fn walk(root: &Value, schema: &Value, value: &mut Value, at: &str) -> Option<String> {
        match value {
            Value::Object(map) => {
                for (key, child) in map.iter_mut() {
                    let path = if at.is_empty() { key.clone() } else { format!("{at}.{key}") };
                    let child_schema = schema.get("properties").and_then(|p| p.get(key)).map(|s| deref(root, s));
                    match child {
                        Value::Object(_) | Value::Array(_) => {
                            if let Some(found) = child_schema.as_ref().and_then(|s| walk(root, s, child, &path)) {
                                return Some(found);
                            }
                        }
                        _ => {
                            // A leaf the schema says is not a string takes the wrong value.
                            if child_schema.is_some_and(|s| !accepts_string(s)) {
                                return Some(path);
                            }
                        }
                    }
                }
                None
            }
            Value::Array(items) => {
                let items_schema = schema.get("items").map(|s| deref(root, s));
                for (index, child) in items.iter_mut().enumerate() {
                    let path = format!("{at}[{index}]");
                    match child {
                        Value::Object(_) | Value::Array(_) => {
                            if let Some(found) = items_schema.as_ref().and_then(|s| walk(root, s, child, &path)) {
                                return Some(found);
                            }
                        }
                        _ => {
                            if items_schema.is_some_and(|s| !accepts_string(s)) {
                                return Some(path);
                            }
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
    let path = walk(root_schema, root_schema, value, "")?;
    let leaf = navigate(value, &path)?;
    Some((leaf, path))
}

/// The schema a `$ref` names, from the root's `$defs`; anything else is itself.
fn deref<'s>(root: &'s Value, schema: &'s Value) -> &'s Value {
    match schema.get("$ref").and_then(|r| r.as_str()) {
        Some(reference) => {
            let name = reference.rsplit('/').next().unwrap_or_default();
            root.get("$defs").and_then(|defs| defs.get(name)).unwrap_or(schema)
        }
        None => schema,
    }
}

/// Whether the schema's `type` allows a string.
fn accepts_string(schema: &Value) -> bool {
    match schema.get("type") {
        Some(Value::String(kind)) => kind == "string",
        Some(Value::Array(kinds)) => kinds.iter().any(|k| k == "string"),
        // No `type` at all: a nested object or an enum the walk descends into or skips.
        _ => false,
    }
}

/// The value at `path`, through objects and `[n]` indices.
fn navigate<'a>(value: &'a mut Value, path: &str) -> Option<&'a mut Value> {
    let mut current = value;
    for segment in path.split('.') {
        let (name, indexes) = match segment.find('[') {
            Some(at) => (&segment[..at], &segment[at..]),
            None => (segment, ""),
        };
        if !name.is_empty() {
            current = current.get_mut(name)?;
        }
        for part in indexes.split('[').filter(|p| !p.is_empty()) {
            let index: usize = part.trim_end_matches(']').parse().ok()?;
            current = current.get_mut(index)?;
        }
    }
    Some(current)
}

#[test]
fn a_fields_doc_comment_becomes_its_description() {
    let map = schema::for_kind("map").unwrap();
    let name = &map["properties"]["name"];
    let described = name["description"].as_str().unwrap_or_else(|| panic!("a map's `name` has a doc comment, so its schema describes it: {map}"));
    assert!(described.contains("snake_case") || described.contains("called"), "the description is the doc comment: {described}");
}

#[test]
fn a_field_that_names_another_files_value_carries_its_kind() {
    // A placement's `descriptor` names a piece, and an editor's go-to-definition follows that kind.
    let map = schema::for_kind("map").unwrap();
    let descriptor = &map["$defs"]["Placed"]["properties"]["descriptor"];
    assert_eq!(descriptor["x-dark-factory-ref"], Value::from("library"), "a placement's descriptor names a library piece: {descriptor}");
    // A stamp names a composition.
    let stamp = &map["$defs"]["Stamped"];
    assert_eq!(stamp["properties"]["of"]["x-dark-factory-ref"], Value::from("compositions"), "{stamp}");
}

#[test]
fn a_schema_says_which_file_it_is_written_to_and_that_a_hand_edit_is_overwritten() {
    for kind in schema::KINDS {
        let generated = schema::for_kind(kind).unwrap();
        assert_eq!(generated["x-dark-factory"], Value::from(*kind), "a schema names its kind: {kind}");
        assert!(generated["$comment"].as_str().is_some_and(|c| c.contains("generated")), "a schema says it is generated, so a hand edit is not kept: {kind}");
    }
}

#[test]
fn the_schemas_are_written_where_an_outside_editor_looks_and_carry_their_hint() {
    // The scratch path says which test wrote it, so two runs cannot collide.
    let root = std::env::temp_dir().join(format!("schema_describes_every_file_kind-{}", std::process::id()));
    let written = schema::write_all(&root).unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(written.len(), schema::KINDS.len(), "one file per kind");
    for path in &written {
        assert!(path.exists(), "{} was written", path.display());
        assert!(path.to_string_lossy().contains(".dark_factory/schema/"), "{} is in the folder an editor looks in", path.display());
        assert!(path.to_string_lossy().ends_with(".schema.json"), "{} has the extension an editor discovers", path.display());
        let text = std::fs::read_to_string(path).expect("readable");
        let value: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(value["$comment"].as_str().is_some_and(|c| c.contains("generated")), "{} says it is generated", path.display());
    }

    // **Every YAML file the editor writes starts with the hint**, so an outside editor finds the schema.
    // **The hint is relative to the file that carries it**, because a YAML language server resolves it
    // from that file's own folder: a map under `assets/grimoire/maps/` is three levels down, and a file
    // at the root is not.
    // The path is the file's, relative to the project root, which is where the `.dark_factory/` folder
    // sits: a map under `assets/grimoire/maps/` is three folders down.
    assert_eq!(
        schema::header("map", "assets/grimoire/maps/town.map.yaml"),
        "# yaml-language-server: $schema=../../../.dark_factory/schema/map.schema.json"
    );
    assert_eq!(
        schema::header("family_list", "family.yaml"),
        "# yaml-language-server: $schema=.dark_factory/schema/family_list.schema.json"
    );
    let hinted = schema::with_header("map", "assets/grimoire/maps/town.map.yaml", "version: 9\n");
    assert!(hinted.starts_with(&schema::header("map", "assets/grimoire/maps/town.map.yaml")), "{hinted}");
    assert_eq!(
        schema::with_header("map", "assets/grimoire/maps/town.map.yaml", &hinted),
        hinted,
        "a file that has the hint gains no second one"
    );

    // A game's own reflected schema is written where an editor looks, and text that is not JSON is
    // refused rather than written.
    let game = schema::write_game(&root, "{\"title\": \"a game's components\"}").unwrap_or_else(|e| panic!("{}", e.line()));
    assert!(game.to_string_lossy().ends_with("game.schema.json"), "{}", game.display());
    let written_game: Value = serde_json::from_str(&std::fs::read_to_string(&game).expect("readable")).expect("JSON");
    assert_eq!(written_game["title"], Value::from("a game's components"));
    assert!(schema::write_game(&root, "not json").is_err(), "a schema an editor cannot read is refused");

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}

/// **The kinds whose YAML carries an enum tag**, checked in: `!Near`, `!Sphere`, `!Placement`.
///
/// A JSON Schema describes JSON, where a tagged enum is an object — `{"Near": {"to": …, "within": …}}` —
/// while `serde_yaml_ng` writes an enum as a YAML tag, so the two forms are one value and not one
/// document. Making them one means an internally tagged enum (`#[serde(tag = "kind")]`), which these
/// types cannot take: `relations::Kind` has both struct variants and tuple variants (`Faces(String)`),
/// and an internally tagged enum supports neither a tuple variant nor a newtype of anything but a
/// struct. Rewriting seven file formats to reach one form is GD-03's (#365) call and not this one's, so
/// **these kinds are not validated here and the gap is counted**, so it cannot grow unseen.
const TAGGED: &[(&str, &str)] = &[
    ("library", "a `Descriptor`'s `mount`, a `Mount`"),
    ("policy", "a `Patch`'s `match`, a `Match`"),
    ("compositions", "a `Composition`'s `envelope`, an `Envelope`"),
    ("rigs", "a slot's `playback`, a `Playback`"),
    ("notes", "a note's `targets`, a `Target`"),
    ("relations", "a relation's `kind`, a `Kind`"),
    ("volume", "an edit's `shape`, a `Shape`"),
];

/// **A literal, not `TAGGED.len()`**: a number that is its own measure can never fail.
const TAGGED_MAX: usize = 7;

#[test]
fn the_tagged_list_only_shrinks() {
    assert_eq!(TAGGED.len(), TAGGED_MAX, "the checked-in tagged list has {} kinds and the ratchet says {TAGGED_MAX}: lower both when a kind's enum becomes one JSON can spell", TAGGED.len());
    // The list is what it says: a listed kind's fixture is one the plain YAML reader cannot take, and an
    // unlisted kind's is one it can.
    for (kind, text) in kinds() {
        let tagged = serde_yaml_ng::from_str::<Value>(&text).is_err();
        let listed = TAGGED.iter().any(|(id, _)| *id == kind);
        assert_eq!(tagged, listed, "`{kind}` is {}tagged and {} on the list: the two must agree, or the gap this test records is not the gap it counts", if tagged { "" } else { "not " }, if listed { "" } else { "not " });
    }
}

#[test]
fn the_schema_files_are_found_by_the_reload_table_at_the_path_they_are_written_to() {
    // **The folder `write_all` writes to and the folder the reload table watches are the same one.** A
    // kind nobody can classify is a file the editor never notices, and nothing else in this file would
    // say so: the schema writes and the table are two statements of one path.
    let root = std::env::temp_dir().join(format!("schema-reload-{}", std::process::id()));
    let written = schema::write_all(&root).expect("the schemas write");
    for path in written {
        let relative = path.to_string_lossy().rsplit_once("/schema/").map(|(_, rest)| format!("schema/{rest}"));
        let relative = relative.unwrap_or_else(|| panic!("{} is under a `schema/` folder", path.display()));
        // The path relative to the **game root**, which is what a `Base::Root` kind is matched against.
        let from_root = format!(".dark_factory/{relative}");
        let kind = bevy_grimoire_core::reload::kind_of(bevy_grimoire_core::reload::Base::Root, &from_root)
            .unwrap_or_else(|| panic!("`{from_root}` is not a kind the reload table knows"));
        assert_eq!(kind.id, "schema", "`{from_root}` is a schema file");
    }
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!("schema-reload-{}", std::process::id())));
}

#[test]
fn a_wrong_value_is_refused_at_the_same_path_by_the_reader_and_by_the_schema() {
    // **GD-01's reader and the JSON Schema must name one path.** They are two readers of one file, and a
    // field one calls `placements[0].turn_deg` while the other calls `/placements/0/yaw` is a person
    // sent to two places. So the edited document goes through `yaml::from_str` too, and the two paths
    // are compared, not each against an expectation of its own. A tagged fixture is in [`TAGGED`] and
    // never reaches here: `serde_yaml_ng` reads a tag only into the type, so there is no plain YAML to
    // compare on both sides.
    for (kind, text) in kinds().into_iter().filter(|(kind, _)| !TAGGED.iter().any(|(id, _)| id == kind)) {
        let generated = schema::for_kind(kind).unwrap_or_else(|e| panic!("{kind}: {}", e.line()));
        let validator = jsonschema::validator_for(&generated).unwrap_or_else(|e| panic!("{kind}: {e}"));
        let mut doc = as_json(kind, &text);
        let (leaf, path) = first_leaf_refusing_a_string(&generated, &mut doc)
            .unwrap_or_else(|| panic!("{kind}: no leaf the schema says is not a string"));
        *leaf = Value::from("not the kind this field is");

        // What the schema says, as the file's own spelling.
        let from_pointer = path.replace('.', "/").replace(['[', ']'], "/").replace("//", "/");
        let pointer = format!("/{}", from_pointer.trim_end_matches('/'));
        let named: Vec<String> = validator.iter_errors(&doc).map(|e| e.instance_path().to_string()).collect();
        assert!(named.iter().any(|p| p == &pointer), "{kind}: the schema did not refuse `{path}` ({pointer}): {named:?}");

        // What GD-01's reader says of the same wrong document. It is written as **text**, not through
        // `yaml_surgery`: the surgery refuses a value of another kind, which is right for an editor and
        // the wrong instrument for asking what the reader makes of a file a person wrote by hand.
        let edited = wrong_file(&doc);
        // The kind's **own type**, not `Value`: a `Value` accepts a string in an `f32` field, so reading
        // it as anything would prove only that YAML is YAML.
        let reader_field = reader_field(kind, &edited)
            .unwrap_or_else(|| panic!("{kind}: the reader accepted `{path}` set to a string\n{edited}"));
        assert_eq!(reader_field.as_str(), path.as_str(), "{kind}: the reader named `{reader_field}` and the schema named `{path}`");
    }
}

/// **The field a kind's own reader refuses `text` at**, or `None` when it reads — the reader's half of
/// "fails at the same path the schema names".
///
/// The types are named here, in the test, rather than through a door in the crate: a public helper whose
/// only caller is a test is part of the published API for no reason.
fn reader_field(kind: &str, text: &str) -> Option<String> {
    fn at<T: serde::de::DeserializeOwned>(text: &str) -> Option<String> {
        bevy_grimoire_core::yaml::from_str::<T>(text, "a project file").err().and_then(|p| p.where_.and_then(|w| w.field))
    }
    match kind {
        "kits" => at::<bevy_grimoire_core::kits::Kits>(text),
        "vocab" => at::<bevy_grimoire_core::vocab::Vocabularies>(text),
        "library" => at::<bevy_grimoire_core::library::Library>(text),
        "policy" => at::<bevy_grimoire_core::policy::Policy>(text),
        "rules" => at::<bevy_grimoire_core::rules::Rules>(text),
        "compositions" => at::<bevy_grimoire_core::composition::Compositions>(text),
        "standards" => at::<bevy_grimoire_core::standards::Standards>(text),
        "repetition" => at::<bevy_grimoire_core::repetition::RepetitionRules>(text),
        "data_registry" => at::<bevy_grimoire_core::data_files::DataFiles>(text),
        "rigs" => at::<bevy_grimoire_core::rigs::Rigs>(text),
        "map" => at::<bevy_grimoire_core::map::Map>(text),
        "notes" => at::<bevy_grimoire_core::notes::Notes>(text),
        "plan" => at::<bevy_grimoire_core::plan::Plan>(text),
        "board" => at::<bevy_grimoire_core::board::Board>(text),
        "relations" => at::<bevy_grimoire_core::relations::Relations>(text),
        "terrain" => at::<bevy_grimoire_core::heightfield::Heightfield>(text),
        "water_plan" => at::<bevy_grimoire_core::water::WaterPlan>(text),
        "water_field" => at::<bevy_grimoire_core::water::WaterField>(text),
        "volume" => at::<bevy_grimoire_core::volume::Volume>(text),
        "family_list" => at::<bevy_grimoire_core::family::Family>(text),
        "agent" => at::<bevy_grimoire_core::note_agent::AgentSettings>(text),
        "play" => at::<bevy_grimoire_core::play::PlayConfig>(text),
        "destructible_marks" => at::<bevy_grimoire_core::destructible::Marks>(text),
        "destructible_bake" => at::<bevy_grimoire_core::destructible::Bake>(text),
        _ => None,
    }
}

/// **The document with one leaf wrong, as the YAML a person could have typed.**
///
/// The wrong value goes into the JSON document the validator is asked about, and that document is written
/// back as YAML — so the file the reader is asked about is the file the schema was asked about, and no
/// byte of it is found by a search: there is no test-only door into the crate's own walk, and nothing to
/// drift from it.
fn wrong_file(doc: &Value) -> String {
    serde_yaml_ng::to_string(doc).unwrap_or_else(|e| panic!("the wrong document does not write as YAML: {e}"))
}

#[test]
fn a_field_that_names_a_piece_a_composition_or_a_rig_carries_its_kind() {
    // **Every field that names another file's value says which kind**, so navigation can follow it. Each
    // is marked once, on the type, with `#[schemars(extend("x-dark-factory-ref" = "<kind>"))]`, and this
    // checks the marking reached the schema an editor reads.
    let named = |schema: &Value, at: &[&str]| -> Value {
        let mut node = schema;
        for step in at {
            node = &node[*step];
        }
        node["x-dark-factory-ref"].clone()
    };
    /// **The variant named `variant` of an `oneOf`**, which `schemars` writes as
    /// `{properties: {<Variant>: {properties: {<field>: …}}}}`.
    fn variant<'s>(schema: &'s Value, variant: &str, field: &str) -> &'s Value {
        let cases = schema["oneOf"].as_array().unwrap_or_else(|| panic!("an enum has `oneOf`: {schema}"));
        for case in cases {
            if let Some(found) = case["properties"].get(variant) {
                return &found["properties"][field];
            }
        }
        panic!("no variant `{variant}` in {schema}");
    }

    // A composition's member names a library piece, and its other variant names a composition.
    // `schemars` writes an enum variant as `properties: {<Variant>: {...}}`.
    let compositions = schema::for_kind("compositions").unwrap();
    let body = &compositions["$defs"]["Body"];
    assert_eq!(variant(body, "Descriptor", "id")["x-dark-factory-ref"].as_str(), Some("library"), "a composition's member names a library piece: {body}");
    assert_eq!(variant(body, "Composition", "id")["x-dark-factory-ref"].as_str(), Some("compositions"), "a member can be another composition: {body}");

    // A map's placement names a library piece; a stamp names a composition.
    let map = schema::for_kind("map").unwrap();
    assert_eq!(named(&map, &["$defs", "Placed", "properties", "descriptor"]).as_str(), Some("library"), "a placement's descriptor names a library piece");
    assert_eq!(named(&map, &["$defs", "Stamped", "properties", "of"]).as_str(), Some("compositions"), "a stamp names a composition");

    // **A relation's `of` and a note's target hold a placement id**, which is an id inside this project's
    // map files and not a value of another file's kind: neither carries a mark, because there is no one
    // other file a reader could be sent to. The test says so, so a mark added back is caught.
    let relations = schema::for_kind("relations").unwrap();
    assert_eq!(named(&relations, &["$defs", "Relation", "properties", "of"]).as_str(), None, "a relation's `of` is a placement id, not another file's value");
    let notes = schema::for_kind("notes").unwrap();
    assert_eq!(note_target_ref(&notes).as_str(), None, "a note's target is a placement id, not another file's value");

    // **A rigs file's `rigs` map is its own definitions**, keyed by rig name; it names nothing outside
    // this file, so it carries no mark. The manifest's own `note` is a string, not a reference either.
    let rigs = schema::for_kind("rigs").unwrap();
    assert_eq!(named(&rigs, &["properties", "rigs"]).as_str(), None, "a rigs file's `rigs` map is its own definitions: {}", rigs["properties"]["rigs"]);
    // And nothing else in the file gained a mark by accident.
    let stray: Vec<&str> = ["rigs", "version", "note"]
        .into_iter()
        .filter(|key| named(&rigs, &["properties", key]).as_str().is_some())
        .collect();
    assert!(stray.is_empty(), "a rigs file names no other file's value, but {} carries a mark", stray.join(", "));
}

/// **The mark on a note's `Target::Placement`**, found inside the enum rather than by searching the
/// serialized schema for the key: a key anywhere in a document says nothing about which field carries it.
fn note_target_ref(schema: &Value) -> Value {
    let target = &schema["$defs"]["Target"];
    let cases = target["oneOf"].as_array().unwrap_or_else(|| panic!("a `Target` is one of its variants: {target}"));
    for case in cases {
        if let Some(placement) = case["properties"].get("Placement") {
            return placement["x-dark-factory-ref"].clone();
        }
    }
    panic!("`Target` has a `Placement` variant: {target}");
}

