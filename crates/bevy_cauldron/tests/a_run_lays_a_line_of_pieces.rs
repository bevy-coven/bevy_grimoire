//! **A run lays a line of pieces at the piece's own pitch** — and the game gets them.
//!
//! # The claim
//!
//! Three things, and the third is the one that matters:
//!
//! 1. **The pitch is the piece's**, not the project's lattice. A 2.064 m fence post repeats every
//!    2.064 m, and *not* on any of the lattice's rungs — which is exactly why a fill could not
//!    draw the fence.
//! 2. **The run is one act**: one undo takes the whole line back.
//! 3. **The game gets the rows.** A run's posts are ordinary placements, so the loader needs no
//!    change and the count in the inspector includes them.
//!
//! The arithmetic itself is tested in `bevy_grimoire_core` (`src/run.rs` and
//! `tests/a_run_is_laid_in_the_piece_s_own_pitch.rs`); what is here is the editor's half — that the
//! commit lays what the run says, that it is one act, and that the rows are ordinary rows.

use crate::fixtures;

use fixtures::Fixture;

/// A piece 2.064 by 0.12 m — Sanctuary's `prop_wooden_fence_single`, the shape the town map's 282
/// fence posts were placed by hand.
const FENCE_W: f32 = 2.064;
const FENCE_D: f32 = 0.12;

fn fence_project(tag: &str) -> std::path::PathBuf {
    Fixture::new(tag)
        .sized_descriptor("fence", "alpha", FENCE_W, FENCE_D)
        .build("m")
}

/// **The run lays the piece at the piece's own pitch.**
///
/// The measurement that decided the feature: the lattice's rungs are 1.000, 0.333 and 0.111 m,
/// so a fill lays a 2.064 m fence 3.000, 2.333 or 2.111 m apart. The run lays it 2.064 m apart.
#[test]
fn a_run_lays_the_piece_at_the_pieces_own_pitch() {
    let root = fence_project("run_pitch");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }

    bevy_cauldron::editor::run_for_test(app.world_mut(), "fence", (0.0, 0.0), (8.0, 0.0));
    app.update();

    let open = app
        .world()
        .resource::<bevy_cauldron::project::OpenMap>();
    let rows: Vec<&bevy_grimoire_core::map::Placed> = open
        .map
        .placements
        .iter()
        .filter(|p| p.descriptor == "fence")
        .collect();
    assert!(!rows.is_empty(), "the run laid nothing: {rows:?}");

    // **Every gap between neighbours is the piece's own width.** The run ends where the pitch puts
    // it — an 8 m run at 2.064 m stands four posts and stops a pitch short, because the next one
    // would be past the end. A run with one short gap in it is a fence with a hole in it.
    for pair in rows.windows(2) {
        let gap = (pair[1].at.0 - pair[0].at.0).abs();
        assert!(
            (gap - FENCE_W).abs() < 1e-4,
            "the posts are {gap} apart, and the piece is {FENCE_W} wide"
        );
    }
    // **And not on a lattice rung.** 1.000, 0.333 and 0.111 are the project's three. The first
    // post is at the origin, which is on every rung — so the claim is about the *pitch*, which the
    // second post shows.
    for rung in [1.0f32, 1.0 / 3.0, 1.0 / 9.0] {
        let second = rows[1].at.0;
        assert!(
            (second / rung).fract().abs() > 1e-3,
            "the run landed on the {rung} rung, which is the grid the fill uses"
        );
    }
}

/// **The run is one act: one undo takes the whole line back.**
///
/// The rule the fill and the box already follow — one act the author performed is one act to take
/// back — and the reason a 40-post run is not forty undos.
#[test]
fn one_undo_takes_the_whole_run_back() {
    let root = fence_project("run_undo");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }

    let before = app
        .world()
        .resource::<bevy_cauldron::project::OpenMap>()
        .map
        .placements
        .len();
    bevy_cauldron::editor::run_for_test(app.world_mut(), "fence", (0.0, 0.0), (8.0, 0.0));
    app.update();
    let after = app
        .world()
        .resource::<bevy_cauldron::project::OpenMap>()
        .map
        .placements
        .len();
    assert!(after > before, "the run laid nothing");

    bevy_cauldron::editor::undo_for_test(app.world_mut());
    app.update();
    assert_eq!(
        app.world()
            .resource::<bevy_cauldron::project::OpenMap>()
            .map
            .placements
            .len(),
        before,
        "one undo must take the whole run back"
    );

    // **And redo puts it back**, so the run is not a one-way door.
    bevy_cauldron::editor::redo_for_test(app.world_mut());
    app.update();
    assert_eq!(
        app.world()
            .resource::<bevy_cauldron::project::OpenMap>()
            .map
            .placements
            .len(),
        after,
        "redo must put the whole run back"
    );
}

/// **The run's rows are ordinary rows, so the game gets them.**
///
/// A run's posts are `Placed` rows like any other: the map file gains rows and nothing else, the
/// game's loader needs no change, and the inspector's count already includes them. This is the
/// previous iteration's contract — a level the author drew is the level the game walks — and a
/// run that broke it would put the fence back where it started.
#[test]
fn the_runs_rows_are_ordinary_rows_the_consumer_gets() {
    let root = fence_project("run_consumer");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }

    bevy_cauldron::editor::run_for_test(app.world_mut(), "fence", (0.0, 0.0), (8.0, 0.0));
    app.update();

    // Save it, so the consumer reads the file the editor wrote.
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

    let data =
        bevy_grimoire_core::project_data::ProjectData::load(&root.join("assets/grimoire"), &["m"])
            .unwrap_or_else(|e| panic!("{e}"));
    let map = data.maps.get("m").expect("the map loaded");
    let fences = map
        .placements
        .iter()
        .filter(|p| p.descriptor == "fence")
        .count();
    assert!(
        fences > 1,
        "the consumer must have the run's rows, and it has {fences}"
    );

    // **And the count the inspector shows is the count the consumer has.**
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
}

/// **The palette takes the keyboard.**
///
/// Measured on Sanctuary's project before this change: the Map tab's PLACE filter was
/// **mouse-only** — `FocusFilter` focused the Tiles tab's candidate list — so reaching one row in a
/// 211-descriptor palette took **~90 keystrokes** of `Shift+Down`, and the walk *wrapped* into the
/// wrong kit. And a click that missed the box fell through to the map and **replaced a house block
/// with a barrel**.
///
/// A piece the author cannot reach by keyboard is a piece they will mis-click, and the run's pitch
/// is the piece's — so finding the piece is part of drawing the run.
#[test]
fn the_map_tabs_palette_filter_takes_the_keyboard() {
    use bevy::input::keyboard::{Key, KeyCode};
    use bevy_cauldron::filter::{Filters, Pane};
    use bevy_cauldron::keys::MOD_KEYS;

    let root = fence_project("filter");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }

    // **Press the filter chord** (`Cmd+F` on macOS, `Ctrl+F` elsewhere), the way a field really receives it — a press and a release written to the
    // `KeyboardInput` message stream, then a frame. `ButtonInput` would not do: every text handler
    // in this crate reads the stream and matches `logical_key`.
    //
    // The command modifier is `MOD_KEYS[0]`: Cmd on a Mac, Ctrl everywhere else. It has to be the chord
    // this platform's editor reads, or the press is a bare key and the test proves nothing about the
    // filter. It is held in `ButtonInput<KeyCode>`, which the key census reads; the key itself arrives
    // through the message stream.
    app.world_mut()
        .resource_mut::<bevy::input::ButtonInput<KeyCode>>()
        .press(MOD_KEYS[0]);
    for state in [
        bevy::input::ButtonState::Pressed,
        bevy::input::ButtonState::Released,
    ] {
        app.world_mut()
            .write_message(bevy::input::keyboard::KeyboardInput {
                key_code: KeyCode::KeyF,
                logical_key: Key::Character("f".into()),
                state,
                text: None,
                repeat: false,
                window: bevy::ecs::entity::Entity::PLACEHOLDER,
            });
    }
    app.update();
    app.world_mut()
        .resource_mut::<bevy::input::ButtonInput<KeyCode>>()
        .release(MOD_KEYS[0]);
    app.update();

    // **The Map tab's filter is the palette's box.** `FocusFilter` has to focus *this* pane, or the
    // author types into a box that does not narrow the list they are looking at.
    let filters = app.world().resource::<Filters>();
    assert_eq!(
        filters.focus_pane(),
        Some(Pane::Palette),
        "the Map tab's filter must be the palette's box"
    );

    // **And the text lands there** — the whole point, since a box that takes the keys but not the
    // letters is a box that swallows them.
    for ch in "fence".chars() {
        app.world_mut()
            .write_message(bevy::input::keyboard::KeyboardInput {
                key_code: KeyCode::Unidentified(bevy::input::keyboard::NativeKeyCode::Unidentified),
                logical_key: Key::Character(ch.to_string().into()),
                state: bevy::input::ButtonState::Pressed,
                text: Some(ch.to_string().into()),
                repeat: false,
                window: bevy::ecs::entity::Entity::PLACEHOLDER,
            });
    }
    app.update();

    let filters = app.world().resource::<Filters>();
    assert_eq!(
        filters.text(Pane::Palette),
        "fence",
        "the text must land in the palette's box"
    );
    assert_eq!(
        filters.text(Pane::Candidates),
        "",
        "and not in the tiles tab's, which is a list the Map tab does not show"
    );
}
