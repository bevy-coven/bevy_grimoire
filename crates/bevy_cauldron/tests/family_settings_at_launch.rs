//! **A settings file already present when the project opens is in force on the first frame** (#98, #96).
//!
//! The watcher reads the files when it is created but reports only *changes*, so settings that
//! were there from the start never reached `FamilySettings` until the file was edited once: the
//! environment preview stayed undressed after a launch.

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::family_host::{reload_settings, FamilySettings};

use crate::fixtures;

#[test]
fn a_settings_file_present_at_launch_is_applied_without_an_edit() {
    let root = fixtures::Fixture::new("family_launch").descriptor("site/wall", "alpha").build("m");
    let file = root.join("family").join("bevy_elements.ron");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "(sun: (hour: 6.0))").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<EditorState>().init_resource::<FamilySettings>().add_systems(Update, reload_settings);
    app.update();
    assert_eq!(app.world().resource::<FamilySettings>().text["bevy_elements"], "(sun: (hour: 6.0))");
}
