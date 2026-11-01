//! **Every system in `Update` reads this frame's pointer, whatever its order** (#180).
//!
//! `view::sense_pointer` wrote `Pointer` in `keys::Phase::Sense`, and most of its readers (the Note
//! tool, the placement tools, their ghosts) are in no set, so Bevy was free to run a reader first:
//! the click then landed where the pointer had been a frame earlier. Measured in
//! `a_person_shift_clicks_two_pieces_into_one_note`, which failed 3 of 24 runs beside the other note
//! tests: its second Shift-click read the first click's pixel and took the first piece back out of
//! the note.
//!
//! The probe is ordered *before* `Phase::Sense`, the worst place an unordered reader can land, so the
//! race is not left to the executor to lose one run in eight.

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::harness;
use bevy_cauldron::keys;
use bevy_cauldron::view::Pointer;

use crate::fixtures;

/// Where the person puts the cursor this frame.
#[derive(Resource, Default)]
struct Aim(Option<Vec2>);

/// What the probe read, one entry per frame.
#[derive(Resource, Default)]
struct Seen(Vec<Option<Vec2>>);

/// The window's cursor moves in `PreUpdate`, as a person's mouse does before the frame's systems.
fn aim(aim: Res<Aim>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(aim.0);
    }
}

fn probe(pointer: Res<Pointer>, mut seen: ResMut<Seen>) {
    seen.0.push(pointer.0);
}

#[test]
fn a_reader_ordered_before_the_sense_phase_reads_this_frames_pointer() {
    let root = fixtures::Fixture::new("pointer_read_first")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Aim>().init_resource::<Seen>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, aim.after(InputSystems));
    app.add_systems(Update, probe.before(keys::Phase::Sense));
    for _ in 0..3 {
        app.update();
    }

    for at in [Vec2::new(100.0, 200.0), Vec2::new(640.0, 360.0), Vec2::new(5.0, 7.0)] {
        app.world_mut().resource_mut::<Aim>().0 = Some(at);
        app.world_mut().resource_mut::<Seen>().0.clear();
        app.update();
        assert_eq!(
            app.world().resource::<Seen>().0,
            [Some(at)],
            "a system in Update read the pointer of the frame before: a click there lands where the cursor was"
        );
    }
}
