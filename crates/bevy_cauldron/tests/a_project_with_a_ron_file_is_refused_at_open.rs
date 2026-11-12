//! **A project with a RON file left over is refused at open, by every way in, and migrating it lets it open** (GD-03, #365; review M5 a, b).
//!
//! The refusal is raised by `Project::open_staged` through `bevy_grimoire_migrate::leftover`, so deleting that call fails every test
//! here. The sentence names the file and the command with **the project's real path**, not a placeholder.

use std::path::PathBuf;

use bevy_cauldron::project::Project;

use crate::fixtures;

/// A fixture project with one leftover RON file of an editor kind.
fn project(tag: &str) -> (PathBuf, String) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("alpha/floor", "alpha").place("alpha/floor", (1.0, 1.0)).build("m");
    let rel = format!("assets/grimoire/{}/forest/yard.ron", fixtures::DEFAULT_KIT);
    let path = root.join(&rel);
    std::fs::create_dir_all(path.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&path, "(version: 1, species: [(name: \"oak\", kind: Pine(height_m: 4.0))])").unwrap_or_else(|e| panic!("{e}"));
    (root, rel)
}

fn assert_refused(said: &str, rel: &str, root: &std::path::Path) {
    assert!(said.contains(rel), "the file is named: {said}");
    assert!(said.contains(&format!("bevy_cauldron migrate {}", root.display())), "and the command, with this project's own path: {said}");
}

#[test]
fn project_open_and_open_staged_refuse_naming_the_file_and_the_command() {
    let (root, rel) = project("ron_open");
    let said = Project::open(&root, None).err().unwrap_or_else(|| panic!("a project holding RON is refused"));
    assert_refused(&said, &rel, &root);
    let said = Project::open_staged(&root, None, &mut |_| {}).err().unwrap_or_else(|| panic!("and so is the staged open"));
    assert_refused(&said, &rel, &root);
}

#[test]
fn the_headless_editor_refuses_it_too() {
    let (root, rel) = project("ron_headless");
    let said = bevy_cauldron::harness::build_headless(&root, "m", None).err().unwrap_or_else(|| panic!("the editor opened a project holding RON"));
    assert_refused(&said, &rel, &root);
}

/// **An agent's `grimoire/project_open` is refused too, with the same sentence** (review M5 a).
///
/// The op checks the folder through `launcher::check_project` and then queues `project_assets::open_project`, which points at the
/// project before any screen moves. So the refusal is what `point_at` answers — a plain string the op logs — and this is the
/// path an agent takes, which the three tests above never exercise: they go through `Project::open_staged` directly.
#[test]
fn an_agents_project_open_is_refused_naming_the_file_and_the_command() {
    let (root, rel) = project("ron_agent_open");
    let mut world = bevy::prelude::World::new();
    let said = bevy_cauldron::project_assets::open_project(&mut world, &root, None)
        .err()
        .unwrap_or_else(|| panic!("an agent opened a project holding RON"));
    assert_refused(&said, &rel, &root);
}

/// **Migrating the project is what lets it open**: the leftover file converts, the project opens with no problem, and the
/// forest reads from its YAML.
#[test]
fn migrating_a_project_holding_a_ron_editor_file_lets_it_open() {
    let (root, rel) = project("ron_migrates");
    assert!(Project::open(&root, None).is_err());
    let report = bevy_grimoire_migrate::convert_project(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(report.converted.len(), 1, "{:?}", report.converted);
    let yaml = root.join(rel.replace(".ron", ".yaml"));
    let text = std::fs::read_to_string(&yaml).unwrap_or_else(|e| panic!("{}: {e}", yaml.display()));
    assert!(text.contains("!Pine"), "the converted forest keeps its variant: {text}");
    Project::open(&root, None).unwrap_or_else(|e| panic!("the migrated project opens: {e}"));
    let _editor = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("and the editor opens it: {e}"));
}
