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
        ("kits", value_of::<bevy_grimoire_core::kits::Kits>(&shipped("kits.ron"))),
        ("library", value_of::<bevy_grimoire_core::library::Library>(&shipped("site/library.ron"))),
        ("policy", value_of::<bevy_grimoire_core::policy::Policy>(&shipped("site/project.ron"))),
        ("vocab", value_of::<bevy_grimoire_core::vocab::Vocabularies>(&shipped("vocab.ron"))),
        ("compositions", value_of::<bevy_grimoire_core::composition::Compositions>(&shipped("compositions.ron"))),
        ("rigs", value_of::<bevy_grimoire_core::rigs::Rigs>(&shipped("rigs.ron"))),
        ("rules", value_of::<bevy_grimoire_core::rules::Rules>(&frozen("core-0.3.0/rules.ron"))),
        ("map", value_of::<bevy_grimoire_core::map::Map>(&shipped("maps/bedroom.map.ron"))),
        ("standards", value_of::<bevy_grimoire_core::standards::Standards>(&frozen("core-0.3.0/standards.ron"))),
        ("repetition", value_of::<bevy_grimoire_core::repetition::RepetitionRules>(&frozen("core-0.3.0/repetition.ron"))),
        ("notes", value_of::<bevy_grimoire_core::notes::Notes>(&frozen("core-0.3.0/notes.ron"))),
        ("plan", value_of::<bevy_grimoire_core::plan::Plan>(&frozen("core-0.3.0/plan.ron"))),
        ("references", value_of::<bevy_grimoire_core::references::References>(&frozen("core-0.3.0/references.ron"))),
        ("relations", value_of::<bevy_grimoire_core::relations::Relations>(&frozen("core-0.3.0/relations.ron"))),
        ("terrain", value_of::<bevy_grimoire_core::heightfield::Heightfield>(&frozen("core-0.3.0/terrain.ron"))),
        ("water_plan", value_of::<bevy_grimoire_core::water::WaterPlan>(&frozen("core-0.3.0/water_plan.ron"))),
        ("water_field", value_of::<bevy_grimoire_core::water::WaterField>(&frozen("core-0.3.0/water_field.ron"))),
        ("volume", value_of::<bevy_grimoire_core::volume::Volume>(&frozen("core-0.3.0/volume.ron"))),
        ("data_registry", value_of::<bevy_grimoire_core::data_files::DataFiles>(&frozen("core-0.3.0/data_registry.ron"))),
        ("family_list", value_of::<bevy_grimoire_core::family::Family>(&frozen("core-0.3.0/family_list.ron"))),
        ("agent", value_of::<bevy_grimoire_core::note_agent::AgentSettings>(&frozen("core-0.3.0/agent.ron"))),
        ("play", value_of::<bevy_grimoire_core::play::PlayConfig>(&frozen("core-0.3.0/play.ron"))),
        ("destructible_marks", value_of::<bevy_grimoire_core::destructible::Marks>(&frozen("editor-3f4084c/destructible.ron"))),
        ("destructible_bake", value_of::<bevy_grimoire_core::destructible::Bake>(&frozen("editor-3f4084c/site__cart.bake.ron"))),
    ]
}

/// A RON fixture as the YAML this crate's writer writes it, so the fixture is in the format the schema
/// describes.
fn value_of<T>(ron: &str) -> String
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let value: T = ron::from_str(ron).unwrap_or_else(|e| panic!("the fixture does not read as RON: {e}"));
    bevy_grimoire_core::yaml::to_string(&value).unwrap_or_else(|e| panic!("{e}"))
}

/// **The fixture as the JSON an outside editor sees**, which is the value the crate's own reader makes of
/// it: a kind whose YAML uses a tag for an enum (`!Near`) is a JSON object to an editor, and this
/// crate's reader is the one that says so.
fn as_json(kind: &str, text: &str) -> Value {
    let value: Value = serde_yaml_ng::from_str(text).unwrap_or_else(|_| {
        // The crate's reader, for the fixture a tag keeps the plain reader from taking: the document an
        // editor's extension shows is the JSON of the value the type holds.
        schema::value_of(kind, text).unwrap_or_else(|e| panic!("{kind}: the fixture is not YAML: {}", e.line()))
    });
    value
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
    for (kind, text) in kinds() {
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
    let header = schema::header("map");
    assert_eq!(header, "# yaml-language-server: $schema=.dark_factory/schema/map.schema.json");
    let hinted = schema::with_header("map", "version: 9\n");
    assert!(hinted.starts_with(&header), "{hinted}");
    assert_eq!(schema::with_header("map", &hinted), hinted, "a file that has the hint gains no second one");

    // A game's own reflected schema is written where an editor looks, and text that is not JSON is
    // refused rather than written.
    let game = schema::write_game(&root, "{\"title\": \"a game's components\"}").unwrap_or_else(|e| panic!("{}", e.line()));
    assert!(game.to_string_lossy().ends_with("game.schema.json"), "{}", game.display());
    let written_game: Value = serde_json::from_str(&std::fs::read_to_string(&game).expect("readable")).expect("JSON");
    assert_eq!(written_game["title"], Value::from("a game's components"));
    assert!(schema::write_game(&root, "not json").is_err(), "a schema an editor cannot read is refused");

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}
