//! **Back and Forward across workspaces, cameras and a follow** (NV-06, #383; series getting around,
//! #377).
//!
//! The history is one resource and one push, so there is one thing to measure: the place a person is
//! standing in, read off the world after the key. Two acceptance items, and the third is the clip the
//! changelog owes:
//!
//! 1. Map → a cue → a file → Back twice lands on the map **with the camera it had before**. The cue
//!    and the file have no workspace yet (NV-08, #385; GD-11, #373), so what the walk restores is the
//!    place's camera and its page, and the *thing* each place names is the map's own: a placement on
//!    it. That is the honest form of the route until those workspaces exist, and it is the same walk
//!    either way — the stack does not know what a page is.
//! 2. **One Back after a follow restores the person's camera**: the follow's own steps are not places.
//! 3. **A place is a workspace, a panel, a selection and a camera**: Back puts the selection back too.
//! 4. **A place whose thing is gone is skipped with a notice**, and the walk carries on to the next.
//! 5. **Back and Forward are the editor's keys only while the tab has the keys**: with a text field
//!    focused (the palette's, a shell's) `Ctrl+[` is the field's.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::editor::{EditorState, TargetLock, lock_on_placement};
use bevy_cauldron::harness;
use bevy_cauldron::keys::{Context, Live};
use bevy_cauldron::nav::{History, NAV_HISTORY_MAX};
use bevy_cauldron::selection::Selection;
use bevy_cauldron::tiles::Mode;
use bevy_cauldron::view::Rig;
use bevy_cauldron::workspace::Workspace;

use crate::fixtures;

/// Frames a workspace switch and its panels need.
const SETTLE: usize = 20;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("nav_{tag}_{}", std::process::id()))
        .sized_descriptor("bed", "alpha", 2.0, 1.0)
        .sized_descriptor("lamp", "alpha", 0.3, 0.3)
        .place_as("bed@0", "bed", (2.0, 2.0))
        .place_as("lamp@7", "lamp", (12.0, -6.0))
        .side(64.0)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

fn chord(app: &mut App, key: KeyCode) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for code in [primary(), key] {
            app.world_mut().write_message(KeyboardInput {
                key_code: code,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        app.update();
    }
    settle(app);
}

/// **One bare key**, with no modifier: a panel slot, not a workspace chord.
fn press(app: &mut App, key: KeyCode) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: key,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
    settle(app);
}

fn camera(app: &App) -> (Vec3, f32) {
    let rig = app.world().resource::<Rig>();
    (rig.focus, rig.height)
}

/// **The route the issue names, walked with the keys**, and Back twice restores the map's camera.
///
/// The middle two steps are workspace switches, which are places: the cue's and the file's own
/// workspaces are not built, so the two stops are the panels a person can actually reach today — and
/// what the test measures is that the *camera* and the *page* come back, which is what a place is.
#[test]
fn map_to_a_panel_and_back_twice_restores_the_map_and_its_camera() {
    let mut app = editor("route");
    // Somewhere the entry frame would not have put it: the far corner, zoomed in.
    {
        let mut rig = app.world_mut().resource_mut::<Rig>();
        rig.focus = Vec3::new(6.0, 0.0, 6.0);
        rig.height = 7.5;
    }
    settle(&mut app);
    let home = camera(&app);

    // **Two moves away, by the key a person presses**, each its own place. Writing the `Workspace`
    // resource directly is not a walk: nothing records the place being left, so a history built from
    // it would be a history of the test's own writes rather than of the editor's navigation.
    // `Cmd`/`Ctrl`+digit walks to a workspace; a **bare** digit moves inside the one you are in
    // (`keys.rs`, `Workspace`'s own note). Both are one press, and each leaves a place.
    chord(&mut app, KeyCode::Digit4);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Kit, "Cmd/Ctrl+4 is the Kit workspace");
    press(&mut app, KeyCode::Digit2);
    assert_ne!(
        app.world().resource::<Mode>().label(),
        Mode::Meshes.label(),
        "a bare 2 moves off the first panel"
    );

    let history = app.world().resource::<History>();
    assert!(
        history.behind() >= 1,
        "a workspace key leaves a place behind, and the history holds {}",
        history.behind()
    );

    chord(&mut app, KeyCode::BracketLeft);
    chord(&mut app, KeyCode::BracketLeft);

    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Map, "Back twice reaches the Map");
    let now = camera(&app);
    assert!(
        now.0.abs_diff_eq(home.0, 1e-3) && (now.1 - home.1).abs() <= 1e-3,
        "Back twice must restore the camera it left: was {home:?}, now {now:?}"
    );
}

/// **After a follow has moved the view, one Back returns the person to what they were reading.**
///
/// The follow's own travel is not a place: one arrival is one Back, not one per interpolation frame.
#[test]
fn one_back_after_a_follow_restores_the_persons_camera() {
    let mut app = editor("yank");
    {
        let mut rig = app.world_mut().resource_mut::<Rig>();
        rig.focus = Vec3::new(3.0, 0.0, 3.0);
        rig.height = 9.0;
    }
    settle(&mut app);
    let reading = camera(&app);

    // **An agent's change on the map**, exactly as `ops_brp` raises one: the follow is asked to frame a
    // piece nowhere near where the person is looking.
    {
        let mut following = app.world_mut().resource_mut::<bevy_cauldron::follow::Following>();
        following.request_named(
            bevy_cauldron::follow::FollowRequest {
                target: bevy_cauldron::editor::FrameTarget::Placements(vec!["lamp@7".to_owned()]),
                ids: vec!["lamp@7".to_owned()],
            },
            "the Familiar placed lamp@7",
        );
    }
    settle(&mut app);
    let moved = camera(&app);
    assert!(
        !moved.0.abs_diff_eq(reading.0, 1e-3),
        "the follow did not move the camera, so the test proves nothing: {moved:?}"
    );

    chord(&mut app, KeyCode::BracketLeft);
    let back = camera(&app);
    assert!(
        back.0.abs_diff_eq(reading.0, 1e-3) && (back.1 - reading.1).abs() <= 1e-3,
        "one Back must return to what the person was reading: was {reading:?}, now {back:?}"
    );
}

/// **The stack is bounded, and the oldest falls off the front** — never the place being stood in.
#[test]
fn the_history_keeps_the_last_hundred_places() {
    let mut history = History::default();
    for n in 0..(NAV_HISTORY_MAX + 25) {
        history.push(bevy_cauldron::nav::Place {
            workspace: Workspace::Map,
            mode: Mode::Map,
            selection: Some(format!("p{n}")),
            camera: None,
        });
    }
    assert_eq!(history.places().len(), NAV_HISTORY_MAX, "the stack is bounded by `NAV_HISTORY_MAX`");
    assert_eq!(
        history.places().first().and_then(|p| p.selection.clone()).as_deref(),
        Some(format!("p{}", 25).as_str()),
        "the oldest is what falls off, so the newest hundred are kept"
    );
}

/// **A place holds the selection too**: Back puts back the piece that was selected, not only the page and
/// the camera.
#[test]
fn back_restores_the_selection_the_place_held() {
    let mut app = editor("selection");
    assert!(lock_on_placement(app.world_mut(), "lamp@7"), "the fixture places `lamp@7`");
    settle(&mut app);
    assert_eq!(app.world().resource::<Selection>().ids(), ["lamp@7"], "the piece is selected before the walk");

    chord(&mut app, KeyCode::Digit4);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Kit);
    // **Nothing selected where the person went**, so a Back that only moved the page would leave it so.
    *app.world_mut().resource_mut::<TargetLock>() = TargetLock::default();
    settle(&mut app);
    assert!(app.world().resource::<Selection>().is_empty(), "the selection is cleared on the Kit page");

    chord(&mut app, KeyCode::BracketLeft);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Map, "Back reaches the Map");
    assert_eq!(
        app.world().resource::<Selection>().ids(),
        ["lamp@7"],
        "the place the person left held `lamp@7` selected, and Back must put it back"
    );
}

/// **A place whose thing is gone is skipped, with a notice, and the walk carries on.** Two places are
/// behind the person: an older one that can be gone to, and a newer one that names a placement no map
/// holds. One Back must land on the older, and say what it skipped.
#[test]
fn a_place_whose_thing_is_gone_is_skipped_and_the_walk_carries_on() {
    let mut app = editor("gone");
    let mut wanted = bevy_cauldron::nav::Camera::of(app.world().resource::<Rig>());
    wanted.focus = Vec3::new(20.0, 0.0, -20.0);
    wanted.height = 12.0;
    {
        let mut history = app.world_mut().resource_mut::<History>();
        history.push(bevy_cauldron::nav::Place {
            workspace: Workspace::Map,
            mode: Mode::Map,
            selection: None,
            camera: Some(wanted),
        });
        history.push(bevy_cauldron::nav::Place {
            workspace: Workspace::Map,
            mode: Mode::Map,
            selection: Some("ghost@9".to_owned()),
            camera: None,
        });
    }
    chord(&mut app, KeyCode::BracketLeft);
    let now = camera(&app);
    assert!(
        now.0.abs_diff_eq(wanted.focus, 1e-3) && (now.1 - wanted.height).abs() <= 1e-3,
        "Back skipped the gone place and landed on the one before it: wanted {:?}, now {now:?}",
        (wanted.focus, wanted.height)
    );
    let told = app
        .world()
        .resource::<EditorState>()
        .status
        .problems()
        .iter()
        .any(|p| p.text.contains("ghost@9"));
    assert!(told, "the notice names the thing that is gone");
}

/// **With a text field holding the keys, `Ctrl+[` is the field's.** The finder's own field is the
/// nearest: it takes the keys the moment it opens, so Back must not walk the editor away under it.
#[test]
fn back_is_not_the_editors_while_a_text_field_has_the_keys() {
    let mut app = editor("typing");
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    chord(&mut app, KeyCode::Digit4);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Kit);
    assert!(app.world().resource::<History>().behind() >= 1, "the Map is behind the person");

    chord(&mut app, KeyCode::KeyP);
    assert_eq!(
        app.world().resource::<Live>().0,
        Context::Typing,
        "the premise: the open finder's field has the keys"
    );
    let behind = app.world().resource::<History>().behind();

    chord(&mut app, KeyCode::BracketLeft);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Kit, "Back did not walk away under the field");
    assert_eq!(app.world().resource::<History>().behind(), behind, "and the history was not touched");
}
