//! **`Spawner`: a thing that asks for its piece every interval, and never makes it.**

use bevy::prelude::*;

/// **How often a spawner asks when a file does not say**, in seconds.
pub const DEFAULT_SPAWN_INTERVAL_S: f32 = 5.0;

/// **Asks for a piece every `interval_s`, naming what to make and where.**
///
/// **It writes a [`SpawnRequested`] message and spawns nothing.** `spawn_descriptor` is this project's only
/// spawner — the editor's preview and the game share it so a piece cannot look one way in one and another
/// in the other — so a behaviour that spawned would be the second path, and the second path is how the two
/// come to disagree. The game answers the message with the one spawner it already has.
///
/// `at_m` is a world position in metres, not an offset: a spawner is placed once and its requests are where
/// its pieces belong, so an agent moving the spawner does not move every piece it has ever made.
///
/// ```yaml
/// rat_hole: { Spawner: { of: rat, interval_s: 2.0, at_m: [1.0, 0.0, 2.0] } }
/// ```
#[derive(Component, Reflect, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
pub struct Spawner {
    /// The id of the descriptor to ask for, which is a piece the game's library holds.
    pub of: String,
    /// How long between requests, in seconds. Zero or less asks every frame, which a file may want for one
    /// frame and a game will not.
    pub interval_s: f32,
    /// Where the piece goes, in world metres.
    pub at_m: [f32; 3],
    /// Seconds waited since the last request. A file may set it, and the spawner moves it.
    pub since_s: f32,
}

impl Default for Spawner {
    fn default() -> Self {
        Self { of: String::new(), interval_s: DEFAULT_SPAWN_INTERVAL_S, at_m: [0.0; 3], since_s: 0.0 }
    }
}

/// **A [`Spawner`] wants one piece made**, now, at a place in the world.
///
/// Written by the behaviour and answered by the game, because the game is the only thing that may call
/// `spawn_descriptor`.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct SpawnRequested {
    /// The spawner that asked.
    pub spawner: Entity,
    /// The descriptor id to spawn.
    pub of: String,
    /// Where it goes, in world metres.
    pub at_m: [f32; 3],
}

/// **Add each spawner's frame to its wait, and ask when the wait is full.**
pub(crate) fn ticks(
    time: Option<Res<Time>>,
    mut spawners: Query<(Entity, &mut Spawner)>,
    mut out: MessageWriter<SpawnRequested>,
) {
    // **No clock, no counting**: a game without `Time` asks for nothing rather than asking every frame.
    let Some(time) = time else { return };
    let delta_s = time.delta_secs();
    for (spawner, mut one) in &mut spawners {
        // **A spawner with no piece named asks for nothing**: an empty `of` is a file that has not finished
        // being written, and asking for a descriptor with no id would be a refusal downstream.
        if one.of.is_empty() {
            continue;
        }
        one.since_s += delta_s;
        if one.since_s < one.interval_s {
            continue;
        }
        // **The remainder carries**, so a spawner asking every 2 s at 60 fps asks on the frame its wait is
        // really up rather than drifting a frame late each time.
        one.since_s -= one.interval_s.max(0.0);
        out.write(SpawnRequested { spawner, of: one.of.clone(), at_m: one.at_m });
    }
}
