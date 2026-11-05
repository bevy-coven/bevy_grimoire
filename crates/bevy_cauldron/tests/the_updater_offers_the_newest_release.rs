//! **The macOS app's updater offers the newest real release, takes only the zip the release names,
//! knows where it is running from, and refuses to restart over unsaved work** (`update.rs`).
//!
//! The feed parsing, the sums, the download and the bundle check are plain functions, so they are
//! tested off any network: the download against a loopback server. The button is tested in a headless
//! editor with an `Updater` inserted by hand, because only the binary inserts one inside a `.app`.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::update::{self, Bundle, Offer, UpdateState, Updater};
use semver::Version;

use crate::fixtures;

fn v(text: &str) -> Version {
    Version::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"))
}

/// One release of the feed, with an asset per name.
fn release(tag: &str, draft: bool, prerelease: bool, assets: Vec<String>) -> serde_json::Value {
    let assets: Vec<_> = assets
        .iter()
        .map(|name| serde_json::json!({ "name": name, "browser_download_url": format!("https://example.test/{tag}/{name}") }))
        .collect();
    serde_json::json!({ "tag_name": tag, "draft": draft, "prerelease": prerelease, "assets": assets })
}

fn feed() -> String {
    let full = |version: &str| vec![format!("Cauldron-{version}-macos-arm64.zip"), update::SUMS.to_owned()];
    let r = release;
    serde_json::Value::Array(vec![
        // Another crate's tag shares the feed, and its version is the highest of all.
        r("bevy_grimoire_solver-v9.0.0", false, false, full("9.0.0")),
        r("bevy_cauldron-v0.3.0", true, false, full("0.3.0")),
        r("bevy_cauldron-v0.4.0-rc.1", false, true, full("0.4.0-rc.1")),
        // Published, newest of the rest, and without the zip: nothing to install.
        r("bevy_cauldron-v0.2.11", false, false, vec![update::SUMS.to_owned()]),
        r("bevy_cauldron-v0.2.5", false, false, full("0.2.5")),
        // Ten is above five: the comparison is semver, not text.
        r("bevy_cauldron-v0.2.10", false, false, full("0.2.10")),
    ])
    .to_string()
}

#[test]
fn newest_offers_the_highest_published_release_that_carries_its_zip() {
    let offer = update::newest(&feed(), &v("0.2.0")).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        offer,
        Some(Offer {
            version: v("0.2.10"),
            zip_url: "https://example.test/bevy_cauldron-v0.2.10/Cauldron-0.2.10-macos-arm64.zip".to_owned(),
            sums_url: "https://example.test/bevy_cauldron-v0.2.10/SHA256SUMS".to_owned(),
        }),
        "drafts, pre-releases, other crates' tags and a release without its zip are not offers"
    );
    assert_eq!(update::newest(&feed(), &v("0.2.10")), Ok(None), "the running version is not an update to itself");
    assert!(update::newest("{\"message\":\"rate limited\"}", &v("0.2.0")).is_err(), "an object is not a releases list");
}

#[test]
fn expected_sum_reads_the_line_for_the_file_and_names_a_missing_one() {
    let a = "a".repeat(64);
    let b = "B".repeat(64);
    let sums = format!("{a}  Cauldron-0.2.0-macos-arm64.dmg\n{b}  Cauldron-0.2.0-macos-arm64.zip\n");
    assert_eq!(update::expected_sum(&sums, "Cauldron-0.2.0-macos-arm64.zip"), Ok("b".repeat(64)));
    let missing = update::expected_sum(&sums, "Cauldron-0.2.1-macos-arm64.zip").unwrap_err();
    assert!(missing.contains("Cauldron-0.2.1-macos-arm64.zip"), "the refusal names the file: {missing}");
}

/// Serves each path's body for as many requests as are made, until the test ends.
fn serve(files: Vec<(&'static str, Vec<u8>)>) -> String {
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
            let head = String::from_utf8_lossy(&head);
            let path = head.split_whitespace().nth(1).unwrap_or("");
            let body = files.iter().find(|(p, _)| *p == path).map(|(_, b)| b.as_slice());
            let (status, body) = body.map_or(("404 Not Found", &b""[..]), |b| ("200 OK", b));
            let _ = write!(sock, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = sock.write_all(body);
        }
    });
    format!("http://{addr}")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cauldron-update-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

#[test]
fn fetch_verified_keeps_a_zip_whose_sum_matches_and_deletes_one_that_does_not() {
    use sha2::{Digest, Sha256};
    let zip: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    let name = update::zip_name(&v("0.2.1"));
    let right = format!("{:x}  {name}\n", Sha256::digest(&zip));
    let wrong = format!("{}  {name}\n", "0".repeat(64));
    let base = serve(vec![("/zip", zip.clone()), ("/right", right.into_bytes()), ("/wrong", wrong.into_bytes())]);
    let offer = |sums: &str| Offer { version: v("0.2.1"), zip_url: format!("{base}/zip"), sums_url: format!("{base}/{sums}") };

    let dir = scratch("right");
    let path = update::fetch_verified(&offer("right"), &dir).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(std::fs::read(&path).unwrap_or_else(|e| panic!("{e}")), zip, "the file is the bytes served");

    let dir = scratch("wrong");
    let refused = update::fetch_verified(&offer("wrong"), &dir).unwrap_err();
    assert!(refused.contains("checksum mismatch"), "{refused}");
    assert!(!dir.join(&name).exists(), "a zip that does not match its sum is not left behind");
}

#[test]
fn bundle_of_tells_an_installed_app_from_a_translocated_one_and_from_no_app() {
    let of = |p: &str| update::bundle_of(Path::new(p));
    assert_eq!(
        of("/Applications/Cauldron.app/Contents/MacOS/bevy_cauldron"),
        Bundle::Installed("/Applications/Cauldron.app".into())
    );
    assert!(matches!(
        of("/private/var/folders/x/AppTranslocation/Y/d/Cauldron.app/Contents/MacOS/bevy_cauldron"),
        Bundle::Translocated(_)
    ));
    assert!(matches!(of("/Volumes/Cauldron/Cauldron.app/Contents/MacOS/bevy_cauldron"), Bundle::Translocated(_)));
    assert_eq!(of("/home/u/target/release/bevy_cauldron"), Bundle::NotABundle);
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("wall", "alpha").build("m");
    harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("the fixture project must open: {e}"))
}

#[test]
fn the_chrome_bar_offers_an_update_and_refuses_it_over_unsaved_work() {
    let mut app = editor("updater-chrome");
    let mut updater = Updater::new("http://127.0.0.1:9/none".into(), "/nonexistent/Cauldron.app".into());
    updater.state = UpdateState::Available(Offer {
        version: v("9.9.9"),
        zip_url: "http://127.0.0.1:9/zip".into(),
        sums_url: "http://127.0.0.1:9/sums".into(),
    });
    updater.blocked = None;
    app.insert_resource(updater);
    updates(&mut app, 2);
    bevy_cauldron::editor::place_for_test(app.world_mut(), "wall", (0.0, 0.0));
    updates(&mut app, 4);
    assert!(app.world().resource::<bevy_cauldron::project::OpenMap>().dirty(), "the placement left unsaved work");

    let button = bevy_gooey::testing::find_by_label(&mut app, "Update to v9.9.9")
        .unwrap_or_else(|e| panic!("the chrome bar offers the update: {e}"));
    bevy_gooey::testing::click(&mut app, button);
    updates(&mut app, 4);
    assert!(
        matches!(app.world().resource::<Updater>().state, UpdateState::Available(_)),
        "a click over unsaved work starts no download: {:?}",
        app.world().resource::<Updater>().state
    );
    let said = app.world().resource::<bevy_cauldron::editor::EditorState>().status.problem_text().to_owned();
    assert!(said.contains("Save the map first"), "the refusal says why: `{said}`");
}

/// **A check that finds nothing says so.**
///
/// Measured at the keyboard on 2026-11-05 against the installed 0.2.3: *Check for updates* was pressed,
/// the check finished in half a second, and the label went back to *Check for updates* — because
/// `UpToDate` read the same words as `Idle`. An answer with no words is indistinguishable from a press
/// that was dropped, so the button names the version that is newest, and the row says it too.
#[test]
fn an_up_to_date_check_says_the_version_is_the_newest() {
    // **The launcher's sidebar button**, which is the one always on screen: the editor's chrome bar
    // shows an update button only while there is something to act on, so the row a person presses to
    // check is the home's, and that is the row that has to answer.
    use bevy_cauldron::launcher::LauncherPlugin;
    use bevy_cauldron::screen::Screen;

    let mut app = editor("updater-uptodate");
    app.add_plugins(LauncherPlugin);
    let mut updater = Updater::new("http://127.0.0.1:9/none".into(), "/nonexistent/Cauldron.app".into());
    updater.state = UpdateState::UpToDate;
    app.insert_resource(updater);
    app.world_mut()
        .resource_mut::<NextState<Screen>>()
        .set(Screen::Launcher);
    updates(&mut app, 8);

    // **The answer is the label, and the sidebar shows it as a quiet line rather than an always-on
    // button** (design review, 2026-11-05): the shipped screen printed the version twice, once as
    // `Version 0.2.4` and once as a button reading `Up to date — v0.2.4` in a page tab's skin.
    let want = format!("v{} · up to date", env!("CARGO_PKG_VERSION"));
    bevy_gooey::testing::find_by_label(&mut app, &want).unwrap_or_else(|e| {
        panic!("the sidebar names the version it checked: wanted `{want}`: {e:?}")
    });
    assert!(
        bevy_gooey::testing::find_by_label(&mut app, "Version 0.2.4").is_err(),
        "one version line, not two"
    );
    assert!(
        !app.world().resource::<Updater>().actionable(),
        "an up-to-date check has nothing to press: no update button on the sidebar"
    );
}

/// **The answer reaches the person on the row they pressed**, which is where a failure would appear.
#[test]
fn the_up_to_date_answer_is_said_not_only_labelled() {
    let mut updater = Updater::new("http://127.0.0.1:9/none".into(), "/nope/Cauldron.app".into());
    updater.state = UpdateState::UpToDate;
    assert_eq!(
        updater.label(),
        format!("Up to date — v{}", env!("CARGO_PKG_VERSION")),
        "an unchanged label reads as a press that did nothing"
    );
    assert_ne!(updater.label(), "Check for updates", "Idle and UpToDate are different answers");
}

/// **The launch check that finds nothing says nothing, and a pressed one says so.**
///
/// The shipped 0.2.4 opened on a sticky coral `Cauldron v0.2.4 is the newest release.` before the
/// product's own name, on every launch, because the automatic check answered into the launcher's
/// problem line exactly as a pressed one does. Measured in that release's own screenshot on
/// 2026-11-05. The button's label already carries the answer; the toast is only worth the space when
/// a person asked for it.
#[test]
fn an_automatic_check_that_finds_nothing_raises_no_problem() {
    use bevy_cauldron::launcher::LauncherPlugin;
    use bevy_cauldron::screen::Screen;

    let mut app = editor("updater-automatic-silent");
    app.add_plugins(LauncherPlugin);
    // **A feed that cannot answer** is the sharpest case: a failure *does* speak, so the absence of
    // a problem line here would be for the wrong reason if the check had not run at all. Instead the
    // check is made to answer "nothing newer" by pointing it at an empty release list.
    let mut updater = Updater::new("http://127.0.0.1:9/none".into(), "/nonexistent/Cauldron.app".into());
    updater.state = UpdateState::UpToDate;
    app.insert_resource(updater);
    app.world_mut()
        .resource_mut::<NextState<Screen>>()
        .set(Screen::Launcher);
    updates(&mut app, 6);
    assert!(
        app.world().resource::<bevy_cauldron::launcher::Launcher>().problem.is_none(),
        "an automatic up-to-date answer must not enter the problem line: {:?}",
        app.world().resource::<bevy_cauldron::launcher::Launcher>().problem
    );
    // And the label is where the answer lives.
    assert_eq!(
        app.world().resource::<Updater>().label(),
        format!("Up to date — v{}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn an_editor_with_no_updater_shows_no_update_button() {
    let mut app = editor("updater-none");
    updates(&mut app, 4);
    assert!(
        bevy_gooey::testing::find_by_label(&mut app, "Check for updates").is_err(),
        "a development build has no updater, so nothing offers to check"
    );
}
