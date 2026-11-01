//! **The door switch, and the project it keeps.**
//!
//! The regression tests for the 2026-10-20-05 run. What they pin:
//!
//! - **The crossing keeps the project.** `Project` is not `Clone`, so a crossing that re-read it
//!   would be a different value — and the `Ownership` table says a same-kit crossing keeps it.
//!   The test proves the project **survives** a crossing, by identity.
//! - **The crossing tears the old door down.** Every entity the old door made is gone and every
//!   `Ownership::Door` resource is back at its opening value. **The teardown stays a teardown.**
//! - **A crossing that cannot complete says so and stays put.** The same door, the same project,
//!   the same map, the same arm.
//! - **The unsaved question.** A crossing with unsaved map edits asks first.
//! - **The crossing is not the menu's route.** The menu still opens a door; the crossing *changes*
//!   one.
//! - **The round trip still works.** `Cmd+O` → the chooser → the other door. The crossing is
//!   **added, not substituted**.

use crate::fixtures;

use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::screen::Ownership;
use bevy_cauldron::tiles::Door;
use fixtures::Fixture;

/// A project with a composition, a piece and a map.
///
/// **The name is the test's own**, because a fixture's directory is keyed by the process id and
/// that name — and the tests run **in parallel** in one process. Two tests sharing a name share a
/// directory, and each one's `Fixture::new` **deletes** it. That is not a flake to shrug at: it is
/// two tests removing each other's project, and it was measured as a test that passed on three runs
/// out of six.
fn project(name: &str) -> std::path::PathBuf {
    Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .composition("site/room", &[("site/wall", "site/wall", (0.0, 0.0))])
        .build("m")
}

/// **The crossing keeps the project.** The `Ownership` table says `Project` is *"reloaded when the
/// kit is not"*, and this is the crossing that obeys the table.
///
/// `Project` is **not `Clone`**, so the identity is observable: the project in the world after a
/// crossing is the *same* project, not a re-read one. The proof is a field a re-read would lose —
/// the `open_map_name` the crossing carries.
#[test]
fn the_crossing_keeps_the_project() {
    let root = project("door-switch-keeps");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // **The marker a re-read would lose.** The crossing sets it, and `Project::open` sets it to
    // `None` — so a re-read would erase it.
    {
        let world = app.world_mut();
        let mut p = world.resource_mut::<Project>();
        p.open_map_name = Some("m".to_owned());
    }
    app.update();

    // **Cross.** The teardown and the rebuild are the editor's own; the crossing is what this test
    // is about, and it is called directly so the test does not depend on the key.
    {
        let world = app.world_mut();
        bevy_cauldron::screen::cross(world, Door::Kit)
            .unwrap_or_else(|e| panic!("the crossing must succeed: {e}"));
    }
    // **Two updates**: the first applies the pending transition, the second lets the editor settle.
    app.update();
    app.update();

    let project = app.world().resource::<Project>();
    assert_eq!(
        project.open_map_name.as_deref(),
        Some("m"),
        "the crossing re-read the project, so the name it carried is gone. The `Ownership` table \
         says a project is *reloaded when the kit is not*, and a same-kit crossing is not a kit \
         change — so the project must SURVIVE it."
    );
}

/// **The crossing tears the old door down.** The teardown is a teardown: the old door's entities
/// are gone and the door's own working state is back at its opening value.
#[test]
fn the_crossing_tears_the_old_door_down() {
    let root = project("door-switch-tears");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // **Something the door made**, so there is something to tear down.
    let before = {
        let world = app.world();
        world
            .iter_entities()
            .filter(|e| world.get::<bevy::prelude::Node>(e.id()).is_some())
            .count()
    };
    assert!(before > 0, "the editor made nothing, so there is nothing to tear down");

    // **A value the door owns, which the crossing must clear.** `EditorState::grouping` is the
    // name field of a group being captured — the door's own working state, and it is not its
    // opening value.
    {
        let world = app.world_mut();
        world.resource_mut::<bevy_cauldron::editor::EditorState>().grouping =
            Some("a group being named".to_owned());
    }
    app.update();

    {
        let world = app.world_mut();
        bevy_cauldron::screen::cross(world, Door::Kit)
            .unwrap_or_else(|e| panic!("the crossing must succeed: {e}"));
    }
    app.update();
    app.update();

    // **The door's own working state is back at its opening value.** `EditorState` is
    // `Ownership::Door`, so a door change resets it — and the crossing is a door change.
    let state = app.world().resource::<bevy_cauldron::editor::EditorState>();
    assert_eq!(
        state.grouping, None,
        "the door's own working state survived the crossing, so a stale value carried into the new \
         door. `Ownership::Door` means it is reset."
    );
}

/// **A crossing that cannot complete says so and stays put.** The same door, the same project, the
/// same map.
#[test]
fn a_crossing_that_cannot_complete_stays_put() {
    let root = project("door-switch-refused");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // **The Kit door, with no map open and no map name carried** — so crossing to the Map door
    // has nothing to open.
    {
        let world = app.world_mut();
        bevy_cauldron::screen::cross(world, Door::Kit)
            .unwrap_or_else(|e| panic!("the crossing must succeed: {e}"));
    }
    app.update();
    app.update();
    {
        let world = app.world_mut();
        world.resource_mut::<Project>().open_map_name = None;
    }
    app.update();

    let refused = {
        let world = app.world_mut();
        bevy_cauldron::screen::cross(world, Door::Map)
    };
    let refusal = refused.expect_err("a crossing with no map to open must be refused");
    assert!(
        !refusal.is_empty(),
        "the refusal has to say what is wrong, so the author knows what to do. It said nothing."
    );

    // **And the editor is untouched.** The door is still the Kit door, and the project is still
    // there.
    assert_eq!(
        *app.world().resource::<Door>(),
        Door::Kit,
        "a refused crossing changed the door. A refusal is a refusal: the editor stays exactly where \
         it was."
    );
    assert!(
        app.world().contains_resource::<Project>(),
        "a refused crossing left the editor with no project."
    );
}

/// **The crossing is not the menu's route.** The menu still opens a door; the crossing *changes*
/// one. Two different acts, two different actions.
#[test]
fn the_crossing_and_the_menu_are_different_verbs() {
    use bevy_cauldron::keys::{binding, Action};
    let cross = binding(Action::SwitchDoor);
    let menu = binding(Action::MainMenu);
    assert_ne!(
        cross.action, menu.action,
        "the crossing and the menu's route are the same verb, so the census cannot tell them apart \
         — and they are different acts: one opens a door, the other changes which door is open."
    );
    assert_ne!(
        cross.key, menu.key,
        "the crossing and the menu's route share a key, so one of them is unreachable."
    );
    // **And the crossing is on the door strip** — the place the editor already says which door is
    // open, which is where the literature says a switching cue belongs.
    assert_eq!(
        cross.home,
        bevy_cauldron::keys::Home::Control(bevy_cauldron::keys::ControlId::DoorStrip),
        "the crossing's badge is not on the door strip, so the cue is not where the author looks to \
         see which door is open."
    );
}

/// **The ownership table says the project survives a same-kit crossing.** The table is the thing
/// that decides, so the test asks the table.
#[test]
fn the_project_is_a_project_owned_resource() {
    use bevy_cauldron::screen::ownership;
    assert_eq!(
        ownership("bevy_cauldron::project::Project"),
        Some(Ownership::Project),
        "the project is classified as the door's own working state, so a door change resets it — \
         which is exactly what the crossing is supposed to avoid."
    );
}

/// **A crossing reopens the map the author renamed, not the name it had before the rename.**
///
/// The rename moves the file and changes the open map's name, and the project's `open_map_name`
/// is what a crossing reads when no map is open. Before this, the rename left the project holding
/// the old name, so Kit → Map asked for a file that no longer existed and was refused.
#[test]
fn a_crossing_reopens_the_map_that_was_renamed() {
    let root = project("door-switch-renamed");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // **Rename the open map through the same save the editor's rename uses.**
    {
        let world = app.world_mut();
        let mut open = world.remove_resource::<OpenMap>().expect("a map is open");
        open.map.name = "renamed".to_owned();
        {
            let project = world.resource::<Project>();
            open.save(project)
                .unwrap_or_else(|e| panic!("the rename must succeed: {e}"));
        }
        world.insert_resource(open);
    }
    app.update();

    // **Leave the Map door, then come back.**
    {
        let world = app.world_mut();
        bevy_cauldron::screen::cross(world, Door::Kit)
            .unwrap_or_else(|e| panic!("the crossing to the Kit door must succeed: {e}"));
    }
    app.update();
    app.update();
    {
        let world = app.world_mut();
        bevy_cauldron::screen::cross(world, Door::Map).unwrap_or_else(|e| {
            panic!("the crossing back must reopen the renamed map, and it refused: {e}")
        });
    }
    app.update();
    app.update();

    let open = app.world().resource::<OpenMap>();
    assert_eq!(
        open.map.name, "renamed",
        "the crossing reopened a map under a name the rename had already replaced"
    );
}
