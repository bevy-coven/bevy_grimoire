const KNOWN: &[&str] = &["bevy_elements", "bevy_flora", "nope_not_listed"];
use bevy_grimoire_core::family::{path_for_project, Family};

#[test]
fn a_project_without_a_family_file_uses_no_crate() {
    let d = std::env::temp_dir().join(format!("grimoire_family_none_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    assert_eq!(Family::load(&d).unwrap(), Family::default());
    assert!(path_for_project(&d).ends_with("family.yaml"));
}

#[test]
fn a_listed_crate_without_its_feature_is_a_problem_not_a_silent_skip() {
    let f = Family::parse("version: 1\ncrates: [bevy_elements, bevy_flora]\n").unwrap();
    let problems = f.problems(KNOWN, &["bevy_elements"]);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("bevy_flora") && problems[0].contains("--features bevy_flora"), "{problems:?}");
    assert_eq!(f.active(&["bevy_elements"]), vec!["bevy_elements"]);
    assert!(f.problems(KNOWN, &["bevy_elements", "bevy_flora"]).is_empty());
}

#[test]
fn an_unknown_name_and_a_repeat_are_refused_or_reported() {
    let f = Family::parse("version: 1\ncrates: [nope]\n").unwrap();
    assert!(f.problems(&["bevy_elements"], &["nope"])[0].contains("not a family crate"));
    assert!(Family::parse("version: 1\ncrates: [isomesh, isomesh]\n").unwrap_err().contains("twice"));
    assert!(Family::parse("version: 2\ncrates: []\n").is_err());
}

const SETTINGS: &str = "Settings(\n    // sun\n    sun: (hour: 6.5, tilt: 0.4),\n    label: \"a, (b)\",\n    wind: 3,\n)\n";

#[test]
fn set_field_rewrites_only_the_named_value_and_returns_the_old_one() {
    use bevy_grimoire_core::family::set_field;
    let (text, old) = set_field(SETTINGS, "sun.hour", "12.0").unwrap();
    assert_eq!(old, "6.5");
    assert_eq!(text, SETTINGS.replace("hour: 6.5", "hour: 12.0"));
    // The change undoes by writing the old value back.
    let (back, _) = set_field(&text, "sun.hour", &old).unwrap();
    assert_eq!(back, SETTINGS);
    // A value holding commas and brackets inside a string is one value.
    let (t, old) = set_field(SETTINGS, "label", "\"x\"").unwrap();
    assert_eq!(old, "\"a, (b)\"");
    assert!(t.contains("label: \"x\",") && t.contains("wind: 3"));
    // A whole struct is a value too.
    let (t, old) = set_field(SETTINGS, "sun", "(hour: 1.0, tilt: 0.0)").unwrap();
    assert_eq!(old, "(hour: 6.5, tilt: 0.4)");
    assert!(t.contains("sun: (hour: 1.0, tilt: 0.0),"));
}

#[test]
fn set_field_names_what_is_missing_and_refuses_a_value_that_is_not_one() {
    use bevy_grimoire_core::family::set_field;
    assert!(set_field(SETTINGS, "sun.nope", "1").unwrap_err().contains("nope"));
    assert!(set_field(SETTINGS, "wind.deep", "1").is_err());
    assert!(set_field(SETTINGS, "wind", "(1, 2").is_err());
    assert!(set_field(SETTINGS, "wind", "1, other: 2").is_err());
}
