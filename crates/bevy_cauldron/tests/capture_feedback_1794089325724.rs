//! **The home has a discrete `Check for updates` control, so an update published after the app opened can be found**
//! (feedback-1794089325724; oracle `spec`).
//!
//! The note, taken on the launcher beside the version line `v0.4.1`: *"May we also have a discrete button stating to refresh
//! whether there is an update or not "Check for Update" or something, in case an update was pushed after opening the app."*
//! The launch check asks the releases feed once; a release published afterwards was never found while the app stayed open,
//! because the home showed a button only once there was something to take. The press already knew how to look again
//! (`update::on_activate` begins a check from `UpToDate`); nothing on the home offered it.
//!
//! What it holds: an up-to-date home offers `Check for updates` and still no `Update to ...` button; pressing it asks the feed
//! again, and an update published in between is then offered; a check that finds nothing says which version is newest; and an
//! app that cannot update where it is says why instead of looking.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::launcher::{Launcher, LauncherPlugin};
use bevy_cauldron::screen::Screen;
use bevy_cauldron::update::{self, UpdateButton, UpdateState, Updater};
use bevy_gooey::Activate;
use bevy_gooey::testing::find_by_label;

use crate::fixtures;

/// The words of the control.
const CHECK: &str = "Check for updates";

/// **A releases feed whose answer the test changes**: the feed as the app opened it, then the feed once a release is out.
fn serve(body: Arc<Mutex<String>>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| panic!("bind: {e}"));
    let addr = listener.local_addr().unwrap_or_else(|e| panic!("{e}"));
    std::thread::spawn(move || {
        for sock in listener.incoming() {
            let Ok(mut sock) = sock else { break };
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") && sock.read(&mut byte).is_ok_and(|n| n == 1) {
                head.push(byte[0]);
            }
            let text = body.lock().map(|b| b.clone()).unwrap_or_default();
            let _ = write!(sock, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
        }
    });
    format!("http://{addr}")
}

/// The feed once Cauldron 9.9.9 is published: a tag, the zip it carries and its sums.
fn feed_with_a_release() -> String {
    let version = semver::Version::new(9, 9, 9);
    serde_json::json!([{
        "tag_name": format!("{}{version}", update::TAG_PREFIX),
        "draft": false,
        "prerelease": false,
        "assets": [
            {"name": update::zip_name(&version), "browser_download_url": "https://example.test/zip"},
            {"name": update::SUMS, "browser_download_url": "https://example.test/sums"},
        ],
    }])
    .to_string()
}

/// The home, with an updater whose launch check found nothing newer.
fn the_home(feed: String) -> App {
    let root = fixtures::Fixture::new(&format!("check_update_{}", std::process::id())).descriptor("wall", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LauncherPlugin);
    let mut updater = Updater::new(feed, "/nonexistent/Cauldron.app".into());
    updater.state = UpdateState::UpToDate;
    app.insert_resource(updater);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    updates(&mut app, 8);
    app
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn update_buttons(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query_filtered::<(), With<UpdateButton>>().iter(world).count()
}

/// Press the control the way a person does: it receives an `Activate`.
fn press(app: &mut App, label: &str) {
    let button = find_by_label(app, label).unwrap_or_else(|e| panic!("`{label}` is not on the home: {e:?}"));
    app.world_mut().commands().trigger(Activate { entity: button });
    updates(app, 2);
}

/// Wait for the check the press began to answer.
fn until_answered(app: &mut App) {
    for _ in 0..200 {
        updates(app, 2);
        if app.world().resource::<Updater>().state != UpdateState::Checking {
            updates(app, 4);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("the check never answered");
}

#[test]
fn an_up_to_date_home_offers_a_discrete_check_and_no_update_button() {
    let mut app = the_home("http://127.0.0.1:9/none".into());
    let version = env!("CARGO_PKG_VERSION");
    find_by_label(&mut app, &format!("v{version} · up to date")).unwrap_or_else(|e| panic!("the version line: {e:?}"));
    find_by_label(&mut app, CHECK).unwrap_or_else(|e| panic!("an up-to-date home offers a way to look again: {e:?}"));
    assert_eq!(update_buttons(&mut app), 0, "and there is still nothing to take, so no Update button");
}

#[test]
fn pressing_it_looks_again_and_offers_an_update_published_after_the_app_opened() {
    let body = Arc::new(Mutex::new("[]".to_owned()));
    let mut app = the_home(serve(body.clone()));
    let version = env!("CARGO_PKG_VERSION");

    // Nothing is newer yet: the press answers in words, so it does not look like a dropped click.
    press(&mut app, CHECK);
    until_answered(&mut app);
    assert_eq!(app.world().resource::<Updater>().state, UpdateState::UpToDate);
    let said = app.world().resource::<Launcher>().problem.clone().unwrap_or_default();
    assert!(said.contains(version) && said.contains("newest"), "a check a person asked for says which version is newest: `{said}`");
    assert_eq!(update_buttons(&mut app), 0);

    // A release is published after the app opened; the same control finds it.
    *body.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = feed_with_a_release();
    press(&mut app, CHECK);
    until_answered(&mut app);
    find_by_label(&mut app, "Update to v9.9.9").unwrap_or_else(|e| panic!("the home offers the release the second look found: {e:?}"));
    assert_eq!(update_buttons(&mut app), 1, "one Update button");
    find_by_label(&mut app, CHECK).unwrap_or_else(|e| panic!("and the control stays, to look for a newer one still: {e:?}"));
}

#[test]
fn an_app_that_cannot_update_where_it_is_says_why_when_asked_to_look() {
    let mut app = the_home("http://127.0.0.1:9/none".into());
    app.world_mut().resource_mut::<Updater>().blocked = Some(update::MOVE_IT_FIRST.to_owned());
    updates(&mut app, 2);
    press(&mut app, CHECK);
    assert_eq!(app.world().resource::<Updater>().state, UpdateState::UpToDate, "nothing was looked for");
    assert_eq!(app.world().resource::<Launcher>().problem.as_deref(), Some(update::MOVE_IT_FIRST), "the reason is said instead");
}
