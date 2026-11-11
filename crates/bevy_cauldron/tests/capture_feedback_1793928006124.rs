//! **A door has one tab strip, however many times you cross** — `feedback-1793928006124`, relation:
//! *cardinality of the strip against the door's own tab list*.
//!
//! The note, from the editor at 2560x1604: *"The "Map / Kit" toggles get confused with the Meshes, Tiles,
//! Compose tabs."* The capture's own frame shows why: the strip reads `1 MESHES · 2 TILES · 3 COMPOSE` in
//! the tab strip's own vocabulary — large, filled, numbered, bottom marker — and immediately to its right,
//! on the same baseline, a small unnumbered `Map` chip. A person reads the `Map` as tab four.
//!
//! It is not a tab: the Kit door's tabs are `[Meshes, Tiles, Compose]` and it "explicitly does not offer
//! the Map" (`Door::tabs`). The `Map` chip is the **Map door's own strip**, still standing beside the Kit
//! door's, because `spawn_tab_strip` runs on `OnEnter(Screen::Editor)` and nothing removes the strip when
//! the door changes — a crossing adds a strip and leaves the last one in the band.
//!
//! # The relation, and why this one
//!
//! A picture has no expected output, so the oracle is a relation between the door and the strip that draws
//! it: **exactly one strip, holding exactly the door's own tabs.** Segura, Fraser, Sánchez &
//! Ruiz-Cortés, *A Survey on Metamorphic Testing* (`10.1109/TSE.2016.2532875`), find the effective relations
//! are those "strongly inspired by the semantics of the program under test". The semantics of a tab strip
//! are that it *is* the door's list — one strip, one door, no leftovers. Counting strips is therefore asked
//! of the door's declaration rather than of any pixel or colour, so a restyle cannot make it pass.
//!
//! The survey's other finding is why this crosses a door rather than reading the strip once: `Door::tabs`
//! is correct and one strip alone draws correctly. The defect only exists **after a crossing**, so a test
//! that builds one door passes on it — `the_door_switch_crosses` already crosses, and would have to be
//! asked the new question.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::harness;
use bevy_cauldron::keys::ControlId;
use bevy_cauldron::tiles::Tab;
use bevy_cauldron::workspace::Workspace;

use crate::fixtures;

/// A headless editor with one door open, stepped far enough to be laid out.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((
        Window {
            resolution: bevy::window::WindowResolution::new(1280, 720),
            ..default()
        },
        bevy::window::PrimaryWindow,
    ));
    for _ in 0..10 {
        app.update();
    }
    app
}

/// **Every tab the editor is drawing**, in the tab strip's own order, by the label the door gives it.
///
/// Read off [`Tab`] — the marker `spawn_tab_strip` puts on each chip — rather than off any drawn words, so
/// a relabelled panel does not change the count. Labels rather than `Mode`s because `Mode` carries no
/// `Debug`, and a test that cannot print what it found cannot say why it failed.
fn shown_tabs(app: &mut App) -> Vec<&'static str> {
    let world = app.world_mut();
    // **Sorted by x**, because a query's order is archetype order, not the strip's: the first run of this
    // test read the Kit door back as `["COMPOSE", "TILES", "MESHES"]` and would have "failed" on a
    // correct strip. What a person sees is left to right.
    let mut q = world.query::<(&Tab, &ComputedNode, &bevy::ui::UiGlobalTransform)>();
    let mut rows: Vec<(f32, &'static str)> = q
        .iter(world)
        .filter(|(_, node, _)| node.size().x > 0.0)
        .map(|(tab, _, tf)| (tf.translation.x, tab.0.label()))
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.into_iter().map(|(_, label)| label).collect()
}

/// **How many strips the door band holds**, found by the marker `spawn_tab_strip` puts on each.
fn strip_count(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<bevy_cauldron::chrome::Control>>();
    q.iter(world)
        .filter(|e| {
            world
                .get::<bevy_cauldron::chrome::Control>(*e)
                .is_some_and(|c| c.0 == ControlId::WorkspaceStrip)
        })
        .count()
}

/// **Crossing to the Kit door leaves exactly one strip, holding exactly the Kit door's tabs.**
///
/// The capture is the Kit door's strip with the Map door's beside it. Asserted twice, because the two
/// failures are different: the count catches the leftover strip, and the list catches a strip that is
/// present but showing another door's tabs.
#[test]
fn crossing_a_door_leaves_one_strip_with_that_doors_tabs() {
    let mut app = editor("capture_1793928006124");

    // The editor opens on the Map door, which has one tab and one strip.
    let before = strip_count(&mut app);
    assert_eq!(before, 1, "the Map door draws one tab strip (found {before})");
    assert_eq!(
        shown_tabs(&mut app),
        Workspace::Map.panels().iter().map(|m| m.label()).collect::<Vec<_>>(),
        "the Map door's strip holds its own tabs"
    );

    // **Cross to the Kit door**, which is the gesture the note describes. The crossing is called
    // directly, as `the_door_switch_crosses` does, so the test is about the strip and not about the key.
    {
        let world = app.world_mut();
        world.resource_mut::<bevy_cauldron::project::Project>().open_map_name = Some("m".to_owned());
    }
    app.update();
    bevy_cauldron::screen::cross(app.world_mut(), Workspace::Kit)
        .unwrap_or_else(|e| panic!("the crossing to the Kit door must succeed: {e}"));
    // Two updates: the first applies the pending transition, the second lets the editor settle.
    app.update();
    app.update();
    for _ in 0..6 {
        app.update();
    }

    let after = strip_count(&mut app);
    assert_eq!(
        after, 1,
        "after crossing doors the band holds {after} tab strips, not one. `spawn_tab_strip` runs on \
         `OnEnter(Screen::Editor)` and nothing removes the previous strip when the door changes, so each \
         crossing adds one — and the Map door's lone `Map` chip then stands beside the Kit door's \
         `1 MESHES / 2 TILES / 3 COMPOSE`, reading as a fourth tab. That is what \"The \\\"Map / Kit\\\" \
         toggles get confused with the Meshes, Tiles, Compose tabs\" reports."
    );
    assert_eq!(
        shown_tabs(&mut app),
        Workspace::Kit.panels().iter().map(|m| m.label()).collect::<Vec<_>>(),
        "the one strip must hold the door's own tabs — the Kit door is Meshes, Tiles and Compose, and it \
         explicitly does not offer the Map"
    );
}
