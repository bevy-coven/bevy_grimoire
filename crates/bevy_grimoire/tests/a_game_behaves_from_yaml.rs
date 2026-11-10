//! **A game's behaviours are components, and a file sets them** (GD-07, #369).
//!
//! Each behaviour below is a reflected component `bevy_grimoire::behaviours` ships, so a game gets it by
//! adding `BehaviourPlugin` and a person or an agent configures it in
//! `<root>/game/<name>.entities.yaml` — the reader GD-05 built. Every test here writes the component in a
//! file and then watches it *act*: a door opening, a pickup going, a state machine stepping. Nothing is
//! built in Rust except the position a body stands at, which the file writes too.
//!
//! **The world is stepped by hand**: `Time` advances a second per frame, so a countdown of two seconds is
//! two frames and a path follow at one metre a second is one metre a frame. A behaviour that needs a clock
//! takes `Option<Res<Time>>`, so a game without one ticks nothing rather than panicking (the rule
//! `entities.rs` states for its own reload).
//!
//! **The frame a file is read is the frame its behaviours first act.** `GameEntitiesPlugin` queues its reads
//! from `Startup`, Bevy flushes that queue before `Update` runs, and the behaviour systems are in `Update` —
//! so a one-second countdown written in a file has already rung when its first frame ends. Every count below
//! is that first frame plus the ones after it.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_grimoire::behaviours::{
    BehaviourPlugin, Body, Collector, Countdown, CountdownElapsed, Damage, Damaged, Died, Door, Health, PathFollow,
    PickedUp, Pickup, Signal, SpawnRequested, Spawner, StateMachine, Switch, Transitioned, Trigger, Triggered,
    WaypointReached, behaviours_named,
};
use bevy_grimoire::entities::{self, GameEntities, GameEntitiesPlugin};

/// **A scratch project of this test's own**: its name and the process id, which the tests that run beside it
/// cannot collide with.
fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(entities::ENTITIES_DIR)).expect("the entity folder is made");
    dir
}

/// Write this test's entity file.
fn write(root: &Path, text: &str) {
    let path = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    std::fs::write(path, text).expect("the entity file is written");
}

/// **A headless game with the behaviours on and the YAML reader beside them.**
///
/// The order matters: the reader resolves a file's type paths through the registry, so the behaviours must be
/// registered first or a file naming one is refused as a type nobody registered.
fn game(root: &Path) -> App {
    let mut app = App::new();
    app.init_resource::<Time>();
    app.add_plugins(BehaviourPlugin).add_plugins(GameEntitiesPlugin { root: root.to_path_buf() });
    app
}

/// **One frame**: the clock moves a second, `update` runs the schedules, and the queued reads reach the
/// world at `finish`/`cleanup`.
fn frame(app: &mut App) {
    let mut time = app.world_mut().resource_mut::<Time>();
    time.advance_by(std::time::Duration::from_secs(1));
    app.update();
    app.finish();
    app.cleanup();
}

/// **The entity a file's `name` stands for**, through the same index `GameEntitiesPlugin` keeps.
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

/// **A `Transform` a file writes**, so a test says where a body stands rather than taking the default.
fn at(x: f32, y: f32, z: f32) -> String {
    format!("{{ translation: [{x}, {y}, {z}], rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0, 1.0, 1.0] }}")
}

/// **The acceptance's first example**: crossing a `Trigger`'s volume sends its event, once on the way in and
/// once on the way out, and says which of the two it was.
#[test]
fn crossing_a_trigger_sends_its_event_and_then_the_one_for_leaving() {
    let root = project("behaviour_trigger");
    // The volume sits at the origin with a metre and a half of radius; the body starts ten metres away and is
    // moved in, then out again, so nothing here depends on a physics engine.
    write(
        &root,
        &format!(
            "entities:\n  tripwire:\n    Trigger: {{ radius_m: 1.5, signal: gate }}\n  walker:\n    Body: {{}}\n    Transform: {}\n",
            at(10.0, 0.0, 0.0)
        ),
    );
    let mut app = game(&root);
    frame(&mut app);
    let tripwire = entity(&app, &root, "tripwire");
    let walker = entity(&app, &root, "walker");
    assert!(sent::<Triggered>(&mut app).is_empty(), "ten metres away is not inside a metre and a half");

    // In: the walker stands on the volume's own position.
    app.world_mut().get_mut::<Transform>(walker).expect("a body has a position").translation = Vec3::ZERO;
    frame(&mut app);
    let entered = sent::<Triggered>(&mut app);
    assert_eq!(entered.len(), 1, "one crossing, one event: {entered:?}");
    assert_eq!(entered[0].trigger, tripwire, "the event names the volume");
    assert_eq!(entered[0].body, walker, "and who crossed it");
    assert!(entered[0].entered, "this one is the way in");
    assert_eq!(entered[0].signal, "gate", "and carries the volume's own signal name");

    // Out: far enough away that the volume no longer holds it.
    app.world_mut().get_mut::<Transform>(walker).expect("a body has a position").translation = Vec3::new(10.0, 0.0, 0.0);
    frame(&mut app);
    let left = sent::<Triggered>(&mut app);
    assert_eq!(left.len(), 1, "leaving is one event of its own: {left:?}");
    assert!(!left[0].entered, "and says it was the way out");
    assert_eq!(left[0].signal, "gate");

    // Standing still sends nothing more: an event is a crossing, not a state.
    frame(&mut app);
    assert!(sent::<Triggered>(&mut app).is_empty(), "no crossing, no event");

    let _ = std::fs::remove_dir_all(&root);
}

/// **The acceptance's second example**: a `Door` linked by name to a `Switch` opens when the switch is
/// pressed, and closes again when it is pressed back.
#[test]
fn a_door_linked_to_a_switch_opens_when_the_switch_is_pressed() {
    let root = project("behaviour_door");
    write(&root, "entities:\n  latch:\n    Switch: { latch: north, on: false }\n  gate:\n    Door: { link: north, open: false }\n");
    let mut app = game(&root);
    frame(&mut app);
    let latch = entity(&app, &root, "latch");
    let gate = entity(&app, &root, "gate");
    assert_eq!(app.world().get::<Door>(gate).map(|d| d.open), Some(false), "a closed door starts closed");

    // A person presses the switch: one named signal, which is all a game's input has to write.
    app.world_mut().write_message(Signal { name: "north".to_owned() });
    frame(&mut app);
    assert_eq!(app.world().get::<Switch>(latch).map(|s| s.on), Some(true), "the signal turns the switch on");
    assert_eq!(app.world().get::<Door>(gate).map(|d| d.open), Some(true), "and the door linked to it opens");

    // Pressed again: the switch is off and the door follows it back.
    app.world_mut().write_message(Signal { name: "north".to_owned() });
    frame(&mut app);
    assert_eq!(app.world().get::<Switch>(latch).map(|s| s.on), Some(false), "a second press turns it off");
    assert_eq!(app.world().get::<Door>(gate).map(|d| d.open), Some(false), "and the door closes with it");

    // A signal naming another latch moves nothing: the link is by name, not by proximity.
    app.world_mut().write_message(Signal { name: "south".to_owned() });
    frame(&mut app);
    assert_eq!(app.world().get::<Switch>(latch).map(|s| s.on), Some(false), "`south` is not this switch's name");

    let _ = std::fs::remove_dir_all(&root);
}

/// **`Health` and `Damage`**: a file that gives an entity both has the damage taken off on the frame the file
/// is read, the damage component go, and the entity say it died when the last point goes.
#[test]
fn damage_comes_off_health_and_the_last_point_says_it_died() {
    let root = project("behaviour_health");
    write(&root, "entities:\n  bandit:\n    Health: { current: 10, max: 10 }\n    Damage: { amount: 4 }\n");
    let mut app = game(&root);
    frame(&mut app);
    let bandit = entity(&app, &root, "bandit");
    assert_eq!(app.world().get::<Health>(bandit).map(|h| h.current), Some(6), "four points came off ten");
    assert_eq!(app.world().get::<Health>(bandit).map(|h| h.max), Some(10), "and `max` is the file's, untouched");
    assert!(app.world().get::<Damage>(bandit).is_none(), "the damage was taken, so it is gone");
    let taken = sent::<Damaged>(&mut app);
    assert_eq!(taken.len(), 1, "one damage, one report: {taken:?}");
    assert_eq!(taken[0].amount, 4);
    assert!(sent::<Died>(&mut app).is_empty(), "six left is not death");

    // Lethal damage: the entity says so, once, and `current` stops at zero rather than going under it.
    app.world_mut().entity_mut(bandit).insert(Damage { amount: 9 });
    frame(&mut app);
    assert_eq!(app.world().get::<Health>(bandit).map(|h| h.current), Some(0), "health never goes below zero");
    assert_eq!(sent::<Died>(&mut app).len(), 1, "and the entity said it died");

    // A later hit on the dead reports nothing: one death, one event.
    app.world_mut().entity_mut(bandit).insert(Damage { amount: 1 });
    frame(&mut app);
    assert!(sent::<Died>(&mut app).is_empty(), "no second death");
    assert_eq!(app.world().get::<Health>(bandit).map(|h| h.current), Some(0), "the dead stay at zero");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A `Health` a file writes is made legal when it arrives**: `current` above `max` is `max`, and below zero
/// is zero, so a save cannot make an entity unkillable or already dead by accident.
#[test]
fn a_health_a_file_writes_is_clamped_to_its_own_maximum() {
    let root = project("behaviour_health_clamped");
    write(
        &root,
        "entities:\n  giant:\n    Health: { current: 99, max: 12 }\n  ghost:\n    Health: { current: -5, max: 8 }\n",
    );
    let mut app = game(&root);
    frame(&mut app);
    let giant = entity(&app, &root, "giant");
    let ghost = entity(&app, &root, "ghost");
    assert_eq!(app.world().get::<Health>(giant).map(|h| h.current), Some(12), "99 of 12 is 12");
    assert_eq!(app.world().get::<Health>(ghost).map(|h| h.current), Some(0), "-5 is zero");
    // A file's own maximum is never moved by the clamp, so a bar still knows what full is.
    assert_eq!(app.world().get::<Health>(ghost).map(|h| h.max), Some(8));

    let _ = std::fs::remove_dir_all(&root);
}

/// **A `Spawner` asks for its piece every interval it is set to**, naming what to make, and does not make it
/// itself — `spawn_descriptor` is this project's only spawner, and a behaviour that spawned would be the
/// second one.
#[test]
fn a_spawner_asks_for_its_piece_and_never_makes_it_itself() {
    let root = project("behaviour_spawner");
    write(&root, "entities:\n  rat_hole:\n    Spawner: { of: rat, interval_s: 2.0, at_m: [1.0, 0.0, 2.0] }\n");
    let mut app = game(&root);
    // The frame the file is read is one second of the spawner's first two, so it has not asked yet.
    frame(&mut app);
    let hole = entity(&app, &root, "rat_hole");
    assert!(sent::<SpawnRequested>(&mut app).is_empty(), "one second is not the two it waits");

    frame(&mut app);
    let first = sent::<SpawnRequested>(&mut app);
    assert_eq!(first.len(), 1, "two seconds ask for one: {first:?}");
    assert_eq!(first[0].spawner, hole, "the request names its spawner");
    assert_eq!(first[0].of, "rat", "and what to make");
    assert_eq!(first[0].at_m, [1.0, 0.0, 2.0], "and where, in world metres");
    // **Nothing was spawned by the behaviour**: the request is the behaviour's whole output, so the game's
    // one spawner is the only thing that can make a piece appear.
    assert_eq!(app.world().get::<Spawner>(hole).map(|s| s.of.as_str()), Some("rat"), "the spawner is still a spawner");

    frame(&mut app);
    assert!(sent::<SpawnRequested>(&mut app).is_empty(), "the interval starts again after each request");
    frame(&mut app);
    assert_eq!(sent::<SpawnRequested>(&mut app).len(), 1, "and two more seconds ask again");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A `Pickup` is taken by a `Collector` that reaches it**, reports what it was and who took it, and is gone
/// — a picked-up thing is not lying on the floor waiting to be picked up twice.
#[test]
fn a_collector_takes_a_pickup_and_the_pickup_is_gone() {
    let root = project("behaviour_pickup");
    write(
        &root,
        &format!(
            "entities:\n  purse:\n    Pickup: {{ item: gold_coin, radius_m: 0.75 }}\n    Transform: {}\n  hero:\n    Collector: {{}}\n    Transform: {}\n",
            at(2.0, 0.0, 0.0),
            at(0.0, 0.0, 0.0)
        ),
    );
    let mut app = game(&root);
    frame(&mut app);
    let purse = entity(&app, &root, "purse");
    let hero = entity(&app, &root, "hero");
    assert!(sent::<PickedUp>(&mut app).is_empty(), "two metres is more than the purse's radius");

    // In reach.
    app.world_mut().get_mut::<Transform>(hero).expect("a collector stands somewhere").translation = Vec3::new(1.5, 0.0, 0.0);
    frame(&mut app);
    let taken = sent::<PickedUp>(&mut app);
    assert_eq!(taken.len(), 1, "one collector takes it once: {taken:?}");
    assert_eq!(taken[0].item, "gold_coin");
    assert_eq!(taken[0].by, hero);
    assert_eq!(taken[0].pickup, purse);
    assert!(app.world().get_entity(purse).is_err(), "the coin is gone from the floor");

    // Standing on the spot it was does not take it again, because it is not there.
    frame(&mut app);
    assert!(sent::<PickedUp>(&mut app).is_empty(), "a coin is taken once");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A `Countdown` counts its own seconds and says so once**, naming its signal, and stays finished.
#[test]
fn a_countdown_says_when_it_is_done_and_only_once() {
    let root = project("behaviour_countdown");
    write(&root, "entities:\n  bell:\n    Countdown: { duration_s: 2.0, signal: dinner }\n");
    let mut app = game(&root);
    // The read frame is the countdown's first second of two.
    frame(&mut app);
    let bell = entity(&app, &root, "bell");
    assert_eq!(app.world().get::<Countdown>(bell).map(|c| c.finished), Some(false), "it starts unfinished");
    assert_eq!(app.world().get::<Countdown>(bell).map(|c| c.elapsed_s), Some(1.0), "one second of two");
    assert!(sent::<CountdownElapsed>(&mut app).is_empty(), "and silent");

    frame(&mut app);
    let rung = sent::<CountdownElapsed>(&mut app);
    assert_eq!(rung.len(), 1, "two seconds of two: {rung:?}");
    assert_eq!(rung[0].entity, bell);
    assert_eq!(rung[0].signal, "dinner", "the event carries the countdown's own signal name");
    assert_eq!(app.world().get::<Countdown>(bell).map(|c| c.finished), Some(true), "and it is finished");
    assert_eq!(app.world().get::<Countdown>(bell).map(|c| c.elapsed_s), Some(2.0), "the clock stops at the duration");

    frame(&mut app);
    frame(&mut app);
    assert!(sent::<CountdownElapsed>(&mut app).is_empty(), "a finished countdown does not ring again");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A `PathFollow` walks its waypoints at its own speed**, reports each one it reaches, and stops at the
/// last rather than wrapping round to the first.
#[test]
fn a_path_follow_walks_its_waypoints_at_its_speed_and_stops_at_the_last() {
    let root = project("behaviour_path");
    write(
        &root,
        &format!(
            "entities:\n  cart:\n    PathFollow: {{ speed_m_s: 1.0, waypoints_m: [[0.0, 0.0, 2.0], [2.0, 0.0, 2.0]] }}\n    Transform: {}\n",
            at(0.0, 0.0, 0.0)
        ),
    );
    let mut app = game(&root);
    // One frame is one metre at a metre a second, and the read frame is the first of them.
    frame(&mut app);
    let cart = entity(&app, &root, "cart");
    let after_one = app.world().get::<Transform>(cart).expect("a follower moves through a transform").translation;
    assert!((after_one.z - 1.0).abs() < 1e-4, "one second at a metre a second is one metre: {after_one:?}");
    assert!(sent::<WaypointReached>(&mut app).is_empty(), "not at the first waypoint yet");

    // The second second reaches it, and says which one.
    frame(&mut app);
    let reached = sent::<WaypointReached>(&mut app);
    assert_eq!(reached.len(), 1, "the first waypoint is reached once: {reached:?}");
    assert_eq!(reached[0].index, 0, "and the event counts from zero");
    let corner = app.world().get::<Transform>(cart).expect("moved").translation;
    assert!(corner.abs_diff_eq(Vec3::new(0.0, 0.0, 2.0), 1e-4), "it landed on the waypoint, not past it: {corner:?}");

    // It turns toward the second waypoint and keeps going, then stops there for good.
    for _ in 0..4 {
        frame(&mut app);
    }
    let end = app.world().get::<Transform>(cart).expect("moved").translation;
    assert!((end.x - 2.0).abs() < 1e-3, "the last waypoint is where it stops: {end:?}");
    assert_eq!(app.world().get::<PathFollow>(cart).map(|f| f.next), Some(2), "past the last waypoint");
    frame(&mut app);
    assert_eq!(app.world().get::<Transform>(cart).expect("still there").translation, end, "and it stays there");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A `StateMachine` steps on the signals its transitions name**: only from the state it is in, only on its
/// own signal, and each step says where it came from and went to.
#[test]
fn a_state_machine_takes_the_transition_its_signal_names() {
    let root = project("behaviour_machine");
    write(
        &root,
        "entities:\n  guard:\n    StateMachine:\n      initial: asleep\n      state: asleep\n      transitions:\n        - { from: asleep, on: noise, to: awake }\n        - { from: awake, on: quiet, to: asleep }\n",
    );
    let mut app = game(&root);
    frame(&mut app);
    let guard = entity(&app, &root, "guard");
    assert_eq!(app.world().get::<StateMachine>(guard).map(|m| m.state.as_str()), Some("asleep"), "it starts asleep");

    // A signal nothing names moves nothing.
    app.world_mut().write_message(Signal { name: "sneeze".to_owned() });
    frame(&mut app);
    assert_eq!(app.world().get::<StateMachine>(guard).map(|m| m.state.as_str()), Some("asleep"), "`sneeze` is in no transition");
    assert!(sent::<Transitioned>(&mut app).is_empty());

    // The one it does name.
    app.world_mut().write_message(Signal { name: "noise".to_owned() });
    frame(&mut app);
    assert_eq!(app.world().get::<StateMachine>(guard).map(|m| m.state.as_str()), Some("awake"), "`noise` wakes it");
    let stepped = sent::<Transitioned>(&mut app);
    assert_eq!(stepped.len(), 1, "one step, one report: {stepped:?}");
    assert_eq!((stepped[0].from.as_str(), stepped[0].to.as_str()), ("asleep", "awake"));
    assert_eq!(stepped[0].on, "noise", "and names the signal that took it");

    // **A transition is only legal from the state it names**: `noise` again does nothing from `awake`.
    app.world_mut().write_message(Signal { name: "noise".to_owned() });
    frame(&mut app);
    assert_eq!(
        app.world().get::<StateMachine>(guard).map(|m| m.state.as_str()),
        Some("awake"),
        "`noise` from `awake` is in no transition"
    );

    // And back, on the transition `awake` carries.
    app.world_mut().write_message(Signal { name: "quiet".to_owned() });
    frame(&mut app);
    assert_eq!(app.world().get::<StateMachine>(guard).map(|m| m.state.as_str()), Some("asleep"), "`quiet` sends it back");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A room built from these components alone**: a door and its latch, a tripwire, a coin, a guard with a
/// state machine and health, a spawner and a bell — every one written in one file, each reaching the world
/// and acting.
#[test]
fn a_room_built_from_these_components_alone_comes_up_from_one_file() {
    let root = project("behaviour_room");
    write(
        &root,
        &format!(
            "entities:\n  \
               latch:\n    Switch: {{ latch: north, on: false }}\n  \
               gate:\n    Door: {{ link: north, open: false }}\n  \
               tripwire:\n    Trigger: {{ radius_m: 1.0, signal: gate_crossed }}\n    Transform: {tripwire}\n  \
               coin:\n    Pickup: {{ item: gold_coin, radius_m: 0.5 }}\n    Transform: {coin}\n  \
               guard:\n    Health: {{ current: 12, max: 12 }}\n    StateMachine:\n      initial: asleep\n      state: asleep\n      transitions:\n        - {{ from: asleep, on: noise, to: awake }}\n  \
               rat_hole:\n    Spawner: {{ of: rat, interval_s: 3.0, at_m: [-2.0, 0.0, 0.0] }}\n  \
               bell:\n    Countdown: {{ duration_s: 1.0, signal: dinner }}\n  \
               cart:\n    PathFollow: {{ speed_m_s: 0.5, waypoints_m: [[0.0, 0.0, 4.0]] }}\n    Transform: {cart}\n  \
               hero:\n    Body: {{}}\n    Collector: {{}}\n    Transform: {hero}\n",
            tripwire = at(8.0, 0.0, 0.0),
            coin = at(3.0, 0.0, 3.0),
            cart = at(0.0, 0.0, 0.0),
            hero = at(3.0, 0.0, 0.0),
        ),
    );
    let mut app = game(&root);
    frame(&mut app);

    // Every component is on its entity, read from the file — this is the "configurable from YAML" claim.
    assert_eq!(app.world().get::<Switch>(entity(&app, &root, "latch")).map(|s| s.latch.as_str()), Some("north"));
    assert_eq!(app.world().get::<Door>(entity(&app, &root, "gate")).map(|d| d.link.as_str()), Some("north"));
    assert_eq!(app.world().get::<Trigger>(entity(&app, &root, "tripwire")).map(|t| t.radius_m), Some(1.0));
    assert_eq!(app.world().get::<Pickup>(entity(&app, &root, "coin")).map(|p| p.item.as_str()), Some("gold_coin"));
    assert_eq!(app.world().get::<Health>(entity(&app, &root, "guard")).map(|h| h.max), Some(12));
    assert_eq!(
        app.world().get::<StateMachine>(entity(&app, &root, "guard")).map(|m| m.transitions.len()),
        Some(1),
        "the transition list is data the file wrote"
    );
    assert_eq!(app.world().get::<Spawner>(entity(&app, &root, "rat_hole")).map(|s| s.of.as_str()), Some("rat"));
    assert_eq!(app.world().get::<Countdown>(entity(&app, &root, "bell")).map(|c| c.duration_s), Some(1.0));
    assert_eq!(app.world().get::<PathFollow>(entity(&app, &root, "cart")).map(|f| f.waypoints_m.len()), Some(1));
    assert!(app.world().get::<Body>(entity(&app, &root, "hero")).is_some(), "and the hero is a body");
    assert!(app.world().get::<Collector>(entity(&app, &root, "hero")).is_some(), "who collects");

    // And they act: the one-second bell has rung, the latch opens the gate, the guard wakes, the coin is still
    // on the floor until the hero reaches it.
    assert!(sent::<CountdownElapsed>(&mut app).iter().any(|e| e.signal == "dinner"), "the bell rang");
    app.world_mut().write_message(Signal { name: "north".to_owned() });
    app.world_mut().write_message(Signal { name: "noise".to_owned() });
    frame(&mut app);
    assert_eq!(app.world().get::<Door>(entity(&app, &root, "gate")).map(|d| d.open), Some(true), "the latch opens the gate");
    assert_eq!(
        app.world().get::<StateMachine>(entity(&app, &root, "guard")).map(|m| m.state.as_str()),
        Some("awake"),
        "and the guard wakes"
    );

    // The hero walks to the coin, and it is taken.
    let hero = entity(&app, &root, "hero");
    app.world_mut().get_mut::<Transform>(hero).expect("a body stands somewhere").translation = Vec3::new(3.0, 0.0, 3.0);
    frame(&mut app);
    let taken = sent::<PickedUp>(&mut app);
    assert_eq!(taken.len(), 1, "the hero took the coin: {taken:?}");
    assert_eq!(taken[0].item, "gold_coin");
    assert!(app.world().get_entity(entity_or_none(&app, &root, "coin")).is_err(), "and it is gone from the floor");

    let _ = std::fs::remove_dir_all(&root);
}

/// **The `Entity` a file's `name` stands for, or nothing** — for a test that asserts something is gone.
fn entity_or_none(app: &App, root: &Path, name: &str) -> Entity {
    let key = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    app.world()
        .resource::<GameEntities>()
        .spawned
        .get(&key.display().to_string())
        .and_then(|by_name| by_name.get(name).copied())
        .unwrap_or(Entity::PLACEHOLDER)
}

/// **The behaviours are the crate's own list, and it is the list a person reads**: every one is registered
/// under its short path, so a file may spell it that way, and `behaviours_named` names the same paths the
/// registry answers — a component renamed without updating the list fails here.
#[test]
fn every_behaviour_is_a_registered_component_a_file_can_name() {
    let app = game(&project("behaviour_registry"));
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let registry = registry.read();

    let named = behaviours_named();
    assert!(named.len() >= 10, "the issue's first set is all of: {named:?}");
    for behaviour in &named {
        assert_eq!(
            registry.get_with_short_type_path(behaviour.short).map(|r| r.type_info().type_path()),
            Some(behaviour.full),
            "`{}` is registered under its short path, and the list names the path the registry has",
            behaviour.short
        );
        assert!(!behaviour.summary.is_empty(), "`{}` says what it does", behaviour.short);
        // **A behaviour is a component**, which is what makes it writable on an entity rather than only as a
        // resource — the reader inserts it by `ReflectComponent`.
        let found = registry.get_with_type_path(behaviour.full).expect("registered by the path asked for");
        assert!(
            found.data::<bevy::ecs::reflect::ReflectComponent>().is_some(),
            "`{}` is a component",
            behaviour.short
        );
    }
    // **`Transition` is a value, not a component**, and the list does not claim it is one: it is registered
    // because a `StateMachine` carries a `Vec` of them and a file writes that list.
    assert!(registry.get_with_short_type_path("Transition").is_some(), "a machine's transitions are registered");
}

/// **A type a file does not name is still refused**, which is what makes the registration above mean
/// something: a behaviour left out is not silently accepted.
#[test]
fn a_behaviour_file_that_names_a_type_nobody_registered_is_refused_by_name() {
    let root = project("behaviour_unregistered");
    write(&root, "entities:\n  door:\n    Levitate: { height_m: 2.0 }\n");
    let mut app = game(&root);
    frame(&mut app);
    let path = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    let document = entities::load(&path).expect("the file reads as YAML");
    let refused = entities::apply(app.world_mut(), &path.display().to_string(), &path.display().to_string(), &document)
        .expect_err("an unregistered type is refused");
    assert!(refused.line().contains("Levitate"), "and the refusal names it: {}", refused.line());
    assert_eq!(refused.line, 3, "at the line the type stands on: {}", refused.line());
    let _ = std::fs::remove_dir_all(&root);
}

/// **Each behaviour is documented by its doc comments, and they reach the type information a schema and a
/// hover are built from** — the issue's own claim, and the reason this crate takes bevy's
/// `reflect_documentation` feature. A field's doc is what an outside editor shows beside its box.
#[test]
fn a_behaviours_fields_carry_their_doc_comments_into_the_type_information() {
    // **Registered here rather than by the plugin**, because `BehaviourPlugin` turns auto-registration off for
    // these types: see `a_behaviour_is_not_in_the_registry_until_a_game_asks_for_it`.
    let mut registry = bevy::reflect::TypeRegistry::default();
    registry.register::<Trigger>();
    registry.register::<Spawner>();
    let trigger = registry
        .get_with_type_path("bevy_grimoire::behaviours::trigger::Trigger")
        .expect("registered above");
    let bevy::reflect::TypeInfo::Struct(info) = trigger.type_info() else { panic!("a trigger is a struct") };
    for (field, expected) in [("radius_m", "world metres"), ("signal", "name")] {
        let docs = info.iter().find(|f| f.name() == field).and_then(|f| f.docs());
        let docs = docs.unwrap_or_else(|| panic!("`{field}` has a doc comment"));
        assert!(docs.contains(expected), "`{field}`'s doc says what it means: {docs}");
    }
    // **A unit in a field's name is what makes a doc say what the number is for**, so the fields a behaviour
    // bounds carry one.
    let spawner = registry.get_with_type_path("bevy_grimoire::behaviours::spawner::Spawner").expect("registered");
    let bevy::reflect::TypeInfo::Struct(info) = spawner.type_info() else { panic!("a spawner is a struct") };
    let names: Vec<&str> = info.iter().map(|f| f.name()).collect();
    for unit in ["interval_s", "at_m"] {
        assert!(names.contains(&unit), "a spaner's `{unit}` names its unit; it has {names:?}");
    }
}

/// **`Transform` is writable in a file**, which is what lets a save say where a body stands — and it needs
/// bevy's `serialize` feature for that, so this is the test that keeps the feature from being dropped as
/// unused. Without it a trigger's position could only be set in Rust.
#[test]
fn a_file_sets_a_bodys_position_because_transform_is_writable() {
    let root = project("behaviour_transform");
    write(
        &root,
        &format!(
            "entities:\n  coin:\n    Pickup: {{ item: gold, radius_m: 0.5 }}\n    Transform: {}\n  hero:\n    Collector: {{}}\n    Transform: {}\n",
            at(0.0, 0.0, 0.0),
            at(0.0, 0.0, 0.0)
        ),
    );
    let mut app = game(&root);
    frame(&mut app);
    // Both stand on the same spot, which the file said; so the coin is taken in the frame the file was read.
    let taken = sent::<PickedUp>(&mut app);
    assert_eq!(taken.len(), 1, "the file put them on one another: {taken:?}");
    assert_eq!(taken[0].item, "gold");
    let _ = std::fs::remove_dir_all(&root);
}

/// **A behaviour is not in the registry until a game adds `BehaviourPlugin`**, which is what keeps every
/// game's own `Health` unambiguous: bevy 0.20 auto-registers every derived type process-wide, so a shipped
/// `Health` that registered itself would make a game's own `Health` ambiguous in that game's files and the
/// reader would refuse both. The opt-in is `#[reflect(no_auto_register)]` on each type.
#[test]
fn a_behaviour_is_not_in_the_registry_until_a_game_asks_for_it() {
    let plain = App::new();
    let registry = plain.world().resource::<AppTypeRegistry>().read();
    assert!(
        registry.get_with_short_type_path("Health").is_none(),
        "an app that never asked for the behaviours has no `Health` of ours competing with its own"
    );
    drop(registry);

    let mut asked = App::new();
    asked.add_plugins(BehaviourPlugin);
    let registry = asked.world().resource::<AppTypeRegistry>().read();
    assert!(registry.get_with_short_type_path("Health").is_some(), "and one that asked has it");
}
