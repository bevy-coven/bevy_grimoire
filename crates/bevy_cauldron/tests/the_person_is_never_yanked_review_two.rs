//! **Review round 2 of AF-04 (#360): the Look button works, the chip exists only while it says
//! something, and a wheel counts as a person working the view.**
//!
//! Each test here failed before the fix it pins, and each names the item it comes from:
//!
//! - **acceptance 3** — `look_at_the_change` was **never registered as an observer** (`grep
//!   add_observer` found only the definition), so the chip's Look button did nothing. And
//!   `look_now` reset the busy clock, which `take_the_follow_request` never reached with the toggle
//!   off: it returned at `if !follow_agent` first, so with the toggle off Look could never frame.
//!   The first test below is the one that proves it: **Look frames inside the dwell**, which a
//!   follow that merely waited out 0.6 s cannot.
//! - **the hidden widget** — `draw_the_look_chip` spawned the chip once and kept it hidden with
//!   `Display::None`: an entity that exists for ever, with a `Text` and a `Button` in it, which is
//!   the trap that protected tests counting buttons and words fall into. It is spawned only while
//!   something waits and despawned when nothing does.
//! - **acceptance 1** — a wheel or trackpad turn over the map (`view.rs` reads
//!   `AccumulatedMouseScroll`) is a person using the view and must hold the follow, exactly as a key
//!   or a button does.

use std::time::Duration;

use bevy::input::mouse::MouseScrollUnit;
use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;

use bevy_cauldron::follow::{Following, LookButton, LookChipNode, FOLLOW_DWELL_S, FOLLOW_IDLE_S};
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;
use bevy_cauldron::settings::EditorSettings;
use bevy_cauldron::view::{MainCamera, Rig};

use crate::fixtures;

const FRAME: f32 = 1.0 / 60.0;

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

fn point_away(app: &mut App) {
    app.world_mut().resource_mut::<Rig>().focus = Vec3::new(-28.0, 0.0, -28.0);
    app.world_mut().resource_mut::<Rig>().height = 4.0;
    for _ in 0..3 {
        app.update();
    }
    // **Settle the camera after the move**, so the only travel this test can see is the follow's.
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S * 2.0) {
        app.update();
    }
}

fn footprint_corners(app: &App, id: &str) -> Option<[Vec3; 4]> {
    use bevy_cauldron::project::OpenMap;
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

/// **How much of the viewport the box fills, and zero when it is not on screen at all.**
///
/// **A box that is off screen fills nothing**, and `Camera::world_to_viewport` answers `Ok` with
/// coordinates outside the viewport for a point beside the camera — so a measurement that only took
/// the corners' span read a piece twenty metres away as "framed". Measured: that made a five-change
/// burst look like two changes arriving 0.02 s apart, because the second piece's off-screen span was
/// large enough to pass the threshold. Every corner has to be inside for the box to count.
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
        if p.x < rect.min.x || p.y < rect.min.y || p.x > rect.max.x || p.y > rect.max.y {
            return 0.0;
        }
        lo = lo.min(p);
        hi = hi.max(p);
    }
    (hi.x - lo.x).abs().max((hi.y - lo.y).abs()) / rect.width().min(rect.height())
}

fn the_one_placed(app: &App) -> String {
    use bevy_cauldron::project::OpenMap;
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

/// **The `n`th placed piece**, by map order rather than by the answer.
fn the_one_placed_from(app: &App, n: usize) -> String {
    use bevy_cauldron::project::OpenMap;
    let open = app.world().resource::<OpenMap>();
    let found: Vec<&str> = open
        .map
        .placements
        .iter()
        .map(|p| p.id.as_str())
        .filter(|id| *id != "bed@0")
        .collect();
    assert!(
        found.len() >= n,
        "the map holds {} placed pieces, fewer than the {n}th asked for: {found:?}",
        found.len()
    );
    found[n - 1].to_owned()
}

/// **Look frames the change inside the dwell**, which only a follow that was actually asked can do:
/// the alternative is waiting `FOLLOW_DWELL_S` out, and this asserts *before* it has passed.
#[test]
fn the_looks_button_frames_the_change_inside_the_dwell() {
    let root = room("round2_look_frames");
    let mut app = editor(&root);
    point_away(&mut app);
    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    // **The first change is framed at once**, which starts the dwell.
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S) {
        app.update();
    }
    // **Two more changes in one frame**, so a burst is waiting and the pace separates them — and Look
    // is what shows the first of them now. **A change arriving alone is not paced** (the round-3 fix to
    // `dwell_from`: the protected `an_agents_undo_frames_where_the_change_was` needs that), so a single
    // second change would have been framed at once and left nothing for Look.
    for at in [(-20.0, -18.0), (0.0, 24.0)] {
        let next = apply(
            &mut app,
            "placement_add",
            serde_json::json!({"piece": "bed", "at": [at.0, at.1]}),
        );
        assert_eq!(next["outcome"], "applied", "{next}");
    }
    // **No `point_away` here**: it presses nothing, but the harness's own `Rig` write would be the
    // only thing that moved the camera, and the wait this test is about is the *second change's*.
    // **The change Look will show**, which is the oldest one waiting: `placed` was framed already, so
    // measuring it would read the camera leaving it as a failure.
    let placed_count = {
        use bevy_cauldron::project::OpenMap;
        let open = app.world().resource::<OpenMap>();
        open.map.placements.iter().filter(|p| p.id != "bed@0").count()
    };
    assert_eq!(placed_count, 3, "the test needs three placements to have a burst waiting");
    let waiting_for = the_one_placed_from(&app, 2);
    let corners = footprint_corners(&app, &waiting_for).expect("the bed measures");
    let before_fill = fill(&mut app, &corners);
    assert!(before_fill < 0.25, "the second change is already framed, so Look has nothing to do");
    // **One frame, so the chip has been drawn**: `draw_the_look_chip` runs in `Update`, and the BRP
    // call above ran outside the schedule.
    app.update();
    assert!(
        app.world().resource::<Following>().pending() > 0,
        "the second change was not held behind the dwell, so there is nothing for Look to show"
    );

    let look = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<LookButton>>();
        q.iter(world).next().unwrap_or_else(|| panic!("the chip draws no Look button while a change waits"))
    };
    app.world_mut().trigger(bevy_gooey::Activate { entity: look });
    // **Half the dwell**, so waiting it out cannot be what produces the frame.
    for _ in 0..frames_of(FOLLOW_DWELL_S * 0.5) {
        app.update();
    }
    let after = fill(&mut app, &corners);
    assert!(
        after >= 0.25,
        "Look did nothing: the change waiting fills {before_fill:.2} of the viewport before the click \
         and {after:.2} half a dwell later, so the button never reached the follow"
    );
}

/// **No chip entity exists while nothing waits; one exists with its Look button while something
/// does.**
///
/// The first version spawned the chip once and hid it with `Display::None` — an entity that lives for
/// the whole session, carrying a `Text` and a `Button`, which is the trap the protected tests that
/// count buttons and words fall into.
#[test]
fn the_chip_exists_only_while_a_change_waits() {
    let root = room("round2_chip_lifetime");
    let mut app = editor(&root);
    // **Nothing waits yet**: no chip, and no Look button.
    for _ in 0..frames_of(0.5) {
        app.update();
    }
    assert!(
        app.world().resource::<Following>().pending() == 0,
        "the test needs an empty queue to prove the chip is absent"
    );
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<LookChipNode>>();
    assert_eq!(q.iter(world).count(), 0, "a chip exists with nothing to show");

    // **A change arrives, and the person is working**, so it waits and the chip is up.
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    for _ in 0..frames_of(0.2) {
        app.update();
    }
    {
        let world = app.world_mut();
        let mut chips = world.query_filtered::<Entity, With<LookChipNode>>();
        assert_eq!(chips.iter(world).count(), 1, "no chip while a change waits");
        let mut looks = world.query_filtered::<Entity, With<LookButton>>();
        assert_eq!(looks.iter(world).count(), 1, "the chip draws no Look button");
    }

    // **The person lets go and the wait runs out**: the change is shown, and the chip goes away.
    let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    mouse.release(MouseButton::Left);
    mouse.clear();
    app.update();
    for _ in 0..frames_of(FOLLOW_IDLE_S + bevy_cauldron::follow::FOLLOW_EASE_S) {
        app.update();
    }
    let world = app.world_mut();
    let mut chips = world.query_filtered::<Entity, With<LookChipNode>>();
    assert_eq!(
        chips.iter(world).count(),
        0,
        "the chip is still there after the change was shown"
    );
}

/// **A wheel or trackpad turn over the map is the person working the view** (acceptance 1): the
/// follow holds, and the chip says what waits.
#[test]
fn a_wheel_over_the_map_holds_the_follow() {
    let root = room("round2_wheel");
    let mut app = editor(&root);
    point_away(&mut app);
    // **The focus, not the height**: the wheel's own job is to zoom, so the height moving is the
    // person's gesture working, not the follow. What must not move is where the camera is looking —
    // a follow that landed them on the change at a scale they did not choose.
    let before = app.world().resource::<Rig>().focus;
    // **A wheel turn**, sent the way the platform sends one: a `MouseWheel` message, which
    // `bevy_input::mouse::accumulate_mouse_scroll_system` turns into the `AccumulatedMouseScroll`
    // resource `view.rs` reads. Writing the resource directly does not survive a frame — that system
    // resets it to zero on every one — which is what a person's wheel is: an event, not a state.
    app.world_mut().write_message(bevy::input::mouse::MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.update();
    // The wheel is cleared by Bevy's own system on the next frame, so one turned wheel is one event.
    assert!(
        app.world().resource::<Following>().busy(),
        "a wheel turn did not reach the follow's idle clock, so the test proves nothing"
    );

    // **Read where the camera is when the change lands**, not where it was before the wheel: the
    // wheel's own zoom eases, and that travel is the person's gesture working.
    let when_it_landed = app.world().resource::<Rig>().focus;
    assert_eq!(
        when_it_landed, before,
        "the wheel moved the camera's focus, so this test cannot tell a follow from a zoom"
    );
    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    // **The BRP call runs outside the schedule**, so the follow has not had a frame yet: this is the
    // state the next frame's gate reads.
    assert!(
        app.world().resource::<Following>().busy(),
        "the wheel's idle window closed before the change arrived"
    );
    // **The camera's own focus is the measurement**, and it is the right one here: the wheel's job is
    // to zoom, so a change in *height* is the person's gesture working — while the focus moving to the
    // new piece is the follow. (An earlier version measured fill, which the zoom alone changes.)
    let placed = the_one_placed(&app);
    let before_fill = {
        let corners = footprint_corners(&app, &placed).expect("the placed bed measures");
        fill(&mut app, &corners)
    };
    // **The idle window is `Time<Real>`**, which `TimeUpdateStrategy` does not drive (that is
    // `Time<Virtual>`), so this waits real seconds rather than a count of frames. Waiting frames
    // instead is what made an earlier version of this test watch a follow that had legitimately
    // landed 2 s later.
    // **The clock, not a stopwatch**: `Time<Real>` is stepped manually here, so its own reading is
    // what the idle window compares against. A wall-clock loop ran a different number of seconds
    // than the editor did (measured: the window closed after 20 frames because each frame moved the
    // clock 1/60 s while the loop watched real time).
    let until = app.world().resource::<Time<Real>>().elapsed_secs_f64()
        + f64::from(FOLLOW_IDLE_S) * 0.75;
    while app.world().resource::<Time<Real>>().elapsed_secs_f64() < until {
        app.update();
        assert_eq!(
            app.world().resource::<Rig>().focus,
            when_it_landed,
            "the camera went to the change while a person was turning the wheel over the map \
             (busy {}, pending {})",
            app.world().resource::<Following>().busy(),
            app.world().resource::<Following>().pending(),
        );
    }
    // **And it was held, not dropped**, so letting go shows it rather than losing it.
    assert_eq!(
        app.world().resource::<Following>().pending(),
        1,
        "the change was dropped rather than held while the person turned the wheel"
    );
    let _ = before_fill;
    // **The chip is up**, so they are told what waits.
    assert!(
        app.world().resource::<bevy_cauldron::follow::LookChip>().sentence().is_some(),
        "the chip says nothing while a wheel-using person has a change waiting"
    );
}

/// **The AGENT block's Look button shows the waiting change**, which is the issue's second Look place:
/// the block sits beside what the agent did, so the answer to "show me" belongs under it.
#[test]
fn the_agent_blocks_look_button_frames_the_change() {
    let root = room("round2_agent_block_look");
    let mut app = editor(&root);
    // **The block is opened by its own menu row**, so this is the block a person has and the same
    // observer does the spawning: `toggle_the_familiar` is an entity observer on View ▸ Show agent.
    let row = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<bevy_cauldron::familiar::FamiliarItem>>();
        q.iter(world).next().expect("the View menu has a Show agent row")
    };
    app.world_mut()
        .trigger(bevy_gooey::ValueChange { source: row, value: true, is_final: true });
    for _ in 0..frames_of(0.2) {
        app.update();
    }
    let look = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<bevy_cauldron::familiar::AgentLookButton>>();
        q.iter(world).next().expect("the AGENT block draws its Look button")
    };

    point_away(&mut app);
    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "bed", "at": [20.0, 18.0]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let placed = the_one_placed(&app);
    let corners = footprint_corners(&app, &placed).expect("the placed bed measures");
    let before = fill(&mut app, &corners);

    app.world_mut().trigger(bevy_gooey::Activate { entity: look });
    for _ in 0..frames_of(bevy_cauldron::follow::FOLLOW_EASE_S) {
        app.update();
    }
    let after = fill(&mut app, &corners);
    assert!(
        after > before && after >= 0.25,
        "the AGENT block's Look did not frame the change: {before:.2} -> {after:.2} of the viewport"
    );
}

/// **The dwell holds five changes apart by `FOLLOW_DWELL_S` each**, measured on the editor's own clock
/// rather than by counting frames: the review asked for the gap itself.
#[test]
fn five_changes_in_one_frame_are_held_apart_by_the_dwell() {
    let root = room("round2_dwell_gap");
    let mut app = editor(&root);
    let mut made = Vec::new();
    for (i, at) in [(20.0, 18.0), (-20.0, 18.0), (20.0, -18.0), (-20.0, -18.0), (0.0, 24.0)].into_iter().enumerate() {
        let answer = apply(
            &mut app,
            "placement_add",
            serde_json::json!({"piece": "bed", "at": [at.0, at.1]}),
        );
        assert_eq!(answer["outcome"], "applied", "{answer}");
        // **The piece the `n`th add made**, by map order: `placement_add` mints its id at commit and
        // the map is what a person sees.
        made.push(the_one_placed_from(&app, i + 1));
    }
    // **The second the camera came to rest on each change**, in the order they were made.
    //
    // **One arrival per change, not one per frame it happens to be framed on.** The first version
    // pushed a row every frame the *next* piece was in view, so an arrival and the arrival after it
    // came from the same frame and were 0.02 s apart by construction. A change counts as arrived on
    // the frame its own box is framed and the camera has stopped, and the next change is looked for
    // only after that.
    let mut arrivals: Vec<(usize, f64)> = Vec::new();
    let limit = app.world().resource::<Time<Real>>().elapsed_secs_f64() + 8.0;
    while app.world().resource::<Time<Real>>().elapsed_secs_f64() < limit && arrivals.len() < made.len() {
        app.update();
        if app.world().resource::<Following>().settling() {
            continue;
        }
        let next = arrivals.len();
        let corners = footprint_corners(&app, &made[next]).expect("the bed measures");
        if fill(&mut app, &corners) >= 0.25 {
            let at = app.world().resource::<Time<Real>>().elapsed_secs_f64();
            arrivals.push((next, at));

        }
    }
    assert_eq!(
        arrivals.len(),
        made.len(),
        "{} of {} changes were framed, so their spacing cannot be measured",
        arrivals.len(),
        made.len()
    );
    // **In order**, and each held at least `FOLLOW_DWELL_S` after the one before.
    let order: Vec<usize> = arrivals.iter().map(|(i, _)| *i).collect();
    assert_eq!(order, (0..made.len()).collect::<Vec<_>>(), "the burst was framed out of order");
    for pair in arrivals.windows(2) {
        let held = pair[1].1 - pair[0].1;
        assert!(
            held >= f64::from(FOLLOW_DWELL_S) - f64::from(FRAME),
            "two changes were framed {held:.2} s apart, less than the {FOLLOW_DWELL_S} s the issue \
             holds them apart by"
        );
    }
}

/// **Another project clears the follow's queue and its chip**, which nothing did: a queued request
/// names a place in metres on *that* project's map, so carrying it over would frame a spot the new
/// project does not have — the same stale-value class `screen::OWNERSHIP` exists for.
#[test]
fn another_project_clears_the_follows_queue() {
    let root = room("round3_project_change");
    let mut app = editor(&root);
    // **A door the follow cannot cross to**, so the request stays queued: on the Map door the very
    // next frame would take it and an empty queue would prove nothing.
    app.world_mut().insert_resource(bevy_cauldron::workspace::Workspace::Kit);
    app.world_mut()
        .resource_mut::<Following>()
        .request(bevy_cauldron::follow::FollowRequest {
            target: bevy_cauldron::editor::FrameTarget::Map,
            ids: Vec::new(),
        });
    app.update();
    assert!(
        app.world().resource::<Following>().pending() > 0
            || app.world().resource::<Following>().settling(),
        "a request is held, so the test proves something"
    );
    assert!(app.world().resource::<Following>().pending() > 0, "a request waits, so the test proves something");
    assert!(
        app.world().resource::<bevy_cauldron::follow::LookChip>().sentence().is_some(),
        "the chip names it while it waits"
    );

    // **Another project**, replaced the way `Project::open` leaves it.
    let fresh = Project::open(&root, None).expect("the same project reopens");
    app.world_mut().insert_resource(fresh);
    app.update();

    assert_eq!(
        app.world().resource::<Following>().pending(),
        0,
        "a project change kept a follow whose place is a metre figure of the previous project's map"
    );
    // **The chip's words are a frame behind `Following` by design** (`follow_the_chip` reads the
    // resource after this frame's other systems), so this waits one frame for it — the queue itself
    // is asserted above, on the frame the project changed.
    app.update();
    assert!(
        app.world().resource::<bevy_cauldron::follow::LookChip>().sentence().is_none(),
        "the chip still names a change from the previous project: {:?}",
        app.world().resource::<bevy_cauldron::follow::LookChip>().sentence()
    );
}

/// **The toggle row writes the user's layer**, which is what `settings.rs`'s own doc says the follow
/// setting is: the person's preference across projects, not this project's (`Assumed decision`).
#[test]
fn the_toggle_row_is_ticked_from_the_setting_and_writes_the_users_layer() {
    let root = room("round2_toggle_row");
    let mut app = editor(&root);
    let user = root.join(format!("round2_user_{}/settings.ron", std::process::id()));
    std::fs::create_dir_all(user.parent().expect("a parent")).expect("the user's dir");
    app.insert_resource(bevy_cauldron::settings::UserSettingsFile(user.clone()));
    for _ in 0..frames_of(0.5) {
        app.update();
    }
    // **The row exists and is ticked**, because the setting is on by default.
    let row = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(Entity, Has<bevy_gooey::Checked>), With<bevy_cauldron::familiar::FollowItem>>();
        q.iter(world).next().expect("the View menu has a Follow the agent row")
    };
    assert!(row.1, "the row is not ticked while the setting is on");

    // **A click turns it off**: `toggle_the_follow` is a global observer, and `ValueChange` is the
    // event gooey's toggle sends, so this is the same message a press makes.
    app.world_mut().trigger(bevy_gooey::ValueChange {
        source: row.0,
        value: false,
        is_final: true,
    });
    app.update();
    for _ in 0..frames_of(0.2) {
        app.update();
    }
    let text = std::fs::read_to_string(&user).unwrap_or_else(|e| {
        panic!(
            "the user's settings file was not written ({e}). The status line says: {:?}",
            app.world().resource::<bevy_cauldron::editor::EditorState>().status.problem_text()
        )
    });
    assert!(
        text.contains("follow_agent: Some(false)"),
        "the row did not write the person's own layer: {text}"
    );
    // **And the project's file is untouched**, because this is not this project's preference.
    let project_file = app.world().resource::<Project>().project_dir.join("editor.ron");
    assert!(
        !project_file.exists() || !std::fs::read_to_string(&project_file).unwrap_or_default().contains("follow_agent"),
        "the row wrote the project's editor.ron, which is another project's preference to set"
    );
    // **And the running editor follows it**, which is the setting reaching its one reader.
    for _ in 0..frames_of(1.5) {
        app.update();
    }
    assert!(
        !app.world().resource::<EditorSettings>().follow_agent,
        "the row's write did not reach the running settings"
    );
}
