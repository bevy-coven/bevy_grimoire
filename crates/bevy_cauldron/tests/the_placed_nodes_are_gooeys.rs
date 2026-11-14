//! **A report's layers are gooey's placed nodes** (#260, UI-99, round 6: bevy_gooey G-84/G-85).
//!
//! The report's frozen-frame root, its frozen image, its marks and its hint were absolute `Node` literals. Each is gooey's `placed` / `cover` now;
//! this file asks an open report what is on screen: a root that covers the window and lets the pointer through, marks of the size they were drawn at.

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::harness;
use bevy_cauldron::keys::MOD_KEYS;
use bevy_cauldron::report::ReportOpen;
use bevy_gooey::{PlacedAt, Role};

use crate::fixtures;

#[derive(Resource, Default)]
struct Script {
    chord: bool,
    held: Vec<KeyCode>,
}

fn drive(mut script: ResMut<Script>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    for key in script.held.drain(..) {
        keys.release(key);
    }
    if std::mem::take(&mut script.chord) {
        let chord = vec![MOD_KEYS[0], KeyCode::KeyD];
        for key in &chord {
            keys.press(*key);
        }
        script.held = chord;
    }
}

/// **A report open on a map**: the platform's modifier and `D`, pressed in one frame.
#[test]
fn an_open_reports_root_covers_the_window_and_lets_the_pointer_through() {
    let root = fixtures::Fixture::new(&format!("placed_report_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    app.world_mut().resource_mut::<Script>().chord = true;
    for _ in 0..4 {
        app.update();
    }
    let report = app.world_mut().query_filtered::<Entity, With<ReportOpen>>().single(app.world()).expect("Cmd+D opened a report");
    assert_eq!(app.world().get::<Role>(report), Some(&Role::Placed), "the report's root is a gooey placed node, not a raw absolute `Node`");
    assert_eq!(app.world().get::<PlacedAt>(report), Some(&PlacedAt(UiRect::all(Val::ZERO))), "it covers the window");
    assert_eq!(app.world().get::<Pickable>(report), Some(&Pickable::IGNORE), "and the interface under it still takes the pointer's hover until the report asks for it");
    let node = app.world().get::<Node>(report).expect("the root has a Node");
    assert_eq!(node.position_type, PositionType::Absolute, "it takes no room in any layout");
    assert_eq!((node.left, node.top, node.right, node.bottom), (Val::ZERO, Val::ZERO, Val::ZERO, Val::ZERO), "pinned at zero on every side");
}
