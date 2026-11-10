//! **Each enabled family crate's own settings, edited by gooey's reflect inspector** (UI-57, #243).
//!
//! The project's `family.yaml` lists the family crates it uses, and each one's settings are its own serde
//! type written as RON in `<project>/family/<crate>.yaml` — the file `family_set` writes and a running
//! game's copy of the plugin reads. #96 put one typed line in the Inspector (`bevy_elements sun.hour 6.5`)
//! and an agent could `family_set` a field; what was missing is what a person reaches for: the fields
//! themselves, drawn from the crate's type and edited in place.
//!
//! **The panel is gooey's reflect inspector over a view of the crate's file.** `bevy_elements`'
//! `ElementsSettings` is the editor's wrapper around the crate's own serde types and is not
//! `#[reflect(Resource)]`, so it cannot be a reflected target here; a reflected write straight onto it
//! would also be a second path to the file, skipping the RON surgery, the refusal and the inverse that
//! `family_set` owns. So the panel edits [`FamilyView`], a reflected view of the file's fields at the
//! dot-paths `family_set` already takes (`clock.hour`, `climate.mean_temperature_c`), and an
//! `InspectorChange` becomes one dispatch — one undo step, one log line, the file on disk. That is the
//! shape `inspector_view.rs` uses for the selection's pose: the editor's own type, one writer per fact.
//!
//! **Acceptance: editing `bevy_elements`' time of day changes the sky within one frame.** That is measured
//! on the sky itself — `bevy_elements`' `SunState` altitude, which its celestial plugin keeps current from
//! the `WorldClock` — not on a picture. Noon's sun is 68 degrees up; midnight's is below the horizon, which
//! a number in a settings file cannot be confused for.
//!
//! **The time of day is the one field that is not a file write, and it is not an exception invented here:**
//! it is the scrub the Environment panel's four hour buttons have set since #98 (`ClockScrub`: camera-style
//! state, no file, no undo step — turning the clock to look at dusk is not an edit to the project), and it
//! is the field whose *file* edits are deliberately walked over a second (#271) so the camera's exposure
//! follows the sky's. Both are asserted below, so the two paths cannot drift into one another.
//!
//! **Two things the panel has to get right beyond that, and both were wrong in its first cut** (`#243`
//! review): a settled panel must not read its files every frame, and the read-only block of a crate this
//! build has no fields for must follow its file rather than showing the text it was spawned with.

#![cfg(feature = "bevy_elements")]

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::family_host::FamilySettings;
use bevy_cauldron::family_panel::{FamilyBlock, FamilyPanel, FamilyView};
use bevy_elements::WorldClock;

use crate::fixtures;

/// How many frames a headless editor is stepped before it is asked what it shows.
const SETTLE_FRAMES: usize = 6;

/// The crate the read-only half of the panel is exercised on: always built (`family_host::built` says
/// so), always listed by a project that names it, and never a crate this build draws fields of — its
/// settings type is `bevy_gooey`'s own skin, which the editor previews through `ui_skin` instead.
const SKIN_CRATE: &str = "bevy_gooey";

/// A partial skin file, which the crate's `#[serde(default)]` reads: the default skin with one field.
const SKIN_BLUE: &str = "{accent: !Srgba {red: 0.2, green: 0.6, blue: 0.9, alpha: 1.0}}";

/// The same skin with the accent moved, for the block to follow.
const SKIN_VERMILION: &str = "{accent: !Srgba {red: 0.9, green: 0.3, blue: 0.2, alpha: 1.0}}";

/// **A project that uses `bevy_elements`**, with the settings an author's project carries, and the editor
/// running `bevy_elements`' own plugins so the preview's sky is the crate's sky (what `main.rs` adds).
fn editor(tag: &str, hour: f64) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("family_panel_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.yaml"), "version: 1\ncrates: [bevy_elements]\n").unwrap();
    let mut settings = bevy_cauldron::environment::ElementsSettings::defaults();
    settings.clock.hour = hour;
    let file = root.join("family/bevy_elements.yaml");
    std::fs::write(&file, settings.to_yaml()).unwrap();

    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // The preview's sky, without a window: `bevy_elements`' celestial plugin keeps `SunState` from the
    // `WorldClock` in `Update`, so the frame after the clock moves is the frame the sun is placed for.
    app.add_plugins(bevy_elements::ElementsPlugins);
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    (app, file)
}

/// **A project that uses `bevy_elements` and `bevy_gooey`**, for the block the panel shows as its file.
fn editor_with_a_file_block(tag: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("family_panel_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.yaml"), format!("version: 1\ncrates: [bevy_elements, {SKIN_CRATE}]\n")).unwrap();
    std::fs::write(root.join("family/bevy_elements.yaml"), bevy_cauldron::environment::ElementsSettings::defaults().to_yaml()).unwrap();
    let skin = root.join(format!("family/{SKIN_CRATE}.yaml"));
    std::fs::write(&skin, SKIN_BLUE).unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_elements::ElementsPlugins);
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    (app, skin)
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// The hour the preview's clock is at.
fn clock_hour(app: &App) -> f64 {
    app.world().get_resource::<WorldClock>().map_or(f64::NAN, |c| c.hour)
}

/// The sun's true altitude above the horizon, degrees: the sky's own answer to "what time is it".
fn sun_altitude(app: &App) -> f32 {
    app.world().get_resource::<bevy_elements::SunState>().map_or(f32::NAN, |s| s.altitude_deg)
}

/// The entity of `bevy_elements`' view.
fn view_of(app: &mut App) -> Entity {
    app.world_mut()
        .query::<(Entity, &FamilyView)>()
        .iter(app.world())
        .find(|(_, v)| v.krate == "bevy_elements")
        .map(|(e, _)| e)
        .expect("the panel has a view for bevy_elements")
}

/// The view's fields, as the inspector's own state holds them.
fn view(app: &mut App) -> FamilyView {
    app.world_mut()
        .query::<&FamilyView>()
        .iter(app.world())
        .find(|v| v.krate == "bevy_elements")
        .cloned()
        .expect("the panel has a view for bevy_elements")
}

/// **What gooey does after a person edits a field**: it writes the view, then triggers the change **on the
/// inspector**, whose `target` is the view — `bevy_gooey::inspector::apply` sends
/// `InspectorChange { entity: inspector, path }`, never the view's own entity, so a test that triggered it
/// on the view would prove a path the person's click never takes.
fn edit(app: &mut App, path: &str, change: impl FnOnce(&mut FamilyView)) {
    let view_entity = view_of(app);
    {
        let mut held = app.world_mut().get_mut::<FamilyView>(view_entity).expect("the view");
        change(&mut held);
    }
    let inspector = app
        .world_mut()
        .query::<(Entity, &bevy_gooey::Inspector)>()
        .iter(app.world())
        .find(|(_, i)| i.target == bevy_gooey::InspectTarget::component::<FamilyView>(view_entity))
        .map(|(e, _)| e)
        .expect("gooey's inspector over the view is in the panel");
    app.world_mut().trigger(bevy_gooey::InspectorChange { entity: inspector, path: path.to_owned() });
}

/// **The panel exists for a crate the project lists, and its view carries that crate's file's fields.**
///
/// The heading names the crate, and the view holds the clock's hour read out of the file — not the
/// editor's clock, not a default: the file an author's project ships.
#[test]
fn a_listed_crate_gets_a_panel_of_its_own_settings() {
    let (mut app, _file) = editor("shows", 6.5);
    settle(&mut app);
    let panel = app.world().resource::<FamilyPanel>().clone();
    assert!(
        panel.crates().any(|c| c == "bevy_elements"),
        "`family.yaml` lists bevy_elements, so the panel must offer it: {:?}",
        panel.crates().collect::<Vec<_>>()
    );
    assert!(panel.has_fields("bevy_elements"), "this build holds the crate's settings type, so it draws its fields");
    let view = app
        .world_mut()
        .query::<&FamilyView>()
        .iter(app.world())
        .find(|v| v.krate == "bevy_elements")
        .cloned()
        .unwrap_or_else(|| panic!("bevy_elements has no view; the panel draws its settings from one"));
    assert_eq!(view.hour, 6.5, "the view reads the file's hour, not the editor's clock: {view:?}");
    let headings: Vec<String> = app.world_mut().query::<&Text>().iter(app.world()).map(|t| t.0.clone()).collect();
    assert!(
        headings.iter().any(|h| h == "BEVY_ELEMENTS"),
        "the panel names the crate it shows, as every section in this editor does: {headings:?}"
    );
}

/// **The acceptance: editing the time of day in the panel changes the sky within one frame.**
///
/// One frame, not a settle. The file holds noon; the panel is edited to midnight; the sun is below the
/// horizon where noon's sun was 68 degrees up.
#[test]
fn editing_the_hour_in_the_panel_moves_the_sky_in_one_frame() {
    let (mut app, file) = editor("sky", 12.0);
    settle(&mut app);
    let noon_altitude = sun_altitude(&app);
    assert!(noon_altitude > 30.0, "noon's sun is high, or this test cannot tell the two skies apart: {noon_altitude}");
    let before = std::fs::read_to_string(&file).expect("the settings file");

    edit(&mut app, "hour", |view| view.hour = 0.0);
    // **One frame**, not a settle: the acceptance is "within one frame".
    app.update();

    let night_altitude = sun_altitude(&app);
    assert!(
        night_altitude < 0.0,
        "the edit is midnight's sky, and the sun is below the horizon there: {night_altitude} degrees (noon was {noon_altitude})"
    );
    assert!((clock_hour(&app) - 0.0).abs() < 1e-6, "the preview's clock is at the hour the panel was set to: {}", clock_hour(&app));
    assert_eq!(
        std::fs::read_to_string(&file).expect("the settings file"),
        before,
        "the hour is camera-style state, as the Environment panel's hour buttons are: looking at dusk is not an edit to the project"
    );
}

/// **The hour the panel shows is the hour it was edited to, and stays there.**
///
/// The hour is a scrub, so the file never learns it — and a panel that rebuilt its view from the file
/// after the edit would put the file's noon back on the next frame, undoing the person's own edit with
/// no word said. The view is read after a settle: the file still says 12, the preview is at midnight, and
/// the field agrees with the preview.
#[test]
fn the_hour_a_person_set_stays_set_after_the_panel_settles() {
    let (mut app, _file) = editor("hour-sticks", 12.0);
    settle(&mut app);

    edit(&mut app, "hour", |v| v.hour = 0.0);
    app.update();
    settle(&mut app);

    assert_eq!(
        view(&mut app).hour,
        0.0,
        "the panel shows the hour the person set, not the file's 12 — a field that snaps back is one the \
         person cannot use (the file still says {}, the preview is at {})",
        12.0,
        clock_hour(&app)
    );
    assert!((clock_hour(&app) - 0.0).abs() < 1e-6, "and the preview is still at that hour: {}", clock_hour(&app));
}

/// **A settled panel reads nothing.**
///
/// The panel is derived from `family.yaml`, `FamilySettings`, the preview's held hour and the project
/// root; a frame in which none of them moved must not load `family.yaml`, read a settings file or parse
/// RON. The first cut did all three sixty times a second on a panel that was not looking at anything new.
#[test]
fn a_settled_panel_reads_its_files_once_and_then_not_again() {
    let (mut app, _file) = editor("settled-reads", 9.0);
    settle(&mut app);
    let reads = app.world().resource::<FamilyPanel>().reads();
    assert!(reads >= 1, "the panel read its files at least once while it was being built");

    for _ in 0..20 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<FamilyPanel>().reads(),
        reads,
        "twenty frames in which the project, `family.yaml`, the settings text and the held hour all stayed \
         put cost the panel no read at all; {reads} reads before, {} after",
        app.world().resource::<FamilyPanel>().reads()
    );

    // **And it reads again the moment an input moves**, or the gate above would be satisfied by a panel
    // that never reads at all.
    app.world_mut().resource_mut::<FamilySettings>().text.insert("bevy_elements".to_owned(), bevy_cauldron::environment::ElementsSettings::defaults().to_yaml());
    app.update();
    assert!(
        app.world().resource::<FamilyPanel>().reads() > reads,
        "a settings text that changed is a read: {} reads, and it was {reads}",
        app.world().resource::<FamilyPanel>().reads()
    );
}

/// **The text a person reads under a crate's heading**, found the way a person finds it: the block names
/// the crate, and the text under that heading that is not the heading itself is the settings shown.
///
/// Not by a marker the implementation carries — the test is of what is on screen, so a block whose label
/// the system failed to update has to fail it however the label is stored.
fn shown(app: &mut App, krate: &str) -> String {
    let block = app
        .world_mut()
        .query::<(Entity, &FamilyBlock)>()
        .iter(app.world())
        .find(|(_, b)| b.krate == krate)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("the panel draws {krate}'s settings as its file"));
    let children: Vec<Entity> = app.world().get::<Children>(block).map(|c| c.iter().collect()).unwrap_or_default();
    let heading = krate.to_uppercase();
    children
        .iter()
        .filter_map(|child| app.world().get::<Text>(*child))
        .map(|t| t.0.clone())
        .find(|t| *t != heading)
        .unwrap_or_else(|| panic!("{krate}'s block shows its settings under the heading"))
}

/// **A crate this build has no fields for shows its settings file, and that text follows the file.**
///
/// `bevy_gooey`'s skin is previewed through `ui_skin` rather than drawn as fields, so its block is its
/// file as RON — the answer the panel gives for any crate whose settings type it does not hold. The
/// block was spawned with the file's text once, so a `family_set` line left it showing the settings from
/// before the edit; the text a person reads has to be the text a game would.
#[test]
fn the_read_only_block_follows_its_crates_file() {
    let (mut app, skin) = editor_with_a_file_block("file-block");
    settle(&mut app);
    assert_eq!(shown(&mut app, SKIN_CRATE), SKIN_BLUE, "the block opens on the file as it is on disk");

    // The edit arrives the way a `family_set` line's does: the text the watch took as good.
    app.world_mut().resource_mut::<FamilySettings>().text.insert(SKIN_CRATE.to_owned(), SKIN_VERMILION.to_owned());
    assert_eq!(std::fs::read_to_string(&skin).expect("the skin file"), SKIN_BLUE, "the file itself is untouched by the panel");
    app.update();
    assert_eq!(
        shown(&mut app, SKIN_CRATE),
        SKIN_VERMILION,
        "the block shows the settings the editor now holds, not the ones it was spawned with"
    );
}

/// **Every field that is not the hour is the project's, written to the file a game reads**: one dispatch
/// through `family_set`, one undo step, and the preview drawn at the new value in the same frame.
#[test]
fn a_field_edit_writes_the_crates_file_and_is_one_undo_step() {
    let (mut app, file) = editor("write", 12.0);
    settle(&mut app);
    let before = app.world().resource::<EditorState>().undo_depth();
    edit(&mut app, "mean_temperature_c", |view| view.mean_temperature_c = -4.0);
    app.update();
    let after = app.world().resource::<EditorState>().undo_depth();
    assert_eq!(after, before + 1, "one edit in the panel is one undo step, as the typed line is");
    let written = std::fs::read_to_string(&file).expect("the settings file");
    assert!(written.contains("mean_temperature_c: -4.0"), "the edit is in the file a game reads:\n{written}");
    let preview = app.world().get_resource::<bevy_elements::Climate>().map(|c| c.mean_temperature_c);
    assert_eq!(preview, Some(-4.0), "and the preview's climate is the file's, in the same frame");
}

/// **A typed month or day outside the calendar is kept inside it**, so a scrub past December writes the
/// month the crate's own type accepts rather than a date `ron` reads back wrong.
#[test]
fn a_month_typed_past_december_writes_december() {
    let (mut app, file) = editor("month", 12.0);
    settle(&mut app);
    edit(&mut app, "month", |view| view.month = 40);
    app.update();
    let written = std::fs::read_to_string(&file).expect("the settings file");
    assert!(written.contains("month: 12"), "a month of 40 is kept inside the year:\n{written}");
}

/// **A crate this build was compiled without has no panel**: it has no type to draw a field from, and
/// `family_host::problems_with` is where an author is told why.
#[test]
fn a_crate_this_build_does_not_have_gets_no_panel() {
    let root = fixtures::Fixture::new(&format!("family_panel_absent_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.yaml"), "version: 1\ncrates: [bevy_carnage]\n").unwrap();
    std::fs::write(root.join("family/bevy_carnage.yaml"), "{hitstop_seconds: 0.05}").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    let built = bevy_cauldron::family_host::built();
    let panel = app.world().resource::<FamilyPanel>().clone();
    if built.contains(&"bevy_carnage") {
        // Built: the crate is offered, and its settings are drawn as its file, not as another crate's fields.
        assert!(panel.crates().any(|c| c == "bevy_carnage"), "{:?}", panel.crates().collect::<Vec<_>>());
        assert!(!panel.has_fields("bevy_carnage"), "this build has no reflected type for bevy_carnage's settings");
        assert!(panel.text_of("bevy_carnage").is_some_and(|t| t.contains("hitstop_seconds")));
    } else {
        assert!(
            !panel.crates().any(|c| c == "bevy_carnage"),
            "a crate this build was compiled without has no panel: {:?}",
            panel.crates().collect::<Vec<_>>()
        );
        let problems = bevy_cauldron::family_host::problems_with(&root, &built);
        assert!(
            problems.iter().any(|p| p.contains("bevy_carnage")),
            "and the author is told in the file's own words why: {problems:?}"
        );
    }
}
