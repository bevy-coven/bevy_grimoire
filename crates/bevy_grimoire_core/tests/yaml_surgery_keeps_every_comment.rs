//! **One value edited out of a YAML file, and every comment still there** (GD-02, #364).
//!
//! A person's comments in a file an agent edited are still there afterwards. So an edit replaces the
//! bytes of exactly one scalar, sequence item or mapping entry, and every other line — a comment, a
//! blank line, a neighbour's alignment — comes through byte for byte. This file proves that with a line
//! diff against a hand-written expectation, not by re-parsing: a re-parse would pass even if the file
//! had been rewritten wholesale and happened to mean the same thing.
//!
//! What the surgery refuses, so that an edit cannot quietly change what a file says:
//!
//! - **a value of another kind** — a number stays a number and a string a string, as `set_leaf` has it;
//! - **a value outside its registered bounds**, and the refusal names the leaf's path.
//!
//! The text is a person's file, with a comment on every line, in the styles this file's kinds are
//! written in: a block sequence indented under its key, one indentless (as the writer writes it), and a
//! small flow collection.

use bevy_grimoire_core::data_files::Bounds;
use serde::Deserialize;
use bevy_grimoire_core::yaml_surgery::{self, Value};

/// The shape `YARD` has, so a test can read an edited file back and check the value came through. Every
/// field is read by a test's assertion; the ones a given test does not look at are still deserialized,
/// which is the point — a field the reader would refuse makes the read fail.
#[derive(Debug, Deserialize)]
struct Yard {
    #[allow(dead_code)] version: u32,
    name: String,
    #[allow(dead_code)] placements: Vec<Row>,
    #[allow(dead_code)] sealed: bool,
}

#[derive(Debug, Deserialize)]
struct Row {
    #[allow(dead_code)] id: String,
    #[allow(dead_code)] turn_deg: f32,
}

/// A project file with a comment on every line. The list is indented, which is a style the writer does
/// not use but a person may, and the surgery must copy the *siblings'* indentation rather than assume
/// the writer's.
const YARD: &str = "\
# The yard, hand written. Every line says what it is. 1
version: 9                                             # the format this build reads 2
name: yard                                             # also the file's name 3
placements:                                            # the pieces on it 4
  # the bed, against the wall 5
  - id: bed                                            # a row's id 6
    turn_deg: 90.0                                     # which way it faces 7
  # the sofa, facing it 8
  - id: sofa                                           # a row's id 9
    turn_deg: 0.0                                      # which way it faces 10
sealed: true                                           # a yard with a wall 11
";

/// The same file with one placement added: the sibling's `-` column, and the sibling's field column.
fn expected_after_add() -> String {
    YARD.replace(
        "    turn_deg: 0.0                                      # which way it faces 10\n",
        "    turn_deg: 0.0                                      # which way it faces 10\n  - id: table\n    turn_deg: 180.0\n",
    )
}

/// The line diff between two texts, as `(-)` for a line only in `before` and `(+)` for one only in
/// `after`, with its number. Empty when the two are the same line for line.
fn line_diff(before: &str, after: &str) -> Vec<String> {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let mut out = Vec::new();
    for n in 0..a.len().max(b.len()) {
        match (a.get(n), b.get(n)) {
            (Some(x), Some(y)) if x == y => {}
            (Some(x), Some(y)) => out.push(format!("line {}: (-){x}\n         (+){y}", n + 1)),
            (Some(x), None) => out.push(format!("line {}: (-){x}", n + 1)),
            (None, Some(y)) => out.push(format!("line {}: (+){y}", n + 1)),
            (None, None) => {}
        }
    }
    out
}

fn set(text: &str, path: &str, value: Value) -> String {
    yaml_surgery::set(text, path, value, None).unwrap_or_else(|e| panic!("`{path}`: {}", e.line()))
}

fn refused(text: &str, path: &str, value: Value, bounds: Option<&Bounds>) -> String {
    let problem = yaml_surgery::set(text, path, value, bounds).expect_err("this edit must be refused");
    problem.where_.as_ref().and_then(|w| w.field.clone()).unwrap_or_else(|| panic!("a refusal names the leaf: {}", problem.line()))
}

#[test]
fn a_set_changes_one_line_and_every_comment_stays() {
    // A value of the same length leaves the line's alignment exactly as it was: the only bytes that
    // move are the value's own.
    let after = set(YARD, "name", Value::Str("shed".into()));
    let diff = line_diff(YARD, &after);
    assert_eq!(diff.len(), 1, "one line changed:\n{}", diff.join("\n"));
    assert!(diff[0].contains("(+)name: shed"), "{}", diff[0]);
    assert!(after.contains("# also the file's name 3"), "the comment is still there:\n{after}");
    assert_eq!(after.lines().count(), YARD.lines().count(), "the set keeps every line it had: one value's text changed in place");

    // A longer value pushes the comment along the line and moves nothing else, because the comment is
    // bytes after the value and the edit rewrites only the value.
    let longer = set(YARD, "name", Value::Str("courtyard".into()));
    assert!(longer.contains("name: courtyard") && longer.contains("# also the file's name 3"), "{longer}");
    assert_eq!(line_diff(YARD, &longer).len(), 1, "a longer value still changes one line");
}

#[test]
fn an_item_added_copies_its_siblings_indentation_and_every_comment_stays() {
    let after = yaml_surgery::add_item(YARD, "placements", "id: table\nturn_deg: 180.0").unwrap_or_else(|e| panic!("{}", e.line()));
    let expected = expected_after_add();
    assert!(line_diff(&expected, &after).is_empty(), "the added row is the siblings' style:\n{}", line_diff(&expected, &after).join("\n"));
    // The two new lines sit where the last item ended, and every line above them is untouched.
    let lines: Vec<&str> = after.lines().collect();
    assert_eq!(lines[10], "  - id: table");
    assert_eq!(lines[11], "    turn_deg: 180.0");
    assert_eq!(lines[12], "sealed: true                                           # a yard with a wall 11");
    for (n, line) in YARD.lines().enumerate().take(10) {
        assert_eq!(lines[n], line, "line {} moved", n + 1);
    }
}

#[test]
fn an_item_removed_takes_only_its_own_lines() {
    let after = yaml_surgery::remove_item(YARD, "placements", 0).unwrap_or_else(|e| panic!("{}", e.line()));
    let expected = YARD
        .replace(
            "  - id: bed                                            # a row's id 6\n    turn_deg: 90.0                                     # which way it faces 7\n",
            "",
        );
    assert!(line_diff(&expected, &after).is_empty(), "only the removed row's lines went:\n{}", line_diff(&expected, &after).join("\n"));
    // The comment above the removed row is on its own line and stays, so the file still says what it
    // said about a row that is now the sofa's.
    assert!(after.contains("# the bed, against the wall 5"), "{after}");
    assert!(!after.contains("id: bed"), "{after}");
}

#[test]
fn the_other_rows_keep_their_ids_so_the_next_edit_still_addresses_them() {
    let after = yaml_surgery::remove_item(YARD, "placements", 0).unwrap_or_else(|e| panic!("{}", e.line()));
    let after = set(&after, "placements[0].turn_deg", Value::Float(45.0));
    assert!(after.contains("  - id: sofa"), "{after}");
    assert!(after.contains("turn_deg: 45.0"), "the remaining row is now item 0:\n{after}");
    assert!(after.contains("# the sofa, facing it 8"), "and its comment is above it:\n{after}");
}

#[test]
fn a_whole_row_can_be_set_field_by_field_and_the_row_stays_one_row() {
    let after = set(YARD, "placements[1].id", Value::Str("bench".into()));
    let after = set(&after, "placements[1].turn_deg", Value::Float(270.5));
    assert!(after.contains("  - id: bench"), "{after}");
    assert!(after.contains("    turn_deg: 270.5"), "{after}");
    assert_eq!(after.lines().count(), YARD.lines().count(), "no line was added or lost:\n{after}");
}

#[test]
fn a_value_of_another_kind_is_refused_naming_the_leaf() {
    // A number stays a number: an editor that wrote `bed` into `turn_deg` would make a file whose
    // reader says "invalid type: string, expected f32", which is a refusal the surgery exists to
    // prevent rather than to cause.
    let path = refused(YARD, "placements[0].turn_deg", Value::Str("bed".into()), None);
    assert_eq!(path, "placements[0].turn_deg");
    // And a float leaf does not become an integer one, as `set_leaf` has it.
    let path = refused(YARD, "placements[0].turn_deg", Value::Int(90), None);
    assert_eq!(path, "placements[0].turn_deg");
    // A string leaf does not become a number.
    let path = refused(YARD, "name", Value::Int(7), None);
    assert_eq!(path, "name");
    // A bool leaf does not become a word, whatever YAML 1.1 would have said about it.
    let path = refused(YARD, "sealed", Value::Str("no".into()), None);
    assert_eq!(path, "sealed");
}

#[test]
fn a_value_out_of_its_bounds_is_refused_naming_the_leaf() {
    let bounds = Bounds { min: Some(-180.0), max: Some(180.0) };
    let path = refused(YARD, "placements[0].turn_deg", Value::Float(400.0), Some(&bounds));
    assert_eq!(path, "placements[0].turn_deg");
    // Inside the bounds, the same value writes.
    let inside = yaml_surgery::set(YARD, "placements[0].turn_deg", Value::Float(400.0), None)
        .unwrap_or_else(|e| panic!("{}", e.line()));
    assert!(inside.contains("turn_deg: 400.0"), "{inside}");
    let in_bounds = yaml_surgery::set(YARD, "placements[0].turn_deg", Value::Float(-179.5), Some(&bounds))
        .unwrap_or_else(|e| panic!("{}", e.line()));
    assert!(in_bounds.contains("turn_deg: -179.5"), "{in_bounds}");
}

#[test]
fn a_path_the_file_does_not_have_is_refused_naming_it() {
    let problem = yaml_surgery::set(YARD, "placements[7].turn_deg", Value::Float(1.0), None)
        .expect_err("there is no eighth row");
    assert_eq!(problem.where_.as_ref().and_then(|w| w.field.as_deref()), Some("placements[7].turn_deg"), "{}", problem.line());
    let problem = yaml_surgery::add_item(YARD, "sealed", "id: x").expect_err("a bool is not a list");
    assert_eq!(problem.where_.as_ref().and_then(|w| w.field.as_deref()), Some("sealed"), "{}", problem.line());
}

#[test]
fn the_writers_own_style_is_edited_the_same_way() {
    // The writer puts a sequence at its key's indentation (indentless), so the `-` is at column 0. The
    // surgery reads the dash column from the siblings, so both styles work and neither is rewritten.
    let written = "\
version: 9
name: yard
placements:
- id: bed
  turn_deg: 90.0
sealed: true
";
    let after = yaml_surgery::add_item(written, "placements", "id: sofa\nturn_deg: 0.0").unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(
        line_diff(
            "version: 9\nname: yard\nplacements:\n- id: bed\n  turn_deg: 90.0\n- id: sofa\n  turn_deg: 0.0\nsealed: true\n",
            &after
        ),
        Vec::<String>::new(),
        "{after}"
    );
    let after = yaml_surgery::remove_item(&after, "placements", 0).unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(line_diff("version: 9\nname: yard\nplacements:\n- id: sofa\n  turn_deg: 0.0\nsealed: true\n", &after), Vec::<String>::new(), "{after}");
}

#[test]
fn a_flow_list_takes_a_leaf_and_an_empty_one_becomes_a_block() {
    // A leaf inside a flow collection is one span, so it is set in place and the braces stay.
    let flow = "placements: [{id: bed, turn_deg: 90.0}]\n";
    let after = set(flow, "placements[0].turn_deg", Value::Float(45.0));
    assert_eq!(after, "placements: [{id: bed, turn_deg: 45.0}]\n");

    // An empty flow list has no siblings to copy, so it is written as a block instead of guessed at.
    let empty = "version: 9\nplacements: []\n";
    let after = yaml_surgery::add_item(empty, "placements", "id: bed\nturn_deg: 90.0").unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(after, "version: 9\nplacements:\n- id: bed\n  turn_deg: 90.0\n");

    // A comment after the `[]` is the KEY's, so it stays on the key's line and only the empty list
    // becomes a block.
    let commented = "version: 9\nplacements: []  # none yet\nsealed: true\n";
    let after = yaml_surgery::add_item(commented, "placements", "id: bed").unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(after, "version: 9\nplacements:  # none yet\n- id: bed\nsealed: true\n");

    // A flow list with an item in it cannot take another without rewriting the line it is on, which
    // would move bytes this edit was not asked for.
    let problem = yaml_surgery::add_item(flow, "placements", "id: sofa").expect_err("a flow list is refused");
    assert_eq!(problem.where_.as_ref().and_then(|w| w.field.as_deref()), Some("placements"), "{}", problem.line());
}

#[test]
fn an_entry_added_and_removed_keeps_every_comment() {
    let text = "\
# the map, by hand 1
version: 9  # 2
locations:  # 3
  yard:  # 4
    note: 'the yard'  # 5
  shed:  # 6
    note: 'the shed'  # 7
note: none  # 8
";
    let after = yaml_surgery::add_entry(text, "locations", "gate:\n  note: 'the gate'").unwrap_or_else(|e| panic!("{}", e.line()));
    let expected = text.replace(
        "  shed:  # 6\n    note: 'the shed'  # 7\n",
        "  shed:  # 6\n    note: 'the shed'  # 7\n  gate:\n    note: 'the gate'\n",
    );
    assert!(line_diff(&expected, &after).is_empty(), "{}", line_diff(&expected, &after).join("\n"));

    let after = yaml_surgery::remove_entry(&after, "locations", "shed").unwrap_or_else(|e| panic!("{}", e.line()));
    assert!(!after.contains("shed"), "{after}");
    assert!(after.contains("# the map, by hand 1"), "{after}");
    assert!(after.contains("note: none  # 8"), "{after}");
    assert!(after.contains("gate:\n    note: 'the gate'"), "{after}");
}

#[test]
fn a_string_that_yaml_would_misread_is_quoted_where_it_lands() {
    // The value's *kind* stays a string; its spelling is whatever the text needs to mean that.
    let after = set(YARD, "name", Value::Str("a: b".into()));
    assert!(after.contains("name: 'a: b'"), "{after}");
    let after = set(YARD, "name", Value::Str("yes".into()));
    assert!(after.contains("name: yes"), "YAML 1.2 reads `yes` as a string, so it needs no quotes:\n{after}");
}

#[test]
fn an_edit_never_produces_a_file_the_reader_refuses() {
    // GD-01's typed read of the whole result still checks the value, so the surgery cannot produce a
    // file the editor refuses. A map is the type that reads all five edits at once.
    let map = "\
version: 9
name: yard
origin: [0.0, 0.0, 0.0]
bounds: [10.0, 4.0, 10.0]
placements:
- id: bed
  descriptor: site/bed
  at: [1.0, 2.0]
  yaw: 90.0
note: null
";
    let after = set(map, "name", Value::Str("courtyard".into()));
    let after = set(&after, "placements[0].yaw", Value::Float(45.0));
    let after = set(&after, "bounds[0]", Value::Float(12.5));
    let after = yaml_surgery::add_item(&after, "placements", "id: sofa\ndescriptor: site/sofa\nat: [3.0, 4.0]\nyaw: 0.0")
        .unwrap_or_else(|e| panic!("{}", e.line()));
    let after = yaml_surgery::remove_item(&after, "placements", 0).unwrap_or_else(|e| panic!("{}", e.line()));
    let read: bevy_grimoire_core::map::Map = bevy_grimoire_core::yaml::from_str(&after, "maps/yard.map.yaml")
        .unwrap_or_else(|e| panic!("the edited file does not read as a map: {}", e.line()));
    assert_eq!(read.name, "courtyard");
    assert_eq!(read.bounds.0, 12.5);
    assert_eq!(read.placements.len(), 1, "one row was removed and one added");
    assert_eq!(read.placements[0].id, "sofa");
}

#[test]
fn a_blank_or_missing_value_is_refused_rather_than_guessed() {
    // A key with no value (`a:` then `b: 1`) is a null, and a null stays a null: nothing is written
    // over it by accident.
    let nulls = "a:\nb: 1\n";
    let problem = yaml_surgery::set(nulls, "a", Value::Str("x".into()), None).expect_err("a null leaf takes no string");
    assert_eq!(problem.where_.as_ref().and_then(|w| w.field.as_deref()), Some("a"), "{}", problem.line());
    let after = yaml_surgery::set(nulls, "a", Value::Null, None).unwrap_or_else(|e| panic!("{}", e.line()));
    assert!(line_diff(nulls, &after).is_empty(), "a null set to a null is the same file:\n{after}");
}

// ------------------------------------------------------------------------------------------------
// Review of #364: what an edit may write, and what a collection's bytes really are.
// ------------------------------------------------------------------------------------------------

#[test]
fn an_edit_whose_result_does_not_read_is_refused_naming_the_path() {
    // `data_files::set_leaf` refuses a result that does not parse, and this is the same rule: an edit
    // that would duplicate a key, write an anchor, or break the text is refused rather than written, so
    // the editor never hands an author a file its own reader will not take.
    let text = "version: 9\nlocations:\n  yard:\n    note: a\n  shed:\n    note: b\n";
    let problem = yaml_surgery::add_entry(text, "locations", "yard: duplicate").expect_err("a duplicate key does not read");
    assert_eq!(problem.where_.as_ref().and_then(|w| w.field.as_deref()), Some("locations"), "{}", problem.line());
    assert!(problem.line().contains("given twice"), "{}", problem.line());

    // An item whose text carries an anchor does not read either.
    let problem = yaml_surgery::add_item("xs:\n- a\n", "xs", "&named b").expect_err("an anchor does not read");
    assert_eq!(problem.where_.as_ref().and_then(|w| w.field.as_deref()), Some("xs"), "{}", problem.line());
}

#[test]
fn a_string_that_would_be_misread_is_quoted_wherever_it_lands() {
    // `#` after a space starts a comment, so `red #1` would read as `red`.
    let after = set(YARD, "name", Value::Str("red #1".into()));
    assert!(after.contains("name: 'red #1'"), "{after}");
    // A newline cannot be written plain over one line.
    let after = set(YARD, "name", Value::Str("a\nb".into()));
    assert!(after.contains("name: \"a\\nb\""), "{after}");
    let yard: Yard = bevy_grimoire_core::yaml::from_str(&after, "maps/yard.yaml").unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(yard.name, "a\nb");
    // A leading or trailing space is not plain either.
    let after = set(YARD, "name", Value::Str(" yard".into()));
    assert!(after.contains("name: ' yard'"), "{after}");
    // YAML's own syntax characters.
    for nasty in ["a: b", "- a", "? a", ": a", "[a]", "{a}", "*a", "&a", "!a", "%a", "@a", "`a", "'a'", "\"a\"", "|a", ">a"] {
        let after = set(YARD, "name", Value::Str(nasty.into()));
        let yard: Yard = bevy_grimoire_core::yaml::from_str(&after, "maps/yard.yaml")
            .unwrap_or_else(|e| panic!("`{nasty}` did not read back: {}\n{after}", e.line()));
        assert_eq!(yard.name, nasty, "`{nasty}` did not come back as itself:\n{after}");
    }
}

#[test]
fn a_float_written_into_an_integer_leaf_is_refused_or_made_whole() {
    // An integer leaf stays an integer: 9.5 into `version` must not write `9.5`, which the reader refuses.
    let problem = yaml_surgery::set(YARD, "version", Value::Float(9.5), None).expect_err("an integer leaf takes a whole number");
    assert_eq!(problem.where_.as_ref().and_then(|w| w.field.as_deref()), Some("version"), "{}", problem.line());
    // A whole value is a whole number, and writes.
    let after = set(YARD, "version", Value::Float(9.0));
    assert!(after.contains("version: 9"), "{after}");
}

#[test]
fn an_infinity_or_a_nan_is_refused_with_no_bounds_too() {
    for bad in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let problem = yaml_surgery::set(YARD, "placements[0].turn_deg", Value::Float(bad), None)
            .expect_err("a file holds no infinity and no NaN");
        assert_eq!(problem.where_.as_ref().and_then(|w| w.field.as_deref()), Some("placements[0].turn_deg"), "{}", problem.line());
    }
}

#[test]
fn a_block_scalar_leaf_is_replaced_and_takes_none_of_the_next_line_with_it() {
    let text = "\
version: 9
placements:
  - id: bed
    note: |
      first line
      second line
    turn_deg: 90.0
sealed: true
";
    // A block scalar's value is a string, so a string is what replaces it; the field after it is intact.
    let after = set(text, "placements[0].note", Value::Str("one line".into()));
    let expected = text.replace("    note: |\n      first line\n      second line\n", "    note: one line\n");
    assert!(line_diff(&expected, &after).is_empty(), "a block scalar was replaced without eating the next field:\n{}", line_diff(&expected, &after).join("\n"));

    // An item whose last field is a block scalar is removed whole, and the next item's first line stays.
    let last_block = "\
version: 9
placements:
  - id: bed
    note: |
      one
      two
  - id: sofa
    note: |-
      three
sealed: true
";
    let after = yaml_surgery::remove_item(last_block, "placements", 0).unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(after, "version: 9\nplacements:\n  - id: sofa\n    note: |-\n      three\nsealed: true\n", "{after}");

    // An item added after one that ends in a block scalar goes below its content, not into it.
    let after = yaml_surgery::add_item(last_block, "placements", "id: table\nnote: x").unwrap_or_else(|e| panic!("{}", e.line()));
    assert!(after.contains("      three\n  - id: table"), "the new item is below the block scalar's content:\n{after}");
    assert!(after.contains("sealed: true"), "{after}");
}

#[test]
fn a_flow_collection_cannot_be_removed_from() {
    // A flow list is one line: removing an item from it would delete the line the key is on.
    let text = "at: [1.0, 2.0]\nnext: 1\n";
    let problem = yaml_surgery::remove_item(text, "at", 0).expect_err("a flow list has no lines to remove");
    assert_eq!(problem.where_.as_ref().and_then(|w| w.field.as_deref()), Some("at"), "{}", problem.line());
    let text = "at: {x: 1.0}\nnext: 1\n";
    let problem = yaml_surgery::remove_entry(text, "at", "x").expect_err("a flow mapping has no lines to remove");
    assert_eq!(problem.where_.as_ref().and_then(|w| w.field.as_deref()), Some("at"), "{}", problem.line());
    assert!(text.contains("at: {x: 1.0}"), "and the file is untouched");
}

#[test]
fn removing_an_items_first_key_keeps_the_item_and_promotes_the_next_key() {
    // `- id: bed` on its own line: removing `id` must not take the `-` with it, or the rest of the row
    // stops being an item of the list.
    let text = "\
placements:
  - id: bed
    turn_deg: 90.0
  - id: sofa
    turn_deg: 0.0
";
    let after = yaml_surgery::remove_entry(text, "placements[0]", "id").unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(after, "placements:\n  - turn_deg: 90.0\n  - id: sofa\n    turn_deg: 0.0\n", "{after}");
    let _ = text;
}

#[test]
fn a_nested_item_keeps_its_own_indentation() {
    let text = "\
items:
- id: a
  patch:
    scale: 2
";
    let after = yaml_surgery::add_item(text, "items", "id: b\npatch:\n  scale: 3").unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(after, "items:\n- id: a\n  patch:\n    scale: 2\n- id: b\n  patch:\n    scale: 3\n", "{after}");
}

#[test]
fn an_item_added_to_a_file_with_no_trailing_newline_does_not_join_its_last_line() {
    let text = "xs:\n- a";
    let after = yaml_surgery::add_item(text, "xs", "b").unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(after, "xs:\n- a\n- b\n", "{after}");
}

#[test]
fn a_surgery_says_where_a_refusal_stands_rather_than_line_one() {
    // The refusal's position is the node's own where it has one, and absent where it has none — never a
    // made-up `line 1, column 1`, which #363 removed from the reader.
    let problem = yaml_surgery::set(YARD, "placements[0].turn_deg", Value::Str("bed".into()), None).expect_err("a wrong kind");
    let why = problem.why.clone().unwrap_or_default();
    assert!(!why.contains("line 1, column 1"), "{why}");
    assert!(problem.where_.as_ref().and_then(|w| w.field.as_deref()).is_some(), "{}", problem.line());
}
