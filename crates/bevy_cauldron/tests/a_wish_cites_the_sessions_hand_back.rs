//! **A wish cites a hand-back this session watched, and the queue outlives a door change**
//! (#353's review, M3 and M4).
//!
//! # M3 — a note from an earlier session is not a hand-back
//!
//! `after: {note: "n3"}` was checked against the notes on disk, and a note resolved last week reads
//! as `done` on disk exactly as one resolved a minute ago does. So the editor has to remember which
//! notes **this session** saw hand back, and refuse the rest: the rule the issue states is "a
//! presentation or a resolved note **from this editor session** that has no wish yet".
//!
//! # M4 — the queue lives in the session
//!
//! `Wishes` and `HandBacks` were `Ownership::Door`, so a door change emptied the queue and dropped
//! every hand-back a wish could cite, and presentation ids restarted at `present-1` on the next
//! door — which made `present-1` on one door and `present-1` on the next the same words for two
//! different hand-backs. The fix is the smallest one that keeps `after: {presentation: "present-N"}`
//! reading exactly as an agent wrote it: both resources are the session's, presentations are
//! numbered by a counter no door change touches, and the one-wish rule compares a key that carries
//! the map a note is on.
//!
//! # What is proved here
//!
//! 1. **A note resolved in this session is a hand-back a wish may cite**, read back `waiting`.
//! 2. **A note already `done` when this session opened its map is refused**, naming the id: it was
//!    handed back in an earlier session, and the record says so. The earlier session is a real one —
//!    a first editor resolved the note and was dropped — so the file on disk is the file a previous
//!    sitting leaves.
//! 3. **The queue and its hand-backs are the session's**: a door change — `reset_door_state`, what
//!    `OnExit(Editor)` runs — leaves them standing, and the next presentation is numbered on from
//!    the last rather than restarting.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::present::{HandBackKind, HandBacks};

use crate::fixtures;

/// **A project with one map and one piece**, saved.
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m")
}

fn editor_with_bridge(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    use bevy::remote::RemoteMethodSystemId;
    use bevy::remote::RemoteMethods;
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{method}: {e:?}"))?
        .map_err(|e| format!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"))
}

/// **Present a result and answer with its id**, which is the hand-back a wish cites.
fn present(app: &mut App) -> String {
    let presented = call(
        app,
        "grimoire/present",
        serde_json::json!({
            "title": "the courtyard",
            "summary": "A 20 x 6 x 20 m courtyard.",
        }),
    )
    .unwrap_or_else(|e| panic!("the present answers: {e}"));
    presented["presentation"]
        .as_str()
        .unwrap_or_else(|| panic!("the present names the presentation: {presented}"))
        .to_owned()
}

/// **Answer the question the way a person does**, so the next one can be asked.
fn press(app: &mut App, code: KeyCode) {
    use bevy::input::keyboard::{Key, KeyboardInput};
    for state in [
        bevy::input::ButtonState::Pressed,
        bevy::input::ButtonState::Released,
    ] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

/// **A wish citing a hand-back**, with every field filled in.
fn wish_body(after: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "after": after,
        "task": "place a wall along the courtyard's east side",
        "hardship": "three calls to grimoire_state to find the wall's piece id",
        "tool": "grimoire_find_piece: searches the library by label and answers the ids.",
        "example_call": "grimoire_find_piece({\"label\": \"wall\"})",
    })
}

/// **Work and resolve a note the way an agent does it**, through the registry, and its id.
///
/// The whole real path — `note_add`, `note_send`, `note_claim`, an operation under the note, and
/// `note_resolve` — so the file that ends up on disk is the file an agent leaves.
fn resolve_a_note(app: &mut App) -> String {
    apply(app, "note_add", serde_json::json!({"cells": [0, 0, 2, 2], "text": "tidy the wall"}));
    apply(app, "note_send", serde_json::json!({}));
    apply(app, "note_claim", serde_json::json!({"id": "n1", "agent": "omp"}));
    let worked = apply(app, "placement_add", serde_json::json!({"piece": "site/wall", "at": [1.0, 1.0]}));
    let operation_id = worked["operation_id"].as_str().expect("the operation has an id").to_owned();
    let resolved = apply(
        app,
        "note_resolve",
        serde_json::json!({
            "id": "n1",
            "agent": "omp",
            "summary": "added a wall",
            "ops": [operation_id],
            "captures": ["before.png", "after.png"],
        }),
    );
    assert_eq!(resolved["outcome"], "applied", "the note resolves: {resolved}");
    // **The editor's own reader of the notes file** has to see it before the record is asked about
    // it: the record reads `NotesView`, not the file.
    for _ in 0..4 {
        app.update();
    }
    "n1".to_owned()
}

/// **The open project's root**, which is what the record compares — the harness canonicalises it.
fn project_root(app: &App) -> std::path::PathBuf {
    app.world().resource::<bevy_cauldron::project::Project>().root.clone()
}

/// **The record's answer for one citation**, asked the way `grimoire/wish` asks it: the kind, the
/// id, this project, and the open map's own file for a note.
fn cited<'a>(app: &'a App, kind: HandBackKind, id: &str) -> Option<&'a bevy_cauldron::present::HandBack> {
    let root = app.world().resource::<bevy_cauldron::project::Project>().root.clone();
    let map = app
        .world()
        .get_resource::<bevy_cauldron::project::OpenMap>()
        .map(|open| open.map_path.clone());
    app.world()
        .resource::<HandBacks>()
        .cited(kind, id, &root, map.as_deref())
}

/// **1. A note resolved in this session is a hand-back a wish may cite.**
#[test]
fn a_wish_cites_a_note_this_session_resolved() {
    let root = project(&format!("wish_note_{}", std::process::id()));
    let mut app = editor_with_bridge(&root);
    let note = resolve_a_note(&mut app);
    assert!(
        cited(&app, HandBackKind::Note, &note).is_some(),
        "a note resolved in this session is a hand-back, and the record does not hold it"
    );

    let made = call(&mut app, "grimoire/wish", wish_body(serde_json::json!({"note": note})))
        .unwrap_or_else(|e| panic!("a wish citing this session's resolved note must be taken: {e}"));
    assert_eq!(
        made["state"], "waiting",
        "the wish was not left waiting in the editor's queue: {made}"
    );
    assert_eq!(made["after"], note.as_str(), "the wish records the note's id: {made}");
}

/// **2. A note resolved in an earlier session is refused**, and the refusal names it.
#[test]
fn a_wish_citing_a_note_from_an_earlier_session_is_refused() {
    let root = project(&format!("wish_note_old_{}", std::process::id()));

    // **The earlier sitting**: an editor resolved the note and was dropped, so the file on disk is
    // the one a previous session leaves.
    {
        let mut before = editor_with_bridge(&root);
        resolve_a_note(&mut before);
    }

    // **This session**, opened on the same project.
    let mut app = editor_with_bridge(&root);
    assert!(
        cited(&app, HandBackKind::Note, "n1").is_none(),
        "a note already `done` when this session opened the map was handed back in an earlier \
         session, and the record must not hold it"
    );
    let refused = call(&mut app, "grimoire/wish", wish_body(serde_json::json!({"note": "n1"})))
        .expect_err("a note from an earlier session is not a hand-back this session watched");
    assert!(
        refused.contains("n1"),
        "the refusal does not name the note it looked for: {refused:?}"
    );
    assert!(
        refused.contains("earlier session"),
        "the refusal does not say why a `done` note is not a hand-back: {refused:?}"
    );
}

/// **3a. A door change keeps the queue and the hand-backs it cites.**
#[test]
fn a_door_change_keeps_the_wishes_and_their_hand_backs() {
    let root = project(&format!("wish_crosses_{}", std::process::id()));
    let mut app = editor_with_bridge(&root);
    let after = present(&mut app);
    // **The root, before the teardown**: `close_the_door` drops the `Project`, and a crossing puts
    // the next door's back — which the harness does not do, because it registers no `ScreenPlugin`.
    let root = project_root(&app);
    call(&mut app, "grimoire/wish", wish_body(serde_json::json!({"presentation": after})))
        .unwrap_or_else(|e| panic!("the wish must be taken: {e}"));
    assert_eq!(
        app.world().resource::<bevy_cauldron::present::Wishes>().queued.len(),
        1,
        "the wish was not queued"
    );

    // **The teardown**, called directly: the headless harness registers no `ScreenPlugin`, so it
    // never runs `OnExit(Editor)`, and `close_the_door` is what that schedule does.
    bevy_cauldron::screen::close_the_door(app.world_mut());

    assert_eq!(
        app.world().resource::<bevy_cauldron::present::Wishes>().queued.len(),
        1,
        "the door change emptied the wish queue. #353 says the queue lives in the editor session \
         and nothing is lost."
    );
    assert!(
        app.world()
            .resource::<HandBacks>()
            .cited(HandBackKind::Presentation, &after, &root, None)
            .is_some(),
        "the door change dropped the hand-back the queued wish cites, so it can no longer be read \
         back or checked against the one-wish rule"
    );
    // **And the queue's one-wish rule still knows that hand-back**: the key survived with the
    // record, so a second wish for it is refused. Asked of the queue directly, because the
    // harness's teardown leaves no `Project` and the handler refuses before the rule is reached.
    assert!(
        app.world()
            .resource::<bevy_cauldron::present::Wishes>()
            .has_wish_for(&after),
        "the door change left the queued wish, and the one-wish rule no longer sees its hand-back"
    );
}

/// **3b. Presentation ids count on across a door change**, so `present-1` never names two turns.
#[test]
fn presentation_ids_do_not_restart_at_a_door_change() {
    let root = project(&format!("wish_ids_{}", std::process::id()));
    let mut app = editor_with_bridge(&root);
    let first = present(&mut app);
    assert_eq!(first, "present-1", "the first presentation of the session is `present-1`");
    let root = project_root(&app);
    press(&mut app, KeyCode::KeyN);

    bevy_cauldron::screen::close_the_door(app.world_mut());

    // **What the teardown did to the counter**, which is what numbers the next one. The headless
    // harness registers no `ScreenPlugin`, so nothing re-installs the `Project` a crossing puts
    // back, and a second `grimoire/present` would be refused for the missing resource rather than
    // counted. The counter itself is the property: it is `Session`, so the teardown leaves it.
    assert_eq!(
        app.world().resource::<HandBacks>().asked,
        1,
        "presentations are numbered for the session: after a door change the next hand-back is \
         `present-2`, not a second `present-1`. The id is what an agent writes in \
         `after: {{presentation: \"present-N\"}}`, so two hand-backs sharing one is two turns an \
         agent cannot tell apart."
    );
    assert!(
        app.world()
            .resource::<HandBacks>()
            .cited(HandBackKind::Presentation, &first, &root, None)
            .is_some(),
        "the door change dropped the hand-back `present-1` names"
    );
}
