//! **A background job is visible in the status band and can be stopped** (UI-22, #223).
//!
//! The editor's long work — today the palette's thumbnail bake — ran with nothing on screen saying so
//! and no way to stop it. `docs/ui/vision.md` P6 is the rule: from one second say something is
//! happening, from ten show a percentage. Three claims, each of which can fail:
//!
//! 1. **A job shows in the band, in its own words.** A band that stayed silent while forty portraits
//!    were baked is the state this issue exists to end.
//! 2. **Nothing running says nothing** — not "0 jobs". A row that is always there is a row a reader
//!    stops seeing, which is the rule `chrome::reload_readout` already keeps.
//! 3. **Stopping the bake stops it, and does not stand it back up.** This is the half that cannot be
//!    satisfied by a resource alone: `Thumbnails::unfinished` asks the library every frame whether
//!    anything is left, so without a stop that the run condition reads, the booth returns on the very
//!    next frame and the button stops nothing. Measured on this test: with the flag, nothing is
//!    staged after the stop and the job leaves the band; without it the bake restarts.
//!
//! The band's readout is `Jobs::line`, and both the system and this file read the same function, so an
//! assertion here is an assertion about what is painted rather than about a resource that happens to
//! hold the same value.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::jobs::{Jobs, JobsItem};

use crate::fixtures;

/// An editor whose library has `pieces` descriptors, each with a mesh, so the bake has work to do.
fn editor(tag: &str, pieces: usize) -> App {
    let mut fixture = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()));
    for i in 0..pieces {
        fixture = fixture.descriptor(&format!("piece_{i}"), "alpha");
    }
    let root = fixture.build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }
    app
}

/// What the band's job item reads, or `None` when the item is not drawn.
fn band_said(app: &mut App) -> Option<String> {
    let drawn: Vec<Entity> = {
        let mut items = app.world_mut().query_filtered::<(&Node, &Children), With<JobsItem>>();
        items
            .iter(app.world())
            .filter(|(node, _)| node.display != Display::None)
            .flat_map(|(_, children)| children.iter().collect::<Vec<_>>())
            .collect()
    };
    let mut words = app.world_mut().query::<&Text>();
    drawn
        .into_iter()
        .filter_map(|child| words.get(app.world(), child).ok().map(|t| t.0.clone()))
        .find(|text| !text.is_empty())
}

/// **The bake says so.** The band carries the job's own words while a portrait is being taken, and it
/// is gone when the library is fully baked — which is the second half of the same assertion: an item
/// that never leaves is not a report of work, it is furniture.
#[test]
fn the_bake_shows_in_the_band_and_leaves_when_it_is_done() {
    let mut app = editor("jobs-band", 3);
    // A headless harness has no GPU, so the bake cannot finish a portrait: it stages and waits. That is
    // exactly the state a person is in for the first thirty seconds of a real session.
    let shown = band_said(&mut app).expect("the band must say a bake is running");
    assert!(
        shown.starts_with("Baking previews"),
        "the band says what the work is in its own words: {shown:?}"
    );
    let jobs = app.world().resource::<Jobs>();
    assert_eq!(jobs.line(), shown, "the band draws `Jobs::line`, not a second answer");
    assert!(
        shown.contains("of 3"),
        "the words say how far along it is, because a bar alone cannot say 'of 3': {shown:?}"
    );
    assert_eq!(jobs.len(), 1, "one bake is one job, not one per piece: {:?}", jobs.snapshot());
}

/// **Stopping it stops it.** The request reaches the producer, the producer's own seam ends the job,
/// the band goes quiet, and the booth does not stand back up — which is the clause `unfinished` is.
#[test]
fn stopping_the_bake_stops_it_and_it_stays_stopped() {
    let mut app = editor("jobs-stop", 3);
    assert!(band_said(&mut app).is_some(), "the bake is running before it is stopped");
    app.world().resource::<Jobs>().request_cancel("thumbs");
    for _ in 0..3 {
        app.update();
    }
    assert!(
        band_said(&mut app).is_none(),
        "a stopped job leaves the band in the same frame the producer gives up"
    );
    assert!(app.world().resource::<Jobs>().is_empty(), "and its record goes with it");

    // **And it stays gone.** `Thumbnails::unfinished` is a run condition that asks the library whether
    // a portrait is missing, so this is the assertion that the stop outranks it.
    let mut baked = 0;
    for _ in 0..20 {
        app.update();
        let thumbs = app.world().resource::<bevy_cauldron::thumbs::Thumbnails>();
        baked += thumbs.is_baked("piece_0") as usize;
    }
    assert_eq!(
        baked, 0,
        "the bake came back after being stopped: `unfinished` asked the library, not the person"
    );
    assert!(
        band_said(&mut app).is_none(),
        "and it must not reappear in the band either — a job that restarts itself is a stop that lies"
    );
}

/// **The band's item is not a button, and says so in its role.**
///
/// The item is deliberately off the `Activate` bus (this editor has twenty-four global `Activate`
/// observers and a `Button` fires them all — `chrome::quiet_row` documents the day that took the
/// application down). Its words therefore must not carry `Role::ButtonText`, which is how a test counts
/// the interface's *buttons*: `a_question_is_asked_in_a_gooey_dialog` reads every `ButtonText` in the
/// world and asserts a dialog's two answers are the whole list. With a job running, a `ButtonText` here
/// made that list three long and both of its tests failed — the full workspace gate caught it, not this
/// file, which is why the shape is pinned here now.
#[test]
fn the_bands_item_does_not_call_itself_a_button() {
    let mut app = editor("jobs-not-a-button", 3);
    let words: Vec<bevy_gooey::Role> = {
        let mut q = app.world_mut().query_filtered::<&bevy_gooey::Role, With<Text>>();
        q.iter(app.world())
            .filter(|role| matches!(**role, bevy_gooey::Role::ButtonText | bevy_gooey::Role::ControlText))
            .copied()
            .collect()
    };
    assert!(
        words.iter().all(|role| *role == bevy_gooey::Role::ControlText),
        "a job's words must not read as a button's: {words:?}"
    );
}

/// **Nothing running says nothing**, which is the state the editor is in almost all the time.
#[test]
fn a_band_with_no_job_draws_no_item() {
    let mut app = editor("jobs-quiet", 0);
    app.world_mut().resource_mut::<Jobs>();
    // With no descriptor there is nothing to bake, so the job never starts.
    assert!(app.world().resource::<Jobs>().is_empty(), "no library, no bake");
    assert_eq!(band_said(&mut app), None, "an item for no job is a row that says nothing forever");
}

/// **The band's bar reads the job's fraction, and two jobs have no one fraction.** The second half is
/// what keeps the band honest: with two jobs running there is no single answer, so the item says both
/// words and draws no bar rather than picking one job's number.
#[test]
fn the_bar_is_the_one_jobs_fraction_and_two_jobs_have_none() {
    let app = editor("jobs-bar", 3);
    let jobs = app.world().resource::<Jobs>();
    assert!(
        jobs.band_progress().is_some(),
        "one job with a measurable length has a fraction for the band's bar"
    );
    jobs.set("label", "Labelling", "", None);
    assert_eq!(jobs.band_progress(), None, "two jobs have no one fraction, so the bar is not drawn");
    assert!(
        jobs.line().contains("+1"),
        "and the band names the second rather than hiding it: {}",
        jobs.line()
    );
}

/// **The band's item opens a list, and its Stop row is what asks the producer to stop.**
///
/// This is the half a band's one line cannot carry: two jobs at once, each stoppable, and stopping one
/// leaves the other running. The click is gooey's own `PointerClick` on the item, which is the event a
/// person's press produces — the item is deliberately not a `Button`, so `Activate` never carries it
/// (see `chrome::quiet_row` for the twenty-four global observers that made that necessary).
#[test]
fn the_list_names_every_job_and_one_stop_row_stops_only_its_own() {
    let mut app = editor("jobs-list", 3);
    let item = {
        let mut items = app.world_mut().query_filtered::<Entity, With<JobsItem>>();
        items.iter(app.world()).next().expect("the band carries a job item")
    };
    // A second job, so "stops only its own" has a second job to leave alone.
    app.world().resource::<Jobs>().set("label", "Labelling", "", None);

    click(&mut app, item);

    let rows: Vec<&'static str> = {
        let mut q = app.world_mut().query::<&bevy_cauldron::jobs::CancelJob>();
        q.iter(app.world()).map(|r| r.0).collect()
    };
    assert_eq!(rows.len(), 2, "the list has one row per running job, and no more: {rows:?}");
    assert!(rows.contains(&"thumbs") && rows.contains(&"label"), "each row names its own job: {rows:?}");

    // Stop the bake, through the row rather than through the resource, so the observer is on the path.
    let bake_row = {
        let mut q = app.world_mut().query::<(Entity, &bevy_cauldron::jobs::CancelJob)>();
        q.iter(app.world()).find(|(_, r)| r.0 == "thumbs").map(|(e, _)| e).expect("a row for the bake")
    };
    click(&mut app, bake_row);
    for _ in 0..3 {
        app.update();
    }

    let jobs = app.world().resource::<Jobs>();
    assert!(!jobs.cancel_requested("label"), "stopping one job leaves the other alone");
    assert!(!jobs.snapshot().iter().any(|(key, _)| *key == "thumbs"), "the stopped bake's record is gone");
}

/// **A primary press on `target`**, through the same `Pointer::new` a real click is made with: the
/// editor draws into an image and a test app has no window, so the pointer names the surface.
fn click(app: &mut App, target: Entity) {
    app.world_mut().commands().trigger(bevy::picking::events::PointerClick {
        entity: target,
        pointer: bevy::picking::events::Pointer::new(
            bevy::picking::pointer::PointerId::Mouse,
            bevy::picking::pointer::Location {
                target: bevy::camera::NormalizedRenderTarget::Image(bevy::camera::ImageRenderTarget {
                    handle: bevy::asset::Handle::default(),
                    scale_factor: 1.0,
                }),
                position: bevy::math::Vec2::new(100.0, 700.0),
            },
        ),
        button: bevy::picking::pointer::PointerButton::Primary,
        hit: bevy::picking::backend::HitData::new(target, 0.0, None, None),
        duration: std::time::Duration::ZERO,
        count: 1,
    });
    app.update();
}

// ── the agent's door ─────────────────────────────────────────────────────────────────────────────

/// **An agent sees the same jobs the band draws and stops one through the same request** (UI-22, #223;
/// `docs/ui/vision.md` P1, "one command, many doors").
///
/// The band is one door onto `Jobs`; `grimoire/state.jobs` is the other, and `grimoire/jobs_stop` makes
/// the same request the popover's Stop row makes — so a job is never visible to one and invisible to
/// the other, and the recorder can aim at the band's words by reading them rather than guessing.
#[test]
fn an_agent_reads_the_jobs_the_band_draws_and_stops_one_through_the_same_request() {
    let mut app = editor("jobs-agent", 3);
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }

    let state = call(&mut app, "grimoire/state", serde_json::json!({}));
    let jobs = &state["jobs"];
    assert_eq!(jobs["count"], serde_json::json!(1), "state: {state}");
    assert_eq!(
        jobs["line"], serde_json::json!(band_said(&mut app).expect("the band says it")),
        "the agent reads exactly the words the band draws, not a second answer"
    );
    assert_eq!(jobs["items"][0]["key"], serde_json::json!("thumbs"), "state: {state}");

    // **A key nobody is running refuses and names what is**, so "stopped nothing" cannot read as success.
    let refused = call(&mut app, "grimoire/jobs_stop", serde_json::json!({"key": "no_such_job"}));
    assert_eq!(refused["outcome"], serde_json::json!("refused"), "answer: {refused}");
    assert_eq!(refused["running"][0]["key"], serde_json::json!("thumbs"), "answer: {refused}");
    assert!(!app.world().resource::<Jobs>().cancel_requested("thumbs"), "a refusal stops nothing");

    let asked = call(&mut app, "grimoire/jobs_stop", serde_json::json!({"key": "thumbs"}));
    assert_eq!(asked["outcome"], serde_json::json!("asked"), "answer: {asked}");
    assert!(
        app.world().resource::<Jobs>().cancel_requested("thumbs"),
        "the door sets the same flag the band's Stop row sets"
    );
    for _ in 0..3 {
        app.update();
    }
    let after = call(&mut app, "grimoire/state", serde_json::json!({}));
    assert_eq!(after["jobs"]["count"], serde_json::json!(0), "the bake stopped when its own seam came: {after}");
}

/// One BRP call, as `a_command_runs_by_id_through_the_agents_door.rs` makes it.
fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("no method `{method}` is registered"),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}
