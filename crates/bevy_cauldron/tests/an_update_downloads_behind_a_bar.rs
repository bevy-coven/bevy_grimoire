//! **A download shows a bar, and the button comes back when it is done** (`feedback-1793903311065`).
//!
//! The note: *"Can we make this a download bar whenever you hit \"Update,\" then when the update is
//! finished, switch back to a button."* An enhancement, and the two halves of it are two assertions:
//!
//! - **while** an update is coming down there is a bar, and it shows how far along it is;
//! - **after** it lands there is none, and the button is what is drawn.
//!
//! The bytes are the state's own: a real download is macOS-only and cannot run in this process, so the
//! download is stood up by writing the state the download writes — which is exactly what the bar reads.
//! The half that talks to the network is `update::stream`, and its contract is the one thing proven
//! without a server: it reports what it wrote, and the response's length, through the sink the bar's
//! channel is fed from.

use bevy::prelude::*;
use bevy_cauldron::update::{Offer, UpdateBar, UpdateButton, UpdatePlugin, UpdateState, Updater, UPDATE_BAR_H};
use semver::Version;

/// An [`Updater`] on a feed nothing will answer, standing on a machine that cannot update: the state is
/// what this test writes, and no job is ever started.
fn an_updater() -> Updater {
    Updater::new("https://example.invalid/releases.json".to_owned(), std::path::PathBuf::from("/tmp/none.app"))
}

fn an_offer() -> Offer {
    Offer {
        version: Version::new(0, 3, 0),
        zip_url: "https://example.invalid/cauldron.zip".to_owned(),
        sums_url: "https://example.invalid/sums.txt".to_owned(),
    }
}

/// **The bar is what the download state draws**, and its meter is the bytes that arrived.
fn the_bar(app: &mut App) -> Option<(f32, f32)> {
    let world = app.world_mut();
    let mut q = world.query_filtered::<&bevy_gooey::Meter, With<UpdateBar>>();
    q.iter(world).next().map(|meter| (meter.value, meter.max))
}

/// The button's own node, so a test can say it is drawn.
fn the_button(app: &mut App) -> Option<Entity> {
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<UpdateButton>>();
    q.iter(world).next()
}

/// **A home with an updater on it**, which is where the note's person was when they pressed Update.
fn the_home_in_a_download(updater: Updater) -> App {
    let root = crate::fixtures::Fixture::new(&format!("update_bar_{}", std::process::id()))
        .descriptor("bench", "props")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_cauldron::launcher::LauncherPlugin);
    // **The updater's systems are the harness's**, asserted rather than added: adding them twice panics,
    // and the point of this test is the bar those systems draw, not the app's plugin list.
    assert!(
        app.is_plugin_added::<UpdatePlugin>(),
        "the harness installs the updater, so this test measures its bar and not its plumbing"
    );
    app.insert_resource(updater);
    app.world_mut().resource_mut::<NextState<bevy_cauldron::screen::Screen>>().set(bevy_cauldron::screen::Screen::Launcher);
    for _ in 0..6 {
        app.update();
    }
    app
}

#[test]
fn a_download_draws_a_bar_that_fills_to_its_length() {
    let mut updater = an_updater();
    updater.state = UpdateState::Downloading { version: Version::new(0, 3, 0), received: 3_000, total: Some(12_000) };
    let mut app = the_home_in_a_download(updater);

    let Some((value, max)) = the_bar(&mut app) else {
        panic!(
            "an update is downloading and nothing draws a bar: the note asks for \"a download bar \
             whenever you hit Update\" ({UPDATE_BAR_H} px tall, beside the button)"
        );
    };
    assert_eq!((value, max), (3_000.0, 12_000.0), "the bar shows the bytes that have arrived of the ones the server named");
}

#[test]
fn the_bar_gives_way_to_the_button_when_the_download_is_done() {
    // The second half of the note, and the half a bar that is never taken down would fail.
    let mut updater = an_updater();
    updater.state = UpdateState::Downloading { version: Version::new(0, 3, 0), received: 6_000, total: Some(12_000) };
    let mut app = the_home_in_a_download(updater);
    assert!(the_bar(&mut app).is_some(), "there is a bar while it downloads");

    // The download lands: the state becomes the offer that was staged, which is what `poll` writes.
    app.world_mut().resource_mut::<Updater>().state = UpdateState::Available(an_offer());
    for _ in 0..3 {
        app.update();
    }
    assert!(the_bar(&mut app).is_none(), "the download is over, so its bar is gone rather than left full");
    assert!(the_button(&mut app).is_some(), "and the button is what is drawn again");
}

#[test]
fn a_download_that_states_no_length_draws_an_empty_bar_rather_than_a_wrong_one() {
    // A server that sends no `Content-Length` cannot be turned into a percentage, and a full bar over
    // an unfinished download is the one thing worse than no bar.
    let mut updater = an_updater();
    updater.state = UpdateState::Downloading { version: Version::new(0, 3, 0), received: 4_000, total: None };
    let mut app = the_home_in_a_download(updater);
    let (value, max) = the_bar(&mut app).expect("a bar is still drawn, so the person sees it working");
    assert_eq!(max, 0.0, "with no stated length the bar has nothing to fill against, so it is empty");
    assert_eq!(value, 0.0);
}
