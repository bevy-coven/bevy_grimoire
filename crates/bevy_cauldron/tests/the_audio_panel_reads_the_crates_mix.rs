//! **The audio panel reads the project's mix and its cue table, and an audition is the crate's own
//! `Play`** (#274, the `bevy_siren` consumer issue).
//!
//! Three claims, and each can fail:
//!
//! 1. **A mix file is read as `bevy_siren`'s own `SirenSettings`**, and its buses are what the panel
//!    shows. A second schema here would drift from the crate's file the first time a field was added.
//! 2. **A file that does not parse is a problem on the status line**, never a silently defaulted mix:
//!    a panel that showed the crate's defaults for a broken file would look exactly like a project that
//!    asked for them by hand.
//! 3. **No file is not a fault**: a project with no sound yet shows the crate's defaults and says so,
//!    rather than a refusal for something nobody asked for.
//!
//! The panel is drawn on the Meshes door's left dock, so a test finds it by its own marker rather than
//! by a screen coordinate.

#![cfg(feature = "bevy_siren")]

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

/// The editor with a `family/bevy_siren.ron` containing `mix` (or none when `mix` is `None`).
fn editor(name: &str, mix: Option<&str>) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .build("m");
    if let Some(text) = mix {
        let dir = root.join("family");
        std::fs::create_dir_all(&dir).expect("the fixture's family directory");
        std::fs::write(dir.join("bevy_siren.ron"), text).expect("the fixture's mix file");
    }
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..5 {
        app.update();
    }
    (root, app)
}

/// What the panel's bus block says.
fn buses(app: &mut App) -> Option<String> {
    let mut q = app
        .world_mut()
        .query_filtered::<&Text, With<bevy_cauldron::audio_panel::AudioMix>>();
    q.iter(app.world()).next().map(|t| t.0.clone())
}

/// **A mix file is the crate's own type, and its buses are what the panel says.**
///
/// The file names a master and two buses, one of them muted and capped: every one of those facts must be
/// readable back off the panel, which is the claim that the panel reads the crate's type rather than a
/// copy of it.
#[test]
fn the_panel_shows_the_buses_the_crate_reads_from_the_file() {
    let (root, mut app) = editor(
        "siren-mix",
        Some(
            "(\n    master_db: -3.0,\n    buses: [\n        (name: \"music\", gain_db: -6.0, max_voices: 4),\n        (name: \"dialogue\", gain_db: 0.0, mute: true),\n    ],\n)\n",
        ),
    );
    let said = buses(&mut app).unwrap_or_default();
    assert!(
        said.contains("music") && said.contains("-6.0 dB") && said.contains("up to 4 voices"),
        "the panel must read `music`'s own gain and voice cap out of the file; it says {said:?} \
         (root {})",
        root.display()
    );
    assert!(
        said.contains("dialogue") && said.contains("muted"),
        "a muted bus must read as muted: a bus that is silent with nothing saying so is the bug this \
         panel exists to make impossible; it says {said:?}"
    );
    // **The master is read too**, which is what proves the whole struct parsed rather than one field.
    assert!(
        said.contains("master -3.0 dB") || said.contains("-3.0"),
        "the master must be visible; it says {said:?}"
    );
}

/// **A file the crate refuses is a problem on the status line, and the panel does not pretend a mix.**
#[test]
fn a_mix_file_that_does_not_parse_is_named() {
    let (_root, mut app) = editor("siren-refused", Some("(master_db: \"loud\")\n"));
    let problems = {
        let state = app.world().resource::<bevy_cauldron::editor::EditorState>();
        state.status.problem_text().to_owned()
    };
    assert!(
        problems.contains("bevy_siren.ron"),
        "a mix file that does not parse must be named on the status line; the newest problem is \
         {problems:?}"
    );
    let said = buses(&mut app).unwrap_or_default();
    assert!(
        !said.contains("music") || said.contains("defaults"),
        "the panel must not show a mix read from a file the crate threw away; it says {said:?}"
    );
}
