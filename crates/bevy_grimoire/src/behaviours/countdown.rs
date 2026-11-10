//! **`Countdown`: seconds that run out, and the one signal that says so.**

use bevy::prelude::*;

/// **How long a countdown runs when a file does not say**, in seconds.
pub const DEFAULT_COUNTDOWN_S: f32 = 1.0;

/// **Counts its own seconds and says so once, naming its signal.**
///
/// It is called a countdown and not a `Timer` because `bevy_time::Timer` already owns that short type path
/// and the reader refuses an ambiguous one — so a component named `Timer` here could never be written in a
/// file by that name, and being writable by name is the whole job.
///
/// **A countdown is one-shot and stays finished**: `elapsed_s` stops at `duration_s` and it rings once. A
/// repeating one is a [`Spawner`](super::Spawner)'s shape, or a game writing a fresh countdown.
///
/// ```yaml
/// bell: { Countdown: { duration_s: 2.0, signal: dinner } }
/// ```
#[derive(Component, Reflect, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
#[type_path = "bevy_grimoire::behaviours"]
pub struct Countdown {
    /// How long it runs for, in seconds.
    pub duration_s: f32,
    /// The signal it names when it rings.
    pub signal: String,
    /// Seconds counted so far. A file may set it, so a save mid-count resumes rather than restarting.
    pub elapsed_s: f32,
    /// Whether it has rung. A file may set it, and a finished countdown stays finished.
    pub finished: bool,
}

impl Default for Countdown {
    fn default() -> Self {
        Self { duration_s: DEFAULT_COUNTDOWN_S, signal: String::new(), elapsed_s: 0.0, finished: false }
    }
}

/// **A [`Countdown`] ran out**, once, carrying the signal name it was set to.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct CountdownElapsed {
    /// The countdown that rang.
    pub entity: Entity,
    /// Its `signal`.
    pub signal: String,
}

/// **Add this frame to every unfinished countdown and ring the ones that reach their duration.**
pub(crate) fn ticks(
    time: Option<Res<Time>>,
    mut countdowns: Query<(Entity, &mut Countdown)>,
    mut out: MessageWriter<CountdownElapsed>,
    mut signals: MessageWriter<super::Signal>,
) {
    // **No clock, no counting**: a game without `Time` counts nothing rather than finishing instantly.
    let Some(time) = time else { return };
    let delta_s = time.delta_secs();
    for (entity, mut countdown) in &mut countdowns {
        if countdown.finished {
            continue;
        }
        // **A duration at or below zero has already run out**, which is what a file setting zero means; it
        // rings on the first frame with a clock rather than waiting for `-1` to pass.
        countdown.elapsed_s += delta_s;
        if countdown.elapsed_s < countdown.duration_s {
            continue;
        }
        countdown.elapsed_s = countdown.duration_s.max(0.0);
        countdown.finished = true;
        out.write(CountdownElapsed { entity, signal: countdown.signal.clone() });
        // **A countdown that rings writes the signal it names**, so a bell can move a machine or a latch
        // without a game's code listening to `CountdownElapsed` first. It rings once, so it writes once; a
        // countdown with no `signal` writes none rather than an empty name.
        if !countdown.signal.is_empty() {
            signals.write(super::Signal { name: countdown.signal.clone() });
        }
    }
}
