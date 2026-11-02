//! **A refusal is a sticky `bevy_gooey` toast, and Show goes to what it is about** (UI-39, #233).
//!
//! The refusal used to be a hand-built card that faded after seven seconds. It is gooey's own alert
//! toast now: it stands until it is dismissed, it carries the refusal's words once, and when the
//! refusal names a library piece or a placement it has a Show button that selects that thing.

use crate::fixtures;

use bevy::prelude::*;
use bevy_cauldron::chrome::{ProblemBanner, ToastLayer};
use bevy_cauldron::editor::{EditorState, TargetLock};
use bevy_cauldron::harness;
use bevy_gooey::{Toast, ToastAction, ToastKind};
use fixtures::Fixture;

fn open(name: &str) -> App {
    let root = Fixture::new(name)
        .descriptor("alpha/floor", "alpha")
        .descriptor("alpha/bench", "alpha")
        .place("alpha/floor", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }
    app
}

/// One toast: its entity, words, kind, whether it is sticky, and its buttons as (label, id).
type Up = (Entity, String, ToastKind, bool, Vec<(String, String)>);

fn toasts(app: &mut App) -> Vec<Up> {
    let mut q = app.world_mut().query::<(Entity, &Toast)>();
    q.iter(app.world())
        .map(|(e, t)| (e, t.text().to_owned(), t.kind(), t.is_sticky(), t.actions().to_vec()))
        .collect()
}

fn layer(app: &mut App) -> Display {
    let mut q = app.world_mut().query_filtered::<&Node, With<ToastLayer>>();
    q.iter(app.world()).next().map(|n| n.display).expect("the toast strip is spawned")
}

/// **A refusal raises one alert toast that stands, with the refusal's own words.**
#[test]
fn a_refusal_stands_as_one_sticky_alert_toast_carrying_its_words() {
    let mut app = open("sticky_toast");
    assert!(toasts(&mut app).is_empty(), "a quiet editor raises nothing");
    assert_eq!(layer(&mut app), Display::None);

    app.world_mut().resource_mut::<EditorState>().status.problem("cannot do that: it is not allowed".to_owned());
    app.update();

    let up = toasts(&mut app);
    assert_eq!(up.len(), 1, "one refusal, one toast: {up:?}");
    let (_, text, kind, sticky, actions) = &up[0];
    assert_eq!(text, "cannot do that: it is not allowed");
    assert_eq!(*kind, ToastKind::Alert);
    assert!(*sticky, "a refusal stands until it is dismissed");
    assert!(actions.is_empty(), "a refusal that names nothing has nothing to show: {actions:?}");
    assert_eq!(layer(&mut app), Display::Flex, "the strip is shown in the frame the refusal is raised");

    // The one text a refusal is read from is the toast's own.
    for _ in 0..3 {
        app.update();
    }
    let mut banners = app.world_mut().query_filtered::<&Text, With<ProblemBanner>>();
    let said: Vec<String> = banners.iter(app.world()).map(|t| t.0.clone()).collect();
    assert_eq!(said, ["cannot do that: it is not allowed"], "exactly one text carries the refusal");

    // Time passes and it is still up: it is not a seven-second card.
    std::thread::sleep(std::time::Duration::from_millis(400));
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(toasts(&mut app).len(), 1);
    assert_eq!(layer(&mut app), Display::Flex);
}

/// **The next refusal replaces it, and taking the problems down takes it down.**
#[test]
fn the_next_refusal_replaces_the_toast_and_esc_takes_it_down() {
    let mut app = open("replace_toast");
    app.world_mut().resource_mut::<EditorState>().status.problem("first refusal".to_owned());
    app.update();
    app.world_mut().resource_mut::<EditorState>().status.problem("second refusal".to_owned());
    app.update();
    let up = toasts(&mut app);
    assert_eq!(up.len(), 1, "{up:?}");
    assert_eq!(up[0].1, "second refusal");

    app.world_mut().resource_mut::<EditorState>().status.dismiss();
    app.update();
    assert!(toasts(&mut app).is_empty(), "no refusal is standing, so no toast is");
    assert_eq!(layer(&mut app), Display::None);
}

/// **A refusal about a placement has a Show that targets it.**
#[test]
fn show_targets_the_placement_a_refusal_names() {
    let mut app = open("show_placement");
    let id = app.world().resource::<bevy_cauldron::project::OpenMap>().map.placements[0].id.clone();
    app.world_mut().resource_mut::<EditorState>().status.problem_about(format!("`{id}` cannot be turned: it is locked in"), &id);
    app.update();

    let up = toasts(&mut app);
    assert_eq!(up.len(), 1, "{up:?}");
    assert_eq!(up[0].4, [("Show".to_owned(), "show".to_owned())]);
    assert_eq!(app.world().resource::<TargetLock>().placement_id(), None, "nothing is targeted yet");

    app.world_mut().trigger(ToastAction { toast: up[0].0, id: "show".to_owned() });
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<TargetLock>().placement_id(), Some(id.as_str()), "Show targets what the refusal named");
}

/// **A refusal about a library piece has a Show that selects it on the shelf.**
#[test]
fn show_selects_the_library_piece_a_refusal_names() {
    let mut app = open("show_piece");
    app.world_mut().resource_mut::<EditorState>().status.problem_about("cannot remove: `alpha/bench` still places it".to_owned(), "alpha/bench");
    app.update();
    let up = toasts(&mut app);
    assert_eq!(up.len(), 1, "{up:?}");

    app.world_mut().trigger(ToastAction { toast: up[0].0, id: "show".to_owned() });
    for _ in 0..3 {
        app.update();
    }
    let target = app.world().resource::<bevy_cauldron::tiles::ImportState>().target();
    assert!(
        matches!(&target, Some(bevy_cauldron::tiles::EditTarget::Library(id)) if id == "alpha/bench"),
        "the shelf is on the piece the refusal named: {target:?}"
    );
}

/// **A refusal about something already gone offers no Show, and one that goes after the toast is up says so.**
#[test]
fn show_is_offered_only_for_what_is_there_and_says_so_when_it_goes() {
    let mut app = open("show_gone");
    app.world_mut().resource_mut::<EditorState>().status.problem_about("`ghost` cannot be turned".to_owned(), "ghost");
    app.update();
    let up = toasts(&mut app);
    assert_eq!(up.len(), 1, "{up:?}");
    assert!(up[0].4.is_empty(), "nothing named `ghost` exists, so there is nowhere to show: {:?}", up[0].4);

    // A piece that is there when the toast is raised and gone when Show is pressed.
    app.world_mut().resource_mut::<EditorState>().status.problem_about("cannot remove: `alpha/bench` still places it".to_owned(), "alpha/bench");
    app.update();
    let up = toasts(&mut app);
    assert_eq!(up.len(), 1, "{up:?}");
    assert_eq!(up[0].4, [("Show".to_owned(), "show".to_owned())]);
    app.world_mut().resource_mut::<bevy_cauldron::project::Project>().library.descriptors.retain(|d| d.id != "alpha/bench");
    app.world_mut().trigger(ToastAction { toast: up[0].0, id: "show".to_owned() });
    for _ in 0..3 {
        app.update();
    }
    let note = app.world().resource::<EditorState>().status.note_text().to_owned();
    assert!(note.contains("`alpha/bench` is not on this map or in the library any more"), "{note}");
    assert_eq!(app.world().resource::<TargetLock>().placement_id(), None);
}

/// **Changing between the Meshes and Tiles tabs does not bring a dismissed toast back**: both read one list of refusals.
#[test]
fn a_dismissed_toast_does_not_return_when_the_tab_changes_to_one_sharing_its_list() {
    let root = Fixture::new("toast_tabs").descriptor("alpha/floor", "alpha").build("m");
    let mut app = harness::build_headless_at(&root, "m", None, bevy_cauldron::tiles::Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }
    app.world_mut().resource_mut::<bevy_cauldron::tiles::ImportState>().status.problem("cannot send that back".to_owned());
    app.update();
    let up = toasts(&mut app);
    assert_eq!(up.len(), 1, "{up:?}");

    // The person presses Dismiss.
    app.world_mut().entity_mut(up[0].0).despawn();
    app.update();
    assert!(toasts(&mut app).is_empty());

    // Another tab with its own list does raise what it holds, and the shared one does not.
    *app.world_mut().resource_mut::<bevy_cauldron::tiles::Mode>() = bevy_cauldron::tiles::Mode::Tiles;
    for _ in 0..3 {
        app.update();
    }
    assert!(toasts(&mut app).is_empty(), "the Tiles tab reads the list the Meshes tab just showed");
}
