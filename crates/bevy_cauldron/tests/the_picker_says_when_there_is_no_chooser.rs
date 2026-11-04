//! **A system with no file chooser says so, in a toast** (#298).
//!
//! Open folder…, Locate…, a new project's Choose… and Add meshes… all ask `picker::PickRequest`. With no
//! chooser service the answer is an error toast with the way round it (a terminal, the folder as the argument), not a
//! typed-path field and not silence. The toast names no program.

use crate::fixtures;

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::picker::{NO_CHOOSER, PickFor, PickRequest, Picked, PickerOff, PickerPlugin};
use bevy_gooey::{Toast, ToastKind};
use fixtures::Fixture;

fn a_run_with_no_chooser(name: &str) -> App {
    let root = Fixture::new(name).descriptor("alpha/floor", "alpha").place("alpha/floor", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(PickerPlugin).insert_resource(PickerOff);
    for _ in 0..3 {
        app.update();
    }
    app
}

fn toasts(app: &mut App) -> Vec<(String, ToastKind, bool)> {
    let mut q = app.world_mut().query::<&Toast>();
    q.iter(app.world()).map(|t| (t.text().to_owned(), t.kind(), t.is_sticky())).collect()
}

#[test]
fn every_kind_of_request_with_no_chooser_raises_the_error_toast_and_answers_nothing() {
    for for_what in [PickFor::OpenFolder, PickFor::Locate("/gone".into()), PickFor::Location, PickFor::AddMeshes] {
        let mut app = a_run_with_no_chooser(&format!("picker_off_{}", std::process::id()));
        app.world_mut().write_message(PickRequest(for_what.clone()));
        for _ in 0..3 {
            app.update();
        }
        let up = toasts(&mut app);
        assert_eq!(up, [(NO_CHOOSER.to_owned(), ToastKind::Alert, true)], "{for_what:?}: one sticky alert with the sentence");
        let answers = app.world().resource::<Messages<Picked>>();
        assert!(answers.is_empty(), "{for_what:?}: nothing was picked");
    }
}

#[test]
fn the_toast_names_no_program_and_says_what_to_do() {
    assert!(!NO_CHOOSER.contains("bevy_cauldron"), "{NO_CHOOSER}");
    assert!(NO_CHOOSER.contains("xdg-desktop-portal") && NO_CHOOSER.contains("terminal"), "{NO_CHOOSER}");
}
