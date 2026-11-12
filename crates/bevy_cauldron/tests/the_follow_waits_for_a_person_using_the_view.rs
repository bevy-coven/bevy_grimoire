//! **The person is never yanked** (AF-04, #360; series agent follow, #356).
//!
//! AF-02 (#358) framed an agent's change and AF-03 (#359) opened the page of one off the map. Both
//! moved the view the instant the change landed — so an agent working while a person orbited, panned
//! or typed took the camera out from under their hands, and twenty changes in one second were a blur
//! rather than twenty views.
//!
//! So the follow now waits for a person who is using the view, and says what it is holding back:
//!
//! 1. **Idle wait.** A person who orbited, panned, dragged, clicked in the viewport or typed in the
//!    last [`FOLLOW_IDLE_S`] seconds holds the follow. The camera does not move; a **look chip** over
//!    the viewport names what the Familiar changed; once the person stops, the change is framed.
//! 2. **Paced bursts.** Requests queue in arrival order and each is shown for at least
//!    [`FOLLOW_DWELL_S`], so five changes in one frame are five views.
//! 3. **The toggle.** View ▸ Follow the agent (`EditorSettings::follow_agent`, on by default) stops
//!    the camera moving at all, while the chip still offers **Look**. The file outlives a restart.
//!
//! # What is measured, not supposed
//!
//! The camera's position is the assertion, not a flag about it: a follow that says it is waiting and
//! then moves the rig anyway is the yank this exists to prevent. `fill` is the same measure
//! `the_editor_follows_an_agents_change` uses — the change's own corners projected through the real
//! `MainCamera` — so "it was framed" cannot pass for a camera that stayed on the whole map.

use std::time::Duration;

use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;

use bevy_cauldron::editor::EditorState;
use bevy_cauldron::follow::{FOLLOW_DWELL_S, FOLLOW_IDLE_S, Following};
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::settings::EditorSettings;
use bevy_cauldron::view::{MainCamera, Rig};

use crate::fixtures;

/// **One frame of the test clock**, seconds: 60 Hz. The pacing is measured in seconds and a headless
/// frame is about a millisecond, so every wait here steps this clock rather than real time.
const FRAME: f32 = 1.0 / 60.0;

/// **How much of the viewport the change's own box fills** for the frame to count as framed. A 1 m
/// piece framed with `FOLLOW_MARGIN_FRAC` fills most of it; the whole 64 m room leaves a few percent.
const FRAMED_MIN_FILL: f32 = 0.25;

fn frames_of(seconds: f32) -> usize {
    (seconds / FRAME).ceil() as usize + 2
}

fn room(tag: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .side(64.0)
        .descriptor("bed", "alpha")
        .place("bed", (-28.0, -28.0))
        .build("m")
}

fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    // **Travel on**, which is the ordinary setting: the harness builds with `MotionSettings::off()`,
    // and a follow that lands at once has no ease for a person's input to interrupt.
    app.insert_resource(bevy_gooey::MotionSettings::default());
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
}

fn rig(app: &App) -> (Vec3, f32) {
    let rig = app.world().resource::<Rig>();
    (rig.focus, rig.height)
}

/// **The camera taken somewhere else**, so a follow that does nothing cannot pass.
fn point_away(app: &mut App) {
    app.world_mut().resource_mut::<Rig>().focus = Vec3::new(-28.0, 0.0, -28.0);
    app.world_mut().resource_mut::<Rig>().height = 4.0;
    for _ in 0..3 {
        app.update();
    }
}

fn footprint_corners(app: &App, id: &str) -> Option<[Vec3; 4]> {
    let open = app.world().resource::<OpenMap>();
    let project = app.world().resource::<Project>();
    let row = open.map.placements.iter().find(|p| p.id == id)?;
    let descriptor = project.library.get(&row.descriptor)?;
    let (w, d) = bevy_grimoire_core::descriptor::placed_footprint(descriptor)?;
    let origin = open.map.origin;
    let (hw, hd) = (w * 0.5, d * 0.5);
    Some([
        Vec3::new(origin.0 + row.at.0 - hw, origin.1, origin.2 + row.at.1 - hd),
        Vec3::new(origin.0 + row.at.0 + hw, origin.1, origin.2 + row.at.1 - hd),
        Vec3::new(origin.0 + row.at.0 + hw, origin.1, origin.2 + row.at.1 + hd),
        Vec3::new(origin.0 + row.at.0 - hw, origin.1, origin.2 + row.at.1 + hd),
    ])
}

/// **How much of the viewport's shorter side the change's own box fills**, `0..=1`.
fn fill(app: &mut App, corners: &[Vec3]) -> f32 {
    let world = app.world_mut();
    let mut cameras = world.query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras.single(world).expect("one main camera");
    let rect = camera.logical_viewport_rect().expect("a viewport");
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for corner in corners {
        let p = camera
            .world_to_viewport(transform, *corner)
            .unwrap_or_else(|e| panic!("{corner:?} is not in view: {e:?}"));
        lo = lo.min(p);
        hi = hi.max(p);
    }
    (hi.x - lo.x).abs().max((hi.y - lo.y).abs()) / rect.width().min(rect.height())
}

/// **The id of the one placement an apply made**, by the map rather than by the answer.
fn the_one_placed(app: &App) -> String {
    let open = app.world().resource::<OpenMap>();
    let found: Vec<&str> = open
        .map
        .placements
        .iter()
        .map(|p| p.id.as_str())
        .filter(|id| *id != "bed@0")
        .collect();
    assert_eq!(found.len(), 1, "one piece was placed: {found:?}");
    found[0].to_owned()
}

/// **A person using the view**: the pointer over the map with its button held, which is a drag in
/// progress. Written straight into `ButtonInput`, which is what the editor's own verbs read.
fn a_person_drags(app: &mut App) {
    use bevy::input::ButtonInput;
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut().resource_mut::<bevy_cauldron::view::Rig>().goal_yaw += 0.3;
    // One frame in which the person is working: a follow that ignores them lands here.
    app.update();
}

fn a_person_lets_go(app: &mut App) {
    use bevy::input::ButtonInput;
    let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    mouse.release(MouseButton::Left);
    mouse.clear();
    // The editor's own input is read a frame late; two frames put the release behind us.
    app.update();
    app.update();
}

/// **`FOLLOW_IDLE_S` and `FOLLOW_DWELL_S`, as constants of the API the issue names.**
#[test]
fn the_wait_and_the_dwell_are_the_seconds_the_issue_names() {
    assert_eq!(FOLLOW_IDLE_S, 2.0, "the idle wait is two seconds (AF-04, #360)");
    assert!(
        (FOLLOW_DWELL_S - 0.6).abs() < f32::EPSILON,
        "each change is shown for 0.6 s (AF-04, #360), and it reads {FOLLOW_DWELL_S}"
    );
}

/// **1. A person orbiting holds the follow; the chip says what it holds; letting go frames it.**
#[test]
fn a_person_working_holds_the_follow_and_the_chip_names_what_it_waits_to_show() {
    let root = room("pace_idle");
    let mut app = editor(&root);
    point_away(&mut app);
    let before = rig(&app);

    // **The person is using the view** when the agent's change lands.
    a_person_drags(&mut app);
    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let placed = the_one_placed(&app);

    // **Two seconds of the person's hands on the view**: no frame of it may move the camera.
    for _ in 0..frames_of(FOLLOW_IDLE_S) {
        app.update();
        assert_eq!(
            rig(&app),
            before,
            "the camera moved while a person was working in the viewport: the follow is a kindness \
             to someone reading a panel and it stops being one the moment they are working"
        );
    }
    let chip = app
        .world()
        .resource::<bevy_cauldron::follow::LookChip>()
        .sentence()
        .unwrap_or_else(|| panic!("the look chip says nothing while a change waits to be shown"));
    assert!(
        chip.contains("bed") && chip.contains("the Familiar"),
        "the chip does not name what the Familiar changed: {chip:?}"
    );
    assert!(
        app.world().resource::<Following>().pending() > 0,
        "the change was dropped rather than held, so letting go would show nothing"
    );

    // **The person lets go**, and the wait runs out.
    a_person_lets_go(&mut app);
    for _ in 0..frames_of(FOLLOW_IDLE_S + bevy_cauldron::follow::FOLLOW_EASE_S) {
        app.update();
    }
    let corners = footprint_corners(&app, &placed).expect("the placed bed measures");
    let fill = fill(&mut app, &corners);
    assert!(
        fill >= FRAMED_MIN_FILL,
        "after the person stopped, the change fills {:.0}% of the viewport, so the camera never \
         went to it",
        fill * 100.0
    );
    assert!(
        app.world().resource::<bevy_cauldron::follow::LookChip>().sentence().is_none(),
        "the chip is still up after the change was shown"
    );
}

/// **2. Five changes in one frame are five views, in order, each held `FOLLOW_DWELL_S`.**
#[test]
fn five_changes_in_one_frame_are_framed_one_after_another() {
    let root = room("pace_burst");
    let mut app = editor(&root);
    point_away(&mut app);

    let mut made = Vec::new();
    for at in [(20.0, 18.0), (-20.0, 18.0), (20.0, -18.0), (-20.0, -18.0), (0.0, 24.0)] {
        let answer = apply(
            &mut app,
            "placement_add",
            serde_json::json!({"piece": "bed", "at": [at.0, at.1]}),
        );
        assert_eq!(answer["outcome"], "applied", "{answer}");
        made.push(the_one_placed_from(&app, made.len() + 1));
    }
    assert_eq!(made.len(), 5, "five placements were made");

    // **Every change is framed in its turn.** The camera's focus is read at each change's own frame,
    // so the assertion is that the five focuses are five different places, in the order they were
    // made — a burst that showed only the last one would leave four positions unvisited.
    let mut arrivals: Vec<Vec3> = Vec::new();
    let mut last = rig(&app).0;
    for _ in 0..frames_of(5.0 * (FOLLOW_DWELL_S + bevy_cauldron::follow::FOLLOW_EASE_S)) {
        app.update();
        let (focus, _) = rig(&app);
        if focus != last {
            // A move has begun or finished; an arrival is the frame the camera comes to rest.
            last = focus;
        }
        if !app.world().resource::<Following>().settling() && arrivals.len() < 5 {
            let corners = footprint_corners(&app, &made[arrivals.len()]).expect("the bed measures");
            if fill(&mut app, &corners) >= FRAMED_MIN_FILL && (arrivals.last().copied() != Some(focus)) {
                arrivals.push(focus);
            }
        }
    }
    assert_eq!(
        arrivals.len(),
        made.len(),
        "{} of {} changes were framed", arrivals.len(), made.len()
    );
    let distinct: std::collections::BTreeSet<[i32; 2]> =
        arrivals.iter().map(|f| [f.x.round() as i32, f.z.round() as i32]).collect();
    assert_eq!(
        distinct.len(),
        made.len(),
        "the burst was framed as {} distinct places rather than {}", distinct.len(), made.len()
    );
}

/// **The placement the `n`th add made**, by map order rather than by the answer: `placement_add`
/// mints its id at commit, and the map is what a person sees.
fn the_one_placed_from(app: &App, n: usize) -> String {
    let open = app.world().resource::<OpenMap>();
    let found: Vec<&str> = open
        .map
        .placements
        .iter()
        .map(|p| p.id.as_str())
        .filter(|id| *id != "bed@0")
        .collect();
    assert_eq!(found.len(), n, "the map holds {n} placed pieces: {found:?}");
    found[n - 1].to_owned()
}

/// **3. With the toggle off, nothing moves — and Look still frames it. The setting survives a
/// restart.**
#[test]
fn with_the_toggle_off_nothing_moves_and_look_still_frames_it() {
    let root = room("pace_toggle");
    let mut app = editor(&root);
    // **The person's setting**, written where the editor reads it, not the resource it resolves to.
    let settings = app.world().resource::<Project>().project_dir.join("editor.yaml");
    std::fs::write(&settings, "{version: 1, follow_agent: false}").expect("editor.yaml");
    // The watcher looks at the file's size and modification time, and again once a second regardless.
    for _ in 0..frames_of(1.5) {
        app.update();
    }
    assert!(
        !app.world().resource::<EditorSettings>().follow_agent,
        "the setting did not reach the running editor, so the test would prove nothing about it"
    );

    point_away(&mut app);
    let before = rig(&app);
    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let placed = the_one_placed(&app);
    for _ in 0..frames_of(FOLLOW_IDLE_S + bevy_cauldron::follow::FOLLOW_EASE_S) {
        app.update();
    }
    assert_eq!(
        rig(&app),
        before,
        "the follow moved the camera with Follow the agent off"
    );
    let chip = app
        .world()
        .resource::<bevy_cauldron::follow::LookChip>()
        .sentence()
        .unwrap_or_else(|| panic!("with the toggle off the chip is the only way to see the change, \
                                    and it says nothing"));
    assert!(chip.contains("Look"), "the chip does not offer Look: {chip:?}");

    // **Look frames it now**: the chip's own button. The key census has no headroom for a new chord
    // (`no_context_gains_a_key` is pinning every context at its measured count), so the issue's other
    // two routes are the ones built — the chip's Look button here, and the AGENT block's line.
    let look = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<bevy_cauldron::follow::LookButton>>();
        q.iter(world).next().expect("the look chip draws its Look button while a change waits")
    };
    app.world_mut().trigger(bevy_gooey::Activate { entity: look });
    app.update();
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S) {
        app.update();
    }
    let corners = footprint_corners(&app, &placed).expect("the placed bed measures");
    let fill = fill(&mut app, &corners);
    assert!(
        fill >= FRAMED_MIN_FILL,
        "Look did not frame the change: it fills {:.0}% of the viewport",
        fill * 100.0
    );
}

/// **3b. The setting is a settings key, so a restart reads the file back.**
#[test]
fn the_follow_setting_survives_a_restart() {
    let root = room("pace_restart");
    {
        let mut app = editor(&root);
        let settings = app.world().resource::<Project>().project_dir.join("editor.yaml");
        std::fs::write(&settings, "{version: 1, follow_agent: false}").expect("editor.yaml");
        for _ in 0..frames_of(1.5) {
            app.update();
        }
        assert!(
            !app.world().resource::<EditorSettings>().follow_agent,
            "the file did not reach the running editor"
        );
    }
    // **A second editor on the same project.** The file is the only thing carried over, which is
    // what "survives a restart" means for a settings key.
    let mut again = editor(&root);
    for _ in 0..frames_of(1.5) {
        again.update();
    }
    assert!(
        !again.world().resource::<EditorSettings>().follow_agent,
        "the setting did not survive a restart: the second editor follows again"
    );
}

/// **3c. An old `editor.yaml` with no `follow_agent` reads as on**, which is the default rather than a
/// stored `false`: the layer carries `Option`, so an absent key falls through.
#[test]
fn an_old_settings_file_with_no_follow_agent_reads_as_on() {
    let root = room("pace_old_settings");
    // **A settings file from before this key existed**: every key it sets is one an older editor
    // could write, and `follow_agent` is not among them.
    let mut app = editor(&root);
    let settings = app.world().resource::<Project>().project_dir.join("editor.yaml");
    std::fs::write(&settings, "{version: 1, show_grid: false, reduced_motion: true}")
        .expect("editor.yaml");
    for _ in 0..frames_of(1.5) {
        app.update();
    }
    let settings = app.world().resource::<EditorSettings>();
    assert!(!settings.show_grid, "the old file's own keys still apply");
    assert!(
        settings.follow_agent,
        "an editor.yaml written before `follow_agent` existed turned the follow off, which would mean \
         the default was read as a stored false"
    );
}

/// **4. The status line is not the chip**: the follow's own state leaves the receipt alone, so a
/// person reading what the agent did does not lose it to a chip update.
#[test]
fn the_chip_is_not_the_receipt() {
    let root = room("pace_receipt");
    let mut app = editor(&root);
    a_person_drags(&mut app);
    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    for _ in 0..frames_of(FOLLOW_IDLE_S) {
        app.update();
    }
    let note = app.world().resource::<EditorState>().status.note_text().to_owned();
    assert!(
        note.contains("the Familiar ") && note.contains("placed"),
        "the receipt was replaced by the chip's words: {note:?}"
    );
}
