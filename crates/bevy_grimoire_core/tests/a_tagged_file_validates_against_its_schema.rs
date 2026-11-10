//! **A tagged file validates against its schema too** (GD-04, #366).
//!
//! Seven kinds write an enum as a YAML tag — `kind: !Near {to, within}`, `shape: !Sphere {...}`,
//! `targets: [!Placement chair@1]` — and `tests/schema_describes_every_file_kind.rs` records them in a
//! `TAGGED` list it does not validate, because the plain YAML→JSON reader it uses **cannot represent a tag**:
//! `serde_yaml_ng::from_str::<Value>` fails on the document rather than handing back a value, so the test had
//! nothing to check the schema against.
//!
//! **The schema was never the gap.** A tag names an enum variant, and `schemars` writes a variant as the key
//! of a one-key object — `{"Near": {...}}` — which is exactly what a tag means. So the missing step was a
//! reader that keeps the tag, and `serde_yaml_ng` already has one: `Value::Tagged`. This file supplies it.
//!
//! What is checked, for every tagged kind, by a **validator and not the Rust type**:
//!
//! - its own fixture validates;
//! - a copy with one field wrong does **not**, and the validator names the same path GD-01's reader names —
//!   including a field *inside* an enum variant, where the error the validator reports at the top is the
//!   variant itself and the field is one `oneOf` branch down.
//!
//! The tagged list here is the same seven, and `tagged_kinds()` fails if a kind's fixture stops carrying a tag
//! or a kind that carries one is missing: the gap cannot grow or shrink without this test saying so.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// **The kinds whose YAML carries an enum tag**, with what carries it. The same seven as the protected test's
/// list, and the same reason: a tagged enum cannot be written as plain JSON by that reader.
const TAGGED: &[(&str, &str)] = &[
    ("library", "a `Descriptor`'s `mount`, a `Mount`"),
    ("policy", "a `Patch`'s `match`, a `Match`"),
    ("compositions", "a `Composition`'s `envelope`, an `Envelope`"),
    ("rigs", "a slot's `playback`, a `Playback`"),
    ("notes", "a note's `targets`, a `Target`"),
    ("relations", "a relation's `kind`, a `Kind`"),
    ("volume", "an edit's `shape`, a `Shape`"),
];

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

/// **The YAML this crate's writer writes for a RON fixture**, so the document checked is the one the reader
/// reads rather than a hand-typed guess at it.
fn value_of<T>(ron: &str) -> String
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let value: T = ron::from_str(ron).unwrap_or_else(|e| panic!("the fixture does not read as RON: {e}"));
    bevy_grimoire_core::yaml::to_string(&value).unwrap_or_else(|e| panic!("{e}"))
}

/// Every tagged kind and its fixture, as YAML.
fn tagged_kinds() -> Vec<(&'static str, String)> {
    vec![
        ("library", value_of::<bevy_grimoire_core::library::Library>(&shipped("site/library.ron"))),
        ("policy", value_of::<bevy_grimoire_core::policy::Policy>(&shipped("site/project.ron"))),
        ("compositions", value_of::<bevy_grimoire_core::composition::Compositions>(&shipped("compositions.ron"))),
        ("rigs", value_of::<bevy_grimoire_core::rigs::Rigs>(&shipped("rigs.ron"))),
        ("notes", value_of::<bevy_grimoire_core::notes::Notes>(&frozen("core-0.3.0/notes.ron"))),
        ("relations", value_of::<bevy_grimoire_core::relations::Relations>(&frozen("core-0.3.0/relations.ron"))),
        ("volume", value_of::<bevy_grimoire_core::volume::Volume>(&frozen("core-0.3.0/volume.ron"))),
    ]
}

/// **The JSON a tagged YAML document holds**, which is what an editor's extension validates.
///
/// A tag is a *name* for the node it sits on, and a JSON Schema describes a named variant as a one-key object,
/// so `!Near {to, within}` is `{"Near": {to, within}}` and `!Faces desk@1` is `{"Faces": "desk@1"}`. The `!` is
/// YAML's sigil for a local tag and is not part of the name.
fn as_json(text: &str) -> Value {
    fn convert(value: serde_yaml_ng::Value) -> Value {
        use serde_yaml_ng::Value as Y;
        match value {
            Y::Tagged(tagged) => {
                let mut map = serde_json::Map::new();
                map.insert(tagged.tag.to_string().trim_start_matches('!').to_owned(), convert(tagged.value));
                Value::Object(map)
            }
            Y::Null => Value::Null,
            Y::Bool(b) => Value::Bool(b),
            Y::Number(n) => serde_json::to_value(n).expect("a YAML number is a JSON number"),
            Y::String(s) => Value::String(s),
            Y::Sequence(s) => Value::Array(s.into_iter().map(convert).collect()),
            Y::Mapping(m) => Value::Object(
                m.into_iter()
                    .map(|(k, v)| (k.as_str().unwrap_or_default().to_owned(), convert(v)))
                    .collect(),
            ),
        }
    }
    let parsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(text)
        .unwrap_or_else(|e| panic!("the fixture is not YAML a reader keeping its tags can take: {e}"));
    convert(parsed)
}

/// **Every kind and every field path a validator refuses**, from the errors a `oneOf` reports as its branches.
///
/// The validator names a variant's field only one level down — the error at `/relations/0/kind` is "not valid
/// under any of `oneOf`", and the field is inside one branch — so the walk is the only way to ask "did it
/// refuse *this* field" of a tagged document.
fn refused_paths(validator: &jsonschema::Validator, doc: &Value) -> Vec<String> {
    fn walk(error: &jsonschema::ValidationError, out: &mut Vec<String>) {
        out.push(error.instance_path().to_string());
        if let jsonschema::error::ValidationErrorKind::OneOfNotValid { context } = error.kind() {
            for branch in context {
                for inner in branch {
                    walk(inner, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    for error in validator.iter_errors(doc) {
        walk(&error, &mut out);
    }
    out
}

/// The `Value` at a field path in the file's own spelling (`relations[0].kind.Near.within`).
fn navigate<'a>(value: &'a mut Value, path: &str) -> Option<&'a mut Value> {
    let mut node = value;
    for step in path.replace('[', ".").replace(']', "").split('.').filter(|s| !s.is_empty()) {
        node = match step.parse::<usize>() {
            Ok(index) => node.get_mut(index)?,
            Err(_) => node.get_mut(step)?,
        };
    }
    Some(node)
}

/// The JSON Pointer the file's own path spells: `relations[0].kind.Near.within` is
/// `/relations/0/kind/Near/within`.
fn pointer_of(path: &str) -> String {
    format!(
        "/{}",
        path.replace('.', "/").replace(['[', ']'], "/").replace("//", "/").trim_end_matches('/')
    )
}

#[test]
fn every_tagged_kinds_fixture_validates_against_its_schema() {
    for (kind, text) in tagged_kinds() {
        let generated = bevy_grimoire_core::schema::for_kind(kind).unwrap_or_else(|e| panic!("{kind}: {}", e.line()));
        let validator = jsonschema::validator_for(&generated)
            .unwrap_or_else(|e| panic!("{kind}: its schema is not a JSON Schema: {e}"));
        let fixture = as_json(&text);
        assert!(
            validator.is_valid(&fixture),
            "{kind}: its own fixture does not validate against its schema: {:?}",
            refused_paths(&validator, &fixture)
        );
    }
}

/// **The list is exactly the kinds whose fixture carries a tag**: a kind that stops, or a new one that starts,
/// fails here rather than quietly joining a list nothing validates.
#[test]
fn the_tagged_kinds_are_the_ones_whose_fixture_carries_a_tag() {
    let carrying: Vec<&str> = tagged_kinds()
        .into_iter()
        .filter(|(_, text)| serde_yaml_ng::from_str::<serde_yaml_ng::Value>(text).is_ok_and(|value| has_a_tag(&value)))
        .map(|(kind, _)| kind)
        .collect();
    let listed: Vec<&str> = TAGGED.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(carrying, listed, "the kinds whose fixture carries a tag and the list must be one list");
    // A literal, so the count cannot silently grow while the list it counts changes.
    assert_eq!(TAGGED.len(), 7, "the gap this file closes is seven kinds; a new one needs its own reader check");
}

/// Whether any node of `value` is a tag.
fn has_a_tag(value: &serde_yaml_ng::Value) -> bool {
    use serde_yaml_ng::Value as Y;
    match value {
        Y::Tagged(_) => true,
        Y::Sequence(items) => items.iter().any(has_a_tag),
        Y::Mapping(map) => map.iter().any(|(_, v)| has_a_tag(v)),
        _ => false,
    }
}

/// **A field wrong inside a variant is refused at its own path**, which is the half that makes a schema worth
/// having: a document whose `within` is text where the schema says a number must not pass with a completion
/// that offers it.
#[test]
fn a_field_wrong_inside_a_variant_is_refused_at_its_own_path() {
    // Three kinds, each wrong in a different place: a number inside a struct variant, a number inside another,
    // and a top-level field of a tagged document (the control, where the path is the error's own).
    let cases = [
        ("relations", "relations[0].kind.Near.within"),
        ("volume", "edits[0].shape.Sphere.radius_m"),
        ("notes", "lease_secs"),
    ];
    let fixtures = tagged_kinds();
    for (kind, path) in cases {
        let (_, text) = fixtures
            .iter()
            .find(|(id, _)| *id == kind)
            .unwrap_or_else(|| panic!("`{kind}` is a tagged kind with a fixture"));
        let generated = bevy_grimoire_core::schema::for_kind(kind).unwrap();
        let validator = jsonschema::validator_for(&generated).unwrap_or_else(|e| panic!("{kind}: {e}"));

        let mut doc = as_json(text);
        let leaf = navigate(&mut doc, path).unwrap_or_else(|| panic!("{kind}: the fixture has `{path}`"));
        *leaf = Value::from("not the kind this field is");

        let pointer = pointer_of(path);
        let refused = refused_paths(&validator, &doc);
        assert!(
            refused.iter().any(|named| named == &pointer),
            "{kind}: a wrong value at `{path}` ({pointer}) was not refused there; the validator named {refused:?}"
        );
    }
}

/// **The reader names the same path the schema does**, which is GD-01's promise and the reason a schema and a
/// reader are worth having together: a person told `relations[0].kind.Near.within` is sent to the field the
/// completion would have offered.
///
/// **The edit keeps the tag.** The document is walked as YAML and the leaf replaced there, because writing it
/// back through JSON would turn `!Near {…}` into `{Near: {…}}` — a different file, which the reader refuses at
/// the variant rather than at the field. That refusal is correct and is not the question here.
#[test]
fn the_reader_names_the_same_path_the_schema_refuses() {
    type Read = fn(&str) -> Option<String>;
    let cases: [(&str, &str, Read); 2] = [
        ("relations", "relations[0].kind.Near.within", reader_field::<bevy_grimoire_core::relations::Relations>),
        ("volume", "edits[0].shape.Sphere.radius_m", reader_field::<bevy_grimoire_core::volume::Volume>),
    ];
    let fixtures = tagged_kinds();
    for (kind, path, read) in cases {
        let (_, text) = fixtures.iter().find(|(id, _)| *id == kind).expect("a tagged kind");
        let mut value: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(text).unwrap_or_else(|e| panic!("{kind}: the fixture is YAML: {e}"));
        let leaf = navigate_tagged(&mut value, path).unwrap_or_else(|| panic!("{kind}: the fixture has `{path}`"));
        *leaf = serde_yaml_ng::Value::String("not the kind this field is".to_owned());

        let edited = serde_yaml_ng::to_string(&value).expect("the wrong document writes as YAML");
        let named = read(&edited).unwrap_or_else(|| panic!("{kind}: the reader accepted `{path}` set to a string\n{edited}"));
        assert_eq!(named.as_str(), path, "{kind}: the reader named `{named}` where the schema named `{path}`");
    }
}

/// **The YAML node at a field path**, descending **through a tag** when the path names the variant: a step that
/// matches a tagged node's name goes into the tagged value, because a tag is a name for the node under it.
fn navigate_tagged<'a>(value: &'a mut serde_yaml_ng::Value, path: &str) -> Option<&'a mut serde_yaml_ng::Value> {
    use serde_yaml_ng::Value as Y;
    let mut node = value;
    for step in path.replace('[', ".").replace(']', "").split('.').filter(|s| !s.is_empty()) {
        match node {
            Y::Tagged(tagged) if tagged.tag.to_string().trim_start_matches('!') == step => node = &mut tagged.value,
            Y::Mapping(map) => node = map.get_mut(step)?,
            Y::Sequence(items) => node = items.get_mut(step.parse::<usize>().ok()?)?,
            _ => return None,
        }
    }
    Some(node)
}

/// The field GD-01's reader refuses `text` at, or `None` when it reads.
fn reader_field<T: serde::de::DeserializeOwned>(text: &str) -> Option<String> {
    bevy_grimoire_core::yaml::from_str::<T>(text, "a project file").err().and_then(|p| p.where_.and_then(|w| w.field))
}
