//! **The editor's chrome says plain words** (#302, refs #227): the other workspace is `Kit` or `Map`, the
//! labeler is silent until configured, the triangle count waits behind View -> Show statistics, the history
//! list draws only rows that exist, and a receipt reads as a person would say it.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_gooey::testing::find_by_label;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    app
}

/// Turns Show statistics on the way a person does: the user's settings file says so, and the editor reads it.
fn statistics_on(app: &mut App, tag: &str) {
    let file = std::env::temp_dir().join(format!("grimoire_stats_{tag}_{}/settings.ron", std::process::id()));
    std::fs::create_dir_all(file.parent().expect("a folder")).expect("a scratch folder");
    std::fs::write(&file, "(version: 1, show_statistics: Some(true))").expect("a settings file");
    app.insert_resource(bevy_cauldron::settings::UserSettingsFile(file));
    for _ in 0..80 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(15));
        if app.world().resource::<bevy_cauldron::settings::EditorSettings>().show_statistics {
            break;
        }
    }
    for _ in 0..3 {
        app.update();
    }
}

#[test]
fn a_receipt_drops_the_instance_number_and_rounds_a_position_to_a_tenth() {
    use bevy_cauldron::editor::plain_receipt;
    assert_eq!(plain_receipt("placed lamp_tall@1 at (2.31, 1.31)"), "placed lamp_tall at (2.3, 1.3)");
    assert_eq!(plain_receipt("placed crate@12 at (-2.00, 2.05)"), "placed crate at (-2.0, 2.0)");
    assert_eq!(plain_receipt("scale 1.25 of tile_a@0"), "scale 1.25 of tile_a", "only a coordinate pair is rounded");
    assert_eq!(plain_receipt("mail@home.example"), "mail@home.example", "an @ that is not an instance stays");
}

#[test]
fn the_strip_names_the_other_workspace_as_map_or_kit() {
    let mut app = editor("plain_strip");
    assert!(find_by_label(&mut app, "Kit").is_ok(), "the Map workspace's strip leads to Kit");
    assert!(find_by_label(&mut app, "the kit door").is_err());
}

#[test]
fn the_labeler_and_the_triangle_count_say_nothing_until_asked() {
    let mut app = editor("plain_status");
    let mut q = app.world_mut().query::<&Text>();
    let texts: Vec<String> = q.iter(app.world()).map(|t| t.0.clone()).collect();
    assert!(!texts.iter().any(|t| t.contains("labeler: not configured")), "an unconfigured labeler is not a status: {texts:?}");
    assert!(!texts.iter().any(|t| t.contains("tris drawn")), "the triangle count is off until View -> Show statistics: {texts:?}");
    statistics_on(&mut app, "chrome");
    let mut q = app.world_mut().query::<&Text>();
    assert!(q.iter(app.world()).any(|t| t.0.contains("tris drawn")), "with statistics on the count shows");
}
