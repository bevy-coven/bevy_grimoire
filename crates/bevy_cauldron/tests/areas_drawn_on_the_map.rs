//! **Areas drawn on the map** (SG-06, #405): an area is a rectangle on its level's map, written to its own `area.yaml` in the
//! design tree by `design_area_set`, and named where the cursor is.
//!
//! What is proved here: an area is created, moved and the move undone, each step one log line and `area.yaml` holding the first
//! rectangle after the undo; an overlapping rectangle is refused naming both areas, and a rectangle off the map is refused; removing an
//! area that has a page moves its folder to `.dark_factory/removed-areas/` and the undo moves it back; the style chain at a point
//! inside the area ends with the area's files; the overlay's areas follow the files within a second, the line under the cursor names
//! the area, and the inspector's Style link opens the area's page.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_cauldron::design_areas::{self, StyleAreas};
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::workspace::Workspace;
use bevy_grimoire_core::design::{self, Tree};
use serde_json::{Value, json};

use crate::fixtures;

fn editor(tag: &str) -> (App, Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..4 {
        app.update();
    }
    let ctx = Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(app.world().resource::<OpenMap>().clone()),
        session: tag.to_owned(),
        ..Ctx::default()
    };
    (app, ctx, root)
}

fn log_lines(ctx: &Ctx) -> usize {
    let dir = &ctx.project.as_ref().expect("a project").project_dir;
    std::fs::read_to_string(dir.join("oplog.jsonl")).map_or(0, |t| t.lines().count())
}

fn set(ctx: &mut Ctx, params: Value) -> String {
    let before = log_lines(ctx);
    match ops::dispatch(ctx, "design_area_set", &params) {
        Ok(Applied::Done { operation_id, .. }) => {
            assert_eq!(log_lines(ctx), before + 1, "{params} is one log line");
            operation_id
        }
        Ok(Applied::Refused(e)) | Err(e) => panic!("{params} was refused: {}", e.0),
    }
}

fn refusal(ctx: &mut Ctx, params: Value) -> String {
    let before = log_lines(ctx);
    let said = match ops::dispatch(ctx, "design_area_set", &params) {
        Ok(Applied::Done { outcome, .. }) => panic!("{params} was accepted: {}", outcome.summary),
        Ok(Applied::Refused(e)) | Err(e) => e.0,
    };
    assert_eq!(log_lines(ctx), before, "a refusal is no log line");
    said
}

fn rect_of(root: &std::path::Path, name: &str) -> Option<[f32; 4]> {
    design::areas_of_level(root, "design/levels/m").into_iter().find(|a| a.name == name).map(|a| a.rect)
}

const DOCKS: [f32; 4] = [-6.0, -6.0, -2.0, -2.0];
const DOCKS_MOVED: [f32; 4] = [-5.0, -5.0, -1.0, -1.0];
const QUAY: [f32; 4] = [2.0, 2.0, 6.0, 6.0];

#[test]
fn an_area_is_created_moved_and_the_move_undone_each_step_one_log_line() {
    let (_app, mut ctx, root) = editor("area_steps");
    set(&mut ctx, json!({"name": "docks", "rect": DOCKS}));
    assert_eq!(rect_of(&root, "docks"), Some(DOCKS));
    let file = std::fs::read_to_string(root.join("design/levels/m/areas/docks/area.yaml")).unwrap();
    assert!(file.contains("version: 1"), "{file}");
    let moved = set(&mut ctx, json!({"name": "docks", "rect": DOCKS_MOVED}));
    assert_eq!(rect_of(&root, "docks"), Some(DOCKS_MOVED));

    let before = log_lines(&ctx);
    ops::undo(&mut ctx, Some(&moved)).unwrap_or_else(|e| panic!("undo refused: {}", e.0));
    assert_eq!(log_lines(&ctx), before + 1, "the undo is one log line");
    assert_eq!(rect_of(&root, "docks"), Some(DOCKS), "area.yaml holds the first rectangle");

    // The map file is the map's own: an area never writes it.
    let map_file = std::fs::read_to_string(root.join("assets/grimoire/maps/m.map.yaml")).unwrap();
    assert!(!map_file.contains("docks"), "a map file is unchanged by having areas");
}

#[test]
fn an_overlap_and_a_rectangle_off_the_map_are_refused_and_nothing_is_written() {
    let (_app, mut ctx, root) = editor("area_refused");
    set(&mut ctx, json!({"name": "docks", "rect": DOCKS}));
    let said = refusal(&mut ctx, json!({"name": "quay", "rect": [-4.0, -4.0, 0.0, 0.0]}));
    assert!(said.contains("quay") && said.contains("docks") && said.contains("overlap"), "both areas are named: {said}");
    assert!(rect_of(&root, "quay").is_none() && !root.join("design/levels/m/areas/quay").exists(), "a refusal writes nothing");

    // Areas that only touch share a line and no cell.
    set(&mut ctx, json!({"name": "quay", "rect": [-2.0, -6.0, 2.0, -2.0]}));
    // Moving an area onto another one is the same refusal.
    let said = refusal(&mut ctx, json!({"name": "quay", "rect": [-3.0, -6.0, 2.0, -2.0]}));
    assert!(said.contains("quay") && said.contains("docks"), "{said}");

    for (what, params) in [
        ("off the map", json!({"name": "far", "rect": [20.0, 20.0, 24.0, 24.0]})),
        ("backwards", json!({"name": "far", "rect": [4.0, 4.0, 0.0, 0.0]})),
        ("a name that climbs out", json!({"name": "../far", "rect": QUAY})),
        ("a map that is not there", json!({"name": "far", "map": "nowhere", "rect": QUAY})),
    ] {
        let said = refusal(&mut ctx, params);
        assert!(said.contains("Nothing was changed") || said.contains("nowhere"), "{what}: {said}");
    }
    assert!(!root.join("design/levels/m/areas/far").exists());
}

#[test]
fn removing_an_area_with_a_page_moves_its_folder_aside_and_the_undo_moves_it_back() {
    let (_app, mut ctx, root) = editor("area_removed");
    set(&mut ctx, json!({"name": "docks", "rect": DOCKS}));
    set(&mut ctx, json!({"name": "bare", "rect": QUAY}));
    let folder = root.join("design/levels/m/areas/docks");
    std::fs::write(folder.join("style.md"), "# Docks\n\nCranes and rope.\n").unwrap();

    // An area that is only its rectangle is taken away.
    set(&mut ctx, json!({"name": "bare"}));
    assert!(!root.join("design/levels/m/areas/bare").exists());
    assert!(!root.join(".dark_factory/removed-areas/bare").exists(), "nothing worth keeping is kept");

    let removal = set(&mut ctx, json!({"name": "docks"}));
    assert!(!folder.exists(), "the area is gone from the level");
    let kept = root.join(".dark_factory/removed-areas/docks");
    assert_eq!(std::fs::read_to_string(kept.join("style.md")).unwrap(), "# Docks\n\nCranes and rope.\n", "its page is kept");
    assert!(kept.join("area.yaml").is_file());
    assert!(refusal(&mut ctx, json!({"name": "docks"})).contains("no area"), "removing what is not there says so");

    ops::undo(&mut ctx, Some(&removal)).unwrap_or_else(|e| panic!("undo refused: {}", e.0));
    assert_eq!(rect_of(&root, "docks"), Some(DOCKS));
    assert_eq!(std::fs::read_to_string(folder.join("style.md")).unwrap(), "# Docks\n\nCranes and rope.\n");
    assert!(!kept.exists(), "the folder moved back rather than being copied");

    // A second removal does not land on the name the first used.
    set(&mut ctx, json!({"name": "docks"}));
    std::fs::create_dir_all(folder.parent().unwrap()).unwrap();
    set(&mut ctx, json!({"name": "docks", "rect": DOCKS}));
    std::fs::write(folder.join("style.md"), "second").unwrap();
    set(&mut ctx, json!({"name": "docks"}));
    assert_eq!(std::fs::read_to_string(root.join(".dark_factory/removed-areas/docks-2/style.md")).unwrap(), "second");
}

#[test]
fn the_style_chain_at_a_point_inside_an_area_ends_with_the_areas_files() {
    let (_app, mut ctx, root) = editor("area_chain");
    std::fs::create_dir_all(root.join("design/levels/m")).unwrap();
    std::fs::write(root.join("design/style.md"), "# Game\n").unwrap();
    std::fs::write(root.join("design/levels/m/style.md"), "# Yard\n").unwrap();
    set(&mut ctx, json!({"name": "docks", "rect": DOCKS}));
    std::fs::write(root.join("design/levels/m/areas/docks/style.md"), "# Docks\n").unwrap();

    let tree = Tree::read(&root);
    assert!(tree.problems.is_empty(), "{:?}", tree.problems.iter().map(|p| p.line()).collect::<Vec<_>>());
    let files = |at: Option<(f32, f32)>| -> Vec<String> {
        tree.chain_to(Some("m"), at).iter().flat_map(|p| p.topic("style")).map(|t| t.file.clone()).collect()
    };
    assert_eq!(files(Some((-4.0, -4.0))), ["design/style.md", "design/levels/m/style.md", "design/levels/m/areas/docks/style.md"], "the area's files come last");
    assert_eq!(files(Some((4.0, 4.0))), ["design/style.md", "design/levels/m/style.md"], "a point outside every area has none");
}

fn pump(app: &mut App, secs: f64) {
    let start = std::time::Instant::now();
    while start.elapsed().as_secs_f64() < secs {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
}

#[test]
fn the_overlays_areas_follow_the_files_and_the_line_under_the_cursor_names_the_area() {
    let (mut app, mut ctx, root) = editor("area_overlay");
    assert!(!app.world().resource::<StyleAreas>().on, "the outline is off by default");
    set(&mut ctx, json!({"name": "docks", "rect": DOCKS}));
    pump(&mut app, design_areas::REFRESH_S as f64 + 0.8);
    let areas = app.world().resource::<StyleAreas>();
    assert_eq!(areas.level.as_deref(), Some("design/levels/m"));
    assert_eq!(areas.area_at((-4.0, -4.0)).map(|a| a.name.as_str()), Some("docks"));
    assert!(areas.area_at((4.0, 4.0)).is_none());

    // An area written by hand is read as well, and a removal takes it off.
    std::fs::create_dir_all(root.join("design/levels/m/areas/yard")).unwrap();
    std::fs::write(root.join("design/levels/m/areas/yard/area.yaml"), "version: 1\nrect: [2.0, 2.0, 6.0, 6.0]\n").unwrap();
    pump(&mut app, design_areas::REFRESH_S as f64 + 0.8);
    assert_eq!(app.world().resource::<StyleAreas>().area_at((3.0, 3.0)).map(|a| a.name.as_str()), Some("yard"));

    // The words the status band's UNDER CURSOR line gets: the piece's own line, then the area.
    assert_eq!(design_areas::under_phrase(String::new(), Some("docks")), "in the area docks");
    assert_eq!(design_areas::under_phrase("Wall  — Cmd+E edits it".to_owned(), Some("docks")), "Wall  — Cmd+E edits it  — in the area docks");
    assert_eq!(design_areas::under_phrase("Wall".to_owned(), None), "Wall", "outside every area the line is the piece's alone");
}

#[test]
fn the_inspectors_style_link_opens_the_areas_page_and_the_levels_when_the_cursor_was_in_none() {
    let (mut app, mut ctx, root) = editor("area_link");
    let label_of = |app: &mut App| bevy_gooey::testing::find_by_label(app, "Open style of this level").ok().or_else(|| bevy_gooey::testing::find_by_label(app, "Open style of area docks").ok());
    pump(&mut app, 1.0);
    assert!(label_of(&mut app).is_none(), "a project with no style page has no Style link");

    std::fs::create_dir_all(root.join("design/levels/m")).unwrap();
    std::fs::write(root.join("design/levels/m/style.md"), "# Yard\n").unwrap();
    set(&mut ctx, json!({"name": "docks", "rect": DOCKS}));
    std::fs::write(root.join("design/levels/m/areas/docks/style.md"), "# Docks\n").unwrap();
    pump(&mut app, design_areas::REFRESH_S as f64 + 0.8);
    let link = bevy_gooey::testing::find_by_label(&mut app, "Open style of this level").unwrap_or_else(|e| panic!("the level's page has a link: {e:?}"));

    // The cursor was last in the area: the link is the area's.
    app.world_mut().resource_mut::<StyleAreas>().cursor_area = Some("docks".to_owned());
    pump(&mut app, 0.3);
    assert!(bevy_gooey::testing::find_by_label(&mut app, "Open style of area docks").is_ok(), "the area's page has the link now (it was {link:?})");
    // **The button the person sees is the one that carries the link**: Activate is what a press on it sends.
    let button = app.world_mut().query_filtered::<Entity, With<design_areas::StyleLink>>().single(app.world()).expect("one Style link");
    app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity: button });
    pump(&mut app, 0.5);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "a click reveals the page");
    assert_eq!(bevy_cauldron::files::active_file(app.world().resource::<bevy_cauldron::session::Session>()).as_deref(), Some("design/levels/m/areas/docks/style.md"));
}

#[test]
fn the_view_row_shows_the_outline_and_its_tick_follows_the_state() {
    let (mut app, _ctx, _root) = editor("area_row");
    let row = app.world_mut().query_filtered::<Entity, With<design_areas::StyleAreasItem>>().single(app.world()).expect("the View menu carries one Show style areas row");
    let ticked = |app: &mut App| app.world().entity(row).contains::<bevy_gooey::Checked>();
    assert!(!ticked(&mut app) && !app.world().resource::<StyleAreas>().on, "off by default, and the row says so");
    for want in [true, false] {
        app.world_mut().commands().trigger(bevy_gooey::ValueChange { source: row, value: want, is_final: true });
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<StyleAreas>().on, want);
        assert_eq!(ticked(&mut app), want, "the tick is the overlay's state");
    }
}

/// **`grimoire/style` with `at` inside an area returns the area's files last** (SG-02, #401, and SG-06's acceptance): the files the chain adds up, in the
/// order it adds them, and an `at` outside every area ends with the level's.
#[test]
fn grimoire_style_with_at_inside_an_area_returns_the_areas_files_last() {
    use bevy_grimoire_core::trust::TrustStore;
    let (mut app, mut ctx, root) = editor("area_style_at");
    app.add_plugins((bevy_debugger_bevy::DebuggerPlugin::default(), bevy_cauldron::ops_brp::OpsBridgePlugin));
    for (file, text) in [("design/style.md", "# Game\n"), ("design/levels/m/style.md", "# Yard\n")] {
        std::fs::create_dir_all(root.join(file).parent().unwrap()).unwrap();
        std::fs::write(root.join(file), text).unwrap();
    }
    set(&mut ctx, json!({"name": "docks", "rect": DOCKS}));
    std::fs::write(root.join("design/levels/m/areas/docks/style.md"), "# Docks\n").unwrap();
    // The person trusts the project, so every file is served.
    let store = root.join("trusted.yaml");
    let mut trusted = TrustStore::default();
    trusted.trust_project(&root).unwrap();
    trusted.save(&store).unwrap();
    app.world_mut().resource_mut::<bevy_cauldron::editor::EditorState>().trust.file = Some(store);
    for _ in 0..4 {
        app.update();
    }
    let paths = |app: &mut App, params: Value| -> Vec<String> {
        use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
        let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/style").cloned() else { panic!("`grimoire/style` is not registered") };
        let answer = app.world_mut().run_system_with(id, Some(params)).unwrap_or_else(|e| panic!("{e:?}")).unwrap_or_else(|e| panic!("{e:?}"));
        answer["files"].as_array().expect("files").iter().map(|f| f["path"].as_str().unwrap().to_owned()).collect()
    };
    assert_eq!(paths(&mut app, json!({"map": "m", "at": [-4.0, -4.0]})), ["design/style.md", "design/levels/m/style.md", "design/levels/m/areas/docks/style.md"]);
    assert_eq!(paths(&mut app, json!({"map": "m", "at": [4.0, 4.0]})), ["design/style.md", "design/levels/m/style.md"], "outside every area it ends with the level's");
}

/// **The cursor's reading, in its three cases** (`StyleAreas::observe_cursor`, which `sense_under_cursor` calls with the ground point): a point
/// inside an area sets it, a point outside every area clears it, and a cursor that is not on the map names none and keeps the last.
#[test]
fn the_cursor_reading_sets_clears_and_keeps_the_area() {
    let area = |name: &str, rect: [f32; 4]| design::AreaOnMap { name: name.to_owned(), path: format!("design/levels/m/areas/{name}"), rect, has_style: true };
    let mut areas = StyleAreas { areas: vec![area("docks", DOCKS), area("quay", QUAY)], ..StyleAreas::default() };
    assert_eq!(areas.observe_cursor(Some((-4.0, -4.0))).as_deref(), Some("docks"));
    assert_eq!(areas.cursor_area.as_deref(), Some("docks"), "a point inside sets it");
    assert_eq!(areas.observe_cursor(None), None, "a cursor on a panel names no area");
    assert_eq!(areas.cursor_area.as_deref(), Some("docks"), "and keeps the last, for the Style link on that panel");
    assert_eq!(areas.observe_cursor(Some((0.0, 0.0))), None);
    assert_eq!(areas.cursor_area, None, "a point outside every area clears it");
    assert_eq!(areas.observe_cursor(Some((4.0, 4.0))).as_deref(), Some("quay"));
}

/// **No undo loses a page.** Undoing the creation of an area that has since been given a page sets the folder aside, and the undo of that
/// undo moves it back; undoing a removal and doing it again sets the page aside again, where it was the first time.
#[test]
fn an_undo_of_a_creation_or_a_removal_keeps_the_page_and_its_own_undo_brings_it_back() {
    let (_app, mut ctx, root) = editor("area_undo_pages");
    let folder = root.join("design/levels/m/areas/docks");
    let id_of = |ctx: &mut Ctx, target: &str| -> String {
        match ops::undo(ctx, Some(target)).unwrap_or_else(|e| panic!("undo refused: {}", e.0)).1 {
            Applied::Done { operation_id, .. } => operation_id,
            Applied::Refused(e) => panic!("undo refused: {}", e.0),
        }
    };

    let created = set(&mut ctx, json!({"name": "docks", "rect": DOCKS}));
    std::fs::write(folder.join("style.md"), "# Docks\n").unwrap();
    let undone = id_of(&mut ctx, &created);
    assert!(!folder.exists(), "the area is gone from the level");
    assert_eq!(std::fs::read_to_string(root.join(".dark_factory/removed-areas/docks/style.md")).unwrap(), "# Docks\n", "its page is set aside, not deleted or left without a rectangle");
    id_of(&mut ctx, &undone);
    assert_eq!(rect_of(&root, "docks"), Some(DOCKS), "undoing the undo brings the area back");
    assert_eq!(std::fs::read_to_string(folder.join("style.md")).unwrap(), "# Docks\n");
    assert!(!root.join(".dark_factory/removed-areas/docks").exists());

    let removal = set(&mut ctx, json!({"name": "docks"}));
    let restored = id_of(&mut ctx, &removal);
    assert_eq!(std::fs::read_to_string(folder.join("style.md")).unwrap(), "# Docks\n");
    id_of(&mut ctx, &restored);
    assert!(!folder.exists(), "doing the removal again leaves no folder behind");
    assert_eq!(std::fs::read_to_string(root.join(".dark_factory/removed-areas/docks/style.md")).unwrap(), "# Docks\n");
}
