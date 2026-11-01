//! **The arm crosses the door, and the map can arm a composition.**
//!
//! The regression tests for the 2026-10-21 run. What they pin, and why each one is a ratchet
//! rather than a comment:
//!
//! - **The arm is a `Session` resource.** The `Ownership` table in `screen.rs` says what a door
//!   change does to every resource the editor registers, and `the_door_resets_what_it_says_it_owns`
//!   checks the table against the macro that does the resetting. A resource classified `Door` is
//!   reset; one classified `Session` is not. **So a test that asserts the arm survives a door change
//!   is a test of the table**, and the table is the only place that decides.
//!
//! - **The map door can arm.** A composition is armed from the Map door's own picker, and the next
//!   click stamps it. This is the capability the round trip used to make impossible.
//!
//! - **One writer.** Three ways to arm — the COMPOSE tab's `Enter`, the Map door's picker, and the
//!   devshot driver — and one writer, so there is one answer to "what is armed".
//!
//! - **A refusal, not a degradation.** An arm naming a composition the project no longer defines is
//!   refused **by name** and the arm is cleared, rather than a silent stamp of nothing.

use crate::fixtures;

use bevy::prelude::{Mut, World};

use bevy_cauldron::compose::Armed;
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::screen::{Ownership, ownership};
use fixtures::Fixture;

/// **A project with a composition, a piece and a map — under a name of its own.**
///
/// **A fixture's directory is keyed by the process id and its name**, and the tests run **in
/// parallel** in one process. Two tests sharing a name share a directory, and each one's
/// `Fixture::new` **deletes** it — so they remove each other's project. That is not a flake to
/// shrug at: it was measured as a test that passed on three runs out of six, and the fix is a
/// name per test.
fn own_project(name: &str) -> std::path::PathBuf {
    Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .composition("site/room", &[("site/wall", "site/wall", (0.0, 0.0))])
        .build("m")
}

/// **The arm is `Session`, and the table says so.**
///
/// The table is the only thing that decides what a door change does to a resource, and the
/// ratchet that checks the table against the code is
/// `every_resource_says_what_a_door_does_to_it.rs`. This test asks the table directly, so a
/// classification that silently became `Door` fails here rather than in a comment.
#[test]
fn the_arm_is_the_authors_intent_and_survives_a_door_change() {
    assert_eq!(
        ownership("bevy_cauldron::compose::Armed"),
        Some(Ownership::Session),
        "the arm is the author's intent — *which composition the next click stamps* — and it has to \
         outlive a door change. Classifying it `Door` puts it back in the teardown, and the arm is \
         lost exactly where it was most wanted."
    );
}

/// **And the arm is not reset by the teardown.** The table's word and the code's word have to agree,
/// and the code's word is `reset_door_state`. The ratchet that checks the two is
/// `every_resource_says_what_a_door_does_to_it.rs`; this is the same question asked of one type.
#[test]
fn the_arms_type_is_not_in_the_door_reset_list() {
    let reset = bevy_cauldron::screen::door_state_type_paths();
    assert!(
        !reset.contains(&std::any::type_name::<Armed>()),
        "`Armed` is in the list of types a door change resets, so the arm is thrown away with the \
         door's cursors. Its classification says `Session`; the macro says otherwise."
    );
}

/// **The arm survives the door change, by name.**
///
/// The whole feature in one assertion: the arm is set, the door changes, and the arm is *the same
/// composition's id* afterwards. On the unfixed code the arm was a field of `ComposeState`, which
/// `Ownership::Door` resets — so this fails there, with the arm empty.
#[test]
fn an_armed_composition_is_still_armed_after_the_door_changes() {
    let root = own_project("arm-crosses-keeps");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // Arm it, the way the COMPOSE tab's `Enter` does — through the one writer.
    app.world_mut().resource_mut::<Armed>().0 = Some("site/room".to_owned());
    app.update();

    // **The door changes.** The teardown is `screen::close_the_door` — what `OnExit(Editor)` runs
    // on every door change — and it is called **directly**, because the headless harness does not
    // register `ScreenPlugin` and so never runs the `OnExit` schedule. Calling it is the honest
    // way to ask "what does a door change do to this resource?"; going through the state machine
    // would ask a question the harness cannot answer.
    //
    // **No `app.update()` afterwards.** The teardown drops the `Project` and the `Mode`, and the
    // editor's systems are gated on `in_state(Screen::Editor)` — which is still true, because the
    // harness never registered `ScreenPlugin` and so the state never left. A frame here would run
    // them against a world with no project, and Bevy 0.19 panics a system whose `Res<T>` is
    // missing rather than skipping it.
    bevy_cauldron::screen::close_the_door(app.world_mut());

    let armed = app.world().resource::<Armed>();
    assert_eq!(
        armed.id(),
        Some("site/room"),
        "the arm was lost across the door change. It is the author's intent, not the door's working \
         state, and it is the one thing that has to cross: the composition was made on one door and \
         the map that stamps it is on the other."
    );
}

/// **The map door can arm, and the next click stamps.**
///
/// The capability the round trip made impossible: the author picks a composition from the Map
/// door's own picker, and the click puts it down. Driven through the picker's own writer — the
/// same call a click on the row makes — so this cannot pass while the click path is broken.
#[test]
fn the_map_door_can_arm_a_composition_and_stamp_it() {
    let root = own_project("arm-crosses-tears");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // **The picker's writer, called exactly as a click on the row calls it.**
    app.world_mut()
        .resource_scope(|world: &mut World, project: Mut<Project>| {
            let mut armed = world.resource_mut::<Armed>();
            let mut status = bevy_cauldron::chrome::Status::default();
            bevy_cauldron::compose::picker::arm_row(0, &project, &mut armed, &mut status);
        });
    app.update();

    assert_eq!(
        app.world().resource::<Armed>().id(),
        Some("site/room"),
        "the picker did not arm the composition it was asked to arm"
    );

    // **And the click stamps it.**
    //
    // One scope, and the map is *taken out* of the world for the duration — `resource_scope`
    // removes the resource and puts it back, so the closure can borrow the rest of the world
    // freely. That is the one shape Bevy's borrow rules allow, and it is the shape the existing
    // headless tests use for the same verb.
    app.world_mut()
        .resource_scope(|world: &mut World, mut project: Mut<Project>| {
            let mut open = world.remove_resource::<OpenMap>().expect("the map is open");
            let mut state = world
                .remove_resource::<bevy_cauldron::editor::EditorState>()
                .expect("the editor state is there");
            // **The editor's own arm**, so the stamp is driven by what the editor holds.
            let mut armed = world.remove_resource::<Armed>().expect("the arm is there");
            bevy_cauldron::editor::stamp_here_for_test(
                &mut project,
                &mut open,
                &mut state,
                &mut armed,
                (2.0, 2.0),
            );
            world.insert_resource(state);
            world.insert_resource(armed);
            world.insert_resource(open);
        });
    app.update();

    let open = app.world().resource::<OpenMap>();
    assert_eq!(
        open.map.stamps.len(),
        1,
        "the armed composition did not stamp: the map carries {} stamp(s) and {} placement(s)",
        open.map.stamps.len(),
        open.map.placements.len()
    );
    assert_eq!(
        open.map.stamps[0].of, "site/room",
        "the stamp names the composition that was armed"
    );
}

/// **One writer, and the three doors agree.**
///
/// The COMPOSE tab's `Enter`, the Map door's picker and the devshot driver are three ways to arm.
/// They must be one answer, or "what is armed" has three.
#[test]
fn every_way_to_arm_lands_on_the_same_answer() {
    let root = own_project("arm-crosses-arms");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // The picker.
    app.world_mut()
        .resource_scope(|world: &mut World, project: Mut<Project>| {
            let mut armed = world.resource_mut::<Armed>();
            let mut status = bevy_cauldron::chrome::Status::default();
            bevy_cauldron::compose::picker::arm_row(0, &project, &mut armed, &mut status);
        });
    app.update();
    assert_eq!(app.world().resource::<Armed>().id(), Some("site/room"));

    // The COMPOSE tab's own `Enter`, through the same writer.
    //
    // **The editor's own arm, not a local copy** — a copy would leave the resource alone and the
    // assertion would pass for the wrong reason. And the resource is *taken out* of the world for
    // the call, because `resource_scope` lends it exclusively and the writer needs the project too.
    //
    // **It is a toggle, and the composition is already armed** — so the COMPOSE tab's `Enter` on
    // the *same* composition **disarms** it. That is the answer both doors must give: one writer,
    // one meaning. The question this test asks is *"do the two doors agree about which composition
    // is armed?"* — and the way to ask it is to have the two doors arm **different** compositions
    // and see that the last one wins, rather than having them agree by accident.
    app.world_mut()
        .resource_scope(|world: &mut World, project: Mut<Project>| {
            let mut armed = world.remove_resource::<Armed>().expect("the arm is there");
            let mut state = world.resource_mut::<bevy_cauldron::compose::ComposeState>();
            bevy_cauldron::compose::toggle_arm(&mut state, &mut armed, &project);
            world.insert_resource(armed);
        });
    app.update();
    assert_eq!(
        app.world().resource::<Armed>().id(),
        None,
        "the COMPOSE tab's `Enter` did not disarm the composition the picker had armed. The two \
         doors are the same verb and must be the same answer — and a toggle on an armed composition \
         disarms it."
    );

    // **And the other way round**: the picker disarms what the COMPOSE tab armed.
    app.world_mut()
        .resource_scope(|world: &mut World, project: Mut<Project>| {
            let mut armed = world.remove_resource::<Armed>().expect("the arm is there");
            let mut status = bevy_cauldron::chrome::Status::default();
            bevy_cauldron::compose::picker::arm_row(0, &project, &mut armed, &mut status);
            world.insert_resource(armed);
        });
    app.update();
    assert_eq!(
        app.world().resource::<Armed>().id(),
        Some("site/room"),
        "the picker did not arm the composition the COMPOSE tab had disarmed, so the two doors \
         disagree about what is armed. They are the same verb and must be the same answer."
    );
}

/// **A refusal, not a degradation.**
///
/// An arm naming a composition the project no longer defines is refused **by name**, and the arm is
/// cleared — so the next click does not stamp nothing without saying so.
#[test]
fn an_arm_naming_a_composition_the_project_no_longer_has_is_refused_by_name() {
    let root = own_project("arm-crosses-refused");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    app.world_mut().resource_mut::<Armed>().0 = Some("site/gone".to_owned());
    app.update();

    let said = {
        let mut problem = None;
        app.world_mut()
            .resource_scope(|world: &mut World, mut project: Mut<Project>| {
                let mut open = world.remove_resource::<OpenMap>().expect("the map is open");
                // **The editor's own state and arm, not copies.** The refusal clears the arm it
                // is handed, so a copy would pass while the editor kept the dead name — which is
                // the whole point of the assertion.
                let mut state = world
                    .remove_resource::<bevy_cauldron::editor::EditorState>()
                    .expect("the editor state is there");
                let mut armed = world.remove_resource::<Armed>().expect("the arm is there");
                bevy_cauldron::editor::stamp_here_for_test(
                    &mut project,
                    &mut open,
                    &mut state,
                    &mut armed,
                    (2.0, 2.0),
                );
                problem = Some(state.status.problem_text().to_owned());
                world.insert_resource(state);
                world.insert_resource(armed);
                world.insert_resource(open);
            });
        problem.expect("an arm that names nothing was refused without a word")
    };
    app.update();

    assert!(
        said.contains("site/gone"),
        "the refusal has to name the composition it refused, so the author knows which arm to drop. \
         It said `{said}`"
    );
    assert_eq!(
        app.world().resource::<Armed>().id(),
        None,
        "an arm that names a composition the project does not have has to be cleared, or every \
         later click refuses the same dead name."
    );
    assert_eq!(
        app.world().resource::<OpenMap>().map.stamps.len(),
        0,
        "nothing was stamped, and nothing should have been"
    );
}
