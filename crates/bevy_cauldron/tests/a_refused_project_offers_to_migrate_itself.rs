//! **A project refused for still holding RON is offered the migration on the screen that refused it** (GD-03, #365).
//!
//! The refusal names `bevy_cauldron migrate <path>`, and a person reading it in a bundle launch has no terminal to run that in.
//! The loading screen's card carries a button that runs the same library call, and this pins the two things the button is: the
//! offer appears for exactly a project the migration has work for, and pressing it converts the files and puts the person on
//! the road that opens the project. A project refused for another reason is offered nothing.

use std::path::PathBuf;

use crate::fixtures;

use bevy_cauldron::loading::{LoadRequest, migrate_now, migrate_offer};

fn project(tag: &str, with_ron: bool) -> PathBuf {
    // **A real fixture project**, which binds a kit and holds a map: a hand-made folder with `bind: []` is refused for a
    // different reason ("binds no kit"), and this file is about the migration one.
    let root = fixtures::Fixture::new(&format!("migrate_offer_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place("alpha/floor", (1.0, 1.0))
        .build("m");
    if with_ron {
        std::fs::write(root.join("assets/grimoire/editor.ron"), "(version: 1, text_scale: Some(1.25))").unwrap_or_else(|e| panic!("{e}"));
    }
    root
}

fn request(root: &std::path::Path) -> LoadRequest {
    LoadRequest(vec![root.to_string_lossy().into_owned()])
}

#[test]
fn a_project_holding_ron_is_offered_the_migration() {
    let root = project("offered", true);
    let offered = migrate_offer(Some(&request(&root)), true);
    assert_eq!(offered.as_deref(), Some(root.as_path()), "the project the screen was asked to open, and nothing else");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_project_that_is_already_yaml_is_offered_nothing() {
    let root = project("nothing", false);
    assert_eq!(migrate_offer(Some(&request(&root)), true), None, "a migration with nothing to do is not offered");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_refusal_that_is_not_the_migrations_offers_nothing() {
    // **A project refused for another reason is never offered a rewrite of itself.** `refused` is false on a screen that is
    // loading, which is every frame before the answer comes back.
    let root = project("loading", true);
    assert_eq!(migrate_offer(Some(&request(&root)), false), None);
    assert_eq!(migrate_offer(None, true), None, "a screen that was asked to open nothing offers nothing");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn pressing_it_converts_the_project_the_way_the_subcommand_does() {
    let root = project("presses", true);
    let lines = migrate_now(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert!(lines.iter().any(|l| l.contains("editor.ron -> assets/grimoire/editor.yaml")), "one line per file, as the command prints: {lines:?}");
    let editor = std::fs::read_to_string(root.join("assets/grimoire/editor.yaml")).unwrap_or_default();
    assert!(editor.contains("text_scale: 1.25"), "the person's setting survived: {editor}");
    assert!(!root.join("assets/grimoire/editor.ron").exists(), "the RON is moved aside");
    assert!(root.join(".dark_factory/migrated-ron/assets/grimoire/editor.ron").exists(), "into the archive");
    // And it is a project the editor can now open, which is the point of the button.
    bevy_cauldron::project::Project::open(&root, None).unwrap_or_else(|e| panic!("the migrated project opens: {e}"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_migration_that_refuses_says_why_and_leaves_the_project_alone() {
    let root = project("refuses", true);
    std::fs::write(root.join("assets/grimoire/editor.ron"), r#"(version: 1, text_scale: "#).unwrap_or_else(|e| panic!("{e}"));
    let refused = migrate_now(&root).expect_err("a file that is not RON stops the migration");
    assert!(refused.line().contains("editor.ron"), "{}", refused.line());
    assert!(!root.join("assets/grimoire/editor.yaml").exists(), "and nothing was written");
    let _ = std::fs::remove_dir_all(&root);
}

/// **A project refused only because a `data.yaml` names a `.ron` file is offered the migration too** (review MUST-FIX 1).
///
/// `leftover` refuses on `plan` *and* the files a registry names, and `migrate_as` converts both — but the offer was decided
/// by `plan` alone. So the one refusal a person most needs the button for (their own registry naming a file they cannot find)
/// showed the sentence with no button, and the doc saying "exactly the projects `bevy_cauldron migrate` would convert" was
/// false for it.
#[test]
fn a_registry_naming_a_ron_file_is_offered_the_migration() {
    let root = project("registry_only", false);
    // **Nothing but a registry names a .ron here**: `editor.yaml` is YAML, so `plan` finds nothing and only the registered file
    // decides the refusal.
    std::fs::write(root.join("assets/grimoire/maps/data.yaml"), "version: 1\nfiles:\n- id: tuning\n  path: mob.ron\n  bounds: {}\n")
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("assets/grimoire/maps/mob.ron"), "(mob: (speed: 1.5))").unwrap_or_else(|e| panic!("{e}"));

    // The refusal is the registry's file, and nothing else: this is what the open says.
    let problem = bevy_grimoire_migrate::leftover(&root, "bevy_cauldron migrate /p")
        .unwrap_or_else(|p| panic!("{}", p.line()))
        .unwrap_or_else(|| panic!("a registry naming a .ron file refuses the open"));
    assert!(problem.line().contains("mob.ron"), "{}", problem.line());

    let offered = migrate_offer(Some(&request(&root)), true);
    assert_eq!(offered.as_deref(), Some(root.as_path()), "and that refusal offers the migration: {problem:?}", problem = problem.line());
    let _ = std::fs::remove_dir_all(&root);
}
