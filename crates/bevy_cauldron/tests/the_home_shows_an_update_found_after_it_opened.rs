//! **The home shows an update the launch check finds after the home was drawn** (`launcher.rs`, `update.rs`).
//!
//! The launch check answers from a thread, half a second after the window opens, and the home's sidebar is
//! built in `OnEnter(Screen::Launcher)`, before it has answered. The sidebar used to be drawn once from the
//! updater as it stood then: `Idle`, which it printed as `v0.3.4 · up to date` with no button, and the
//! answer that 0.3.6 was out never reached it. Reported at the keyboard on 2026-11-06: "there's no Update
//! button" on a 0.3.4 home while 0.3.5 and 0.3.6 were published.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::launcher::LauncherPlugin;
use bevy_cauldron::screen::Screen;
use bevy_cauldron::update::{Offer, UpdateButton, UpdateState, Updater};

use crate::fixtures;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn buttons(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(), With<UpdateButton>>();
    q.iter(world).count()
}

#[test]
fn an_update_found_after_the_home_opened_is_offered_on_it() {
    let root = fixtures::Fixture::new("home-late-update").descriptor("wall", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.add_plugins(LauncherPlugin);
    // **The launch check in flight**: a feed nobody answers and no job to poll, so the state stays where the
    // test puts it. This is the updater as the home finds it at launch.
    let mut updater = Updater::new("http://127.0.0.1:9/none".into(), "/nonexistent/Cauldron.app".into());
    updater.state = UpdateState::Checking;
    app.insert_resource(updater);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    updates(&mut app, 8);

    let version = env!("CARGO_PKG_VERSION");
    assert!(
        bevy_gooey::testing::find_by_label(&mut app, &format!("v{version} · up to date")).is_err(),
        "a check that has not answered has not found the app up to date"
    );
    bevy_gooey::testing::find_by_label(&mut app, &format!("v{version}"))
        .unwrap_or_else(|e| panic!("the sidebar names the running version while the check runs: {e:?}"));

    // The check answers, as `update::poll` writes it.
    app.world_mut().resource_mut::<Updater>().state = UpdateState::Available(Offer {
        version: semver::Version::new(9, 9, 9),
        zip_url: "http://127.0.0.1:9/zip".into(),
        sums_url: "http://127.0.0.1:9/sums".into(),
    });
    updates(&mut app, 4);
    bevy_gooey::testing::find_by_label(&mut app, "Update to v9.9.9")
        .unwrap_or_else(|e| panic!("the home offers the update its launch check found: {e:?}"));
    assert_eq!(buttons(&mut app), 1, "one update button on the home");

    // An answer of nothing newer takes the button away again and says so on the version line.
    app.world_mut().resource_mut::<Updater>().state = UpdateState::UpToDate;
    updates(&mut app, 4);
    assert_eq!(buttons(&mut app), 0, "nothing to take leaves nothing to press");
    bevy_gooey::testing::find_by_label(&mut app, &format!("v{version} · up to date"))
        .unwrap_or_else(|e| panic!("the line says what the check found: {e:?}"));
}
