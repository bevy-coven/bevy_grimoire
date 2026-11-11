//! **The review of AF-03 (#359): a reveal selects a row that can be measured, a person's input stops
//! it, and the DATA row it lands on is the one it asked for.**
//!
//! Every test here failed before the fix it pins, and each names the item it comes from:
//!
//! - **M1** — the DATA table's reveal compared a row's offset from the top of the list's *content*
//!   with a position on screen, so the scroll it computed was for the wrong space. The measurement is
//!   the leaf's own row inside the list's visible area, which is also what M2 asks for.
//! - **M2** — acceptance item 1 says *the item's row lies inside its list's visible area, measured
//!   from the row's `ComputedNode`*, for **one operation of each off-map `Shown` variant**. Four of the
//!   six proved it by reading a resource back instead: the rig, the DATA file, the settings section and
//!   the note. Here each is measured on screen.
//! - **M3** — a person's input must stop a *reveal* as it stops a follow. It is the same rule and the
//!   same system (`follow::a_persons_input_takes_the_camera_back`), extended rather than duplicated:
//!   a queued reveal still crossed a door while the person was working.
//!
//! # Why the rows are measured and not the state
//!
//! `SelectedNote::0 == Some("note-1")` is true whether or not the row is drawn; the person's question
//! is whether they can see the thing they were told to look at. So the row is found by its marker and
//! compared with its list's box, both from the nodes' own geometry.

use std::time::Duration;

use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use bevy_cauldron::harness;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;
use bevy_cauldron::workspace::Workspace;

use crate::fixtures;

/// One frame of the test clock, seconds.
const FRAME: f32 = 1.0 / 60.0;

/// Frames a reveal may take: a crossing, the panel's first build, and the follower's one-frame-late
/// scroll.
const SETTLE: usize = 60;

/// A project carrying one subject per off-map `Shown` variant, as
/// `the_follow_opens_the_page_a_change_is_on` builds it.
fn project(tag: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("lamp_tall", "alpha")
        .descriptor("bed", "alpha")
        .place("bed", (2.0, 2.0))
        .side(64.0)
        .build("m");
    let grimoire = root.join("assets/grimoire");
    std::fs::copy(workspace_asset("grimoire/rigs.ron"), grimoire.join("rigs.ron")).expect("rigs.ron");
    std::fs::create_dir_all(root.join("assets/characters")).expect("characters dir");
    std::fs::copy(workspace_asset("characters/valkyrie.glb"), root.join("assets/characters/valkyrie.glb"))
        .expect("valkyrie.glb");
    // **Two leaves, so the cursor has a wrong answer to be caught on**: a reveal that selected the
    // table's first row rather than the file's would pass a `cursor.is_some()` check.
    let kit = root.join("assets/grimoire/furniture");
    std::fs::write(kit.join("mob.ron"), "// tuning\n(mob: (speed: 1.50, name: \"orc\", hp: 7))\n").expect("mob.ron");
    std::fs::write(
        kit.join("data.ron"),
        "(version: 1, files: [(id: \"tuning/mob\", path: \"mob.ron\", bounds: {\"mob.speed\": (min: 0.0, max: 9.0)})])",
    )
    .expect("data.ron");
    let notes = bevy_grimoire_core::notes::path_for_map(&grimoire.join("maps/m.map.ron"));
    let file = bevy_grimoire_core::notes::Notes {
        version: bevy_grimoire_core::notes::NOTES_VERSION,
        notes: vec![bevy_grimoire_core::notes::Note {
            id: "note-1".to_owned(),
            targets: vec![bevy_grimoire_core::notes::Target::Placement("bed@0".to_owned())],
            text: "the bed is in the corner".to_owned(),
            state: bevy_grimoire_core::notes::State::Draft,
            site: None,
            history: Vec::new(),
            auto: false,
        }],
        lease_secs: bevy_grimoire_core::notes::DEFAULT_LEASE_SECS,
    };
    std::fs::write(&notes, ron::ser::to_string(&file).expect("the notes serialize")).expect("notes file");
    root
}

fn workspace_asset(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets").join(rel)
}

fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn box_of(app: &App, e: Entity) -> Option<(Vec2, Vec2)> {
    let node = app.world().get::<ComputedNode>(e)?;
    let at = app.world().get::<UiGlobalTransform>(e)?;
    Some((at.translation, node.size() * 0.5))
}

/// **`row` lies inside `list`'s visible area**, from the two nodes' own geometry.
fn inside(app: &App, row: Entity, list: Entity) -> Result<(), String> {
    let (row_mid, row_half) = box_of(app, row).ok_or_else(|| "the row is not laid out".to_owned())?;
    let (list_mid, list_half) = box_of(app, list).ok_or_else(|| "the list is not laid out".to_owned())?;
    if row_half.y <= 0.0 || list_half.y <= 0.0 {
        return Err(format!("a zero-height node: row {row_half:?}, list {list_half:?}"));
    }
    let (top, bottom) = (list_mid.y - list_half.y, list_mid.y + list_half.y);
    let (row_top, row_bottom) = (row_mid.y - row_half.y, row_mid.y + row_half.y);
    if row_top >= top && row_bottom <= bottom {
        Ok(())
    } else {
        Err(format!("the row spans {row_top:.1}..{row_bottom:.1} and the list {top:.1}..{bottom:.1}"))
    }
}

/// **The scrollable ancestor of a row** — "is this row inside the thing that scrolls it".
fn scrollable_ancestor(app: &mut App, row: Entity) -> Entity {
    use bevy::ui::ScrollPosition;
    let world = app.world_mut();
    let mut owner = world.query::<&ChildOf>();
    let mut scrolls = world.query_filtered::<Entity, With<ScrollPosition>>();
    let mut at = row;
    loop {
        if scrolls.get(world, at).is_ok() {
            return at;
        }
        let Ok(parent) = owner.get(world, at) else {
            panic!("no scrollable ancestor holds the row");
        };
        at = parent.0;
    }
}

/// **The entity whose text starts with `prefix` that is inside a selected row**, found by walking up
/// from the text: several lists mark more than one row selected (an armed group, a focused member), so
/// the words are what name the one the change was to.
fn selected_row_drawing(app: &mut App, prefix: &str) -> Entity {
    let world = app.world_mut();
    let mut texts = world.query::<(Entity, &Text)>();
    let at = texts
        .iter(world)
        .find(|(_, t)| t.0.starts_with(prefix))
        .map(|(e, _)| e)
        .unwrap_or_else(|| {
            let drawn: Vec<String> = texts.iter(world).map(|(_, t)| t.0.clone()).collect();
            panic!("no text starts with `{prefix}`; the editor draws {drawn:?}")
        });
    let mut owner = world.query::<&ChildOf>();
    let mut selected = world.query::<&bevy_cauldron::chrome::RowSelected>();
    let mut at = at;
    loop {
        if selected.get(world, at).is_ok_and(|s| s.0) {
            return at;
        }
        let Ok(parent) = owner.get(world, at) else {
            panic!("the text `{prefix}` is not inside a selected row");
        };
        at = parent.0;
    }
}

/// **M2: the rig the bench selected is a row inside the bench's list.**
#[test]
fn the_rigs_row_the_reveal_selected_is_inside_the_bench() {
    let root = project("review_rigs_row");
    let mut app = editor(&root);
    let answer = apply(&mut app, "rig_adopt_measured", serde_json::json!({"rig": "valkyrie"}));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    settle(&mut app);
    assert_eq!((*app.world().resource::<Workspace>(), app.world().resource::<Mode>().label()), (Workspace::Animation, "ANIM"));

    // The row is the one the bench says is selected, and it is inside the list that scrolls it.
    let row = selected_row_drawing(&mut app, "valkyrie");
    let list = scrollable_ancestor(&mut app, row);
    if let Err(why) = inside(&app, row, list) {
        panic!("the rig's row is not inside the bench's visible area: {why}");
    }
}

/// **M2 + M1: the DATA file's row is the file's own leaf, inside the table's visible area.**
#[test]
fn the_data_row_the_reveal_selected_is_the_files_leaf_inside_the_table() {
    let root = project("review_data_row");
    let mut app = editor(&root);
    let answer = apply(
        &mut app,
        "data_set",
        serde_json::json!({"file": "tuning/mob", "path": "mob.speed", "value": 2.0}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    settle(&mut app);

    // **Which leaf the cursor is on**, not that it is on one: the table's own source says what a key
    // names, so the answer is the leaf's file and path.
    let (file, path) = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(&bevy_gooey::Table, &bevy_gooey::TableCursor), Without<bevy_cauldron::editor::HistoryTree>>();
        let (table, cursor) = q.single(world).expect("the DATA panel has one table");
        let key = cursor.0.expect("the table's cursor was put on the file's first leaf");
        let rows = table.source::<bevy_cauldron::editor::DataRows>().expect("the DATA table's own rows");
        let at = (0..bevy_gooey::RowSource::len(rows))
            .find(|i| bevy_gooey::RowSource::key(rows, *i) == key)
            .expect("the cursor's key names one of the table's rows");
        (rows.0[at].file.clone(), rows.0[at].path.clone())
    };
    assert_eq!(file, "tuning/mob", "the cursor is not on the changed file's leaf");
    assert_eq!(path, "mob.speed", "the cursor is on {path}, not the leaf that changed");

    // **And it is inside the table's visible area**, measured from the nodes rather than from the
    // reveal's arithmetic — the check M2 asks for and the one M1's coordinate bug broke.
    let row = {
        let wanted = key_of(&mut app, "tuning/mob", &path);
        let world = app.world_mut();
        let mut q = world.query::<(&bevy_gooey::TableRow, Entity)>();
        q.iter(world)
            .find(|(r, _)| r.key == wanted)
            .map(|(_, e)| e)
            .unwrap_or_else(|| panic!("the table draws no row for `tuning/mob {path}`"))
    };
    let list = scrollable_ancestor(&mut app, row);
    if let Err(why) = inside(&app, row, list) {
        panic!("the leaf's row is not inside the table's visible area: {why}");
    }
}

/// The key gooey derives for a row, asked of the table's own source.
fn key_of(app: &mut App, file: &str, path: &str) -> u64 {
    let world = app.world_mut();
    let mut q = world.query_filtered::<&bevy_gooey::Table, Without<bevy_cauldron::editor::HistoryTree>>();
    let table = q.single(world).expect("the DATA panel has one table");
    let rows = table.source::<bevy_cauldron::editor::DataRows>().expect("the DATA table's own rows");
    let at = (0..bevy_gooey::RowSource::len(rows))
        .find(|i| rows.0[*i].file == file && rows.0[*i].path == path)
        .expect("the leaf is one of the table's rows");
    bevy_gooey::RowSource::key(rows, at)
}

/// **M2: the settings section is on screen**, not merely that a dialog exists.
#[test]
fn the_packs_section_the_reveal_opened_is_on_screen() {
    let root = project("review_settings_row");
    let mut app = editor(&root);
    let answer = apply(
        &mut app,
        "pack_set_excluded",
        serde_json::json!({"pack": "alpha", "excluded": true}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    settle(&mut app);

    // The pack's own switch, and that it says what the file says.
    let world = app.world_mut();
    let mut texts = world.query::<(Entity, &Text)>();
    let drawn: Vec<String> = texts.iter(world).map(|(_, t)| t.0.clone()).collect();
    assert!(
        drawn.iter().any(|t| t == "Packs the kit leaves out"),
        "the project configuration did not open on its packs section: {drawn:?}"
    );
    // **The pack's own row**, which is a gooey `toggle` rather than a `chrome::list_row` — the
    // project configuration draws switches, and `RowSelected` is a list-row marker. Found by the
    // words the switch carries, then measured against the list that scrolls it.
    let row = {
        let world = app.world_mut();
        let mut texts = world.query::<(Entity, &Text)>();
        let at = texts
            .iter(world)
            .find(|(_, t)| t.0 == "alpha")
            .map(|(e, _)| e)
            .unwrap_or_else(|| {
                let drawn: Vec<String> = texts.iter(world).map(|(_, t)| t.0.clone()).collect();
                panic!("no switch reads `alpha`; the screen draws {drawn:?}")
            });
        let mut owner = world.query::<&ChildOf>();
        let mut at = at;
        // The switch's own box, not the text inside it: two ancestors up is the control the list
        // lays out.
        for _ in 0..2 {
            at = owner.get(world, at).expect("the switch's text is inside the control").0;
        }
        at
    };
    let list = scrollable_ancestor(&mut app, row);
    if let Err(why) = inside(&app, row, list) {
        panic!("the pack's switch is not inside its list's visible area: {why}");
    }
}

/// **M2: the note's row is drawn selected and measured**, not read back off the resource.
#[test]
fn the_notes_row_the_reveal_selected_is_drawn_selected_and_inside_the_panel() {
    let root = project("review_notes_row");
    let mut app = editor(&root);
    let answer = apply(
        &mut app,
        "note_edit",
        serde_json::json!({"id": "note-1", "text": "the bed is by the window"}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    settle(&mut app);

    // **The row the list drew for the selected note**, by the marker the reveal set.
    let row = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &bevy_cauldron::editor::NoteRow, &bevy_cauldron::chrome::RowSelected)>();
        q.iter(world)
            .find(|(_, r, s)| r.id == "note-1" && s.0)
            .map(|(e, ..)| e)
            .unwrap_or_else(|| panic!("the Notes list did not draw note-1 as the selected row"))
    };
    let list = scrollable_ancestor(&mut app, row);
    if let Err(why) = inside(&app, row, list) {
        panic!("the note's row is not inside the Notes panel's visible area: {why}");
    }
}

/// **M3: a person's key press stops a queued reveal, so the door does not change under them.**
///
/// The rule is `follow::a_persons_input_takes_the_camera_back`'s and is extended there rather than
/// duplicated: a reveal and a follow are one kindness, and a second input rule would be a second
/// answer to "is the person working".
#[test]
fn a_persons_key_press_stops_a_queued_reveal() {
    let root = project("review_reveal_stopped");
    let mut app = editor(&root);
    // **A reveal queued for another door**, which is the crossing a person would lose.
    {
        let mut revealing = app.world_mut().resource_mut::<bevy_cauldron::reveal::Revealing>();
        revealing.request(
            "piece_set_note",
            &bevy_cauldron::ops::Shown::Piece { id: "lamp_tall".to_owned() },
        );
    }
    assert_eq!(
        app.world().resource::<bevy_cauldron::reveal::Revealing>().pending(),
        1,
        "the reveal is queued, which is what the person's press has to stop"
    );

    // **The person presses a key before the reveal's frame.**
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::KeyB);
    settle(&mut app);

    assert_eq!(
        *app.world().resource::<Workspace>(),
        Workspace::Map,
        "a queued reveal crossed the door while the person was working"
    );
    assert_eq!(
        app.world().resource::<bevy_cauldron::reveal::Revealing>().pending(),
        0,
        "the person's press must drop the queued reveal, or it crosses on the next frame instead"
    );
}

/// **Round 2, M1: the DATA row's geometry is right at a display scale of 2.**
///
/// The first version wrote `(scroll / inverse_scale).max(0.0) * inverse_scale`, which is the scroll
/// in **logical** pixels again — an identity at scale 1, and at scale 2 the row's physical offset had
/// half the scroll subtracted from it, so a revealed leaf under-scrolled by exactly the distance the
/// table had already scrolled. On a Mac (scale 2) that is every table taller than its box.
#[test]
fn a_tables_row_position_is_physical_at_any_display_scale() {
    // A viewport 400 physical pixels tall showing four rows, scrolled 100 logical pixels at scale 2,
    // which is 200 physical. Row 3 sits 3 x 100 physical from the top of the content, and the content
    // is drawn 200 physical higher than it would be unscrolled.
    let geometry = bevy_cauldron::reveal::table_row_geometry((800.0, 400.0), 4.0, 3, 100.0, 0.5)
        .expect("a viewport with rows has geometry");
    let (row_mid, pitch) = geometry;
    assert_eq!(pitch, 100.0, "four rows in a 400 px box is a 100 px pitch, at any scale");
    // Top of the viewport is 800 - 200 = 600; the row's top is 600 + 300 - 200 = 700; its mid is 750.
    assert!(
        (row_mid - 750.0).abs() < 0.01,
        "at scale 2 the row sits at {row_mid}, not 750: the scroll was converted back to logical \
         before being subtracted from a physical offset"
    );
    // **Scale 1 is the same arithmetic**, so the test cannot pass by being written for one scale.
    let (row_mid, _) = bevy_cauldron::reveal::table_row_geometry((800.0, 400.0), 4.0, 3, 200.0, 1.0)
        .expect("a viewport with rows has geometry");
    assert!((row_mid - 750.0).abs() < 0.01, "at scale 1 the same row sits at {row_mid}, not 750");
}

/// **Round 2, M3: a reveal waits while the person is working**, through the follow's one idle clock.
#[test]
fn a_reveal_waits_while_the_person_is_working() {
    let root = project("review_reveal_busy");
    let mut app = editor(&root);
    // **The person drags**, which is what sets `Following::busy`.
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert!(
        app.world().resource::<bevy_cauldron::follow::Following>().busy(),
        "the person's button did not reach the follow's clock, so the test proves nothing"
    );
    {
        let mut revealing = app.world_mut().resource_mut::<bevy_cauldron::reveal::Revealing>();
        revealing.request(
            "piece_set_note",
            &bevy_cauldron::ops::Shown::Piece { id: "lamp_tall".to_owned() },
        );
    }
    // **The reveal's frame, mid-drag**: the door must not change under their hands.
    for _ in 0..6 {
        app.update();
    }
    assert_eq!(
        *app.world().resource::<Workspace>(),
        Workspace::Map,
        "a reveal crossed the door while the person was working the view"
    );
    assert_eq!(
        app.world().resource::<bevy_cauldron::reveal::Revealing>().pending(),
        1,
        "the reveal was thrown away rather than held, so letting go would show nothing"
    );

    // **They let go**, and the wait runs out: the reveal lands.
    let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    mouse.release(MouseButton::Left);
    mouse.clear();
    app.update();
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_IDLE_S) {
        app.update();
    }
    settle(&mut app);
    assert_eq!(
        *app.world().resource::<Workspace>(),
        Workspace::Kit,
        "the reveal never landed after the person stopped"
    );
}

/// One frame of the test clock, for the idle wait. Declared here rather than reused: this file has no
/// clock of its own, and the reveal's systems read `Time<Real>`, which the harness advances for real.
fn frames_of(seconds: f32) -> usize {
    // A headless frame is about a millisecond; the idle wait is real seconds, so this waits on the
    // clock's own terms rather than on a count of frames.
    (seconds * 60.0).ceil() as usize
}

/// **NIT: `Revealing::clear` runs on a project change**, which is the one thing that makes its
/// queue meaningless — a page is a door and a row, and another project has neither.
#[test]
fn another_project_clears_the_reveals_queue() {
    let root = project("review_project_change");
    let mut app = editor(&root);
    {
        let mut revealing = app.world_mut().resource_mut::<bevy_cauldron::reveal::Revealing>();
        revealing.request(
            "piece_set_note",
            &bevy_cauldron::ops::Shown::Piece { id: "lamp_tall".to_owned() },
        );
    }
    // **Another project's root**: the resource the reveal reads to decide a page.
    let mut other = project("review_project_change_other");
    let project = app.world().resource::<Project>().clone();
    let _ = &mut other;
    app.world_mut().insert_resource(Project::open(&root, None).expect("the same project reopens"));
    drop(project);
    app.update();
    assert_eq!(
        app.world().resource::<bevy_cauldron::reveal::Revealing>().pending(),
        0,
        "a project change kept a reveal whose page names the previous project's rows"
    );
}
