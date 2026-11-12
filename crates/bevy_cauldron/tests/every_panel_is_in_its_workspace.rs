//! **Every part in its workspace** (NV-08, #385; series getting around, #377).
//!
//! Until NV-08 the Map's INSPECTOR held the terrain line, the environment, the light pool and the audio
//! table one scroll below the next, and `docs/ui/navigation-baseline.md` measured five to sixty wheel
//! notches to reach a heading. A workspace is the *job* (`docs/ui/vision.md` §4), so each part now lives
//! in the workspace that does its job, and **nothing is reached a second way**: the old route is gone with
//! the move, not kept beside the new one.
//!
//! Two claims, both measured on the real editor:
//!
//! - **found by label in its workspace**: the editor opens on Map, switches by the workspace's command
//!   (the route the chord, the palette, the activity bar and an agent all take), and the panel's own
//!   heading is on screen;
//! - **not found by the old route**: on the Map the heading is not there, however far a person looks.
//!
//! Each row names the heading a person reads. A heading that comes from a family crate is asserted only
//! in a build that has the crate, because a build without it draws no such panel (and says so).

use std::path::PathBuf;

use bevy::prelude::*;

use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::workspace::Workspace;
use bevy::ui::ComputedNode;

use crate::fixtures;

/// **Frames the editor is given to settle** after a switch: the transition runs in the frame after the
/// key, and the panels are spawned on the frame after that.
const SETTLE_FRAMES: usize = 12;

fn project(tag: &str) -> PathBuf {
    fixtures::Fixture::new(&format!("panel_home_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m")
}

fn editor(tag: &str) -> App {
    let mut app = harness::build_headless(&project(tag), "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// **Switch the way the palette does**: the workspace's command id, its chord queued and pressed.
fn switch_to(app: &mut App, workspace: Workspace) {
    let id = format!("workspace.{}", workspace.id());
    let command = commands::by_id(&id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(app);
    assert_eq!(app.world().resource::<Workspace>(), &workspace, "the `{id}` command did not open the {} workspace", workspace.title());
}

/// **Is a heading with exactly these words on screen?** A `Text` whose words are `label`, drawn: visible and laid
/// out with a box. (A panel another workspace hides is `Display::None`, which lays out to nothing.)
fn shows(app: &mut App, label: &str) -> bool {
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &InheritedVisibility, &ComputedNode)>();
    q.iter(world).any(|(text, shown, node)| shown.get() && text.0.trim() == label && node.size().x > 0.0 && node.size().y > 0.0)
}

/// **The headings each workspace's panels draw**, which are what "finds the panel by label" means.
///
/// `(workspace, headings that are there in every build)`. The family crates' own headings are in
/// [`family_headings`].
fn headings() -> Vec<(Workspace, Vec<&'static str>)> {
    vec![
        (Workspace::Map, vec!["INSPECTOR", "PLACE", "DATA", "NOTES", "PROBLEMS"]),
        (Workspace::World, vec!["TERRAIN"]),
        (Workspace::Kit, vec!["MESHES", "TILES", "COMPOSE"]),
    ]
}

/// The headings a family crate's panel draws, in a build that has the crate.
fn family_headings() -> Vec<(Workspace, &'static str)> {
    let mut found = Vec::new();
    if cfg!(feature = "bevy_elements") {
        found.push((Workspace::World, "ENVIRONMENT"));
    }
    if cfg!(feature = "bevy_lights") {
        found.push((Workspace::World, "LIGHTS"));
    }
    if cfg!(feature = "bevy_siren") {
        found.push((Workspace::Sound, "AUDIO"));
    }
    found
}

/// **Every panel is found by its label once its workspace is open.**
#[test]
fn every_panel_is_found_by_label_in_its_workspace() {
    let mut app = editor("found");
    let mut checked = 0;
    for (workspace, labels) in headings() {
        if workspace != Workspace::Map {
            switch_to(&mut app, workspace);
        }
        for label in labels {
            assert!(
                shows(&mut app, label),
                "the {} workspace does not show `{label}`; the panels are {:?}",
                workspace.title(),
                workspace.panels().iter().map(|m| m.label()).collect::<Vec<_>>()
            );
            checked += 1;
        }
        // Back to Map for the next row, so each row starts from where a person starts.
        if workspace != Workspace::Map {
            switch_to(&mut app, Workspace::Map);
        }
    }
    for (workspace, label) in family_headings() {
        switch_to(&mut app, workspace);
        assert!(shows(&mut app, label), "the {} workspace does not show `{label}`", workspace.title());
        checked += 1;
        switch_to(&mut app, Workspace::Map);
    }
    // **A floor, so a table that stopped being read cannot pass.** Map's five, World's one and Kit's three.
    assert!(checked >= 9, "only {checked} headings were checked");
}

/// **The Sound workspace is registered, and says what it holds.** With `bevy_siren` it is the audio
/// table; without it, the sentence that says this build has none — never an empty column.
#[test]
fn the_sound_workspace_shows_the_audio_panel_or_says_why_it_cannot() {
    let mut app = editor("sound");
    assert!(Workspace::Sound.is_built(), "the Sound workspace is registered in every build");
    switch_to(&mut app, Workspace::Sound);
    if cfg!(feature = "bevy_siren") {
        assert!(shows(&mut app, "AUDIO"), "the Sound workspace does not show the AUDIO panel");
    } else {
        assert!(
            shows(&mut app, bevy_cauldron::workspace_panels::NO_AUDIO),
            "the Sound workspace, in a build without audio, does not say so"
        );
    }
}

/// **No panel is reachable by its old route.** The Map's INSPECTOR is where these headings used to be
/// found, a wheel-scroll down; they are not in the world at all now, so no scroll finds them. The Map keeps
/// what is the Map's, which is asserted first so this cannot pass on an editor that drew nothing.
#[test]
fn no_moved_panel_is_reachable_from_the_map() {
    let mut app = editor("old_route");
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map, "the editor opens on Map");
    assert!(shows(&mut app, "INSPECTOR") && shows(&mut app, "NOTES"), "the Map's own panels are not on screen");
    for label in ["TERRAIN", "ENVIRONMENT", "LIGHTS", "LIGHT MAP", "AUDIO"] {
        assert!(!shows(&mut app, label), "`{label}` is still on the Map, so it is reached two ways");
    }
    // Every row of the Map's inspector scroll is in the world whether or not the person has scrolled to it,
    // so "not found" is the whole claim: there is nothing further down to find.
    let all: Vec<String> = {
        let world = app.world_mut();
        world.query::<&Text>().iter(world).map(|t| t.0.trim().to_owned()).collect()
    };
    for label in ["TERRAIN", "ENVIRONMENT", "LIGHTS", "AUDIO"] {
        assert!(!all.iter().any(|t| t == label), "a `{label}` heading exists on the Map, scrolled out of sight");
    }
}

/// **A workspace shows only its own panel.** World's terrain line is not on the Sound workspace and the
/// Sound panel is not on World, so the panels did not simply move to wherever the last switch left them.
#[test]
fn a_workspaces_panel_is_not_on_the_others() {
    let mut app = editor("exclusive");
    switch_to(&mut app, Workspace::World);
    assert!(shows(&mut app, "TERRAIN"));
    assert!(!shows(&mut app, "DATA"), "the Map's DATA is still on the World workspace");
    switch_to(&mut app, Workspace::Sound);
    assert!(!shows(&mut app, "TERRAIN"), "the World's TERRAIN is still on the Sound workspace");
}
