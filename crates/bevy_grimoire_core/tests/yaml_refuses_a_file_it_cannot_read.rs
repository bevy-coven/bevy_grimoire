//! **One reader for a project file, and the rules it refuses by** (GD-01, #363).
//!
//! Every project file this series moves to YAML is parsed by `bevy_grimoire_core::yaml`, and a file
//! that cannot be read is one `Problem` (P7, `docs/ui/vision.md`): the file, the line and column, the
//! field path (`placements[3].turn_deg`), what was expected, and the fix.
//!
//! The reader refuses what a person or an agent could otherwise misread, rather than guessing:
//!
//! - an unknown field, a duplicate key;
//! - an anchor, an alias and a `<<` merge key — a file must say everything where it stands, so a
//!   value reached through `*b` is refused at the `&b` that put it somewhere else;
//! - more than one document;
//! - a tab used for indentation.
//!
//! It follows YAML 1.2 through `serde_yaml_ng`, so only `true` and `false` are booleans: `no`, `on`
//! and `yes` are strings, and a file that gives one to a `bool` is refused with a sentence saying so.

use bevy_grimoire_core::problem::Problem;
use bevy_grimoire_core::yaml;
use serde::Deserialize;

/// The file every test in this file names.
const PATH: &str = "maps/yard.map.yaml";

/// A file shaped like a project's: a version, a name, a list, and a switch.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Yard {
    version: u32,
    name: String,
    placements: Vec<Placement>,
    sealed: bool,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Placement {
    id: String,
    turn_deg: f32,
}

/// A file that reads, in the style this reader writes: two spaces per level.
const GOOD: &str = "\
version: 9
name: yard
placements:
  - id: bed
    turn_deg: 90.0
sealed: true
";

#[allow(clippy::result_large_err)] // the refusal is `Problem`, the one shape every surface renders
fn read(text: &str) -> Result<Yard, Problem> {
    yaml::from_str::<Yard>(text, PATH)
}

fn refused(text: &str) -> Problem {
    read(text).expect_err("this file must not read")
}

/// The field path a refusal names, when it names one.
fn field(problem: &Problem) -> Option<&str> {
    problem.where_.as_ref().and_then(|where_| where_.field.as_deref())
}

/// Why a refusal refused, as a sentence.
fn why(problem: &Problem) -> String {
    problem.why.clone().unwrap_or_else(|| panic!("a refusal says why: {}", problem.line()))
}

#[test]
fn a_file_reads_as_its_type() {
    assert_eq!(
        read(GOOD).unwrap_or_else(|e| panic!("{}", e.line())),
        Yard {
            version: 9,
            name: "yard".into(),
            placements: vec![Placement { id: "bed".into(), turn_deg: 90.0 }],
            sealed: true,
        }
    );
}

#[test]
fn a_file_read_from_disk_reads_as_its_type() {
    // The scratch path says which test wrote it, so two runs cannot collide.
    let name = format!("yaml_refuses_a_file_it_cannot_read-{}.yaml", std::process::id());
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, GOOD).expect("the test can write its own scratch file");
    let yard: Yard = yaml::read(&path).unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(yard.name, "yard");
    std::fs::remove_file(&path).expect("the test removes its own scratch file");
}

#[test]
fn a_file_that_is_not_there_is_refused_naming_it() {
    let path = std::env::temp_dir().join(format!(
        "yaml_refuses_a_file_it_cannot_read-{}-absent.yaml",
        std::process::id()
    ));
    let problem = yaml::read::<Yard>(&path).expect_err("a file that is not there cannot read");
    assert_eq!(problem.where_.as_ref().and_then(|w| w.file.as_deref()), Some(path.display().to_string().as_str()));
    assert!(problem.fix.is_some(), "a refusal says what to do: {}", problem.line());
}

#[test]
fn an_unknown_field_is_refused_at_its_line_and_path() {
    let text = format!("{GOOD}floor: 2\n");
    let problem = refused(&text);
    assert_eq!(field(&problem), Some("floor"));
    let why = why(&problem);
    assert!(why.contains("line 7"), "the refusal names the line: {why}");
    assert!(why.contains("floor"), "the refusal names the field it refused: {why}");
    assert!(problem.fix.is_some(), "the refusal names the fix: {}", problem.line());
}

#[test]
fn a_key_given_twice_is_refused_at_its_line_and_path() {
    let text = GOOD.replace("name: yard\n", "name: yard\nname: court\n");
    let problem = refused(&text);
    assert_eq!(field(&problem), Some("name"), "the duplicate is named by its path: {}", problem.line());
    let why = why(&problem);
    assert!(why.contains("line 3"), "the refusal names the line the second key is on: {why}");
    assert!(why.contains('2'), "the refusal says where the key was first given: {why}");
}

#[test]
fn a_key_given_twice_inside_a_list_is_named_by_its_whole_path() {
    let text = GOOD.replace("    turn_deg: 90.0\n", "    turn_deg: 90.0\n    turn_deg: 45.0\n");
    let problem = refused(&text);
    assert_eq!(field(&problem), Some("placements[0].turn_deg"), "{}", problem.line());
}

#[test]
fn an_anchor_and_the_alias_it_feeds_are_refused_where_the_anchor_stands() {
    // One construct, refused once: a file that points at itself says less where it stands, and the
    // `&` is where that starts, so the alias it feeds is refused with it rather than after it.
    let text = "\
version: 9
name: yard
base: &b
  id: bed
  turn_deg: 90.0
placements:
  - *b
sealed: true
";
    let problem = refused(text);
    assert_eq!(field(&problem), Some("base"), "{}", problem.line());
    let why = why(&problem);
    assert!(why.contains("line 3"), "the refusal names the line the anchor is on: {why}");
    assert!(problem.fix.is_some(), "{}", problem.line());
}

#[test]
fn an_alias_with_nothing_to_point_at_is_refused_at_its_line() {
    let text = GOOD.replace("sealed: true", "sealed: *nowhere");
    let problem = refused(&text);
    assert_eq!(field(&problem), Some("sealed"), "the refusal names the field the alias stands at: {}", problem.line());
    let why = why(&problem);
    assert!(why.contains("line 6"), "the refusal names the line the alias is on: {why}");
    assert!(why.contains("alias"), "the refusal says what it refused, which is an alias and not `text that is not YAML`: {why}");
    assert!(problem.fix.is_some(), "{}", problem.line());
}

#[test]
fn a_broken_anchor_is_refused_as_a_broken_anchor_and_not_as_an_alias() {
    // `a: &` names nothing, so there is no alias in the file: the parser's complaint is about the `&`
    // itself (`did not find expected alphabetic or numeric character`). Calling that an alias would name
    // a construct the file does not have.
    let text = GOOD.replace("name: yard", "name: &");
    let problem = refused(&text);
    assert_eq!(field(&problem), Some("name"), "{}", problem.line());
    // The refusal's own sentence is the `what`; what it *found* is in `why`, and there is no alias in
    // the file for it to name.
    let why = why(&problem);
    assert!(why.contains("a broken anchor"), "{why}");
    assert!(why.contains("line 2"), "{why}");
    assert!(!why.contains("an alias (`"), "there is no alias in the file: {why}");
}

#[test]
fn a_merge_key_is_refused_where_it_copies_another_mapping_in() {
    let text = "\
version: 9
name: yard
placements:
  - <<: {id: bed, turn_deg: 90.0}
sealed: true
";
    let problem = refused(text);
    assert_eq!(field(&problem), Some("placements[0]"), "{}", problem.line());
    let why = why(&problem);
    assert!(why.contains("line 4"), "the refusal names the line the merge key is on: {why}");
}

#[test]
fn a_second_document_is_refused_at_its_line() {
    let text = format!("{GOOD}---\nversion: 9\nname: court\nplacements: []\nsealed: false\n");
    let problem = refused(&text);
    let why = why(&problem);
    assert!(why.contains("line 7"), "the refusal names the line the second document starts on: {why}");
    assert!(why.contains("one document"), "the refusal says what it expects: {why}");
    assert!(problem.fix.is_some(), "{}", problem.line());
}

#[test]
fn one_explicit_document_marker_is_one_document_and_reads() {
    let text = format!("---\n{GOOD}");
    assert_eq!(read(&text).unwrap_or_else(|e| panic!("{}", e.line())).version, 9);
}

#[test]
fn a_tab_used_for_indentation_is_refused_at_its_line_and_path() {
    let text = "version: 9\nname: yard\nplacements:\n\t- id: bed\n    turn_deg: 90.0\nsealed: true\n";
    let problem = refused(text);
    assert_eq!(field(&problem), Some("placements"), "{}", problem.line());
    let why = why(&problem);
    assert!(why.contains("line 4"), "the refusal names the line the tab is on: {why}");
    assert!(why.contains("tab"), "{why}");
    assert!(problem.fix.is_some(), "{}", problem.line());
}

#[test]
fn a_tab_at_the_start_of_a_line_is_refused_though_a_reader_might_take_it() {
    let text = "\tversion: 9\nname: yard\nplacements: []\nsealed: true\n";
    let problem = refused(text);
    let why = why(&problem);
    assert!(why.contains("line 1"), "{why}");
    assert!(why.contains("tab"), "{why}");
}

#[test]
fn a_tab_inside_a_later_block_scalar_does_not_replace_the_refusal_before_it() {
    // The duplicate key is the file's first fault, and it is what a person has to fix: a tab that is
    // content of a block scalar further down must not become the answer.
    let text = "version: 9\nversion: 9\nname: |\n  a\n  \tb\nplacements: []\nsealed: true\n";
    let problem = refused(text);
    let why = why(&problem);
    assert!(why.contains("`version` is given twice"), "{why}");
    assert!(why.contains("line 2"), "the second `version` is on line 2: {why}");
}

#[test]
fn a_block_scalar_after_a_duplicate_key_is_not_known_yet_and_takes_the_answer_from_it() {
    // `saphyr-parser` stops at the duplicate key on line 2, so the block scalar on line 2 onward was
    // never recorded: its content tab must not be read as indentation, and the duplicate key — the first
    // fault, and the one a person has to fix — is the answer. The slack a tab may take from the walk's
    // own tab report applies to a tab report only, never to a duplicate, an anchor or a second document.
    let text = "version: 9\nversion: |\n  \tb\nname: x\nplacements: []\nsealed: true\n";
    let problem = refused(text);
    let why = why(&problem);
    assert!(why.contains("`version` is given twice"), "{why}");
    assert!(why.contains("line 2"), "{why}");
}

#[test]
fn a_tab_inside_a_string_is_content_and_reads() {
    let text = GOOD.replace("name: yard", "name: \"a\\tb\"");
    let yard = read(&text).unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(yard.name, "a\tb");
}

#[test]
fn a_tab_inside_a_block_scalar_is_content_and_reads() {
    let text = GOOD.replace("name: yard", "name: |\n  a\n  \tb");
    let yard = read(&text).unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(yard.name, "a\n\tb\n");
}

#[test]
fn no_on_and_yes_read_as_strings() {
    // YAML 1.2 reads only `true` and `false` as booleans, so the three words this file's kind of
    // prose is made of stay what they were written as.
    for word in ["no", "on", "yes", "y", "n", "off"] {
        let text = GOOD.replace("name: yard", &format!("name: {word}"));
        let yard = read(&text).unwrap_or_else(|e| panic!("`{word}` into a String: {}", e.line()));
        assert_eq!(yard.name, word);
    }
}

#[test]
fn no_on_and_yes_are_refused_for_a_bool_and_the_refusal_says_so() {
    for word in ["no", "on", "yes"] {
        let text = GOOD.replace("sealed: true", &format!("sealed: {word}"));
        let problem = refused(&text);
        assert_eq!(field(&problem), Some("sealed"), "{}", problem.line());
        let why = why(&problem);
        assert!(why.contains("expected a boolean"), "the refusal says what it expected: {why}");
        assert!(why.contains(word), "the refusal names what it found: {why}");
    }
}

#[test]
fn a_value_of_the_wrong_type_is_refused_at_its_whole_path() {
    let text = GOOD.replace("    turn_deg: 90.0\n", "    turn_deg: bed\n");
    let problem = refused(&text);
    assert_eq!(field(&problem), Some("placements[0].turn_deg"), "{}", problem.line());
    assert!(why(&problem).contains("expected"), "{}", problem.line());
}

#[test]
fn a_refusal_that_has_a_position_gives_it_and_one_that_has_none_claims_none() {
    // Every refusal this reader can reach from a file has a position, and says it. One that has none —
    // `serde_yaml_ng` answers none for a stream it could not shape into a document — says none rather
    // than `line 1, column 1`; that half is `yaml::tests::a_refusal_with_no_position_invents_none`.
    let problem = refused("just a string\n");
    let why = why(&problem);
    assert!(why.starts_with("line "), "a refusal that has a position gives it: {why}");
    assert!(why.contains("expected struct Yard"), "{why}");
}

#[test]
fn every_refusal_reads_as_one_sentence_that_names_the_file() {
    let cases = [
        format!("{GOOD}floor: 2\n"),
        GOOD.replace("name: yard\n", "name: yard\nname: court\n"),
        GOOD.replace("sealed: true", "sealed: no"),
        format!("{GOOD}---\nversion: 9\n"),
    ];
    for text in cases {
        let problem = refused(&text);
        let line = problem.line();
        assert!(line.starts_with("maps/yard.map.yaml did not read"), "{line}");
        assert!(line.contains("Fix: "), "{line}");
        assert!(!line.contains("  "), "no run of blanks in a refusal: {line}");
    }
}
