//! **A generated JSON Schema is a checked file kind** (#366): `reload_check` reads it as one JSON value, so
//! no kind of project file is left that nothing reads.

use bevy_grimoire_core::{reload::KINDS, reload_check::{check, Check}};

#[test]
fn a_schema_file_is_valid_json_or_refused_by_name() {
    let kind = KINDS.iter().find(|k| k.id == "schema").expect("the schema kind is declared");
    assert!(matches!(check(kind, "map.schema.json", r#"{"$schema": "https://json-schema.org/draft/2020-12/schema"}"#), Check::Valid), "a JSON object reads");
    let Check::Refused(refusal) = check(kind, "map.schema.json", r#"{"type": "obj"#) else {
        panic!("half a schema file is refused, not reported as unchecked or valid");
    };
    assert_eq!(refusal.kind, "schema");
    assert_eq!(refusal.file, "map.schema.json");
}
