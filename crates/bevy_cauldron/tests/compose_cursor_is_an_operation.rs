//! **The COMPOSE tab's cursor keys are operations, so a key and an agent's call are one** (E-002, #129 slice 23).
//!
//! Six verbs leave the allowlist of unmigrated actions: `ComposePrev` and `ComposeNext` (up and down, which walk
//! whichever list has the focus, Shift striding five), `ComposeMemberPrev` and `ComposeMemberNext` (left and right,
//! which choose the list the arrows walk) and `CarouselPrev` and `CarouselNext` (`O` and `P`, which step the carousel
//! round its ring). They dispatch `compose_walk`, `compose_focus` and `compose_carousel_step`; an agent calls the same
//! three through `grimoire/operation_apply`, and the editor's own `ComposeState` holds the answer either way.
//!
//! **The rules live in the operation, once**: the two lists clamp at their ends, the carousel wraps, and choosing
//! another group puts the member cursor back on the first member.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::compose::{ComposeState, Pane};
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;
use serde_json::{Value, json};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let members = [("post_a", "site/wall", (0.0, 0.0)), ("post_b", "site/wall", (2.0, 0.0)), ("post_c", "site/wall", (4.0, 0.0))];
    let root = fixtures::Fixture::new(&format!("compose_cursor_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .composition("arch", &members)
        .composition("door", &members)
        .composition("gate", &members)
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    // **A frame is 1/60 s of the clock, not of the host** (assumed owner exception, 2026-11-13: #322): the key repeat counts
    // the app's clock, so on a quiet host 21 headless frames took under REPEAT_SECS of real time and the hold never repeated.
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
    *app.world_mut().resource_mut::<Mode>() = Mode::Compose;
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **Through the registered method**, as an agent's request goes.
fn apply(app: &mut App, operation: &str, parameters: Value) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/operation_apply").cloned() else {
        panic!("grimoire/operation_apply is not registered");
    };
    app.world_mut()
        .run_system_with(id, Some(json!({"operation": operation, "parameters": parameters, "dry_run": false})))
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"))
}

fn send(app: &mut App, key: KeyCode, state: ButtonState) {
    app.world_mut().write_message(KeyboardInput {
        key_code: key,
        logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        state,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

/// **A key as a person presses it**, with `held` down around it.
fn tap(app: &mut App, held: &[KeyCode], key: KeyCode) {
    for h in held {
        send(app, *h, ButtonState::Pressed);
    }
    send(app, key, ButtonState::Pressed);
    app.update();
    send(app, key, ButtonState::Released);
    for h in held {
        send(app, *h, ButtonState::Released);
    }
    app.update();
    app.update();
}

fn logged(app: &App, operation: &str) -> Vec<(String, Value)> {
    let root = app.world().resource::<Project>().root.clone();
    let text = std::fs::read_to_string(bevy_cauldron::ops::oplog_path_at(&root)).unwrap_or_default();
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["op"] == operation)
        .map(|v| (v["source"].as_str().unwrap_or_default().to_owned(), v))
        .collect()
}

/// `(selected group, member, focused list)`: the whole cursor.
fn cursor(app: &App) -> (usize, usize, Pane) {
    let s = app.world().resource::<ComposeState>();
    (s.selected, s.member, s.focus)
}

/// **Down walks the groups through the operation, Shift strides five, and the walk clamps at both ends**; an agent walks
/// the same cursor with the same rule.
#[test]
fn the_walk_keys_and_an_agent_move_one_cursor_and_clamp_at_the_ends() {
    let mut app = editor("walk");
    assert_eq!(cursor(&app), (0, 0, Pane::Groups), "a session opens on the first group, in the COMPOSITIONS list");
    tap(&mut app, &[], KeyCode::ArrowDown);
    assert_eq!(cursor(&app).0, 1, "down walks to the next group");
    let lines = logged(&app, "compose_walk");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"]["by"], "next", "{:?}", lines[0]);

    let said = apply(&mut app, "compose_walk", json!({"by": "next"}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert_eq!(cursor(&app).0, 2, "an agent's walk moves the editor's own cursor");
    apply(&mut app, "compose_walk", json!({"by": "next"}));
    assert_eq!(cursor(&app).0, 2, "and it stops at the end of the list rather than wrapping");
    tap(&mut app, &[KeyCode::ShiftLeft], KeyCode::ArrowUp);
    assert_eq!(cursor(&app).0, 0, "Shift+up strides five and stops at the start");
    apply(&mut app, "compose_walk", json!({"by": "next", "rows": 5}));
    assert_eq!(cursor(&app).0, 2, "an agent strides too, by `rows`");
    let refused = apply(&mut app, "compose_walk", json!({"by": "sideways"}));
    assert_eq!(refused["outcome"], "refused", "a direction that is not one is refused: {refused}");
}

/// **Right hands the arrows to the MEMBERS list through the operation, the walk then moves the member cursor, and
/// choosing another group puts it back on the first member.**
#[test]
fn the_focus_keys_and_an_agent_choose_the_list_and_the_walk_follows() {
    let mut app = editor("focus");
    tap(&mut app, &[], KeyCode::ArrowRight);
    assert_eq!(cursor(&app).2, Pane::Members, "right hands the arrows to the members");
    let lines = logged(&app, "compose_focus");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    tap(&mut app, &[], KeyCode::ArrowDown);
    assert_eq!(cursor(&app), (0, 1, Pane::Members), "down now walks the members, and the group stays");
    apply(&mut app, "compose_walk", json!({"by": "next", "rows": 5}));
    assert_eq!(cursor(&app).1, 2, "an agent's walk of the members clamps at the last one");
    let said = apply(&mut app, "compose_focus", json!({"by": "prev"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(cursor(&app).2, Pane::Groups, "and an agent hands the arrows back");
    tap(&mut app, &[], KeyCode::ArrowLeft);
    assert_eq!(cursor(&app).2, Pane::Members, "left chooses the other list too: there are two, so it is the same toggle");
    apply(&mut app, "compose_focus", json!({"by": "sideways"}));
    assert_eq!(cursor(&app).2, Pane::Members, "a direction that is not one changes nothing");
}

/// **`O` and `P` step the carousel round its ring through the operation**, leaving the focus alone and putting the
/// member cursor back on the first member; an agent steps the same ring.
#[test]
fn the_carousel_keys_and_an_agent_step_one_ring() {
    let mut app = editor("carousel");
    apply(&mut app, "compose_focus", json!({"by": "next"}));
    apply(&mut app, "compose_walk", json!({"by": "next"}));
    assert_eq!(cursor(&app), (0, 1, Pane::Members), "setup: the member cursor is off the first member");
    tap(&mut app, &[], KeyCode::KeyP);
    assert_eq!(cursor(&app), (1, 0, Pane::Members), "P steps to the next group, the members list keeps the arrows, and the member cursor resets");
    let lines = logged(&app, "compose_carousel_step");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].1["params"]["by"], "next", "{:?}", lines[0]);
    apply(&mut app, "compose_carousel_step", json!({"by": "next"}));
    apply(&mut app, "compose_carousel_step", json!({"by": "next"}));
    assert_eq!(cursor(&app).0, 0, "the ring wraps from the last group to the first, where the lists clamp");
    tap(&mut app, &[], KeyCode::KeyO);
    assert_eq!(cursor(&app).0, 2, "O steps back round the ring");
    let refused = apply(&mut app, "compose_carousel_step", json!({"by": "sideways"}));
    assert_eq!(refused["outcome"], "refused", "{refused}");
}

/// **A press writes one line; holding the key writes no more, and the cursor still moves every repeat** (E-002, #129
/// review M2). The walks drive `keys::repeating` at `REPEAT_SECS`, so ten seconds on `↓` used to be about a hundred
/// `compose_walk` lines in the project's own log, and `oplog` re-parses the whole file for every Cmd+Z. `oplog.jsonl`
/// records the changes a person and an agent made; "I held the key down" is one act.
#[test]
fn a_held_arrow_writes_one_line_and_still_walks_every_repeat() {
    let mut app = editor("held");
    // **Held for a little over three repeat intervals**: the press, then three or four repeats.
    let frames = (bevy_cauldron::keys::REPEAT_SECS * 3.5 / (1.0 / 60.0)).ceil() as usize;
    send(&mut app, KeyCode::ArrowDown, ButtonState::Pressed);
    for _ in 0..frames {
        app.update();
    }
    let walked = cursor(&app).0;
    send(&mut app, KeyCode::ArrowDown, ButtonState::Released);
    app.update();
    assert!(walked > 1, "the hold walked the cursor more than the press alone: it reached {walked}");
    let lines = logged(&app, "compose_walk");
    assert_eq!(
        lines.len(),
        1,
        "one press and {walked} steps, and the log holds the press alone: {lines:?}"
    );
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
}

/// **The six actions name the operations they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_six_actions_name_the_operations_they_dispatch() {
    assert_eq!(Action::ComposePrev.operation(), Some("compose_walk"));
    assert_eq!(Action::ComposeNext.operation(), Some("compose_walk"));
    assert_eq!(Action::ComposeMemberPrev.operation(), Some("compose_focus"));
    assert_eq!(Action::ComposeMemberNext.operation(), Some("compose_focus"));
    assert_eq!(Action::CarouselPrev.operation(), Some("compose_carousel_step"));
    assert_eq!(Action::CarouselNext.operation(), Some("compose_carousel_step"));
}
