//! **`Cmd+D` anywhere in Cauldron: point at what is wrong, say so, and the people who make it get a
//! report.**
//!
//! The chord works on the Projects home as well as in the editor — that is the whole point of this
//! channel, and the home is where a person is most likely to notice something wrong before any
//! project is open. The gesture is driven the way a person drives it: the keys go down, the pointer
//! moves, the button is pressed and released.
//!
//! **What a headless app proves and what it cannot.** There is no render world, so no screenshot is
//! read back and no PNG can be written: the report opens ready to point, and saving it is refused in
//! a sentence that names the renderer, writing nothing. Everything before the bytes is proved here.
//! The bytes are proved by a live run under Xvfb.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::{EditorState, Tool};
use bevy_cauldron::harness;
use bevy_cauldron::keys::MOD_KEYS;
use bevy_cauldron::launcher::LauncherPlugin;
use bevy_cauldron::report::{Phase, ReportBox, ReportEdit, ReportOpen};
use bevy_cauldron::screen::Screen;
use bevy_grimoire_core::feedback::{Audience, PairKind, Subject};

use crate::fixtures;

#[derive(Default)]
struct Frame {
    keys: Vec<KeyCode>,
    pointer: Option<Vec2>,
    mouse: Option<bool>,
}

#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
    held: Vec<KeyCode>,
}

fn drive(
    mut script: ResMut<Script>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let frame = script.next.take();
    for key in script.held.drain(..) {
        keys.release(key);
    }
    let Some(frame) = frame else {
        return;
    };
    for key in &frame.keys {
        keys.press(*key);
    }
    script.held = frame.keys;
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(frame.pointer);
    }
    match frame.mouse {
        Some(true) => mouse.press(MouseButton::Left),
        Some(false) => mouse.release(MouseButton::Left),
        None => {}
    }
}

fn step(app: &mut App, frame: Frame) {
    app.world_mut().resource_mut::<Script>().next = Some(frame);
    app.update();
}

/// The chord: the platform's primary modifier and `D`, pressed in one frame.
fn press_cmd_d(app: &mut App, shift: bool) {
    let mut keys = vec![MOD_KEYS[0], KeyCode::KeyD];
    if shift {
        keys.insert(0, KeyCode::ShiftLeft);
    }
    step(app, Frame { keys, ..default() });
}

fn tap(app: &mut App, logical: Key, code: KeyCode) {
    step(app, Frame { keys: vec![code], ..default() });
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

/// **The editor, with a Window the pointer can be moved in**, and the launcher's plugin so the home
/// has something on it to point at.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LauncherPlugin);
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **The Projects home, with its cards and its New project button.**
fn home(tag: &str) -> App {
    let mut app = editor(tag);
    app.world_mut()
        .resource_mut::<NextState<Screen>>()
        .set(Screen::Launcher);
    for _ in 0..6 {
        app.update();
    }
    app
}

/// Where a labelled widget's centre is, in surface pixels — what a person points at.
fn centre_of(app: &mut App, label: &str) -> Vec2 {
    let widget = bevy_gooey::testing::find_by_label(app, label)
        .unwrap_or_else(|e| panic!("no `{label}` on screen: {e:?}"));
    let node = *app
        .world()
        .get::<ComputedNode>(widget)
        .unwrap_or_else(|| panic!("`{label}` is laid out"));
    let at = *app
        .world()
        .get::<UiGlobalTransform>(widget)
        .unwrap_or_else(|| panic!("`{label}` has a place"));
    // `UiGlobalTransform` is the node's centre in physical surface pixels, which is exactly the space
    // the pointer is in (`view::Pointer`): no conversion, and `node` is read to prove it is laid out.
    assert!(
        node.size().min_element() > 0.0,
        "`{label}` has no size, so nobody could point at it"
    );
    at.translation
}

/// **The subject of the one pick a plain click makes**, which is all these gesture tests are about.
fn one_pick(report: &ReportEdit) -> Option<Subject> {
    let picks = report.picks();
    assert_eq!(
        picks.len(),
        1,
        "a plain click picks one thing, and this test means one: {picks:?}"
    );
    picks.first().map(|pick| pick.subject.clone())
}

/// Press and release the button in one place: a click, which picks what is under it.
fn click_at(app: &mut App, at: Vec2) {
    step(
        app,
        Frame {
            pointer: Some(at),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        app,
        Frame {
            pointer: Some(at),
            mouse: Some(false),
            ..default()
        },
    );
}

/// Where a report's files would go, for the audience under test.
fn report_folder(app: &App, audience: Audience) -> std::path::PathBuf {
    match audience {
        Audience::Developers => bevy_grimoire_core::user_dirs::feedback_dir()
            .expect("a test machine has a data directory"),
        Audience::Project => app
            .world()
            .resource::<bevy_cauldron::project::Project>()
            .root
            .join(bevy_grimoire_core::feedback::PROJECT_DIR),
    }
}

fn files_in(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir).map(|entries| entries.count()).unwrap_or(0)
}

/// **`Cmd+D` on the Projects home opens a report for the developers.**
///
/// The home is the screen with no door and no project, which is exactly where the editor's own
/// plugins have not run — so this is also the proof that the report is in the shared plugin list.
#[test]
fn cmd_d_opens_a_report_on_the_projects_home() {
    let mut app = home("report_home");
    assert_eq!(
        *app.world().resource::<State<Screen>>().get(),
        Screen::Launcher,
        "the test is on the Projects home"
    );
    press_cmd_d(&mut app, false);
    let report = app.world().resource::<ReportEdit>();
    assert_eq!(
        report.audience(),
        Some(Audience::Developers),
        "Cmd+D reports to the people who make Cauldron"
    );
    assert_eq!(
        report.phase(),
        Some(Phase::Picking),
        "a headless editor has no picture to read back, so it is ready to point"
    );
}

/// **A click picks the button under the pointer, not the words on it.**
///
/// The chain runs from the outermost node to the innermost, so the words on a button are a step of
/// their own — and a report that said "ButtonText" would not tell anyone what was pressed.
#[test]
fn a_click_picks_the_button_under_the_pointer() {
    let mut app = home("report_click");
    let at = centre_of(&mut app, "New project");
    press_cmd_d(&mut app, false);
    click_at(&mut app, at);

    let report = app.world().resource::<ReportEdit>();
    assert_eq!(report.phase(), Some(Phase::Note), "the click picks a subject");
    let Some(Subject::Element(element)) = one_pick(report) else {
        panic!("a click on a button picks a node, got {:?}", report.picks());
    };
    let last = element.path.last().expect("the chain has a step");
    assert_eq!(
        last.text.as_deref(),
        Some("New project"),
        "the pick names the words on the button: {:?}",
        element.path
    );
    assert!(
        element
            .path
            .iter()
            .any(|step| step.role.as_deref() == Some("Button")),
        "and the chain that contains it names the button: {:?}",
        element.path
    );
    assert!(
        element.rect_px[2] > element.rect_px[0] && element.rect_px[3] > element.rect_px[1],
        "the record carries the node's rectangle: {:?}",
        element.rect_px
    );
    // **Not a toast's stack.** gooey's `ToastStack` is a full-window anchor present whether or not a
    // toast is on it, so a hit test that took the topmost node blindly picked `ToastStack` for
    // everything — measured live on 2026-11-05, pointing at the toolbar. It is excluded; a toast that
    // is actually up is still picked, because its own node is inside the stack and is not.
    assert!(
        !element
            .path
            .iter()
            .any(|step| step.role.as_deref() == Some("ToastStack")),
        "a toast's empty stack must not answer for the window: {:?}",
        element.path
    );
}

/// **The wheel picks the button instead of its words.**
///
/// One notch away from the innermost step moves the pick outward, which is how a person says "the
/// button" about a control whose words they were pointing at.
#[test]
fn the_wheel_picks_the_button_instead_of_its_words() {
    let mut app = home("report_wheel");
    let at = centre_of(&mut app, "New project");
    press_cmd_d(&mut app, false);
    // Hover first, so there is a chain and a step to move within.
    step(
        &mut app,
        Frame {
            pointer: Some(at),
            ..default()
        },
    );
    let (before, len) = app
        .world()
        .resource::<ReportEdit>()
        .picked_step()
        .expect("the pointer is over a chain");
    assert!(len > 1, "the words on a button sit inside it: {len} steps");

    app.world_mut().write_message(bevy::input::mouse::MouseWheel {
        unit: bevy::input::mouse::MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    // The pointer has to be there on the frame the wheel is read.
    step(
        &mut app,
        Frame {
            pointer: Some(at),
            ..default()
        },
    );
    let (after, _) = app
        .world()
        .resource::<ReportEdit>()
        .picked_step()
        .unwrap_or((before, len));
    assert!(
        after < before,
        "one notch away from the centre of the chain: {before} became {after}"
    );
}

/// **A drag picks the box it drew, and `Enter` with nothing picked takes the whole screen.**
#[test]
fn a_drag_picks_an_area_and_enter_takes_the_whole_screen() {
    let mut app = home("report_area");
    let from = Vec2::new(120.0, 120.0);
    let to = Vec2::new(320.0, 260.0);
    press_cmd_d(&mut app, false);
    step(
        &mut app,
        Frame {
            pointer: Some(from),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(to),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(to),
            mouse: Some(false),
            ..default()
        },
    );
    let report = app.world().resource::<ReportEdit>();
    let Some(Subject::Area { rect_px }) = one_pick(report) else {
        panic!("a drag picks a box, got {:?}", report.picks());
    };
    assert!(
        (rect_px[0] - from.x).abs() < 2.0
            && (rect_px[1] - from.y).abs() < 2.0
            && (rect_px[2] - to.x).abs() < 2.0
            && (rect_px[3] - to.y).abs() < 2.0,
        "the box is the drag: {rect_px:?} vs {from:?}..{to:?}"
    );

    // A fresh report, and Enter with nothing picked. (The box arrives a frame late — see
    // `report_box` — so `Esc` here has to reach the report itself, which is what `report_escape` is
    // for.)
    tap(&mut app, Key::Escape, KeyCode::Escape);
    assert!(
        app.world().resource::<ReportEdit>().phase().is_none(),
        "Esc closes a report that has not been described yet"
    );
    press_cmd_d(&mut app, false);
    tap(&mut app, Key::Enter, KeyCode::Enter);
    assert_eq!(
        one_pick(app.world().resource::<ReportEdit>()),
        Some(Subject::Screen),
        "Enter with nothing picked is the whole screen"
    );
}

/// **Saving in a headless editor is refused, names the renderer, and writes nothing.**
#[test]
fn saving_a_report_here_is_refused_and_writes_nothing() {
    for audience in [Audience::Developers, Audience::Project] {
        let mut app = editor(&format!("report_save_{audience:?}"));
        let dir = report_folder(&app, audience);
        let _ = std::fs::remove_dir_all(&dir);
        press_cmd_d(&mut app, audience == Audience::Project);
        assert_eq!(
            app.world().resource::<ReportEdit>().audience(),
            Some(audience),
            "the chord picks the channel"
        );
        tap(&mut app, Key::Enter, KeyCode::Enter);
        assert_eq!(
            app.world().resource::<ReportEdit>().phase(),
            Some(Phase::Note),
            "the whole screen is a subject with nothing to pick"
        );
        // **The box arrives one frame later**, so the `Enter` that chose the subject is not its
        // first newline: see `report_box`.
        app.update();
        // The note box is up; Save is its button.
        let save = bevy_gooey::testing::find_by_label(&mut app, "Save report")
            .unwrap_or_else(|e| panic!("no Save button: {e:?}"));
        app.world_mut().commands().trigger(bevy_gooey::Activate { entity: save });
        for _ in 0..4 {
            app.update();
        }
        let report = app.world().resource::<ReportEdit>();
        assert!(
            report.phase().is_none(),
            "a refused save closes the report rather than leaving it open"
        );
        // The refusal is a toast, which is an entity carrying gooey's own kind and text.
        let said = toasts(&mut app);
        assert!(
            said.iter().any(|line| line.contains("render")),
            "the refusal names the renderer, got {said:?}"
        );
        assert_eq!(files_in(&dir), 0, "{audience:?} wrote a file headless");
    }
}

/// Every toast line on screen, for a test that asserts about what a person was told.
fn toasts(app: &mut App) -> Vec<String> {
    let mut query = app.world_mut().query::<&Text>();
    let world = app.world();
    query
        .iter(world)
        .map(|text| text.0.clone())
        .filter(|line| {
            line.contains("nothing was saved")
                || line.contains("report")
                || line.contains("no render world")
        })
        .collect()
}

/// **`Esc` while pointing writes nothing and leaves no box behind.**
#[test]
fn escape_writes_nothing() {
    let mut app = editor("report_escape");
    let dir = report_folder(&app, Audience::Developers);
    let _ = std::fs::remove_dir_all(&dir);
    press_cmd_d(&mut app, false);
    assert!(app.world().resource::<ReportEdit>().phase().is_some());
    tap(&mut app, Key::Escape, KeyCode::Escape);
    assert!(
        app.world().resource::<ReportEdit>().phase().is_none(),
        "Esc closes a report that is still pointing"
    );
    assert_eq!(files_in(&dir), 0, "a cancelled report writes no file");
}

/// **`Cmd+Shift+D` needs a project, and on the home it says so in a sentence that names both
/// chords.**
#[test]
fn the_project_chord_needs_a_project() {
    let mut app = home("report_no_project");
    press_cmd_d(&mut app, true);
    assert!(
        app.world().resource::<ReportEdit>().phase().is_none(),
        "no project is open, so nothing is collected"
    );
    let said = toasts(&mut app);
    assert!(
        said.iter()
            .any(|line| line.contains("Cmd+D") && line.contains("Cmd+Shift+D")),
        "the refusal names both chords, got {said:?}"
    );
}

/// **In the editor both chords work, and each opens its own channel.**
///
/// This is the exact-modifier check: `Shift+Cmd+D` must not also satisfy `Cmd+D`, or a person who
/// meant to report on their project would file it with the editor's own bug list.
#[test]
fn each_chord_opens_its_own_channel() {
    let mut app = editor("report_two_channels");
    press_cmd_d(&mut app, false);
    assert_eq!(
        app.world().resource::<ReportEdit>().audience(),
        Some(Audience::Developers)
    );
    tap(&mut app, Key::Escape, KeyCode::Escape);
    press_cmd_d(&mut app, true);
    assert_eq!(
        app.world().resource::<ReportEdit>().audience(),
        Some(Audience::Project),
        "Shift+Cmd+D is the project's channel, and only that one"
    );
}

/// **While a report is open the tab's keys are quiet.** `R` arms removal everywhere else, and a
/// report about a panel must not also delete the piece behind it.
#[test]
fn a_report_open_keeps_the_census_quiet() {
    let mut app = editor("report_census");
    press_cmd_d(&mut app, false);
    step(&mut app, Frame { keys: vec![KeyCode::KeyR], ..default() });
    assert!(
        !matches!(app.world().resource::<EditorState>().tool, Tool::Remove),
        "R must not arm removal under an open report"
    );
}

/// **Saving takes the note box away in the same frame, not the next one.**
///
/// Measured live on 2026-11-05: the note box is a gooey `dialog`, so for the frame between a save and
/// the box's despawn its scrim still covered the whole window and the next report's click picked
/// `Scrim` instead of the control under the pointer. A person does not see a frame, but the click
/// they make in it is the one that goes wrong.
#[test]
fn saving_takes_the_note_box_away_at_once() {
    let mut app = editor("report_close_now");
    press_cmd_d(&mut app, false);
    tap(&mut app, Key::Enter, KeyCode::Enter);
    // The box arrives a frame later than the subject: see `report_box`.
    app.update();
    let save = bevy_gooey::testing::find_by_label(&mut app, "Save report")
        .unwrap_or_else(|e| panic!("no Save button: {e:?}"));
    app.world_mut()
        .commands()
        .trigger(bevy_gooey::Activate { entity: save });
    // **One frame**, and the box is gone with the report: no frame in which a scrim is left over.
    app.update();
    assert!(
        app.world().resource::<ReportEdit>().phase().is_none(),
        "the save finished"
    );
    let boxes = app
        .world_mut()
        .query_filtered::<Entity, With<ReportBox>>()
        .iter(app.world())
        .count();
    assert_eq!(boxes, 0, "the note box went with the report");
}

/// **The overlay is over the whole window and above a modal**, because a report has to be able to
/// point at a dialog and at everything a dialog covers.
#[test]
fn the_report_draws_over_everything() {
    let mut app = editor("report_over_modal");
    press_cmd_d(&mut app, false);
    // A headless editor has no frozen picture, but the layer that would carry it is up.
    let root = app
        .world_mut()
        .query_filtered::<Entity, With<ReportOpen>>()
        .single(app.world())
        .unwrap_or_else(|e| panic!("the report draws a layer: {e}"));
    let z = app
        .world()
        .get::<GlobalZIndex>(root)
        .expect("the layer declares its z");
    assert!(
        z.0 > bevy_cauldron::chrome::MODAL_Z,
        "a report must be able to point at a dialog: {} vs {}",
        z.0,
        bevy_cauldron::chrome::MODAL_Z
    );
    let boxes = app
        .world_mut()
        .query_filtered::<Entity, With<ReportBox>>()
        .iter(app.world())
        .count();
    assert_eq!(
        boxes, 0,
        "the note box is not up while the person is pointing"
    );
}

/// **`Tab` steps down the stack to the node behind the pointer.**
///
/// The bug this exists for: the wheel used to walk the *ancestor chain* of the topmost hit node. An
/// overlapping label is usually a **sibling** of what covers it, so no ancestor walk can reach it —
/// there is nowhere on screen where only the label is under the cursor. Two absolutely-positioned
/// siblings with the same rectangle and different `GlobalZIndex` are that shape in miniature: only the
/// front one is a click, and only `Tab` can reach the one behind it.
#[test]
fn tab_steps_down_the_stack_to_the_node_behind_the_pointer() {
    let mut app = home("report_stack");
    let at = centre_of(&mut app, "New project");
    // Two nodes over the same pixels. The second is in front, so it is what a click takes.
    //
    // **Full-window, not a small box at `at`.** `Pointer` and `ComputedNode` are in *physical* surface
    // pixels while `Val::Px` is logical, and the editor runs at a UI scale of 1.2 — a box placed at
    // `at.x - 40` lands ~20% away from the pointer and the test then asserts about whatever node
    // really was under it. A filled layer is under the pointer wherever it is, which is the case this
    // test is about: two siblings that share their rectangle exactly.
    let covering = |z: i32, name: &str| {
        (
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Name::new(name.to_owned()),
            GlobalZIndex(z),
        )
    };
    let behind = app.world_mut().spawn(covering(1, "behind")).id();
    let front = app.world_mut().spawn(covering(2, "in front")).id();
    for _ in 0..3 {
        app.update();
    }

    press_cmd_d(&mut app, false);
    // Hover first: the stack is built while nothing is pressed.
    step(
        &mut app,
        Frame {
            pointer: Some(at),
            ..default()
        },
    );
    let (before, len) = app
        .world()
        .resource::<ReportEdit>()
        .picked_step()
        .expect("the pointer is over a stack");
    assert!(len >= 2, "both nodes are under the pointer: {len}");
    let picked_before = app
        .world()
        .resource::<ReportEdit>()
        .picked_entity()
        .expect("a stack with an entry has a picked entity");
    assert_eq!(
        picked_before, front,
        "the front-most node is what a click would take"
    );

    // **The pointer is on the frame the key arrives**, as a hand holds it: `report_pick` answers
    // about the stack under the pointer, so a frame with no pointer has no stack to step.
    step(
        &mut app,
        Frame {
            pointer: Some(at),
            keys: vec![KeyCode::Tab],
            ..default()
        },
    );
    let (after, len_after) = app
        .world()
        .resource::<ReportEdit>()
        .picked_step()
        .expect("the stack is still there");
    assert_eq!(len_after, len, "the same stack");
    assert!(
        after < before,
        "`Tab` steps down the held stack, toward the back: {before} became {after}"
    );
    assert_eq!(
        app.world()
            .resource::<ReportEdit>()
            .picked_entity(),
        Some(behind),
        "and the outlined node is the one behind"
    );
}

/// **A shift-click adds a second pick, and Enter finishes**: the record carries both, and the note
/// box says how they sit against each other.
///
/// This is the whole reason for the feature: "this label is under the viewport" is a fact about two
/// elements, and a record that holds one makes a person file two reports and describe the relation in
/// prose.
#[test]
fn shift_click_adds_a_second_pick_and_enter_finishes() {
    let mut app = home("report_multi");
    let first = centre_of(&mut app, "New project");
    press_cmd_d(&mut app, false);

    click_at(&mut app, first);
    assert_eq!(
        app.world().resource::<ReportEdit>().phase(),
        Some(Phase::Note),
        "a plain click finishes the pick, as it always did"
    );
    // Back to pointing, so a second can be added.
    tap(&mut app, Key::Escape, KeyCode::Escape);
    press_cmd_d(&mut app, false);

    // Shift-click twice, two different elements of the home.
    let second = centre_of(&mut app, "Open folder…");
    step(
        &mut app,
        Frame {
            pointer: Some(first),
            keys: vec![KeyCode::ShiftLeft],
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(first),
            keys: vec![KeyCode::ShiftLeft],
            mouse: Some(true),
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(first),
            keys: vec![KeyCode::ShiftLeft],
            mouse: Some(false),
        },
    );
    let report = app.world().resource::<ReportEdit>();
    assert_eq!(
        report.picks().len(),
        1,
        "a shift-click takes one thing, and stays pointing: {:?}",
        report.picks()
    );
    assert_eq!(
        report.phase(),
        Some(Phase::Picking),
        "and the report is still pointing, so another can be added"
    );

    step(
        &mut app,
        Frame {
            pointer: Some(second),
            keys: vec![KeyCode::ShiftLeft],
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(second),
            keys: vec![KeyCode::ShiftLeft],
            mouse: Some(true),
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(second),
            keys: vec![KeyCode::ShiftLeft],
            mouse: Some(false),
        },
    );
    let report = app.world().resource::<ReportEdit>();
    assert_eq!(report.picks().len(), 2, "{:?}", report.picks());
    assert_eq!(report.phase(), Some(Phase::Picking));

    tap(&mut app, Key::Enter, KeyCode::Enter);
    let report = app.world().resource::<ReportEdit>();
    assert_eq!(
        report.phase(),
        Some(Phase::Note),
        "Enter finishes what the shift-clicks accumulated"
    );
    assert_eq!(report.picks().len(), 2, "and carries both picks");
    // **The relation is worked out, not asked**: two elements of the home are somewhere against each
    // other, and the record says which.
    let relations = report.relations();
    assert_eq!(relations.len(), 1, "two picks are one pair: {relations:?}");
    assert_eq!(relations[0].a, 0);
    assert_eq!(relations[0].b, 1);
    // The note box is up, one frame later, and it names the relation.
    app.update();
    let mut texts = app.world_mut().query::<&Text>();
    let shown: Vec<String> = texts.iter(app.world()).map(|t| t.0.clone()).collect();
    let relation_word = match relations[0].kind {
        PairKind::Contains => "contains",
        PairKind::Overlaps => "overlaps",
        PairKind::Touches => "touches",
        PairKind::Apart => "apart",
    };
    assert!(
        shown.iter().any(|line| line.contains(relation_word)),
        "the note box states the relation `{relation_word}`, got {shown:?}"
    );
}

/// **A plain click after a shift-click collapses back to one pick.** A person who changed their mind
/// does not have to start the report again, and the record never carries a pick they replaced.
#[test]
fn a_plain_click_replaces_the_picks_it_finds() {
    let mut app = home("report_replace");
    let first = centre_of(&mut app, "New project");
    let second = centre_of(&mut app, "Open folder…");
    press_cmd_d(&mut app, false);
    step(
        &mut app,
        Frame {
            pointer: Some(first),
            keys: vec![KeyCode::ShiftLeft],
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(first),
            keys: vec![KeyCode::ShiftLeft],
            mouse: Some(true),
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(first),
            keys: vec![KeyCode::ShiftLeft],
            mouse: Some(false),
        },
    );
    assert_eq!(app.world().resource::<ReportEdit>().picks().len(), 1);

    click_at(&mut app, second);
    let report = app.world().resource::<ReportEdit>();
    assert_eq!(report.phase(), Some(Phase::Note));
    assert_eq!(
        report.picks().len(),
        1,
        "the plain click replaced what was there: {:?}",
        report.picks()
    );
    assert!(
        report.relations().is_empty(),
        "one pick has no relation to anything"
    );
}
