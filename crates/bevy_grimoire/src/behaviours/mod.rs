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
//! - **One signal vocabulary, and the behaviours write it too.** A `Trigger` that a body enters, and a
//!   `Countdown` that rings, each write a [`Signal`] named by their own `signal` field, so a volume and a
//!   bell press a `Switch` or step a `StateMachine` that names them without a game's code in between. The
//!   producer runs before the consumer in the same frame ([`BehaviourSystems`]), so a crossing opens its door
//!   on the frame it happened. A `signal` left empty writes nothing, rather than a name that would match
//!   every other nameless part.
//! - **A behaviour that needs a clock takes `Option<Res<Time>>`**, so a game without one ticks nothing
//!   rather than panicking — the rule `entities.rs` states for its own reload.
//! - **A behaviour reads `GlobalTransform` and acts in `PostUpdate` after `TransformSystems::Propagate`**, so
//!   a body moved this frame is where it is this frame. Two exceptions, stated where they belong: a
//!   `PathFollow` *writes* its own local `Transform` (so it walks in its parent's frame, not the world's), and
//!   `GlobalTransform` is one frame behind a follower that moved this frame — a follower never reads its own.
//!
//! # The name `Timer` is taken
//!
//! A countdown is [`Countdown`], not `Timer`: `bevy_time::Timer` already owns the short type path `Timer`,
//! and the reader refuses an ambiguous short path — so a component called `Timer` here could never be
//! written in a file by that name, which is the whole point of it. [`Health`] is the same collision with a
//! game that ships its own `Health`: such a game writes `bevy_grimoire::behaviours::Health`, which is this
//! type's reflected path — its one path, since the submodules are private.

// **The submodules are private, and each type names its own path** with
// `#[reflect(type_path = "bevy_grimoire::behaviours")]`: one path per type, which is the path the docs, the
// guide and [`behaviours_named`] use, rather than a public `behaviours::health::Health` beside a re-exported
// `behaviours::Health`.
mod countdown;
mod door;
mod health;
mod machine;
mod path;
mod pickup;
mod signal;
mod spawner;
mod trigger;

use bevy::prelude::*;
use bevy::transform::TransformSystems;

/// **Which half of a behaviour runs first.**
///
/// A behaviour that *writes* a [`Signal`] (a volume crossed, a countdown rung) runs in [`BehaviourSystems::Produces`];
/// one that *reads* it (a switch and its door, a state machine) runs in [`BehaviourSystems::Consumes`], after it.
/// Two sets rather than one tuple because the order inside a tuple is not defined: a door that happened to be
/// polled before the volume that pressed it would open a frame late on some runs and not others.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BehaviourSystems {
    /// The behaviours that write a [`Signal`] this frame.
    Produces,
    /// The behaviours that read one, after the frame's signals have been written.
    Consumes,
}

pub use countdown::{Countdown, CountdownElapsed};
pub use door::{Door, Switch};
pub use health::{DEFAULT_DAMAGE, DEFAULT_HEALTH, Damage, Damaged, Died, Health};
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
///
/// **`register` is what makes the list the only one.** [`BehaviourPlugin`] registers by walking
/// [`behaviours_named`], so a behaviour added to the list is registered and a behaviour left off it is not
/// there for a file to name — there is no second list of `register_type::<…>` calls to keep in step. The list
/// is public because it is what a palette, a completion list or a guide's table reads when it wants to say
/// which behaviours a build has.
#[derive(Clone, Copy)]
pub struct Behaviour {
    /// The short type path, which is what a file usually writes.
    pub short: &'static str,
    /// The full type path, which is what a file writes when a game has a type of the same short name.
    pub full: &'static str,
    /// What it does, in one sentence — the line a person reads in a list of behaviours.
    pub summary: &'static str,
    /// Register the type on an app, which is what makes one writable in a file.
    pub register: fn(&mut App),
}

impl std::fmt::Debug for Behaviour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Behaviour").field("short", &self.short).field("full", &self.full).field("summary", &self.summary).finish()
    }
}

impl PartialEq for Behaviour {
    fn eq(&self, other: &Self) -> bool {
        self.full == other.full
    }
}

impl Eq for Behaviour {}

/// **Every behaviour this crate ships**, in the order the module documents them.
pub fn behaviours_named() -> Vec<Behaviour> {
    /// One row, with the paths taken from the type itself and the registration its own.
    fn of<T: bevy::reflect::TypePath + bevy::reflect::GetTypeRegistration>(summary: &'static str) -> Behaviour {
        fn register<T: bevy::reflect::GetTypeRegistration>(app: &mut App) {
            app.register_type::<T>();
        }
        Behaviour {
            short: <T as bevy::reflect::TypePath>::short_type_path(),
            full: <T as bevy::reflect::TypePath>::type_path(),
            summary,
            register: register::<T>,
        }
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
///
/// # It needs `TransformPlugin`, and does not add it
///
/// The behaviours read `GlobalTransform`, which only propagation writes, so the app must have
/// `TransformPlugin` — **`DefaultPlugins` provides it**, and a game that builds its app from a bare
/// `App::new()` adds it itself. This plugin deliberately adds no plugin of its own: `DefaultPlugins` is a
/// plugin *group*, and `PluginGroupBuilder::finish` panics on a plugin already in the app, so
/// `App::new().add_plugins((BehaviourPlugin, DefaultPlugins))` — behaviours first, the group after — would
/// panic if this added `TransformPlugin` here. A guard for the other order (`is_plugin_added`) only moves the
/// panic to the order the guard does not cover.
///
/// `behaviour_plugin_first_and_a_transform_plugin_group_after_it_builds` builds that order, and
/// `game()` in `a_game_behaves_from_yaml.rs` adds `TransformPlugin` before this plugin, which is what a game
/// does implicitly by taking `DefaultPlugins`.
pub struct BehaviourPlugin;

impl Plugin for BehaviourPlugin {
    fn build(&self, app: &mut App) {
        // **The list is the registration.** One row per behaviour, walked here, so a type cannot be shipped
        // and left unregistered, and a row cannot name a type the registry does not get.
        for behaviour in behaviours_named() {
            (behaviour.register)(app);
        }
        // **`Transition` is registered beside the list**, because it is a value a `StateMachine` carries and
        // not a component of its own: it is writable in a file, so it must be in the registry, but it is not
        // a behaviour a game puts on an entity.
        app.register_type::<Transition>()
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
            .configure_sets(
                PostUpdate,
                (
                    BehaviourSystems::Produces.after(TransformSystems::Propagate),
                    BehaviourSystems::Consumes.after(BehaviourSystems::Produces),
                ),
            )
            // **`PostUpdate`, after `Propagate`.** A behaviour compares world positions, and this frame's
            // `GlobalTransform` is only written by propagation in `PostUpdate` — running in `Update` would
            // read last frame's positions, which is the whole of a body's move lost at a walk of a metre a
            // second. After `Propagate`, a body moved this frame is where it is this frame.
            //
            // **Producers before consumers**, so a volume a body crossed this frame and a bell that rang this
            // frame have written their `Signal` before the switch and the machine that name it read one.
            .add_systems(
                PostUpdate,
                (
                    trigger::crossings,
                    countdown::ticks,
                    health::take_damage,
                    spawner::ticks,
                    pickup::taken,
                    path::walks,
                )
                    .in_set(BehaviourSystems::Produces),
            )
            .add_systems(
                PostUpdate,
                (door::switched, machine::steps).in_set(BehaviourSystems::Consumes),
            );
    }
}
