//! **The editor's picture and the game's world are the same picture.**
//!
//! # The claim
//!
//! A map's stamps are expanded by one function, [`bevy_grimoire_core::composition::expand`]. The
//! editor calls it to draw; the consumer calls it to load. So a group an author stamped appears
//! in the editor **and** in the game, at the same place, under the same id.
//!
//! Before this was fixed the second half was false: the editor drew the group's rows, and the
//! game — loading the same file — did not have them. The editor said "saved" and the map file
//! said "1 stamp", and the game ran a level with a hole in it.
//!
//! # What is tested here
//!
//! The claim spans two crates, so it is tested across them: the editor's expansion, the
//! consumer's load, and the equality of the two. The expansion's own arithmetic is tested in
//! `bevy_grimoire_core` (`tests/stamps_reach_the_consumer.rs`); the consumer's refusals are tested
//! there too. What is left for here is the part only this crate can see: **that the editor's
//! drawn rows and the consumer's loaded rows are the same rows.**

use crate::fixtures;

use fixtures::Fixture;

/// The rows a stamp stands for, as the editor's own expander produces them.
fn editor_rows(app: &bevy::prelude::App) -> Vec<(String, String, (f32, f32))> {
    let open = app
        .world()
        .resource::<bevy_cauldron::project::OpenMap>();
    let project = app
        .world()
        .resource::<bevy_cauldron::project::Project>();
    let expansion = bevy_grimoire_core::composition::expand_map(
        &open.map,
        &project.compositions.compositions,
        &project.library,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    expansion
        .placements
        .iter()
        .map(|p| (p.id.clone(), p.descriptor.clone(), p.at))
        .collect()
}

/// The rows the **game** gets, by loading the same project through the consumer's own door.
fn consumer_rows(root: &std::path::Path, map: &str) -> Vec<(String, String, (f32, f32))> {
    let grimoire = root.join("assets/grimoire");
    let data = bevy_grimoire_core::project_data::ProjectData::load(&grimoire, &[map])
        .unwrap_or_else(|e| panic!("{e}"));
    let map = data.maps.get(map).expect("the map loaded");
    map.placements
        .iter()
        .filter(|p| p.id.contains('/'))
        .map(|p| (p.id.clone(), p.descriptor.clone(), p.at))
        .collect()
}

/// **The editor drew exactly what the game loads.**
///
/// The same ids, the same descriptors, the same positions, in the same order. A difference in
/// any of them is the level the author saw and the level the player walks drifting apart, which
/// is the failure this whole change is about.
#[test]
fn the_editor_draws_exactly_what_the_game_loads() {
    let root = Fixture::new("agree")
        .descriptor("table", "alpha")
        .descriptor("chair", "alpha")
        .composition(
            "break_table",
            &[
                ("table", "table", (0.0, 0.0)),
                ("chair_north", "chair", (0.0, -1.0)),
            ],
        )
        .build("m");

    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }

    // Arm and stamp, through the same calls the click makes.
    {
        let world = app.world_mut();
        world.resource_scope(
            |world, mut project: bevy::prelude::Mut<bevy_cauldron::project::Project>| {
                world.resource_scope(
                    |world, mut open: bevy::prelude::Mut<bevy_cauldron::project::OpenMap>| {
                        let mut state =
                            world.resource_mut::<bevy_cauldron::editor::EditorState>();
                        let mut armed = bevy_cauldron::compose::Armed(Some(
                            "break_table".to_owned(),
                        ));
                        bevy_cauldron::editor::stamp_here_for_test(
                            &mut project,
                            &mut open,
                            &mut state,
                            &mut armed,
                            (2.0, 2.0),
                        );
                    },
                );
            },
        );
    }
    app.update();

    let drawn = editor_rows(&app);
    assert_eq!(drawn.len(), 2, "the group has two members: {drawn:?}");

    // **Save it**, so the game loads the file the editor wrote rather than the editor's memory.
    {
        let world = app.world_mut();
        world.resource_scope(
            |_world, project: bevy::prelude::Mut<bevy_cauldron::project::Project>| {
                _world.resource_scope(
                    |_world, mut open: bevy::prelude::Mut<bevy_cauldron::project::OpenMap>| {
                        open.save(&project).unwrap_or_else(|e| panic!("{e}"));
                    },
                );
            },
        );
    }

    let loaded = consumer_rows(&root, "m");
    assert_eq!(
        drawn, loaded,
        "the editor's rows and the game's rows must be the same rows"
    );
}

/// **The saved file still holds a reference, not the rows.**
///
/// The other half of the claim: the *file* keeps the reference, so editing the group still moves
/// every map that stamped it. A file that held the rows would pass the test above — the game
/// would get the same rows — while losing the reason the reference model was chosen.
#[test]
fn the_saved_file_holds_a_reference_and_not_the_rows() {
    let root = Fixture::new("reference")
        .descriptor("table", "alpha")
        .composition("solo", &[("table", "table", (0.0, 0.0))])
        .build("m");

    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }
    {
        let world = app.world_mut();
        world.resource_scope(
            |world, mut project: bevy::prelude::Mut<bevy_cauldron::project::Project>| {
                world.resource_scope(
                    |world, mut open: bevy::prelude::Mut<bevy_cauldron::project::OpenMap>| {
                        let mut state =
                            world.resource_mut::<bevy_cauldron::editor::EditorState>();
                        let mut armed =
                            bevy_cauldron::compose::Armed(Some("solo".to_owned()));
                        bevy_cauldron::editor::stamp_here_for_test(
                            &mut project,
                            &mut open,
                            &mut state,
                            &mut armed,
                            (2.0, 2.0),
                        );
                    },
                );
            },
        );
    }
    app.update();
    {
        let world = app.world_mut();
        world.resource_scope(
            |_world, project: bevy::prelude::Mut<bevy_cauldron::project::Project>| {
                _world.resource_scope(
                    |_world, mut open: bevy::prelude::Mut<bevy_cauldron::project::OpenMap>| {
                        open.save(&project).unwrap_or_else(|e| panic!("{e}"));
                    },
                );
            },
        );
    }

    let text = std::fs::read_to_string(root.join("assets/grimoire/maps/m.map.ron"))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        text.contains("stamps:"),
        "the saved file holds a reference: {text}"
    );
    assert!(
        !text.contains("solo/table"),
        "the saved file must not hold the rows a stamp stands for: {text}"
    );
}

/// **The editor's panel counts the rows the game will have.**
///
/// The number an author reads beside "placed" is the number the game spawns. Before this, the
/// panel counted the hand-authored rows and the game had more — so the panel understated the
/// level.
#[test]
fn the_editor_counts_the_rows_the_game_will_have() {
    let root = Fixture::new("count")
        .descriptor("table", "alpha")
        .descriptor("chair", "alpha")
        .composition(
            "break_table",
            &[
                ("table", "table", (0.0, 0.0)),
                ("chair_north", "chair", (0.0, -1.0)),
            ],
        )
        .build("m");

    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }
    {
        let world = app.world_mut();
        world.resource_scope(
            |world, mut project: bevy::prelude::Mut<bevy_cauldron::project::Project>| {
                world.resource_scope(
                    |world, mut open: bevy::prelude::Mut<bevy_cauldron::project::OpenMap>| {
                        let mut state =
                            world.resource_mut::<bevy_cauldron::editor::EditorState>();
                        let mut armed = bevy_cauldron::compose::Armed(Some(
                            "break_table".to_owned(),
                        ));
                        bevy_cauldron::editor::stamp_here_for_test(
                            &mut project,
                            &mut open,
                            &mut state,
                            &mut armed,
                            (2.0, 2.0),
                        );
                    },
                );
            },
        );
    }
    app.update();
    {
        let world = app.world_mut();
        world.resource_scope(
            |_world, project: bevy::prelude::Mut<bevy_cauldron::project::Project>| {
                _world.resource_scope(
                    |_world, mut open: bevy::prelude::Mut<bevy_cauldron::project::OpenMap>| {
                        open.save(&project).unwrap_or_else(|e| panic!("{e}"));
                    },
                );
            },
        );
    }

    // The consumer's count.
    let data =
        bevy_grimoire_core::project_data::ProjectData::load(&root.join("assets/grimoire"), &["m"])
            .unwrap_or_else(|e| panic!("{e}"));
    let map = data.maps.get("m").expect("the map loaded");

    // The editor's count, through the one counter.
    let open = app
        .world()
        .resource::<bevy_cauldron::project::OpenMap>();
    let project = app
        .world()
        .resource::<bevy_cauldron::project::Project>();
    let counted = bevy_grimoire_core::census::of_map(
        &open.map,
        &project.compositions.compositions,
        &project.library,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(
        counted.total(),
        map.placements.len(),
        "the editor's count and the consumer's count must agree"
    );
    assert_eq!(counted.stamped, 2, "the group brought two rows");
}
