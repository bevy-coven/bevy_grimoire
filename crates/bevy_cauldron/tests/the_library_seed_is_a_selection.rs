//! **Arriving on the Meshes shelf is a selection, so it goes through `piece_select` (E-050).**
//!
//! `right` and a click on the `MESHES` chip both point the shelf at a piece. They wrote
//! `selected_library_id` by hand while the arrow one press later dispatched the operation, so the
//! operation log recorded a walk and not the arrival — and an agent watching the log could not see
//! the shelf pointed itself at anything until someone pressed an arrow. These tests read the log on
//! disk, which is what an agent reads.
//!
//! The Tiles tab keeps the direct write: `piece_select` names the Meshes shelf, and on that tab the
//! list is a tile's source, which no operation names yet. That half is asserted too, so a later
//! migration cannot quietly log a Meshes selection for a tile's source.

use bevy::input::{ButtonInput, InputSystems};
use bevy::picking::events::{Pointer, PointerClick};
use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{ImportState, Mode, Shelf, ShelfChip};

use crate::fixtures;

fn editor_at(name: &str, mode: Mode) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("oldest", "alpha")
        .descriptor("newest", "alpha")
        .build("m");
    let mut app =
        harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn selected(app: &App) -> Option<String> {
    app.world().resource::<ImportState>().selected_library_id.clone()
}

/// **The `piece_select` lines in the operation log**, counted from disk — the same file an agent
/// reads, so a line the editor kept in memory does not count.
fn selections(app: &App) -> Vec<serde_json::Value> {
    let log = app
        .world()
        .resource::<Project>()
        .project_dir
        .join("oplog.jsonl");
    std::fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["op"] == "piece_select")
        .collect()
}

fn press_right_once(mut keys: ResMut<ButtonInput<KeyCode>>, mut done: Local<bool>) {
    if !*done {
        keys.press(KeyCode::ArrowRight);
        *done = true;
    }
}

/// **`right`, injected where the other key tests inject it**: in `PreUpdate` after `InputSystems`,
/// because that is what clears the keys each frame.
fn press_right(app: &mut App) {
    app.add_systems(PreUpdate, press_right_once.after(InputSystems));
    app.update();
    app.update();
}

/// **`right` with nothing selected lands on a row AND says so in the log.**
#[test]
fn the_library_focus_key_selects_through_the_operation() {
    let mut app = editor_at("seed-key", Mode::Meshes);
    press_right(&mut app);

    assert_eq!(
        selected(&app).as_deref(),
        Some("newest"),
        "`right` seeds the shelf on the newest piece, as it did before the seed became an operation"
    );
    let lines = selections(&app);
    assert_eq!(
        lines.len(),
        1,
        "arriving on the shelf is one selection, so it is one `piece_select` line: {lines:?}"
    );
    assert_eq!(lines[0]["source"], "human", "a key press is the person's: {:?}", lines[0]);
    assert_eq!(lines[0]["params"]["id"], "newest", "{:?}", lines[0]);
}

/// **The `MESHES` chip and the key are one press**, so a click leaves the same line.
#[test]
fn the_library_shelf_chip_selects_through_the_operation() {
    let mut app = editor_at("seed-chip", Mode::Meshes);
    let chip = {
        let world = app.world_mut();
        let mut chips = world.query::<(Entity, &ShelfChip)>();
        chips
            .iter(world)
            .find(|(_, c)| c.0 == Shelf::Library)
            .map(|(entity, _)| entity)
            .expect("the Meshes shelf strip draws a MESHES chip")
    };
    use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
    use bevy::picking::backend::HitData;
    use bevy::picking::pointer::{Location, PointerButton, PointerId};
    app.world_mut().trigger(PointerClick {
        entity: chip,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Image(ImageRenderTarget {
                    handle: Default::default(),
                    scale_factor: 1.0,
                }),
                position: Vec2::ZERO,
            },
        ),
        button: PointerButton::Primary,
        hit: HitData {
            camera: Entity::PLACEHOLDER,
            depth: 0.0,
            position: None,
            normal: None,
            extra: None,
        },
        duration: std::time::Duration::ZERO,
        count: 1,
    });
    app.update();
    app.update();

    assert_eq!(
        selected(&app).as_deref(),
        Some("newest"),
        "the chip lands where the key lands"
    );
    let lines = selections(&app);
    assert_eq!(
        lines.len(),
        1,
        "a chip click is the same selection as the key, so it is one `piece_select` line: {lines:?}"
    );
    assert_eq!(lines[0]["params"]["id"], "newest", "{:?}", lines[0]);
}

/// **On the Tiles tab the seed stays a direct write**, and the log gains nothing.
#[test]
fn the_tiles_tab_seed_is_not_logged_as_a_meshes_selection() {
    let mut app = editor_at("seed-tiles", Mode::Tiles);
    press_right(&mut app);

    assert!(
        selected(&app).is_some(),
        "the Tiles tab still seeds the list the arrows walk"
    );
    let lines = selections(&app);
    assert!(
        lines.is_empty(),
        "`piece_select` names the Meshes shelf, so a tile source must not be logged as one: \
         {lines:?}"
    );
}
