//! **The workspaces replace the doors** (NV-02, #379; series getting around, #377).
//!
//! A door was a *place* and a *job* welded into one value: the Kit door was the meshes, the tiles and
//! the compositions; the Map door was the level. So crossing between them reloaded the project, and
//! there were three of them because there were three jobs.
//!
//! A workspace is only the job. What is on screen is a second axis — the dock layout — and because
//! the two are no longer one value, switching keeps the project, the open map and the selection. This
//! file is that claim, measured, plus the three things `docs/ui/vision.md` §4 says a workspace has:
//!
//! - **a layout of its own**, so a divider moved in Map is where it was left in Map and somewhere
//!   else in Kit;
//! - **a key and a command**, nine of them, `Cmd`/`Ctrl`+`1`…`9` in a fixed order, so a person's
//!   fingers keep their meaning across releases;
//! - **no placeholder screen.** A workspace whose part is not built is not registered, and its
//!   command says which issue brings it.
//!
//! `tiles::Door` no longer exists: the last check is that no source file names it.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;

use bevy_cauldron::commands;
use bevy_cauldron::harness;
use bevy_cauldron::keys::{binding, Action, Live};
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::selection::Selection;
use bevy_cauldron::tiles::Mode;
use bevy_cauldron::workspace::Workspace;
use bevy_gooey::{DockLayout, SplitAxis};

use crate::fixtures;

/// **Frames the editor is given to settle** after a switch: the transition runs in the frame after
/// the key, and the panels are spawned on the frame after that.
const SETTLE_FRAMES: usize = 10;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("ws_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .descriptor("alpha/wall", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .place_as("wall@0", "alpha/wall", (2.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{_e}", _e = e));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    app
}

/// **Hold a chord down for two frames and let it go**, exactly as `command_run` does for an agent and
/// as a person's keyboard does: the modifier and the key together, so `needs_mod` sees one state.
fn press(app: &mut App, action: Action) {
    let row = binding(action);
    let mut keys = vec![row.key];
    if row.needs_mod {
        keys.push(if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft });
    }
    if row.needs_shift == Some(true) {
        keys.push(KeyCode::ShiftLeft);
    }
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for code in &keys {
            app.world_mut().write_message(KeyboardInput {
                key_code: *code,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        app.update();
        app.update();
    }
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

fn shown(app: &mut App) -> DockLayout {
    let world = app.world_mut();
    let mut q = world.query::<&bevy_gooey::Dock>();
    q.iter(world).next().expect("the shell is a gooey Dock").0.clone()
}

/// **Every built workspace is reachable by its key, and a round trip leaves the map and the selection
/// alone.**
#[test]
fn a_round_trip_through_the_workspaces_keeps_the_map_and_the_selection() {
    let mut app = editor("round_trip");
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map, "the editor opens on Map");

    // Something to lose: the map's name and a selected piece.
    let map = app.world().resource::<OpenMap>().map.name.clone();
    assert!(
        bevy_cauldron::editor::lock_on_placement(app.world_mut(), "floor@0"),
        "the test's piece must be on the map to be selected"
    );
    for _ in 0..3 {
        app.update();
    }
    let before = app.world().resource::<Selection>().ids().to_vec();
    assert_eq!(before, ["floor@0"], "the test's own selection must be the one the editor holds");

    // **By the key**, for every workspace the editor opens today, and back to Map.
    let built: Vec<Workspace> = Workspace::ALL.into_iter().filter(|w| w.is_built()).collect();
    assert!(built.len() >= 3, "the built workspaces must include Map, Kit and Animation");
    assert!(built.contains(&Workspace::Map), "Map is built");
    for workspace in built.iter().copied().filter(|w| *w != Workspace::Map).chain([Workspace::Map]) {
        let action = Action::ALL
            .into_iter()
            .find(|a| a.workspace() == Some(workspace))
            .unwrap_or_else(|| panic!("no chord goes to the {} workspace", workspace.title()));
        press(&mut app, action);
        assert_eq!(
            app.world().resource::<Workspace>(),
            &workspace,
            "the key did not switch to the {} workspace",
            workspace.title()
        );
        assert_eq!(
            app.world().resource::<OpenMap>().map.name,
            map,
            "switching to {} reloaded the map, which the shared viewport forbids",
            workspace.title()
        );
        assert_eq!(
            app.world().resource::<Selection>().ids(),
            before.as_slice(),
            "switching to {} dropped the selection, and the decision says switching never drops it",
            workspace.title()
        );
    }
}

/// **The panel follows the workspace when the open one is not shown there.**
///
/// Kit shows Meshes/Tiles/Compose and Animation shows `ANIM`, so a switch from Kit's Tiles to
/// Animation must land on `ANIM` — a panel the workspace does not show would draw no strip entry and
/// no content.
#[test]
fn the_panel_follows_the_workspace() {
    let mut app = editor("panel_follows");
    let kit = Action::ALL.into_iter().find(|a| a.workspace() == Some(Workspace::Kit)).expect("Kit's chord");
    press(&mut app, kit);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Kit);
    assert_eq!(app.world().resource::<Mode>().label(), "MESHES", "Kit opens on its first panel");

    let anim = Action::ALL.into_iter().find(|a| a.workspace() == Some(Workspace::Animation)).expect("Animation's chord");
    press(&mut app, anim);
    assert_eq!(
        app.world().resource::<Mode>().label(),
        "ANIM",
        "the Animation workspace shows `ANIM`, so a switch from Meshes must move the panel with it"
    );
}

/// **A divider moved in one workspace is not moved in another, and both survive a restart.**
#[test]
fn each_workspace_keeps_its_own_arrangement() {
    let root = fixtures::Fixture::new(&format!("ws_layout_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place("alpha/floor", (0.0, 0.0))
        .build("m");
    let user = root.join(format!("user_{}/settings.ron", std::process::id()));
    std::fs::create_dir_all(user.parent().unwrap()).unwrap();

    // **The arrangement no default has**: the docks and the viewport swap sides.
    let moved = DockLayout::split(
        SplitAxis::Horizontal,
        0.78,
        DockLayout::group(["place"]),
        DockLayout::split(SplitAxis::Horizontal, 0.22, DockLayout::group(["viewport"]), DockLayout::group(["inspector"])),
    );
    bevy_cauldron::setting_catalogue::edit_layer(&user, |layer| {
        layer.workspace_layout.insert("map".to_owned(), moved.clone());
        Ok(())
    })
    .unwrap_or_else(|e| panic!("{e}"));

    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{_e}", _e = e));
    app.insert_resource(bevy_cauldron::settings::UserSettingsFile(user.clone()));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    assert_eq!(shown(&mut app), moved, "Map opens on the arrangement in the file");

    let kit = Action::ALL.into_iter().find(|a| a.workspace() == Some(Workspace::Kit)).expect("Kit's chord");
    press(&mut app, kit);
    assert_eq!(
        Workspace::Kit.default_layout(),
        shown(&mut app),
        "Kit showed Map's arrangement, so the layouts are not per workspace"
    );
    assert_ne!(
        Workspace::Kit.default_layout(),
        Workspace::Map.default_layout(),
        "the two defaults are the same arrangement, so this test could not tell a per-workspace \
         layout from a shared one"
    );

    // **And a restart finds both**: Map from the file, Kit still its default.
    let mut again = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{_e}", _e = e));
    again.insert_resource(bevy_cauldron::settings::UserSettingsFile(user));
    for _ in 0..SETTLE_FRAMES {
        again.update();
    }
    assert_eq!(shown(&mut again), moved, "the restart lost the arrangement Map was left in");
}

/// **An unbuilt workspace is refused with the reason, and its chord does not move the screen.**
///
/// The vision's *"no placeholder screens"*: a workspace whose part is not built is not in the
/// registry, and its command says which issue brings it rather than doing nothing.
#[test]
fn an_unbuilt_workspace_says_which_issue_brings_it() {
    let unbuilt: Vec<Workspace> = Workspace::ALL.into_iter().filter(|w| !w.is_built()).collect();
    assert!(!unbuilt.is_empty(), "at least one workspace is not built yet, or this test checks nothing");

    for workspace in &unbuilt {
        let id = format!("workspace.{}", workspace.id());
        let command = commands::by_id(&id).unwrap_or_else(|| panic!("`{id}` must still be a command, so its chord answers"));
        let why = command
            .enabled(Live::default())
            .expect_err("an unbuilt workspace must be disabled, so nothing opens a placeholder");
        assert!(
            why.contains('#') || why.contains("GD-"),
            "the refusal for `{id}` names no issue, so a person pressing the chord learns nothing: {why}"
        );
    }

    // **And on screen**: pressing it leaves the workspace where it was.
    let mut app = editor("unbuilt");
    let before = *app.world().resource::<Workspace>();
    for workspace in &unbuilt {
        let action = Action::ALL
            .into_iter()
            .find(|a| a.workspace() == Some(*workspace))
            .unwrap_or_else(|| panic!("no chord goes to the {} workspace", workspace.title()));
        press(&mut app, action);
        assert_eq!(
            app.world().resource::<Workspace>(),
            &before,
            "the {} workspace is not built, and its key moved the screen onto a placeholder",
            workspace.title()
        );
    }

    // A built one is enabled.
    let id = format!("workspace.{}", Workspace::Map.id());
    assert!(commands::by_id(&id).expect("Map is a command").enabled(Live::default()).is_ok());
}

/// **`tiles::Door` no longer exists**, and no source file names it.
///
/// The acceptance item is exact: the type is gone. This reads the crate's own sources rather than the
/// compiled symbols, because a test cannot name a type that is not there — and a dead alias left in
/// place is exactly the second path the acceptance forbids.
#[test]
fn no_source_names_a_door_any_more() {
    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    let mut dirs = vec![crate_root];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.unwrap_or_else(|e| panic!("{e}")).path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    assert!(files.len() > 100, "the walk found {} files, so it is not reading the crate", files.len());

    let mut offenders = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        for (n, line) in text.lines().enumerate() {
            // **`Door` as a type, not the word in prose.** `bevy_grimoire`'s behaviour crate has a
            // `Door` component that has nothing to do with this, and a comment about the doors is
            // history worth keeping — what must not survive is a *use* of the deleted editor type.
            let code = line.split("//").next().unwrap_or("");
            let mentions = code.contains("tiles::Door")
                || code.contains("Door::")
                || code.contains("resource::<Door>")
                || code.contains("OtherDoorMark")
                || code.contains("OtherDoorLabel")
                || code.contains("DoorStrip");
            if mentions {
                offenders.push(format!("{}:{}: {}", file.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these lines still name the doors the workspaces replaced:\n{}",
        offenders.join("\n")
    );
}

/// **The mode is still the panel axis**, and every panel has exactly one workspace.
#[test]
fn every_panel_has_one_workspace_and_the_strip_lists_that_workspace_s_panels() {
    for mode in Mode::ALL {
        let homes: Vec<Workspace> = Workspace::ALL.into_iter().filter(|w| w.shows(mode)).collect();
        assert_eq!(homes.len(), 1, "`{}` is shown by {} workspaces", mode.label(), homes.len());
        assert!(homes[0].is_built(), "`{}` lives in a workspace that is not built", mode.label());
    }
    assert_eq!(Workspace::showing(Mode::Tiles), Workspace::Kit);
    assert_eq!(Workspace::showing(Mode::Anim), Workspace::Animation);
    assert_eq!(Workspace::showing(Mode::Map), Workspace::Map);

    let mut app = editor("strip");
    let kit = Action::ALL.into_iter().find(|a| a.workspace() == Some(Workspace::Kit)).expect("Kit's chord");
    press(&mut app, kit);
    let mut strip: Vec<String> = {
        let world = app.world_mut();
        let mut q = world.query::<(&bevy_cauldron::tiles::TabLabel, &Text)>();
        q.iter(world).map(|(_, t)| t.0.clone()).collect()
    };
    let mut want: Vec<String> = Workspace::Kit.panels().iter().map(|m| m.label().to_owned()).collect();
    // A query visits entities in archetype order, not strip order; the claim is *which* panels.
    strip.sort();
    want.sort();
    assert_eq!(strip, want, "the strip lists exactly the Kit workspace's panels");
}
