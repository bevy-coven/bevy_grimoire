//! **A jump to a history position is one undo group, and it says who made each change (E-018).**
//!
//! The history list lets the author move to any earlier point in one step. These drive the real
//! `jump` through its test driver, and assert on the stacks, the map, and the list's rows. The
//! agent's change goes through the registered BRP method, the way an agent's request goes, so the
//! row can be labelled as the agent's.

use bevy::prelude::*;
use bevy_cauldron::chrome::BODY_LINE_H;
use bevy_cauldron::editor::{
    EditorState, jump_for_test, place_for_test, plant_stale_move_for_test, redo_for_test,
    undo_for_test,
};
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

use fixtures::Fixture;

fn booted(name: &str) -> App {
    let root = Fixture::new(name)
        .descriptor("tile_a", "alpha")
        .place("tile_a", (0.5, 0.5))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }
    app
}

fn pieces(app: &App) -> usize {
    app.world().resource::<OpenMap>().map.placements.len()
}

fn state(app: &App) -> &EditorState {
    app.world().resource::<EditorState>()
}

/// **Three edits, then one jump back over all of them**: one group on the redo stack, not three.
#[test]
fn a_jump_back_over_three_edits_is_one_group() {
    let mut app = booted("jumpgroup");
    for x in [1.5, 2.5, 3.5] {
        place_for_test(app.world_mut(), "tile_a", (x, 0.5));
        app.update();
    }
    assert_eq!(pieces(&app), 4, "the fixture's piece and the three placed");
    assert_eq!(state(&app).undo_depth(), 3);

    let rows = state(&app).history();
    assert_eq!(rows.len(), 3, "one row per edit: {rows:?}");
    assert_eq!(
        rows.iter().map(|r| r.position).collect::<Vec<_>>(),
        [1, 2, 3]
    );

    jump_for_test(app.world_mut(), 0);
    app.update();
    {
        let s = state(&app);
        assert_eq!(s.undo_depth(), 0, "the jump took all three back");
        assert_eq!(
            s.redo_depth(),
            1,
            "one jump is one group, so one redo entry"
        );
        assert_eq!(pieces(&app), 1, "only the fixture's piece is left");
        let rows = s.history();
        assert_eq!(rows.len(), 1, "the group is one row: {rows:?}");
        assert!(
            rows[0].label.contains("3 changes"),
            "label: {}",
            rows[0].label
        );
    }

    jump_for_test(app.world_mut(), 1);
    app.update();
    assert_eq!(
        state(&app).undo_depth(),
        1,
        "the group came back as one undo entry"
    );
    assert_eq!(pieces(&app), 4, "the jump forward gave all three back");

    undo_for_test(app.world_mut());
    app.update();
    assert_eq!(pieces(&app), 1, "one Cmd+Z reverses the whole jump");
    assert_eq!(state(&app).undo_depth(), 0);
}

/// **A refused entry stops the jump where it is and stays on its stack** (E-018).
#[test]
fn a_refused_entry_stops_the_jump_and_is_kept() {
    let mut app = booted("jumpstop");
    place_for_test(app.world_mut(), "tile_a", (1.5, 0.5));
    app.update();
    place_for_test(app.world_mut(), "tile_a", (2.5, 0.5));
    app.update();
    plant_stale_move_for_test(app.world_mut(), "ghost");
    app.update();
    assert_eq!(state(&app).undo_depth(), 3);
    assert_eq!(pieces(&app), 3);

    jump_for_test(app.world_mut(), 0);
    app.update();
    let s = state(&app);
    assert_eq!(
        s.undo_depth(),
        3,
        "the refused entry is kept and nothing beneath it moved"
    );
    assert_eq!(
        s.redo_depth(),
        0,
        "nothing applied, so there is no group to redo"
    );
    assert_eq!(pieces(&app), 3, "the placements are where they were");
    assert!(
        s.status.problem_text().contains("stopped after 0 of 3"),
        "the jump must say how far it got: {:?}",
        s.status.problem_text()
    );
}

/// The agent-side fixture, with the BRP bridge an agent's request goes through.
fn agent_editor(name: &str) -> App {
    let root = Fixture::new(name)
        .descriptor("tile_a", "alpha")
        .place("tile_a", (0.5, 0.5))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **Through the registered method**, as an agent's request goes.
fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

/// **A person's edit and an agent's edit are labelled as who made them**, and a jump over both is
/// one row that names the agent, because one of its parts is the agent's.
#[test]
fn the_list_says_who_made_each_change() {
    let mut app = agent_editor("jumpwho");
    place_for_test(app.world_mut(), "tile_a", (1.5, 0.5));
    app.update();
    let answer = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "map_set_settings", "parameters": {"axis": "y", "axis_value": 15.0}, "dry_run": false}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    app.update();

    let rows = state(&app).history();
    assert_eq!(rows.len(), 2, "rows: {rows:?}");
    assert!(
        !rows[0].agent,
        "the person's placement is the person's: {rows:?}"
    );
    assert!(
        rows[1].agent,
        "the agent's settings change is the agent's: {rows:?}"
    );

    jump_for_test(app.world_mut(), 0);
    app.update();
    let rows = state(&app).history();
    assert_eq!(rows.len(), 1, "one jump, one row: {rows:?}");
    assert!(
        rows[0].agent,
        "a group with an agent's part is labelled agent: {rows:?}"
    );
}

/// **A click on a history row is the jump, and is one group (E-018 option C).** The row is the
/// `Pointer<Click>` the mouse sends, delivered to the row entity the inspector spawned, so this goes
/// through the same observer a click does. The keyboard route to the same row is k presses of Cmd+Z,
/// which `a_jump_back_over_three_edits_is_one_group` already covers through `jump_for_test`.
#[test]
fn clicking_a_history_row_jumps_there_as_one_group() {
    use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
    use bevy::picking::backend::HitData;
    use bevy::picking::events::{Pointer, PointerClick};
    use bevy::picking::pointer::{Location, PointerButton, PointerId};

    let mut app = booted("jumpclick");
    for x in [1.5, 2.5, 3.5] {
        place_for_test(app.world_mut(), "tile_a", (x, 0.5));
        app.update();
    }
    assert_eq!(pieces(&app), 4);

    // The row that jumps to position 1: gooey's pool row whose key is that line's position.
    let tree = app
        .world_mut()
        .query_filtered::<Entity, With<bevy_cauldron::editor::HistoryTree>>()
        .single(app.world())
        .expect("the INSPECTOR draws one history list");
    let row = {
        let world = app.world_mut();
        let mut rows = world.query::<(Entity, &bevy_gooey::TableRow)>();
        rows.iter(world)
            .find(|(_, row)| row.table == tree && row.key == 1)
            .map(|(entity, _)| entity)
            .expect("a history row for position 1 is on screen")
    };

    app.world_mut().trigger(PointerClick {
        entity: row,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Image(ImageRenderTarget {
                    handle: Default::default(),
                    scale_factor: 1.0,
                }),
                position: Vec2::ZERO,
            },
        ),
        button: PointerButton::Primary,
        hit: HitData {
            camera: Entity::PLACEHOLDER,
            depth: 0.0,
            position: None,
            normal: None,
            extra: None,
        },
        duration: std::time::Duration::ZERO,
        count: 1,
    });
    app.update();

    assert_eq!(
        state(&app).undo_depth(),
        1,
        "the map is as it was at position 1"
    );
    assert_eq!(pieces(&app), 2, "the fixture's piece and the first placed");

    // One redo brings back both edits the jump took back: the jump was one group, not two.
    redo_for_test(app.world_mut());
    app.update();
    assert_eq!(
        state(&app).undo_depth(),
        2,
        "the first change, then the one group the redo put back"
    );
    assert_eq!(pieces(&app), 4, "one redo restores the whole jump");

    // And one undo takes the whole group back: the jump was one group, not two.
    undo_for_test(app.world_mut());
    app.update();
    assert_eq!(state(&app).undo_depth(), 1);
    assert_eq!(pieces(&app), 2, "one undo removes the whole group");
}

/// **The line the editor stands on is the selected row, and every row drawn takes the pointer (E-018, UI-53).**
///
/// The list is gooey's virtualized `tree` now, so its rows are the pool: gooey rebinds them as the list scrolls
/// and every one of them is a jump target. What is left of the hand-built window's rule is the two facts a click
/// depends on — the row is hoverable, so it lights under the pointer, and its text is `Pickable::IGNORE`, so the
/// pointer hits the row and not a word inside it. A click on the line the editor stands on is not a jump because
/// the panel keeps that line selected: see `a_click_on_a_line_steps_back_to_it`.
#[test]
fn every_row_takes_the_pointer_and_its_text_does_not() {
    use bevy::picking::Pickable;
    let mut app = booted("rowpick");
    place_for_test(app.world_mut(), "tile_a", (1.5, 0.5));
    app.update();
    let tree = app
        .world_mut()
        .query_filtered::<Entity, With<bevy_cauldron::editor::HistoryTree>>()
        .single(app.world())
        .expect("the INSPECTOR draws one history list");
    let depth = state(&app).undo_depth();
    let selected = app.world().get::<bevy_gooey::TableSelection>(tree).cloned().unwrap_or_default();
    assert_eq!(
        selected,
        bevy_gooey::TableSelection::of(depth as u64),
        "the line the editor stands on is the selected row"
    );
    let rows: Vec<_> = {
        let world = app.world_mut();
        world
            .query::<(&bevy_gooey::TableRow, Option<&Pickable>, &Children)>()
            .iter(world)
            .filter(|(row, _, _)| row.table == tree)
            .map(|(row, pick, children)| {
                (row.key, pick.is_none_or(|p| p.is_hoverable), children.iter().collect::<Vec<Entity>>())
            })
            .collect()
    };
    assert!(!rows.is_empty(), "the list draws the lines it holds");
    let world = app.world();
    for (key, hoverable, children) in rows {
        assert!(hoverable, "the row for line {key} takes the pointer");
        for child in children {
            assert_eq!(
                world.get::<Pickable>(child).copied(),
                Some(Pickable::IGNORE),
                "what the row for line {key} draws must not be a hit of its own"
            );
        }
    }
}

/// **A history row names the change it stands for, on both sides of the undo.** A placement is
/// "place 1" while it is on the undo stack, and "place 1 again" once undone, because the row is the
/// same change either way. Before this, the undo side said "remove 1", which is what Cmd+Z does, so
/// one change had opposite labels on the two sides of the undo.
#[test]
fn a_native_row_names_the_change_on_both_sides_of_undo() {
    let mut app = booted("rowlabel");
    place_for_test(app.world_mut(), "tile_a", (1.5, 0.5));
    app.update();
    let rows = state(&app).history();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].label, "place 1");
    assert!(!rows[0].agent);

    undo_for_test(app.world_mut());
    app.update();
    let rows = state(&app).history();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].label, "place 1 again");
}

/// **A history row holds its inset and one line of text (E-018, UI-53).** The rule the hand-built window needed
/// still has to hold for gooey's rows: a row is one line tall with its own inset inside it, so a label can never
/// be cut and a row can never grow the list.
///
/// Failed before the window's fix: its wrapper was `BODY_LINE_H` tall, and the row's 3 px top inset plus its
/// 13.2 px line is 16.2 px, so the wrapper that clipped the row was 2.2 px short of it. This asserts the layout's
/// own lengths rather than drawn boxes (headless text measures shorter).
#[test]
fn a_history_row_holds_its_inset_and_one_line() {
    let mut app = booted("rowclip");
    place_for_test(app.world_mut(), "tile_a", (1.5, 0.5));
    for _ in 0..3 {
        app.update();
    }

    let tree = app
        .world_mut()
        .query_filtered::<Entity, With<bevy_cauldron::editor::HistoryTree>>()
        .single(app.world())
        .expect("the INSPECTOR draws one history list");
    let world = app.world_mut();
    let rows: Vec<Entity> = world
        .query::<(Entity, &bevy_gooey::TableRow)>()
        .iter(world)
        .filter(|(_, row)| row.table == tree)
        .map(|(entity, _)| entity)
        .collect();
    assert!(!rows.is_empty(), "no history row is in the world");
    for row in rows {
        let clip = px(world.get::<Node>(row).expect("a row has a node").height, "row height");
        let pad = world.get::<Node>(row).expect("a row has a node").padding;
        let inset = px(pad.top, "row top inset") + px(pad.bottom, "row bottom inset");
        let need = inset + BODY_LINE_H;
        assert!(
            clip >= need - 0.01,
            "a history row is {clip} px tall, but the row needs {need} px: its {inset} px \
             inset and a {BODY_LINE_H} px line"
        );
    }
}

/// **A layout length in logical pixels.** A history row's lengths are fixed px; anything else is a change
/// to the row that this test has to be taught about, not a case to pass over.
fn px(v: Val, what: &str) -> f32 {
    match v {
        Val::Px(p) => p,
        other => panic!("{what} is {other:?}, not a fixed px length"),
    }
}
