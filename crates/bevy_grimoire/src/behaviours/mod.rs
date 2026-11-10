//! **Game behaviours that ship as components, configured entirely from YAML** (GD-07, #369).
//!
//! A behaviour here is a **reflected component**. `BehaviourPlugin` registers each one, so a file in
//! `<root>/game/<name>.entities.yaml` writes it by its short type path — the reader GD-05 built
//! (`bevy_grimoire::entities`) resolves the path through the engine's registry and nothing else. There is
//! no second registry and no hand-written mapping, so what a file may say is exactly what this crate
//! accepts, and a type this module stops shipping stops being writable in the same commit.
//!
//! ```yaml
//! entities:
//!   latch:   { Switch: { latch: north, on: false } }
//!   gate:    { Door: { link: north, open: false } }
//!   tripwire: { Trigger: { radius_m: 1.5, signal: gate_crossed } }
//!   bell:    { Countdown: { duration_s: 2.0, signal: dinner } }
//! ```
//!
//! # Data first, script second
//!
//! A behaviour whose fields can describe it lives here; anything else is a script (GD-06), and this module
//! never grows an interpreter. The list grows from what the sample game and the agents' wishes ask for, one
//! component at a time, each with its test.
//!
//! # What a behaviour may be
//!
//! - **A field names its unit** (`radius_m`, `delay_s`, `speed_m_s`), so the bounds a project declares and
//!   the schema an outside editor completes from both address it by a name that says what the number means.
//! - **Every field is writable in a file**, including the state a behaviour keeps (`Switch.on`,
//!   `StateMachine.state`, `Countdown.elapsed_s`). A save that omits a field sets it to the type's default,
//!   and a key the type does not have is refused rather than ignored (`deny_unknown_fields`), so a typo is
//!   a refusal at its line instead of a behaviour that quietly does nothing.
//! - **A behaviour never spawns anything.** `spawn_descriptor` is this project's only spawner, so a
//!   `Spawner` writes a [`SpawnRequested`] message and the game answers it.
//! - **A behaviour that needs a clock takes `Option<Res<Time>>`**, so a game without one ticks nothing
//!   rather than panicking — the rule `entities.rs` states for its own reload.
//! - **Positions are world positions**, read from `GlobalTransform`. A behaviour therefore sees a body
//!   one frame after it moves, because `GlobalTransform` is computed in `PostUpdate` after these systems
//!   run in `Update`.
//!
//! # The name `Timer` is taken
//!
//! A countdown is [`Countdown`], not `Timer`: `bevy_time::Timer` already owns the short type path `Timer`,
//! and the reader refuses an ambiguous short path — so a component called `Timer` here could never be
//! written in a file by that name, which is the whole point of it. [`Health`] is the same collision with a
//! game that ships its own `Health`: such a game writes `bevy_grimoire::behaviours::Health`.

pub mod countdown;
pub mod door;
pub mod health;
pub mod machine;
pub mod path;
pub mod pickup;
pub mod signal;
pub mod spawner;
pub mod trigger;

use bevy::prelude::*;
use bevy::transform::TransformSystems;

pub use countdown::{Countdown, CountdownElapsed};
pub use door::{Door, Switch};
pub use health::{Damage, Damaged, Died, Health};
pub use machine::{StateMachine, Transition, Transitioned};
pub use path::{PathFollow, WaypointReached};
pub use pickup::{Body, Collector, Pickup, PickedUp};
pub use signal::Signal;
pub use spawner::{SpawnRequested, Spawner};
pub use trigger::{Trigger, Triggered};

/// **Whether one world position is within `radius_m` of another**, as squared distance so no square root is
/// taken for a comparison that needs none — the two callers are the trigger volume and the pickup's reach.
pub(crate) fn within(radius_m: f32, here: Vec3, there: Vec3) -> bool {
    let radius_m = radius_m.max(0.0);
    here.distance_squared(there) <= radius_m * radius_m
}

/// **One behaviour this crate ships**, as the crate itself names it.
///
/// The type paths are asked of [`TypePath`](bevy::reflect::TypePath) rather than typed out, so this list
/// cannot name a path the registry does not know: a component renamed without updating the list fails
/// `every_behaviour_is_a_registered_component_a_file_can_name`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Behaviour {
    /// The short type path, which is what a file usually writes.
    pub short: &'static str,
    /// The full type path, which is what a file writes when a game has a type of the same short name.
    pub full: &'static str,
    /// What it does, in one sentence — the line a person reads in a list of behaviours.
    pub summary: &'static str,
}

/// **Every behaviour this crate ships**, in the order the module documents them.
pub fn behaviours_named() -> Vec<Behaviour> {
    /// One row, with the paths taken from the type itself.
    fn of<T: bevy::reflect::TypePath>(summary: &'static str) -> Behaviour {
        Behaviour { short: <T as bevy::reflect::TypePath>::short_type_path(), full: <T as bevy::reflect::TypePath>::type_path(), summary }
    }
    vec![
        of::<Trigger>("A volume that says when a body enters it and when it leaves"),
        of::<Switch>("A named latch a signal turns on and off"),
        of::<Door>("A door that opens and closes with the switch of its name"),
        of::<Spawner>("Asks for its piece every interval it is set to"),
        of::<Pickup>("Something a collector that reaches it takes once"),
        of::<Health>("Hit points an entity has left, out of its maximum"),
        of::<Damage>("Hit points taken off health on the next frame, then gone"),
        of::<Countdown>("Counts its own seconds and says so once, naming its signal"),
        of::<PathFollow>("Walks its waypoints at its speed and stops at the last"),
        of::<StateMachine>("States and the transitions between them, stepped by signals"),
        of::<Body>("Marks an entity a trigger volume notices"),
        of::<Collector>("Marks an entity that takes a pickup it reaches"),
    ]
}

/// **The behaviours, their events and their systems.**
///
/// Registering the types is what makes them writable in a file, so this plugin is not optional for a game
/// that uses one: a file naming a behaviour the game did not register is refused, naming the type.
pub struct BehaviourPlugin;

impl Plugin for BehaviourPlugin {
    fn build(&self, app: &mut App) {
        // **A behaviour acts on a world position, which is `GlobalTransform`**, and `GlobalTransform` is only
        // written by propagation. `DefaultPlugins` adds `TransformPlugin`; a bare `App` does not, and there
        // every `GlobalTransform` stays identity, so a trigger would fire at the origin and nowhere else.
        // Added only when absent: Bevy panics on a unique plugin's second `add`.
        if !app.is_plugin_added::<bevy::transform::TransformPlugin>() {
            app.add_plugins(bevy::transform::TransformPlugin);
        }
        app.register_type::<Trigger>()
            .register_type::<Switch>()
            .register_type::<Door>()
            .register_type::<Spawner>()
            .register_type::<Pickup>()
            .register_type::<Body>()
            .register_type::<Collector>()
            .register_type::<Health>()
            .register_type::<Damage>()
            .register_type::<Countdown>()
            .register_type::<PathFollow>()
            .register_type::<StateMachine>()
            .register_type::<Transition>()
            // **Only what a file may write is registered.** A message is something that happened, and a save
            // that could write one would fire it on every reload, so the reader refusing a file that names
            // one is the correct answer rather than an omission.
            .init_resource::<trigger::Inside>()
            .add_message::<Triggered>()
            .add_message::<Signal>()
            .add_message::<SpawnRequested>()
            .add_message::<PickedUp>()
            .add_message::<Damaged>()
            .add_message::<Died>()
            .add_message::<CountdownElapsed>()
            .add_message::<WaypointReached>()
            .add_message::<Transitioned>()
            // **`PostUpdate`, after `Propagate`.** A behaviour compares world positions, and this frame's
            // `GlobalTransform` is only written by propagation in `PostUpdate` — running in `Update` would
            // read last frame's positions, which is the whole of a body's move lost at a walk of a metre a
            // second. After `Propagate`, a body moved this frame is where it is this frame.
            .add_systems(
                PostUpdate,
                (
                    trigger::crossings,
                    door::switched,
                    health::take_damage,
                    spawner::ticks,
                    pickup::taken,
                    countdown::ticks,
                    path::walks,
                    machine::steps,
                )
                    .after(TransformSystems::Propagate),
            );
    }
}
