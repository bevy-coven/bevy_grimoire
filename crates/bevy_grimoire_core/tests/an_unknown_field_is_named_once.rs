//! **A field the type does not know is named once** (#200, H3).
//!
//! `reload_check` reads a refused file again with `serde_path_to_error` to say where it is wrong. For
//! an unknown field the path already ends with the key the type refused, so adding the field ron
//! names produced `undergrow.undergrow` for a tree file with a misspelt `undergrow:`. The refusal names
//! the field as the author wrote it, at the top of a file and inside a list.

use bevy_grimoire_core::reload::kind_by_id;
use bevy_grimoire_core::reload_check::{self, Check, Refusal};
use bevy_grimoire_core::rules::RULES_VERSION;

fn refused(text: &str) -> Refusal {
    let kind = kind_by_id("rules").expect("a registry kind");
    match reload_check::check(kind, "rules.ron", text) {
        Check::Refused(r) => r,
        other => panic!("a file with a field the type does not know must be refused, got {other:?}"),
    }
}

#[test]
fn an_unknown_field_at_the_top_of_a_file_is_named_as_itself() {
    let r = refused(&format!("(version: {RULES_VERSION}, bogus: 2)"));
    assert_eq!(r.field.as_deref(), Some("bogus"), "{r:?}");
}

#[test]
fn an_unknown_field_inside_a_list_is_named_by_its_whole_path() {
    let r = refused(&format!("(version: {RULES_VERSION}, blocks: [(bogus: 1)])"));
    assert_eq!(r.field.as_deref(), Some("blocks[0].bogus"), "{r:?}");
}

#[test]
fn a_missing_field_is_still_named_inside_its_struct() {
    let r = refused(&format!("(version: {RULES_VERSION}, blocks: [(example: \"yard\")])"));
    assert_eq!(r.field.as_deref(), Some("blocks[0].cell"), "{r:?}");
}
