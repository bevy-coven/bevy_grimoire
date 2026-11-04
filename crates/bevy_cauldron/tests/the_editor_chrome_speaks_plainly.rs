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

/// Steps until `done` holds, sleeping between frames for the settings watcher's thread, and says what it waited for.
fn until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
    for _ in 0..120 {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    panic!("never saw: {what}");
}

fn says_tris_drawn(app: &mut App) -> bool {
    let mut q = app.world_mut().query::<&Text>();
    q.iter(app.world()).any(|t| t.0.contains("tris drawn"))
}

/// **View -> Show statistics is a tick that shows and hides the triangle readout** (#302): choosing the row
/// writes the user's settings file, the editor reads it back, and the readout and the tick follow; choosing it
/// again takes both away.
#[test]
fn choosing_show_statistics_toggles_the_triangle_readout() {
    let mut app = editor("plain_toggle");
    let file = std::env::temp_dir().join(format!("grimoire_stats_toggle_{}/settings.ron", std::process::id()));
    std::fs::create_dir_all(file.parent().expect("a folder")).expect("a scratch folder");
    std::fs::write(&file, "(version: 1)").expect("a settings file");
    app.insert_resource(bevy_cauldron::settings::UserSettingsFile(file));
    for _ in 0..3 {
        app.update();
    }
    assert!(!says_tris_drawn(&mut app), "the readout starts hidden");

    let item = |app: &mut App| {
        app.world_mut()
            .query_filtered::<Entity, With<bevy_cauldron::menu_bar::StatisticsItem>>()
            .single(app.world())
            .expect("View has a Show statistics row")
    };
    let ticked = |app: &mut App| {
        let row = item(app);
        app.world().get::<bevy_gooey::Checked>(row).is_some()
    };
    assert!(!ticked(&mut app), "and the tick starts off");

    let row = item(&mut app);
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: row });
    until(&mut app, "the readout after choosing the row", says_tris_drawn);
    app.update();
    assert!(ticked(&mut app), "the tick follows the setting");

    let row = item(&mut app);
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: row });
    until(&mut app, "the readout gone after choosing the row again", |app| !says_tris_drawn(app));
    app.update();
    assert!(!ticked(&mut app), "and the tick goes with it");
}

/// **The history draws only the rows that exist and still holds the room of all seven** (#302): a fresh
/// map has one entry (where it stands), so one row draws, and the six spare ones are hidden, not removed,
/// so nothing below them moves when an edit fills one.
#[test]
fn the_history_draws_only_real_rows_and_keeps_its_height() {
    use bevy::ui::ComputedNode;
    let mut app = editor("plain_history");
    let mut q = app.world_mut().query::<(&bevy_cauldron::editor::HistorySlot, &Visibility, &ComputedNode)>();
    let rows: Vec<(bool, f32)> = q.iter(app.world()).map(|(_, v, n)| (*v != Visibility::Hidden, n.size().y)).collect();
    assert_eq!(rows.len(), 7, "the list is seven rows tall");
    assert_eq!(rows.iter().filter(|(shown, _)| *shown).count(), 1, "one row draws on a fresh map: {rows:?}");
    assert!(rows.iter().all(|(_, height)| *height > 0.0), "a hidden row still holds its height: {rows:?}");
    let first = rows[0].1;
    assert!(rows.iter().all(|(_, height)| (*height - first).abs() < 0.01), "every row is the same height: {rows:?}");
}

/// The left and right edges of a laid-out text's box, in logical px.
fn edges_of(app: &mut App, entity: Entity) -> (f32, f32) {
    use bevy::ui::{ComputedNode, UiGlobalTransform};
    let node = app.world().get::<ComputedNode>(entity).expect("a laid-out node");
    let at = app.world().get::<UiGlobalTransform>(entity).expect("a placed node");
    let (centre, width) = (at.translation.x * node.inverse_scale_factor, node.size().x * node.inverse_scale_factor);
    (centre - width / 2.0, centre + width / 2.0)
}

/// The text whose words are `words`.
fn text_called(app: &mut App, words: &str) -> Entity {
    let mut q = app.world_mut().query::<(Entity, &Text)>();
    let found: Vec<Entity> = q.iter(app.world()).filter(|(_, t)| t.0 == words).map(|(e, _)| e).collect();
    assert_eq!(found.len(), 1, "exactly one text says `{words}`: {found:?}");
    found[0]
}

/// **A label never touches its value, and every value of the readout starts in one column** (#302):
/// `RELATIONS` fits its column and `UNDER CURSOR` stands on two lines inside it, where either used to push
/// its own value right of the others.
#[test]
fn every_readout_value_starts_where_the_brush_value_does_and_clear_of_its_label() {
    let mut app = editor("plain_columns");
    let mut value_left = Vec::new();
    for label in ["BRUSH", "EDGES", "UNDER CURSOR", "RELATIONS"] {
        let label_text = text_called(&mut app, label);
        let row = app.world().get::<ChildOf>(label_text).expect("a label stands in a row").parent();
        let value = app.world().get::<Children>(row).and_then(|kids| kids.get(1).copied()).expect("a row has a value after its label");
        let (_, label_right) = edges_of(&mut app, label_text);
        let (value_start, _) = edges_of(&mut app, value);
        assert!(value_start >= label_right + 1.0, "{label}: its value starts at {value_start} and the label ends at {label_right}: they touch");
        value_left.push((label, value_start));
    }
    let brush = value_left[0].1;
    for (label, left) in &value_left {
        assert!((left - brush).abs() < 0.5, "{label}'s value starts at {left}, BRUSH's at {brush}: {value_left:?}");
    }
}

/// **A receipt and the BRUSH row name a piece the way the palette does** (#302): `lamp_tall` is `Lamp tall`,
/// and a word the library does not know keeps its underscore.
#[test]
fn a_receipt_names_a_piece_in_words_and_leaves_other_words_alone() {
    use bevy_cauldron::editor::named_receipt;
    use bevy_grimoire_core::descriptor::Descriptor;
    use bevy_grimoire_core::library::Library;
    let library = Library {
        descriptors: vec![
            Descriptor { id: "site/lamp_tall".into(), ..Default::default() },
            Descriptor { id: "crate".into(), ..Default::default() },
        ],
        ..Default::default()
    };
    assert_eq!(named_receipt("placed lamp_tall at (2.3, 1.3)", &library), "placed lamp tall at (2.3, 1.3)");
    assert_eq!(named_receipt("placed crate at (2.3, 1.3)", &library), "placed crate at (2.3, 1.3)", "an id with no underscore is already words");
    assert_eq!(named_receipt("wrote my_notes.txt", &library), "wrote my_notes.txt", "a word the library does not know is left alone");
    assert_eq!(named_receipt("added `bath` \u{2014} it is in the palette", &library), "added Bath \u{2014} it is in the palette", "a quoted id is the palette's name, unquoted");
    assert_eq!(named_receipt("added `lamp_tall`", &library), "added Lamp tall");
    assert_eq!(named_receipt("wrote `kits.ron`, undo with `Ctrl+Z`", &library), "wrote `kits.ron`, undo with `Ctrl+Z`", "a quoted file or chord is not a name");
    assert_eq!(named_receipt("a stray ` tick", &library), "a stray ` tick", "an odd number of ticks is not a quoting");
}

/// **The BRUSH and UNDER CURSOR rows and the status hint say no file-safe id** (#302): the armed piece is
/// `Lamp tall` as in the palette, and the hint names the project the way out goes to.
#[test]
fn the_brush_row_and_the_status_hint_speak_in_words() {
    let root = fixtures::Fixture::new("plain_brush").descriptor("lamp_tall", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    app.world_mut().resource_mut::<bevy_cauldron::editor::EditorState>().brush = Some(0);
    for _ in 0..3 {
        app.update();
    }
    let mut q = app.world_mut().query::<&Text>();
    let said: Vec<String> = q.iter(app.world()).map(|t| t.0.clone()).collect();
    assert!(said.iter().any(|t| t == "Lamp tall"), "the BRUSH row reads as the palette does: {said:?}");
    assert!(!said.iter().any(|t| t == "lamp_tall"), "and never as the id: {said:?}");
    let hint = said.iter().find(|t| t.contains("every key lands on what it does")).expect("the status band holds the key hint");
    assert!(!hint.contains("kits & maps"), "there is no kits and maps screen: {hint}");
    let project = root.file_name().expect("a folder name").to_string_lossy().into_owned();
    assert!(hint.ends_with(&format!("back to {project}")), "the hint names where the way out goes: {hint}");
}
