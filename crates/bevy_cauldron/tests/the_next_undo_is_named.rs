//! **The next undo is named, in the INSPECTOR and to the agent.**
//!
//! Before Cmd+Z, the author cannot see what it will take back, and an agent reading `grimoire/state`
//! cannot see the author's undo stack at all. The INSPECTOR now has an UNDO row (what Cmd+Z takes
//! back) and a REDO row (what Cmd+Shift+Z puts back), each one line, and `grimoire/state` carries the
//! same two names.
//!
//! These drive the human front end end to end and read what was DRAWN, in the headless layout, so
//! a row that exists in the data but not on the screen fails here.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

fn editor(tag: &str, id: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .sized_descriptor(id, "alpha", 2.064, 0.12)
        .mounted_descriptor("market_stall_awning_with_lanterns", "alpha", "worktop")
        .surface_descriptor("worktop_table", "alpha", "worktop")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    // **The agent's door**, as `an_agent_can_drive_the_editor` opens it: the state is read over BRP.
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..6 {
        app.update();
    }
    app
}

/// **The hint the ghost writes over bare floor for `id`.** The core's refusal for a surface piece
/// with no surface under it, put through the editor's own `ghost_hint`.
fn ghost_text(app: &App, id: &str) -> String {
    let project = app.world().resource::<Project>();
    let map = &app.world().resource::<OpenMap>().map;
    let d = project
        .library
        .descriptors
        .iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("no descriptor {id}"));
    let ys = bevy_grimoire_core::stack::resolve_y(map, &project.library).expect("heights resolve");
    let refusal = match bevy_grimoire_core::stack::placement_at(map, &project.library, &ys, d, (0.0, 0.0)) {
        Err(e) => e,
        Ok(_) => panic!("{id} found a surface at the origin; the fixture should have none"),
    };
    bevy_cauldron::editor::ghost_hint(d, &refusal)
}

fn tap(app: &mut App, logical: Key, code: KeyCode) {
    for state in [
        bevy::input::ButtonState::Pressed,
        bevy::input::ButtonState::Released,
    ] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

/// **A chord**: the platform modifier (and Shift, when asked) held while `key` is pressed.
fn chord(app: &mut App, key: KeyCode, shift: bool) {
    let held: Vec<KeyCode> = std::iter::once(bevy_cauldron::keys::MOD_KEYS[0])
        .chain(shift.then_some(KeyCode::ShiftLeft))
        .collect();
    for k in &held {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(*k);
    }
    tap(
        app,
        Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        key,
    );
    for k in &held {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(*k);
    }
    app.update();
}

/// **Resize one axis through the size field**, as a person does: open it, type, Enter.
fn resize(app: &mut App, axis: char, metres: &str) {
    bevy_cauldron::editor::size_field_for_test(app.world_mut(), axis);
    app.update();
    for c in metres.chars() {
        let code = match c {
            '0' => KeyCode::Digit0,
            '1' => KeyCode::Digit1,
            '2' => KeyCode::Digit2,
            '3' => KeyCode::Digit3,
            '4' => KeyCode::Digit4,
            '5' => KeyCode::Digit5,
            '6' => KeyCode::Digit6,
            '7' => KeyCode::Digit7,
            '8' => KeyCode::Digit8,
            '9' => KeyCode::Digit9,
            _ => panic!("not a digit: {c}"),
        };
        tap(app, Key::Character(c.to_string().into()), code);
    }
    tap(app, Key::Enter, KeyCode::Enter);
}

fn map(app: &App) -> bevy_grimoire_core::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

/// **Every drawn text and its box**, in physical pixels: (text, top y, height).
fn drawn_texts(app: &mut App) -> Vec<(String, f32, f32)> {
    for _ in 0..3 {
        app.update();
    }
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &ComputedNode, &UiGlobalTransform, &bevy::ui::Node)>();
    q.iter(world)
        .filter(|(_, node, tf, style)| {
            style.display == bevy::ui::Display::Flex
                && node.size().x > 0.0
                && node.size().y > 0.0
                && tf.translation.y.is_finite()
        })
        .map(|(t, node, tf, _)| {
            (
                t.0.clone(),
                tf.translation.y - node.size().y / 2.0,
                node.size().y,
            )
        })
        .collect()
}

/// **The box height of the drawn text `want`.** A row that wraps is taller than one line.
fn height_of(texts: &[(String, f32, f32)], want: &str) -> f32 {
    texts
        .iter()
        .find(|(t, _, _)| t == want)
        .map(|(_, _, h)| *h)
        .unwrap_or_else(|| panic!("no drawn text reads {want:?}; drawn: {texts:?}"))
}

/// **Call one of the editor's BRP commands, the way the MCP server does.**
fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::RemoteMethodSystemId;
    use bevy::remote::RemoteMethods;
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("no method `{method}` is registered"),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

/// **The UNDO and REDO rows name the newest change, and the rows never change height.**
///
/// The fence run is the older change, the resize the newer one. Cmd+Z takes the resize back, so
/// the UNDO row then names the run and the REDO row names the resize.
#[test]
fn the_inspector_names_the_next_undo_and_redo() {
    let mut app = editor("nextundo", "fence");
    let start = map(&app);

    // **Nothing to undo or redo yet**: both rows say so, in one line each.
    let empty = drawn_texts(&mut app);
    let empty_height = height_of(&empty, "nothing");
    assert!(
        empty.iter().filter(|(t, _, _)| t == "nothing").count() >= 2,
        "the UNDO and REDO rows must both say `nothing` on an empty history; drawn: {empty:?}"
    );

    // **The older change: a run of fence.**
    bevy_cauldron::editor::run_for_test(app.world_mut(), "fence", (0.0, 0.0), (8.0, 0.0));
    app.update();
    let added = map(&app).placements.len() - start.placements.len();
    assert!(
        added > 0,
        "the run laid nothing, so there is nothing to name"
    );
    let run_name = format!("remove {added} fence");
    let filled = drawn_texts(&mut app);
    assert!(
        filled.iter().any(|(t, _, _)| t == &run_name),
        "the UNDO row must name the run as {run_name:?}; drawn: {filled:?}"
    );
    assert_eq!(
        height_of(&filled, &run_name),
        empty_height,
        "the UNDO row changed height when it filled, so the panel would move under the pointer"
    );

    // **The newer change: a resize.** Two entries are stacked, so the row says so.
    let before_resize = map(&app).bounds;
    resize(&mut app, 'y', "44");
    let resized = map(&app).bounds;
    assert_ne!(
        resized.1, before_resize.1,
        "the size field did not resize the map, so the test drives nothing"
    );
    let resize_name = format!(
        "map size back to {} x {} x {}",
        before_resize.0, before_resize.1, before_resize.2
    );
    let stacked = drawn_texts(&mut app);
    assert!(
        stacked
            .iter()
            .any(|(t, _, _)| t == &format!("{resize_name} (+1)")),
        "the UNDO row must name the resize and say one more is stacked, as \
         {resize_name:?} (+1); drawn: {stacked:?}"
    );

    // **Cmd+Z takes the resize back**: UNDO now names the run, REDO names the resize.
    chord(&mut app, KeyCode::KeyZ, false);
    let after_undo = drawn_texts(&mut app);
    assert!(
        after_undo.iter().any(|(t, _, _)| t == &run_name),
        "after Cmd+Z the UNDO row must name the run as {run_name:?}; drawn: {after_undo:?}"
    );
    // Redo puts the size back to the resized one, so it reads "to", not "back".
    let redo_name = format!("map size to {} x {} x {}", resized.0, resized.1, resized.2);
    assert!(
        after_undo.iter().any(|(t, _, _)| t == &redo_name),
        "after Cmd+Z the REDO row must name the resize as {redo_name:?}; drawn: {after_undo:?}"
    );

    // **The agent reads the same two names** from `grimoire/state`.
    let state = call(&mut app, "grimoire/state", serde_json::json!({}));
    let editor = &state["editor"];
    assert_eq!(
        editor["undo_next"],
        serde_json::json!(run_name),
        "grimoire/state must carry the UNDO name; state: {state}"
    );
    assert_eq!(editor["undo_depth"], serde_json::json!(1));
    assert_eq!(
        editor["redo_next"],
        serde_json::json!(redo_name),
        "grimoire/state must carry the REDO name; state: {state}"
    );
    assert_eq!(editor["redo_depth"], serde_json::json!(1));
}

/// **A long name stays on one line.** The phrase is clamped, so the row cannot grow.
#[test]
fn a_long_name_does_not_wrap_the_row() {
    let long_id = "fence_with_a_very_long_descriptor_name_for_the_inspector_row";
    let mut app = editor("longname", long_id);
    let start = map(&app);
    let empty = drawn_texts(&mut app);
    let empty_height = height_of(&empty, "nothing");

    bevy_cauldron::editor::run_for_test(app.world_mut(), long_id, (0.0, 0.0), (8.0, 0.0));
    app.update();
    let added = map(&app).placements.len() - start.placements.len();
    assert!(
        added > 0,
        "the run laid nothing, so there is nothing to name"
    );

    let filled = drawn_texts(&mut app);
    let undo_text = filled
        .iter()
        .map(|(t, _, _)| t)
        .find(|t| t.starts_with("remove "))
        .unwrap_or_else(|| panic!("the UNDO row names nothing; drawn: {filled:?}"))
        .clone();
    assert!(
        undo_text.chars().count() <= 42,
        "the UNDO phrase is {} characters, past the ~42 that fit one line: {undo_text:?}",
        undo_text.chars().count()
    );
    assert_eq!(
        height_of(&filled, &undo_text),
        empty_height,
        "a long name wrapped the UNDO row, so the panel moves"
    );
}

/// **The drawn top of the row labelled `label`**, in physical pixels.
fn top_of(texts: &[(String, f32, f32)], label: &str) -> f32 {
    texts
        .iter()
        .find(|(t, _, _)| t == label)
        .unwrap_or_else(|| panic!("no drawn `{label}` row; drawn: {texts:?}"))
        .1
}

/// **A grouped change names every part.** A place over pieces already there replaces them: one act
/// that removes the old rows and adds the new. UNDO must say both halves, and after Cmd+Z the REDO
/// row must say both halves the other way round.
#[test]
fn a_grouped_change_names_every_part() {
    let mut app = editor("nextundogroup", "fence");
    let start = map(&app).placements.len();
    bevy_cauldron::editor::run_for_test(app.world_mut(), "fence", (0.0, 0.0), (8.0, 0.0));
    app.update();
    let run = map(&app).placements.len() - start;
    assert!(
        run > 1,
        "the run laid {run} piece(s); a place over needs two to replace"
    );

    // One piece dropped at x = 1 overlaps the run's first two pieces: the place removes them and
    // adds one, so the change is one group of two parts.
    let before = map(&app).placements.len();
    bevy_cauldron::editor::place_for_test(app.world_mut(), "fence", (1.0, 0.0));
    app.update();
    let after = map(&app).placements.len();
    let removed = before + 1 - after;
    assert!(
        removed > 0,
        "the place over replaced nothing, so the group is not exercised"
    );

    // The run is still on the undo stack beneath the place over, so the row carries " (+1)".
    let undo_name = format!("remove 1 fence, restore {removed} fence (+1)");
    let drawn = drawn_texts(&mut app);
    assert!(
        drawn.iter().any(|(t, _, _)| t == &undo_name),
        "the UNDO row must name every part of the place over as {undo_name:?}; drawn: {drawn:?}"
    );

    chord(&mut app, KeyCode::KeyZ, false);
    let redo_name = format!("remove {removed} fence, restore 1 fence");
    let after_undo = drawn_texts(&mut app);
    assert!(
        after_undo.iter().any(|(t, _, _)| t == &redo_name),
        "after Cmd+Z the REDO row must name every part of the place over as {redo_name:?}; drawn: \
         {after_undo:?}"
    );
}

/// **A hint or a receipt does not move the UNDO row.** The hint row is empty most of the time and
/// grows when it speaks, so it sits below UNDO and REDO where it cannot push them down. A receipt
/// that fits the two lines B-009 reserves does not move them either.
#[test]
fn a_hint_or_a_receipt_does_not_move_the_undo_row() {
    let mut app = editor("nextundomove", "fence");
    let still = top_of(&drawn_texts(&mut app), "UNDO");

    let hint = ghost_text(&app, "market_stall_awning_with_lanterns");
    app.world_mut().resource_mut::<EditorState>().hint = hint.to_owned();
    let hinted = drawn_texts(&mut app);
    assert!(
        hinted.iter().any(|(t, _, _)| *t == hint),
        "the hint did not draw, so the test drives nothing; drawn: {hinted:?}"
    );
    assert_eq!(
        top_of(&hinted, "UNDO"),
        still,
        "setting the hint moved the UNDO row; drawn: {hinted:?}"
    );

    let receipt = "ran 8 piece(s) along the line, and this receipt is long enough to wrap";
    app.world_mut()
        .resource_mut::<EditorState>()
        .status
        .note(receipt.to_owned());
    let receipted = drawn_texts(&mut app);
    assert_eq!(
        top_of(&receipted, "UNDO"),
        still,
        "a receipt moved the UNDO row; drawn: {receipted:?}"
    );
}

/// **A stacked long name stays on one line.** The phrase is clamped to 36 characters and the stack
/// count goes after it, so the row keeps its one line with the count on it.
#[test]
fn a_stacked_long_name_stays_on_one_line() {
    let long_id = "fence_with_a_very_long_descriptor_name_for_the_inspector_row";
    let mut app = editor("longstacked", long_id);
    let start = map(&app).placements.len();
    let empty_height = height_of(&drawn_texts(&mut app), "nothing");

    bevy_cauldron::editor::run_for_test(app.world_mut(), long_id, (0.0, 0.0), (8.0, 0.0));
    app.update();
    bevy_cauldron::editor::run_for_test(app.world_mut(), long_id, (0.0, 2.0), (8.0, 2.0));
    app.update();
    let laid = map(&app).placements.len() - start;
    assert!(
        laid > 0,
        "the runs laid nothing, so there is nothing to name"
    );

    let drawn = drawn_texts(&mut app);
    let undo_text = drawn
        .iter()
        .map(|(t, _, _)| t)
        .find(|t| t.starts_with("remove ") && t.ends_with(" (+1)"))
        .unwrap_or_else(|| {
            panic!("the UNDO row does not say one more is stacked; drawn: {drawn:?}")
        })
        .clone();
    let name = undo_text.strip_suffix(" (+1)").unwrap_or_default();
    assert!(
        name.chars().count() <= 36,
        "the clamped name is {} characters, past 36: {name:?}",
        name.chars().count()
    );
    assert!(
        undo_text.chars().count() <= 42,
        "the stacked UNDO row is {} characters, past the ~42 that fit one line: {undo_text:?}",
        undo_text.chars().count()
    );
    assert_eq!(
        height_of(&drawn, &undo_text),
        empty_height,
        "a stacked long name wrapped the UNDO row, so the panel moves"
    );
}
