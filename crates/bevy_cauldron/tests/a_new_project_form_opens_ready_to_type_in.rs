//! **The New project form is a dialog and its Name field has the keyboard when it opens** (#297).
//!
//! Measured on the first-run walk, 2026-11-04: the form was a plain panel pinned to the top-left of the home, and typing went
//! nowhere until the field was clicked. It is a gooey `dialog` now, as the project page's New map is: centred, modal, and the
//! first field holds the focus.

use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_cauldron::launcher::{Launcher, LauncherPlugin};
use bevy_cauldron::new_project::{FormCard, NameField, NewProjectForm};
use bevy_cauldron::screen::Screen;
use bevy_gooey::Dialog;

use crate::fixtures::Fixture;

fn on_the_home(tag: &str) -> App {
    let root = Fixture::new(&format!("form_focus_{tag}_{}", std::process::id())).descriptor("bench", "props").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..4 {
        app.update();
    }
    app
}

fn open_the_form(app: &mut App) {
    let button = bevy_gooey::testing::find_by_label(app, "New project").unwrap_or_else(|e| panic!("{e:?}"));
    app.world_mut().commands().trigger(Activate { entity: button });
    for _ in 0..4 {
        app.update();
    }
}

#[test]
fn the_name_field_has_the_keyboard_as_the_form_opens() {
    let mut app = on_the_home("focus");
    open_the_form(&mut app);
    let name = app.world_mut().query_filtered::<Entity, With<NameField>>().single(app.world()).unwrap_or_else(|e| panic!("{e}"));
    // The focus is the field's text box, which is inside the `NameField` the form marks.
    let focused = app.world().resource::<InputFocus>().get().expect("something has the keyboard");
    let inside = std::iter::successors(Some(focused), |e| app.world().get::<ChildOf>(*e).map(ChildOf::parent)).any(|e| e == name);
    assert!(inside, "the Name field has the keyboard, so typing goes into it; focused: {:?}", bevy_gooey::testing::focused_label(&app));
}

#[test]
fn the_form_is_a_modal_dialog_and_escape_closes_it() {
    let mut app = on_the_home("dialog");
    open_the_form(&mut app);
    let card = app.world_mut().query_filtered::<Entity, (With<FormCard>, With<Dialog>)>().single(app.world()).unwrap_or_else(|e| panic!("the form is a dialog: {e}"));
    app.world_mut().commands().trigger(bevy_gooey::Cancel { entity: card });
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().get_resource::<NewProjectForm>().is_none(), "Esc is Cancel: the form is gone");
    assert!(!app.world().resource::<Launcher>().new_project_asked, "and it is not asked for again");
    assert_eq!(app.world_mut().query::<&FormCard>().iter(app.world()).count(), 0, "and so is its card");
}
