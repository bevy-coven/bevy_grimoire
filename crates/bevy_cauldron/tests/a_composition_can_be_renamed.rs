//! **The COMPOSE tab can rename the composition it is showing.**
//!
//! The editor-side half of the 2026-10-20-06 run. The core's writer is tested in
//! `bevy_grimoire_core/tests/a_composition_can_be_renamed.rs`; this file is about the *editor*:
//!
//! - **The name is forced as it is typed**, so an illegal id is never reachable.
//! - **A rename that would collide is refused, and nothing is written.**
//! - **Every map that stamps the composition follows** — and that is the editor's own door,
//!   `Project::rename_composition`, because `tests/compose_is_read_only.rs` says *"project.rs is the
//!   only module that names the file, and every tab reaches it there."*
//! - **The one writer is the one the crate names.** The core's ratchet asserts the file is not
//!   written from a tab; this asserts the tab *reaches* the door.

use crate::fixtures;

use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};
use fixtures::Fixture;

/// A project with one composition and a map that stamps it.
fn project(name: &str) -> std::path::PathBuf {
    let root = Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .composition("site/room", &[("site/wall", "site/wall", (0.0, 0.0))])
        .build("m");
    // **A stamp of the composition**, so the rename has a reference to follow. The fixture's map
    // carries none — the shipped project's maps carry none either, which is why the case has to be
    // built to be tested.
    let map = root.join("assets/grimoire/maps/m.map.ron");
    let text = std::fs::read_to_string(&map).expect("the map is there");
    // **The fixture's map already carries an empty `stamps:`**, so the stamp goes *inside* it —
    // a second `stamps` key would be a duplicate field, and the map would not parse.
    let stamped = text.replace(
        "stamps: [],",
        r#"stamps: [
        (
            id: "room@1",
            of: "site/room",
            at: (0.0, 0.0),
            yaw: 0.0,
            overrides: [],
            of_fingerprint: None,
            owned: false,
            owned_because: None,
            note: None,
        ),
    ],"#,
    );
    assert_ne!(
        stamped, text,
        "the fixture's map has no `stamps: [],` to fill. The map's shape changed and this test \
         would be stamping nothing."
    );
    std::fs::write(&map, stamped).expect("the map is writable");
    root
}

/// **The rename goes through the door, and the map follows.**
#[test]
fn the_rename_reaches_the_door_and_the_map_follows() {
    let root = project("compose-rename-follows");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // **The door does the work.** The map is taken out of the world for the call, because the
    // writer wants the project and the map at once and the world will not lend both.
    {
        let world = app.world_mut();
        let mut open = world.remove_resource::<OpenMap>();
        let mut project = world.resource_mut::<Project>();
        project
            .rename_composition("site/room", "site/room_v2", open.as_mut())
            .unwrap_or_else(|e| panic!("the rename must succeed: {e}"));
        if let Some(open) = open {
            world.insert_resource(open);
        }
    }
    app.update();

    // **The composition's file says the new name.**
    let comps = std::fs::read_to_string(root.join("assets/grimoire/compositions.ron"))
        .expect("the compositions file is there");
    assert!(
        comps.contains(r#"id: "site/room_v2""#),
        "the composition is not under its new name"
    );
    assert!(
        !comps.contains(r#"id: "site/room""#),
        "the composition is still under its old name"
    );

    // **And the map followed.**
    let map = std::fs::read_to_string(root.join("assets/grimoire/maps/m.map.ron"))
        .expect("the map is there");
    assert!(
        map.contains(r#"of: "site/room_v2""#),
        "the map's stamp still names the old composition, so the map has a dangling reference and \
         will not open. The map reads:\n{map}"
    );
    assert!(
        !map.contains(r#"of: "site/room""#),
        "the map's stamp still names the old composition"
    );
}

/// **The rename refuses a name that is taken, and changes nothing.**
#[test]
fn a_rename_that_would_collide_is_refused_and_changes_nothing() {
    let root = project("compose-rename-collides");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // **A second composition, so there is a name to collide with.**
    {
        let world = app.world_mut();
        let mut project = world.resource_mut::<Project>();
        let mut comp = project.compositions.compositions[0].clone();
        comp.id = "site/taken".to_owned();
        project
            .commit_composition(comp)
            .unwrap_or_else(|e| panic!("the commit must succeed: {e}"));
    }
    app.update();

    let before = std::fs::read_to_string(root.join("assets/grimoire/compositions.ron"))
        .expect("the compositions file is there");
    let refused = {
        let world = app.world_mut();
        let mut project = world.resource_mut::<Project>();
        project.rename_composition("site/room", "site/taken", None)
    };
    assert!(
        refused.is_err(),
        "renaming to a name that is already taken must be refused"
    );
    let after = std::fs::read_to_string(root.join("assets/grimoire/compositions.ron"))
        .expect("the compositions file is there");
    assert_eq!(
        before, after,
        "a refused rename changed the file. A refusal is a refusal."
    );
}

/// **The name the author typed reaches the writer whole.**
///
/// **The defect this run's visual review found.** The field forced the name with
/// `naming::to_snake_case`, and `to_snake_case` **drops every character that is not a letter or a
/// digit** — including the `/`. So an author who typed `town/house_cottage_brick` got
/// `town_house_cottage_brick`: **a different id, in a different namespace, colliding with nothing**,
/// so no refusal was owed and no error was shown. The list then carried two rows that looked alike
/// and meant different things.
///
/// **A composition id is per-segment** — `site/wall_corner` is *"which kit"* and *"which piece"* —
/// and the crate already says so in `naming::is_id`. What was missing was the *forcing*: `to_id`,
/// added by this run.
#[test]
fn the_namespace_survives_the_field() {
    assert_eq!(
        bevy_grimoire_core::naming::to_id("town/house_cottage_brick"),
        "town/house_cottage_brick",
        "the namespace must survive the field. A name that loses its namespace is a different \
         composition, and it collides with nothing, so nothing would ever say it was wrong."
    );
    assert_eq!(
        bevy_grimoire_core::naming::to_id("Town/House Cottage Brick"),
        "town/house_cottage_brick",
        "and the spelling is still forced, per segment"
    );
}

/// **The name field holds the keyboard while it is open.**
///
/// **The defect this run's second visual review found.** `Action::ComposeArm` is `Enter` on the
/// COMPOSE tab, and the rename's field is committed with `Enter` too. So the same keystroke named
/// the composition *and* armed it — and the arm's message overwrote the rename's receipt. The
/// author renamed a composition and was told it was *armed*, which is a different fact, and the one
/// that mattered — *"and every map that stamped it follows"* — was gone before it could be read.
///
/// **The crate already has the answer**, and its own doc says where it lives:
/// `editor::Fields::typing` — *"Adding a field is adding a line, here and nowhere else."* While a
/// field is typing, `keys::Context::Typing` holds the keyboard and every other action is refused.
#[test]
fn the_name_field_holds_the_keyboard_while_it_is_open() {
    let src = std::fs::read_to_string("src/editor.rs").expect("editor.rs is there");
    // **The guard's own list.** One line, and it names the COMPOSE tab's field.
    assert!(
        src.contains("self.compose.renaming.is_some()"),
        "the COMPOSE tab's rename prompt is not in `editor::Fields::typing`, so `Enter` both names \
         the composition and arms it, and the arm's message overwrites the rename's receipt. See \
         `editor::Fields` — \"Adding a field is adding a line, here and nowhere else.\""
    );
}

/// **The door is where the write happens.** `tests/compose_is_read_only.rs` holds the line that
/// `project.rs` is the only module that names the compositions file; this test is the other half of
/// it — the tab *reaches* the door rather than writing around it.
#[test]
fn the_compose_tab_reaches_the_door_rather_than_writing_itself() {
    let src = std::fs::read_to_string("src/compose.rs").expect("compose.rs is there");
    let code: String = src
        .lines()
        .take_while(|l| !l.trim_start().starts_with("#[cfg(test)]"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code.contains("Compositions::FILE"),
        "the Compose tab names the compositions file. `project.rs` is the one module allowed to, and \\
         every tab reaches it there — see `tests/compose_is_read_only.rs`."
    );
    assert!(
        code.contains("Project::rename_composition"),
        "the Compose tab does not reach the door to rename. The rename is the door's verb, and the \\
         tab asks."
    );
}
