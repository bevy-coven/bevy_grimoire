//! **Only the JSON view refuses `.nan` and `.inf`** (#367's review): JSON has no spelling for them, so
//! `yaml::to_json` refuses them, naming the line; a typed reader (`yaml::from_str`) takes a plain `.inf` as
//! the float it is, so a project file a kind wrote with one still reads back.

use bevy_grimoire_core::yaml;

#[test]
fn a_typed_reader_reads_an_infinity_and_the_json_view_refuses_it() {
    let text = "limit: .inf\n";
    let read: std::collections::BTreeMap<String, f64> = yaml::from_str(text, "limits.yaml").unwrap_or_else(|e| panic!("{}", e.line()));
    assert!(read["limit"].is_infinite(), "a typed reader keeps the float the file says");
    let refused = yaml::to_json(text, "limits.yaml").expect_err("JSON cannot hold an infinity");
    assert!(refused.line().contains("line 1"), "the refusal names the line: {}", refused.line());
}
