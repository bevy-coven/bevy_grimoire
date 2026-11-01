//! **`reload_check::read_lines` names the first damaged line of a JSON-lines file and the field in it**
//! (#200, H3), the way `check` names the field of a RON file: a refusal says which file, which line,
//! which field and why, and counts the damaged lines after the first.

use bevy_grimoire_core::reload_check::read_lines;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Row {
    n: u64,
    at: Vec<f64>,
    note: Option<String>,
}

fn field_of(text: &str) -> Option<String> {
    read_lines::<Row>("oplog", "oplog.jsonl", text).err().and_then(|r| r.field)
}

#[test]
fn a_log_that_reads_gives_every_line_and_skips_blank_ones() {
    let text = "{\"n\":1,\"at\":[0.5]}\n\n   \n{\"n\":2,\"at\":[],\"note\":\"x\"}\n";
    let rows = read_lines::<Row>("oplog", "oplog.jsonl", text).unwrap();
    assert_eq!(rows, [Row { n: 1, at: vec![0.5], note: None }, Row { n: 2, at: vec![], note: Some("x".into()) }]);
    assert!(read_lines::<Row>("oplog", "oplog.jsonl", "").unwrap().is_empty());
}

#[test]
fn the_line_is_its_number_in_the_file_blank_lines_counted() {
    assert_eq!(field_of("{\"n\":1,\"at\":[]}\n\n{\"n\":\"two\",\"at\":[]}\n").as_deref(), Some("line 3.n"));
}

#[test]
fn a_field_is_named_by_its_path_inside_the_line() {
    assert_eq!(field_of("{\"n\":1,\"at\":[1.0,\"x\"]}").as_deref(), Some("line 1.at[1]"));
}

#[test]
fn a_missing_field_is_named_and_an_optional_one_is_not_missing() {
    assert_eq!(field_of("{\"at\":[]}").as_deref(), Some("line 1.n"));
    assert_eq!(field_of("{\"n\":1,\"at\":[]}"), None);
}

#[test]
fn text_that_is_not_json_names_the_line_alone() {
    assert_eq!(field_of("not json").as_deref(), Some("line 1"));
}

#[test]
fn trailing_text_is_a_damaged_line_as_from_str_reads_it() {
    let glued = "{\"n\":1,\"at\":[]}{\"n\":2,\"at\":[]}";
    assert!(serde_json::from_str::<serde_json::Value>(glued).is_err(), "the reference reader refuses it");
    assert_eq!(field_of(glued).as_deref(), Some("line 1"));
}

#[test]
fn the_reason_counts_the_lines_after_the_first_and_never_names_line_one_of_a_line() {
    let text = "{\"n\":1,\"at\":[]}\n{\"n\":\"x\",\"at\":[]}\nbroken\n{\"at\":[]}\n";
    let r = read_lines::<Row>("oplog", "oplog.jsonl", text).unwrap_err();
    assert_eq!(r.kind, "oplog");
    assert_eq!(r.file, "oplog.jsonl");
    assert_eq!(r.field.as_deref(), Some("line 2.n"));
    assert!(r.reason.ends_with("; 2 more lines do not read"), "{}", r.reason);
    assert!(!r.reason.contains("line 1"), "serde_json's own position is a position in the line, not in the file: {}", r.reason);
    assert!(r.reason.contains("(column "), "{}", r.reason);
    assert_eq!(r.sentence(), format!("oplog.jsonl (line 2.n): {}", r.reason));
    let one_more = read_lines::<Row>("oplog", "f", "bad\nworse\n").unwrap_err();
    assert!(one_more.reason.ends_with("; 1 more line does not read"), "{}", one_more.reason);
}
