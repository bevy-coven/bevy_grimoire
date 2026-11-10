//! **The two ways a room's consumers disagreed about one frame** (GD-07 review, #369).
//!
//! The first half: pass 183 made **one** volume write one signal per frame, so a `Switch` and a `StateMachine`
//! agreed about a crossing. It did not make two *producers* agree: a tripwire and a bell that both name one signal
//! write it twice in one frame, the switch flips once (it dedupes with `contains`) and the machine steps twice. That
//! is the same disagreement one level up, and it is what the consumer side has to settle.
//!
//! The second half: a body that is **already inside** a volume when the volume first sees it — the shape of a save
//! that puts someone in a doorway — was taken as a crossing, so a reload flipped its switch. A crossing is a
//! transition nobody observed, not a state that was simply true on the first frame.
//!
//! Every test writes its components in `<root>/game/main.entities.yaml`, which is the acceptance's own door: a
//! behaviour is a reflected component a file sets, and nothing here is built in Rust except the position a body
//! stands at.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_grimoire::behaviours::{
    BehaviourPlugin, Countdown, Door, Signal, StateMachine, Switch, Transitioned, Triggered,
};
use bevy_grimoire::entities::{self, GameEntities, GameEntitiesPlugin};

/// **A scratch project of this test's own**: its name and the process id.
fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(entities::ENTITIES_DIR)).expect("the entity folder is made");
    dir
}

fn write(root: &Path, text: &str) {
    let path = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    std::fs::write(path, text).expect("the entity file is written");
}

/// **A headless game with the behaviours on and the YAML reader beside them.**
fn game(root: &Path) -> App {
    let mut app = App::new();
    app.init_resource::<Time>();
    app.add_plugins(bevy::transform::TransformPlugin)
        .add_plugins(BehaviourPlugin)
        .add_plugins(GameEntitiesPlugin { root: root.to_path_buf() });
    app
}

/// **One frame**: the clock moves a second, so a countdown of a second has rung in its first frame.
fn frame(app: &mut App) {
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
    app.update();
    app.finish();
    app.cleanup();
}

/// **The entity a file's `name` stands for**, through the index `GameEntitiesPlugin` keeps.
fn entity(app: &App, root: &Path, name: &str) -> Entity {
    let key = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    app.world()
        .resource::<GameEntities>()
        .spawned
        .get(&key.display().to_string())
        .and_then(|by_name| by_name.get(name).copied())
        .unwrap_or_else(|| panic!("`{name}` was spawned for {}", key.display()))
}

/// **The messages of one type this frame**, drained so a later frame's count is its own.
fn sent<M: Message + Clone>(app: &mut App) -> Vec<M> {
    app.world_mut().resource_mut::<Messages<M>>().drain().collect()
}

fn at(x: f32, y: f32, z: f32) -> String {
    format!("{{ translation: [{x}, {y}, {z}], rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0, 1.0, 1.0] }}")
}

/// **Two producers naming one signal in one frame is one event to every consumer.**
///
/// A tripwire a body walks into and a bell that rings on the same frame both write `Signal { name: "crowd" }`. The
/// machine's two steps are legal on that one signal, so a second signal is visible as a second step: one signal is
/// `calm → alarmed`, a second would take it on to `panicked`. The switch flips once either way — it is the machine
/// that used to disagree, and it is the machine this test is about.
#[test]
fn two_producers_naming_one_signal_in_one_frame_move_a_machine_once() {
    let root = project("behaviour_one_signal_two_producers");
    write(
        &root,
        &format!(
            "entities:\n  \
               latch:\n    Switch: {{ latch: crowd, on: false }}\n  \
               gate:\n    Door: {{ link: crowd, open: false }}\n  \
               guard:\n    StateMachine:\n      initial: calm\n      state: calm\n      transitions:\n        - {{ from: calm, on: crowd, to: alarmed }}\n        - {{ from: alarmed, on: crowd, to: panicked }}\n  \
               tripwire:\n    Trigger: {{ radius_m: 1.5, signal: crowd }}\n  \
               bell:\n    Countdown: {{ duration_s: 1.0, signal: crowd }}\n  \
               walker:\n    Body: {{}}\n    Transform: {}\n",
            at(10.0, 0.0, 0.0)
        ),
    );
    let mut app = game(&root);
    frame(&mut app);
    let walker = entity(&app, &root, "walker");
    let latch = entity(&app, &root, "latch");
    let guard = entity(&app, &root, "guard");
    let gate = entity(&app, &root, "gate");
    // **The bell rang in the first frame**, so its signal is spent; the frame this test measures is the one where
    // the body walks in and the bell is still what it rang.
    // **Drained, so the next count is this frame's**: the bell's signal and the one step it caused are spent.
    let _ = sent::<Signal>(&mut app);
    let _ = sent::<Transitioned>(&mut app);
    assert_eq!(app.world().get::<Switch>(latch).map(|s| s.on), Some(true), "the bell alone pressed the switch");
    assert_eq!(
        app.world().get::<StateMachine>(guard).map(|m| m.state.as_str()),
        Some("alarmed"),
        "and stepped the machine once"
    );

    // **Both write in one frame**: the walker crosses the volume, and a second bell rings on the same frame.
    app.world_mut().spawn(Countdown { duration_s: 0.0, signal: "crowd".to_owned(), elapsed_s: 0.0, finished: false });
    app.world_mut().get_mut::<Transform>(walker).expect("a body has a position").translation = Vec3::ZERO;
    frame(&mut app);

    // **One signal, so one flip and one step.** A second flip would be off and a second step would be `panicked`.
    assert_eq!(app.world().get::<Switch>(latch).map(|s| s.on), Some(false), "one crossing, one flip");
    assert_eq!(
        app.world().get::<StateMachine>(guard).map(|m| m.state.as_str()),
        Some("panicked"),
        "one signal, so one step from `alarmed` — not two"
    );
    let steps = sent::<Transitioned>(&mut app);
    assert_eq!(steps.len(), 1, "one step was reported, not one per producer: {steps:?}");
    assert_eq!(app.world().get::<Door>(gate).map(|d| d.open), Some(false), "and the door follows its switch");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A body already inside a volume when it is first seen is not a crossing**, so a save that puts someone in a
/// doorway does not press the switch it stands in.
///
/// The save is the shape the issue names: the game is started with the body written **inside** the volume's radius.
/// Nothing has moved, so nothing has happened, and a switch that flips on a load is a door that opens by itself when
/// a project reopens.
#[test]
fn a_body_saved_inside_a_volume_does_not_press_its_switch() {
    let root = project("behaviour_body_saved_inside");
    write(
        &root,
        &format!(
            "entities:\n  \
               latch:\n    Switch: {{ latch: gate_crossed, on: false }}\n  \
               gate:\n    Door: {{ link: gate_crossed, open: false }}\n  \
               tripwire:\n    Trigger: {{ radius_m: 2.0, signal: gate_crossed }}\n  \
               walker:\n    Body: {{}}\n    Transform: {}\n",
            at(0.5, 0.0, 0.0)
        ),
    );
    let mut app = game(&root);
    frame(&mut app);
    let walker = entity(&app, &root, "walker");
    let latch = entity(&app, &root, "latch");
    let gate = entity(&app, &root, "gate");

    // **Nothing was crossed**, so nothing was reported and nothing was pressed.
    assert!(sent::<Triggered>(&mut app).is_empty(), "a body that was always there did not enter");
    assert_eq!(app.world().get::<Switch>(latch).map(|s| s.on), Some(false), "the switch was not pressed");
    assert_eq!(app.world().get::<Door>(gate).map(|d| d.open), Some(false), "and the gate is still shut");

    // **A frame later it is still there**, and still nothing has happened: a state is not an event.
    frame(&mut app);
    assert!(sent::<Triggered>(&mut app).is_empty(), "standing still sends nothing");
    assert_eq!(app.world().get::<Switch>(latch).map(|s| s.on), Some(false), "and the switch is still off");

    // **Leaving is a crossing**, and it is reported: the way out is what a listener may want to know.
    app.world_mut().get_mut::<Transform>(walker).expect("a body has a position").translation = Vec3::new(10.0, 0.0, 0.0);
    frame(&mut app);
    let left = sent::<Triggered>(&mut app);
    assert_eq!(left.len(), 1, "leaving is a crossing: {left:?}");
    assert!(!left[0].entered, "and it says it was the way out");
    // **The signal is the way in only**, so leaving wrote none and the switch is still off.
    assert_eq!(app.world().get::<Switch>(latch).map(|s| s.on), Some(false), "leaving wrote no signal");

    // **Walking back in is the first crossing this volume ever saw**, and it counts.
    app.world_mut().get_mut::<Transform>(walker).expect("a body has a position").translation = Vec3::ZERO;
    frame(&mut app);
    assert_eq!(app.world().get::<Switch>(latch).map(|s| s.on), Some(true), "walking in after leaving presses the switch");
    assert_eq!(app.world().get::<Door>(gate).map(|d| d.open), Some(true), "and opens the gate");

    let _ = std::fs::remove_dir_all(&root);
}
