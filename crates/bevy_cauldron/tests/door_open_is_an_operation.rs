//! **Opening a door is a registered operation (cycle 8, item 1).**
//!
//! `door_open` names the door and, optionally, a panel of it. The answer is the target; the crossing
//! happens in the editor. These tests cover the operation's own contract: that it is registered,
//! that it refuses what it cannot do with the reason in the words, and that it accepts a request
//! with the exact target in its answer. Nothing here writes a map.

use bevy_cauldron::editor::{DoorRequest, EditorState};
use bevy_cauldron::tiles::{Door, Mode, Tab};
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Applied, Ctx, Source};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::json;
use bevy::ecs::change_detection::DetectChanges;

use crate::fixtures;

fn empty_ctx() -> Ctx {
    Ctx {
        root: None,
        project: None,
        open: None,
        source: Source::Human,
        session: String::new(),
        proposal: None,
    }
}

fn refusal_text(params: serde_json::Value) -> String {
    let mut ctx = empty_ctx();
    match ops::dispatch(&mut ctx, "door_open", &params) {
        Err(refusal) => refusal.to_string(),
        Ok(applied) => panic!("expected a refusal, got {:?}", summary(applied)),
    }
}

fn summary(applied: Applied) -> String {
    match applied {
        Applied::Done { outcome, .. } => outcome.summary,
        other => format!("{other:?}"),
    }
}

#[test]
fn the_registry_has_door_open() {
    assert!(
        ops::registry().iter().any(|op| op.name() == "door_open"),
        "the registry has no `door_open`, so an agent cannot open a door through an operation"
    );
}

#[test]
fn an_unknown_door_is_refused_and_the_doors_are_named() {
    let text = refusal_text(json!({"door": "attic"}));
    assert!(text.contains("attic"), "the refusal names what was asked: {text}");
    for door in ["kit", "map", "rigs"] {
        assert!(text.contains(door), "the refusal lists the doors: {text}");
    }
}

#[test]
fn a_panel_the_door_does_not_show_is_refused() {
    // The Kit door shows its own panels; a Map-door panel asked for on Kit must be refused.
    let text = refusal_text(json!({"door": "kit", "tab": "map"}));
    assert!(
        text.contains("does not show"),
        "a panel the door does not show is refused with that reason: {text}"
    );
}

#[test]
fn the_map_door_needs_a_map_to_open_onto() {
    let text = refusal_text(json!({"door": "map"}));
    assert!(
        text.contains("no map is open"),
        "the Map door with no map open is refused with the reason: {text}"
    );
}

#[test]
fn an_accepted_request_names_its_exact_target() {
    // **A project is open, as it is in the editor.** The operation writes its log line to the
    // project, so a dispatch with no project is refused before any door is named.
    let root = fixtures::Fixture::new("door_open_accepted").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let mut ctx = Ctx {
        root: None,
        project: app.world_mut().remove_resource::<Project>(),
        open: app.world_mut().remove_resource::<OpenMap>(),
        source: Source::Human,
        session: String::new(),
        proposal: None,
    };
    let applied = ops::dispatch(&mut ctx, "door_open", &json!({"door": "KIT", "tab": "Meshes"}))
        .unwrap_or_else(|e| panic!("door_open refused a valid request: {e}"));
    let Applied::Done { outcome, .. } = applied else {
        panic!("door_open did not apply");
    };
    assert_eq!(
        outcome.after,
        Some(json!({"door": "kit", "tab": "meshes"})),
        "the answer names the door and panel the editor must cross to"
    );
    assert!(
        outcome.inverse.is_none(),
        "opening a door changes no document, so there is nothing to take back"
    );
}

#[test]
fn a_stored_request_crosses_the_door_on_the_next_frame() {
    // **The BRP path stores the request; the editor crosses it in the next frame.** This is the
    // half an agent's `door_open` reaches: `request_door` puts it in `EditorState::door_request`
    // and `apply_pending_door` crosses it.
    let root = fixtures::Fixture::new("door_open_crosses").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    assert_ne!(
        *app.world().resource::<Door>(),
        Door::Kit,
        "the test starts on another door, so a crossing to Kit is visible"
    );
    app.world_mut().resource_mut::<EditorState>().door_request =
        Some(DoorRequest { door: Door::Kit, tab: Some(Mode::Tiles) });
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(*app.world().resource::<Door>(), Door::Kit, "the request crossed to the Kit door");
    assert!(*app.world().resource::<Mode>() == Mode::Tiles, "and opened the Tiles panel");
    assert!(
        app.world().resource::<EditorState>().door_request.is_none(),
        "a crossed request is consumed, so it does not cross again"
    );
}

#[test]
fn a_map_that_does_not_parse_is_refused_not_answered_as_applied() {
    // **The Map door crosses onto the map on disk.** A map that will not parse cannot be crossed to,
    // and the crossing used to refuse after the answer had already said "applied". The read is the one
    // the crossing makes, so it belongs in the plan, before anything is answered.
    let root = fixtures::Fixture::new("door_open_bad_map").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let mut project = app.world_mut().remove_resource::<Project>().expect("a project is open");
    let open = app.world_mut().remove_resource::<OpenMap>().expect("a map is open");
    let map_path = open.map_path.clone();
    std::fs::write(&map_path, "this is not a map").expect("the map file can be written");
    project.open_map_name = Some("m".to_owned());
    let mut ctx = Ctx {
        root: None,
        project: Some(project),
        open: None,
        source: Source::Human,
        session: String::new(),
        proposal: None,
    };
    let text = match ops::dispatch(&mut ctx, "door_open", &json!({"door": "map"})) {
        Err(refusal) => refusal.to_string(),
        Ok(Applied::Refused(refusal)) => refusal.to_string(),
        Ok(done) => panic!("a map that does not parse was answered as applied: {}", summary(done)),
    };
    assert!(
        text.contains(&map_path.display().to_string()),
        "the refusal names the map file that would not open: {text}"
    );
}

#[test]
fn a_dirty_map_asked_to_leave_says_it_waits_for_the_person() {
    // **Leaving unsaved edits is the person's question.** The answer is accepted and names what it
    // waits on; the editor asks the question. It must not read as "opened".
    let root = fixtures::Fixture::new("door_open_dirty").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let project = app.world_mut().remove_resource::<Project>().expect("a project is open");
    let mut open = app.world_mut().remove_resource::<OpenMap>().expect("a map is open");
    open.map.note = Some("an edit that is not saved".to_owned());
    assert!(open.dirty(), "the test makes the map unsaved");
    let mut ctx = Ctx {
        root: None,
        project: Some(project),
        open: Some(open),
        source: Source::Human,
        session: String::new(),
        proposal: None,
    };
    let applied = ops::dispatch(&mut ctx, "door_open", &json!({"door": "kit"}))
        .unwrap_or_else(|e| panic!("door_open refused a request the person decides: {e}"));
    let Applied::Done { outcome, .. } = applied else {
        panic!("door_open did not accept the request");
    };
    assert_eq!(
        outcome.after.as_ref().and_then(|a| a["waiting"].as_str()),
        Some("the map has unsaved changes"),
        "the answer says it waits on the person, and why: {:?}",
        outcome.after
    );
}

#[test]
fn a_dirty_map_asked_for_the_map_door_waits_for_nobody() {
    // **Already on the Map door, the request crosses nothing**, so nothing is waiting on the person.
    let root = fixtures::Fixture::new("door_open_dirty_map").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let project = app.world_mut().remove_resource::<Project>().expect("a project is open");
    let mut open = app.world_mut().remove_resource::<OpenMap>().expect("a map is open");
    open.map.note = Some("an edit that is not saved".to_owned());
    let mut ctx = Ctx {
        root: None,
        project: Some(project),
        open: Some(open),
        source: Source::Human,
        session: String::new(),
        proposal: None,
    };
    let applied = ops::dispatch(&mut ctx, "door_open", &json!({"door": "map"}))
        .unwrap_or_else(|e| panic!("door_open refused the Map door: {e}"));
    let Applied::Done { outcome, .. } = applied else {
        panic!("door_open did not accept the request");
    };
    assert!(
        outcome.after.as_ref().is_some_and(|a| a.get("waiting").is_none()),
        "no leaving question for a request that stays on the Map door: {:?}",
        outcome.after
    );
}

/// **Opening the door you are on changes no document.** `open_door` used to take `Project` and
/// `OpenMap` out of the world and put them back, which marks both changed on every tab press, so
/// every system that watches the map believes it was edited. A probe reads the change flags the way
/// a real system does.
#[derive(bevy::prelude::Resource, Default)]
struct DocumentProbe {
    project_changed: bool,
    open_changed: bool,
}

fn probe_document(
    project: Option<bevy::prelude::Res<Project>>,
    open: Option<bevy::prelude::Res<OpenMap>>,
    mut seen: bevy::prelude::ResMut<DocumentProbe>,
) {
    seen.project_changed = project.is_some_and(|p| p.is_changed());
    seen.open_changed = open.is_some_and(|o| o.is_changed());
}

#[test]
fn opening_the_door_you_are_on_marks_no_document_changed() {
    let root = fixtures::Fixture::new("door_open_marks_nothing").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<DocumentProbe>();
    app.add_systems(bevy::prelude::Update, probe_document);
    for _ in 0..4 {
        app.update();
    }
    // The precondition: a frame that does nothing to the map reads unchanged, so the probe is honest.
    app.update();
    let quiet = app.world().resource::<DocumentProbe>();
    assert!(
        !quiet.project_changed && !quiet.open_changed,
        "a quiet frame reads unchanged before the press, so a changed read means the press did it"
    );
    let door = *app.world().resource::<Door>();
    let tab = *app.world().resource::<Mode>();
    bevy_cauldron::editor::open_door(app.world_mut(), door, Some(tab));
    app.update();
    let after = app.world().resource::<DocumentProbe>();
    assert!(!after.project_changed, "the press on the current door marked the project changed");
    assert!(!after.open_changed, "the press on the current door marked the open map changed");
}

#[test]
fn a_click_on_a_tab_chip_writes_the_same_log_line_a_key_press_does() {
    // **A chip click is the same entry as the slot key** (cycle 8 gap 2). The key goes through
    // `open_door`, which logs a `door_open` line; a click that went straight to `enter_tab` logs
    // nothing, so the log misses a press the person made.
    use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
    use bevy::picking::backend::HitData;
    use bevy::picking::events::{Pointer, PointerClick};
    use bevy::picking::pointer::{Location, PointerButton, PointerId};
    use bevy::prelude::{Entity, Vec2};
    let root = fixtures::Fixture::new("door_open_click_logs").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    app.world_mut().resource_mut::<EditorState>().door_request =
        Some(DoorRequest { door: Door::Kit, tab: None });
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(*app.world().resource::<Door>(), Door::Kit, "the test stands on the Kit door");
    let log = app.world().resource::<Project>().project_dir.join("oplog.jsonl");
    let door_lines = |path: &std::path::Path| {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .filter(|l| {
                serde_json::from_str::<serde_json::Value>(l)
                    .ok()
                    .is_some_and(|v| v["op"] == "door_open")
            })
            .count()
    };
    let before = door_lines(&log);
    let chip = {
        let world = app.world_mut();
        let mut tabs = world.query::<(Entity, &Tab)>();
        tabs.iter(world)
            .find(|(_, tab)| tab.0 == Mode::Tiles)
            .map(|(entity, _)| entity)
            .expect("the Kit door shows a Tiles chip")
    };
    app.world_mut().trigger(PointerClick {
        entity: chip,
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
    for _ in 0..4 {
        app.update();
    }
    assert!(*app.world().resource::<Mode>() == Mode::Tiles, "the chip opened its tab");
    assert_eq!(
        door_lines(&log),
        before + 1,
        "the click wrote one door_open line, as a key press does"
    );
}

/// **A second door cannot be stored over the first (cycle 8, item 10).** The first request waits for the
/// next frame. Storing a second one over it drops the first, and the agent that asked for the first is
/// answered "applied" for a door it never reaches.
#[test]
fn a_second_door_request_while_one_waits_is_refused_and_the_first_stays() {
    let mut state = EditorState::default();
    bevy_cauldron::editor::request_door(&mut state, Some(&json!({"door": "kit"})));
    let first = state.door_request;
    assert!(first.is_some(), "the first request must be stored");
    bevy_cauldron::editor::request_door(&mut state, Some(&json!({"door": "map"})));
    assert!(
        state.door_request == first,
        "a second request must not replace the first one waiting"
    );
    assert!(
        !state.status.problems().is_empty(),
        "the refused second request must say so in the journal"
    );
}

/// **A door asked for while the editor asks a question is refused (cycle 8, item 6).** The question
/// owns the next frame, so a crossing taken under it would act on an answer the person has not given.
#[test]
fn a_door_is_refused_while_the_editor_is_asking_a_question() {
    let state = EditorState::default();
    assert!(
        bevy_cauldron::editor::door_refusal(&state, true).is_some(),
        "a door must be refused while a question is on screen"
    );
    assert!(
        bevy_cauldron::editor::door_refusal(&state, false).is_none(),
        "a door with nothing waiting and no question is allowed"
    );
}

/// **An agent's receipt survives the crossing it asked for (cycle 8, gap 7).** The agent's door is
/// crossed on a later frame, and the crossing resets `EditorState`, which erased the receipt that said
/// who asked. The person changed doors with no word of the agent.
#[test]
fn an_agents_receipt_survives_the_crossing_it_asked_for() {
    let root = fixtures::Fixture::new("door_receipt_survives").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    assert_ne!(
        *app.world().resource::<Door>(),
        Door::Kit,
        "the test starts on another door, so a crossing to Kit is visible"
    );
    app.world_mut()
        .resource_mut::<EditorState>()
        .status
        .note("agent: door_open: opened the Kit door");
    app.world_mut().resource_mut::<EditorState>().door_request =
        Some(DoorRequest { door: Door::Kit, tab: Some(Mode::Tiles) });
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(*app.world().resource::<Door>(), Door::Kit, "the request crossed to the Kit door");
    assert_eq!(
        app.world().resource::<EditorState>().status.note_text(),
        "agent: door_open: opened the Kit door",
        "the agent's receipt is what the person reads after the crossing"
    );
}
