//! **`PathFollow`: waypoints, walked at a speed, stopping at the last.**

use bevy::prelude::*;

/// **How fast a follower walks when a file does not say**, in metres a second.
pub const DEFAULT_PATH_SPEED_M_S: f32 = 1.0;

/// **How close counts as arrived**, in metres. A follower moves to a waypoint, not onto it: floating point
/// never lands exactly on one, and a tolerance of zero would leave it jittering at the last centimetre.
pub const ARRIVAL_TOLERANCE_M: f32 = 1e-3;

/// **Walks its waypoints at `speed_m_s`, saying which one it reached, and stops at the last.**
///
/// **The waypoints are world positions in metres**, not offsets from where the follower stands: a save that
/// moves a follower does not move its route, and a route written by an agent is a list of places.
///
/// **It does not turn the entity**, so a game is free to face it however it likes — a piece's own yaw is the
/// editor's, and a behaviour that wrote it would fight the map. A follower with no waypoints, or one that
/// has reached the last, does not move.
///
/// ```yaml
/// cart: { PathFollow: { speed_m_s: 1.0, waypoints_m: [[0.0, 0.0, 2.0], [2.0, 0.0, 2.0]] } }
/// ```
#[derive(Component, Reflect, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default, no_auto_register)]
#[serde(default, deny_unknown_fields)]
#[require(Transform)]
pub struct PathFollow {
    /// How fast it walks, in world metres a second.
    pub speed_m_s: f32,
    /// The route, in world metres, in the order it is walked.
    pub waypoints_m: Vec<[f32; 3]>,
    /// The index of the next waypoint to reach. A file may set it, so a save mid-route resumes.
    pub next: usize,
}

impl Default for PathFollow {
    fn default() -> Self {
        Self { speed_m_s: DEFAULT_PATH_SPEED_M_S, waypoints_m: Vec::new(), next: 0 }
    }
}

/// **A [`PathFollow`] reached the waypoint at `index`**, counting from zero.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct WaypointReached {
    /// The follower.
    pub entity: Entity,
    /// Which waypoint, in the order the file listed them.
    pub index: usize,
}

/// **Move every follower toward its next waypoint, reporting each it reaches**, and stop at the last.
pub(crate) fn walks(
    time: Option<Res<Time>>,
    mut followers: Query<(Entity, &mut PathFollow, &mut Transform), Without<Camera>>,
    mut out: MessageWriter<WaypointReached>,
) {
    // **No clock, no walking**: a game without `Time` moves nothing rather than teleporting.
    let Some(time) = time else { return };
    let delta_s = time.delta_secs();
    for (entity, mut follow, mut transform) in &mut followers {
        // **A frame's allowance, spent across as many waypoints as it covers**, so a fast follower crossing
        // a corner in one frame takes the corner rather than cutting across it — and a frame with a very
        // large delta cannot leave one waypoint unreported.
        let mut allowance_m = follow.speed_m_s.max(0.0) * delta_s;
        while allowance_m > 0.0 {
            let Some(&target) = follow.waypoints_m.get(follow.next) else { break };
            let target = Vec3::from_array(target);
            let gap = target - transform.translation;
            let distance_m = gap.length();
            if distance_m <= ARRIVAL_TOLERANCE_M {
                // **Arrived**: report it and move on, spending none of the allowance, which is what lets a
                // zero-length leg not stall the walk.
                arrive(entity, &mut follow, &mut out);
                continue;
            }
            if allowance_m < distance_m {
                transform.translation += gap / distance_m * allowance_m;
                allowance_m = 0.0;
                continue;
            }
            // **The allowance reaches past this waypoint**: land on it exactly, report it, and spend what is
            // left on the next leg.
            transform.translation = target;
            allowance_m -= distance_m;
            arrive(entity, &mut follow, &mut out);
        }
    }
}

/// **Report the waypoint at `follow.next` and point it at the next one**, which is the same step wherever it
/// is reached from.
fn arrive(entity: Entity, follow: &mut PathFollow, out: &mut MessageWriter<WaypointReached>) {
    out.write(WaypointReached { entity, index: follow.next });
    follow.next += 1;
}
