//! **A registered data file is read by one reader that names the file and the leaf** (#200, H3).
//!
//! A game's tuning file has no type here, so the registry says what it must hold: every leaf it bounds
//! must be there, a number and inside its bounds. `data_files::check` refuses a file that does not,
//! naming the file and the leaf; `data_files::read` is the reader under it, the one a data edit uses.

use bevy_grimoire_core::data_files::{self, DataFile, DataFiles};
use bevy_grimoire_core::reload_check::Refusal;

const REGISTRY: &str = "version: 1\nfiles:\n- id: tuning/mob\n  path: mob.yaml\n  bounds:\n    mob.speed:\n      min: 0.0\n      max: 9.0\n    mob.hp:\n      min: 1.0\n";
const GOOD: &str = "# tuning\nmob:\n  speed: 1.5\n  name: orc\n  hp: 7\nfast: false\n";

fn entry() -> DataFile {
    DataFiles::parse(REGISTRY).unwrap().files.remove(0)
}

fn refused(text: &str) -> Refusal {
    data_files::check("mob.yaml", &entry(), text).expect_err("the file was accepted")
}

#[test]
fn a_file_that_holds_every_bounded_leaf_inside_its_bounds_reads() {
    assert_eq!(data_files::check("mob.yaml", &entry(), GOOD), Ok(()));
    let leaves = data_files::read("mob.yaml", GOOD).unwrap();
    assert!(leaves.contains(&("mob.speed".to_owned(), "1.5".to_owned())), "{leaves:?}");
}

#[test]
fn a_file_cut_off_is_refused_at_the_leaf_where_reading_stopped() {
    // **A file that stopped mid-value**, not one that stopped after a key: `name: ` alone is a null, which
    // YAML reads happily. A document cut off after `name: or` is the file that does not read, and the leaf it
    // stopped at is the one the refusal names.
    let r = refused("# tuning\nmob:\n  speed: 1.5\n  name: \"orc");
    assert_eq!((r.kind, r.file.as_str(), r.field.as_deref()), ("game_data", "mob.yaml", Some("mob.name")), "{r:?}");
    assert!(r.sentence().starts_with("mob.yaml (mob.name): "), "{}", r.sentence());
}

#[test]
fn text_that_is_not_yaml_is_refused_with_the_file_and_no_field() {
    let r = refused("this is not a tuning file");
    assert_eq!((r.file.as_str(), r.field.as_deref()), ("mob.yaml", None), "{r:?}");
    assert!(r.sentence().starts_with("mob.yaml: "), "{}", r.sentence());
    assert_eq!(refused("").field, None, "an empty file is not a tuning file");
}

#[test]
fn a_bounded_leaf_of_the_wrong_type_is_refused_by_its_path() {
    let r = refused("mob:\n  speed: fast\n  name: orc\n  hp: 7\nfast: false\n");
    assert_eq!((r.file.as_str(), r.field.as_deref()), ("mob.yaml", Some("mob.speed")), "{r:?}");
    assert!(r.reason.contains("a string") && r.reason.contains("number"), "{}", r.reason);
}

#[test]
fn a_bounded_leaf_the_file_lacks_is_refused_by_its_path_and_the_nearest_are_named() {
    let r = refused("mob:\n  speed: 1.5\n  name: orc\nfast: false\n");
    assert_eq!((r.file.as_str(), r.field.as_deref()), ("mob.yaml", Some("mob.hp")), "{r:?}");
    assert!(r.reason.contains("no such leaf") && r.reason.contains("`mob.speed`"), "{}", r.reason);
}

#[test]
fn a_bounded_leaf_outside_its_bounds_is_refused_by_its_path_with_the_range() {
    let r = refused("mob:\n  speed: 99.0\n  name: orc\n  hp: 7\nfast: false\n");
    assert_eq!(r.field.as_deref(), Some("mob.speed"), "{r:?}");
    assert!(r.reason.contains("99.0") && r.reason.contains("[0, 9]"), "{}", r.reason);
    let open = refused("mob:\n  speed: 1.0\n  name: orc\n  hp: 0\nfast: false\n");
    assert_eq!(open.field.as_deref(), Some("mob.hp"));
    assert!(open.reason.contains("[1, inf]"), "an open end is `inf`: {}", open.reason);
}

#[test]
fn only_the_leaves_the_registry_bounds_are_held_to_a_type() {
    // `name` and `fast` are the game's own business: this crate has no type for them.
    assert_eq!(data_files::check("mob.yaml", &entry(), "mob:\n  speed: 1.5\n  name: 3\n  hp: 7\nfast: \"yes\"\n"), Ok(()));
}

#[test]
fn the_first_refusal_by_path_order_is_the_one_named() {
    let r = refused("mob:\n  speed: 99.0\n  name: orc\nfast: false\n");
    assert_eq!(r.field.as_deref(), Some("mob.hp"), "`mob.hp` sorts before `mob.speed`: {r:?}");
}
