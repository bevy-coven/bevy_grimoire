//! **The first-map tour** (#253): four steps on a real map — pick a piece, place it, turn it, save — offered once and always
//! available from Help.
//!
//! # It is the agent's guide, not a second one
//!
//! `guided.rs` forbids a second way to load a script: an agent posts one over BRP, so the editor's own "start the tour" posts
//! `guides/first_map.json` through the **same handler** ([`bevy_debugger_bevy::handle_guide`]) and the card is the same overlay
//! (`GuideOverlayPlugin`). Nothing here knows what a step looks like; the steps' conditions are checkpoints registered in
//! `guided.rs` like every other.
//!
//! # Who watches the checkpoints
//!
//! `bevy_fairy/guide+watch` is the watching half and it is a BRP *watching* method: nothing runs it unless a client is attached.
//! An agent is that client; a person following the tour is not, so while a tour runs [`drive_the_tour`] runs the same
//! [`bevy_debugger_bevy::watch_guide`] every frame, and the step advances when its condition arrives. It runs only while a
//! [`TourRunning`] stands, so an agent's own script is never read by a second consumer.
//!
//! # Pulled, never pushed
//!
//! The editor offers the tour **once** ([`offer_the_tour`], `Recent::tour_offered`), as a toast the person can ignore. It stays
//! at Help → Take the tour. A person who chose *Take the tour* on the home's Learn page ([`crate::launcher::TourRequested`]) has
//! already said yes: the tour starts when the editor opens.
//!
//! The overlay's words are drawn in Bevy's default font, which has 95 characters: the script is plain ASCII.

use bevy::prelude::*;
use bevy_debugger_bevy::{Guide, handle_guide, watch_guide};
use bevy_gooey::{ToastAction, toast, toast_alert};
use serde_json::Value;

use crate::editor::EditorState;
use crate::launcher::TourRequested;
use crate::project::{OpenMap, Project};
use crate::screen::Screen;
use crate::tiles::Door;

/// The script the tour posts: the file an agent could post, compiled in so an app bundle has it.
pub const SCRIPT: &str = include_str!("../guides/first_map.json");

/// What the toast says.
pub const OFFER: &str = "New here? Take the 2-minute tour";

/// The toast's buttons, by what they say and the id each sends.
pub const START_LABEL: &str = "Start";
pub const NOT_NOW_LABEL: &str = "No thanks";
const START: &str = "tour-start";
const NOT_NOW: &str = "tour-no";

/// **The map as it stood when the tour began**, so a step can ask what the person did and not what was already there: the
/// example map arrives with thirteen pieces placed and a turn of one of them is not a turn made in the tour.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct Tour {
    /// How many placements the map had.
    pub placements: usize,
    /// Each placement's yaw, in the map's order.
    pub yaws: Vec<f32>,
    /// The yaw the piece in hand was at.
    pub brush_yaw: f32,
}

impl Tour {
    /// The baseline of `open` with `state`'s piece in hand.
    pub fn of(open: &OpenMap, state: &EditorState) -> Tour {
        Tour {
            placements: open.map.placements.len(),
            yaws: open.map.placements.iter().map(|p| p.yaw).collect(),
            brush_yaw: state.brush_yaw,
        }
    }

    /// Has a piece been placed since?
    pub fn placed(&self, open: &OpenMap) -> bool {
        open.map.placements.len() > self.placements
    }

    /// **Has anything been turned since**: the piece in hand, one that was down, or one that was placed after.
    pub fn turned(&self, open: &OpenMap, state: &EditorState) -> bool {
        let placement_turned = |(at, placed): (usize, &bevy_grimoire_core::map::Placed)| match self.yaws.get(at) {
            Some(was) => (*was - placed.yaw).abs() > f32::EPSILON,
            // A piece placed in the tour starts upright unless it was turned in hand.
            None => placed.yaw.abs() > f32::EPSILON,
        };
        (state.brush_yaw - self.brush_yaw).abs() > f32::EPSILON || open.map.placements.iter().enumerate().any(placement_turned)
    }
}

/// **A tour is to start in the first frame the editor is up.** Entering the editor seats the first piece in hand and the like; a tour
/// started inside that would be started on a state it is about to change, so it waits one frame ([`start_the_due_tour`]).
#[derive(Resource, Debug, Clone, Copy)]
struct TourDue;

/// **A tour is running**: [`drive_the_tour`] watches its steps while this stands.
#[derive(Resource, Debug, Clone, Copy)]
pub struct TourRunning;

pub struct TourPlugin;

impl Plugin for TourPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_the_offer_answered)
            .add_systems(OnEnter(Screen::Editor), offer_the_tour)
            .add_systems(OnExit(Screen::Editor), end_the_tour)
            .add_systems(Update, (start_the_due_tour.run_if(resource_exists::<TourDue>), drive_the_tour.run_if(resource_exists::<TourRunning>)).chain().run_if(in_state(Screen::Editor)));
    }
}

/// **Why the tour cannot start here**, or `None`. It is on a map, with a piece to pick.
pub fn why_not(world: &World) -> Option<&'static str> {
    if world.get_resource::<State<Screen>>().is_none_or(|s| *s.get() != Screen::Editor) {
        return Some("The tour is on a map: open a project first.");
    }
    if world.get_resource::<Door>().is_none_or(|d| *d != Door::Map) || world.get_resource::<OpenMap>().is_none() {
        return Some("The tour is on a map: switch to the Map first.");
    }
    if world.get_resource::<Project>().is_none_or(|p| p.library.descriptors.is_empty()) {
        return Some("This project has no pieces yet: add some meshes, then take the tour.");
    }
    None
}

/// **Start the tour**: nothing in hand, the map's baseline taken, and the script posted through the agent's handler.
pub fn start(world: &mut World) -> Result<(), String> {
    if let Some(why) = why_not(world) {
        return Err(why.to_owned());
    }
    let script: Value = serde_json::from_str(SCRIPT).map_err(|e| format!("the tour's script does not read: {e}"))?;
    // Nothing in hand, so step one is a thing the person does.
    world.resource_mut::<EditorState>().brush = None;
    let tour = Tour::of(world.resource::<OpenMap>(), world.resource::<EditorState>());
    world.insert_resource(tour);
    match world.run_system_cached_with(handle_guide, Some(script)) {
        Ok(Ok(_)) => {
            world.insert_resource(TourRunning);
            Ok(())
        }
        Ok(Err(e)) => Err(format!("the tour was not posted: {}", e.message)),
        Err(e) => Err(format!("the tour was not posted: {e}")),
    }
}

/// Start the tour, or say why not in a toast.
pub fn start_or_say_why(world: &mut World) {
    if let Err(why) = start(world) {
        world.spawn(toast_alert(why).sticky());
    }
}

/// The frame after the editor was entered: start the tour that was asked for.
fn start_the_due_tour(world: &mut World) {
    world.remove_resource::<TourDue>();
    start_or_say_why(world);
}

/// **Watch the running tour's step**, as a client of `guide+watch` would. The script finishing, or failing to be watched at all,
/// ends the run; the card hides itself once the last step's confirmation has been shown.
fn drive_the_tour(world: &mut World) {
    match world.run_system_cached_with(watch_guide, None) {
        Ok(Ok(Some(answer))) if answer.get("done").and_then(Value::as_bool) == Some(true) => finish(world),
        Ok(Ok(_)) => {}
        Ok(Err(e)) => {
            warn!("the tour cannot be watched: {}", e.message);
            end(world);
        }
        Err(e) => {
            warn!("the tour cannot be watched: {e}");
            end(world);
        }
    }
}

/// The script is done: stop watching, and let the card show its last confirmation and go.
fn finish(world: &mut World) {
    world.remove_resource::<TourRunning>();
    world.remove_resource::<Tour>();
}

/// The tour is over before its script is: take the card down.
fn end(world: &mut World) {
    if let Some(mut guide) = world.get_resource_mut::<Guide>() {
        guide.clear();
    }
    finish(world);
}

/// A tour belongs to the map it was started on: closing the project takes it down.
fn end_the_tour(world: &mut World) {
    if world.contains_resource::<TourRunning>() {
        end(world);
    }
}

/// **On entering the editor: start the tour a person asked for, or offer it once.**
fn offer_the_tour(
    mut commands: Commands,
    recents: Option<Res<crate::recent::RecentProjects>>,
    requested: Option<Res<TourRequested>>,
    project: Option<Res<Project>>,
) {
    let requested = requested.is_some();
    if requested {
        commands.remove_resource::<TourRequested>();
    }
    let Some(file) = recents.as_deref().map(|r| r.0.clone()) else {
        if requested {
            commands.insert_resource(TourDue);
        }
        return;
    };
    let offered = match crate::recent::Recent::load(&file) {
        Ok(recent) => recent.tour_offered,
        Err(e) => {
            warn!("the tour is not offered: {e}");
            return;
        }
    };
    if requested {
        commands.insert_resource(TourDue);
    } else if !offered && project.is_some_and(|p| !p.library.descriptors.is_empty()) {
        commands.spawn(toast(OFFER).action(START_LABEL, START).action(NOT_NOW_LABEL, NOT_NOW));
    } else {
        return;
    }
    // Said once: whichever way it is answered, or ignored.
    if !offered
        && let Err(e) = crate::recent::mark_tour_offered(&file)
    {
        warn!("the tour's offer was not remembered: {e}");
    }
}

/// The offer's **Start**.
fn on_the_offer_answered(action: On<ToastAction>, mut commands: Commands) {
    if action.id == START {
        commands.queue(start_or_say_why);
    }
}
