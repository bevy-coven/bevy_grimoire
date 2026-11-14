//! **Work that runs while a person works** (UI-22, #223): one record per background job, said in the
//! status band, listed in a popover, and stoppable.
//!
//! # Why the record is here rather than inside each producer
//!
//! The editor already ran long jobs with no way to see or stop them: the palette's thumbnail bake
//! (`thumbs.rs`), a model's labelling round trip (`labels.rs`), an accepted generate (`compose.rs`).
//! Each kept its own counters, and all a person could see was that the editor was slow.
//! `docs/ui/vision.md` P6 is the rule this breaks: from one second, say something is happening; from
//! ten, show a percent-done bar.
//!
//! **One record, keyed by producer.** A producer's key is a `&'static str` it owns (`"thumbs"`), so
//! there is exactly one place that decides what the band reads, and a producer that stops reporting
//! leaves nothing behind — a `Vec` with generated ids cannot say that, which is why this is not one.
//!
//! # Cancel is a request, and the producer owns the answer
//!
//! [`Jobs::cancel_requested`] answers whether a person asked to stop. This module never stops anything
//! itself: a job torn out from under its own in-flight work is how `thumbs.rs` came to panic on a
//! despawned screenshot entity (#264). The producer polls the flag at a seam it knows is safe — between
//! two pieces of the bake, never during a portrait's readback.
//!
//! # Why the state is behind a `Mutex` rather than `ResMut`
//!
//! The bake reports once per frame, and `ResMut` marks the resource changed on every deref — so a
//! change-gated band would rebuild every frame of the bake and a change-gated popover would respawn
//! itself just as often. A producer holds `Res<Jobs>` and calls [`Jobs::set`], which compares before
//! writing; the band compares the words it is about to write, which is `no_system_writes_every_frame`'s
//! second shape. Nothing here is gated on change detection at all.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use bevy::picking::events::PointerClick;
use bevy::prelude::*;
use bevy_gooey::Placement;

/// **One job, as the editor reports it.**
///
/// `title` is what the work is ("Baking previews") and `detail` how far along it is in the producer's
/// own words ("3 of 40"). Both are the producer's, because only it knows what a step is: this module
/// formats nothing, so a job cannot be made to say a number its producer did not compute.
#[derive(Clone, Debug)]
pub struct Job {
    /// What the work is, as the band and the popover say it.
    pub title: String,
    /// How far along, in the producer's words. Empty when it has nothing to add.
    pub detail: String,
    /// How much is done, `0.0` to `1.0`, when the work has a measurable length. `None` for work whose
    /// end nobody can estimate, which draws as a spinner rather than a bar that lies.
    pub progress: Option<f32>,
    /// Set when a person asks for it to stop. The producer polls it and is the one that stops.
    cancel: Arc<AtomicBool>,
}

impl Job {
    /// Whether a person has asked this job to stop.
    pub fn cancel_requested(&self) -> bool {
        self.cancel.load(Ordering::Acquire)
    }
}

/// **Every job the editor is running, keyed by the producer that runs it.**
///
/// Classified [`crate::screen::Ownership::Door`]: a door change tears its own work down, and a job
/// carried across would name a bake happening in a project nobody is looking at.
#[derive(Resource, Default)]
pub struct Jobs {
    open: Mutex<BTreeMap<&'static str, Job>>,
}

impl Jobs {
    fn open(&self) -> MutexGuard<'_, BTreeMap<&'static str, Job>> {
        // A producer panicking while holding this lock would otherwise take every later reader with
        // it; the map is replaced wholesale and there is no invariant to repair, so a poisoned lock is
        // recovered rather than propagated into a second panic.
        self.open.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// **Say that `key`'s job is running, and how far along it is.** Upserts.
    ///
    /// The cancel flag is the **same allocation** across reports: a request made while the job was at
    /// 3 of 40 is still the request its producer reads at 4 of 40. Re-making the flag per report was
    /// the first draft's bug, and it presented as a Cancel button that did nothing, with no failing
    /// assertion anywhere.
    pub fn set(
        &self,
        key: &'static str,
        title: impl Into<String>,
        detail: impl Into<String>,
        progress: Option<f32>,
    ) {
        let title = title.into();
        let detail = detail.into();
        // Clamped here rather than at the draw site: a producer that overshoots (a count taken before
        // a prune) would otherwise draw a fill past its own track.
        let progress = progress.map(|p| if p.is_finite() { p.clamp(0.0, 1.0) } else { 0.0 });
        let mut open = self.open();
        match open.get_mut(key) {
            Some(job) => {
                if job.title != title {
                    job.title = title;
                }
                if job.detail != detail {
                    job.detail = detail;
                }
                if job.progress != progress {
                    job.progress = progress;
                }
            }
            None => {
                open.insert(
                    key,
                    Job { title, detail, progress, cancel: Arc::new(AtomicBool::new(false)) },
                );
            }
        }
    }

    /// **`key`'s job is over** — done, given up, or stopped. Removing a job that is not there is not
    /// an error: a producer removes its own job on every frame it has nothing to do, which is most.
    pub fn finish(&self, key: &'static str) {
        self.open().remove(key);
    }

    /// Whether a person asked `key`'s job to stop. `false` for a job that is not running, so a
    /// producer polling after its job is gone reads "carry on" rather than an answer about nothing.
    pub fn cancel_requested(&self, key: &'static str) -> bool {
        self.open().get(key).is_some_and(Job::cancel_requested)
    }

    /// **Ask `key`'s job to stop.** A request, not a stop: see the module note.
    pub fn request_cancel(&self, key: &'static str) {
        if let Some(job) = self.open().get(key) {
            job.cancel.store(true, Ordering::Release);
        }
    }

    /// Every running job, in key order, so a list does not reshuffle as jobs are added.
    pub fn snapshot(&self) -> Vec<(&'static str, Job)> {
        self.open().iter().map(|(key, job)| (*key, job.clone())).collect()
    }

    /// How many jobs are running.
    pub fn len(&self) -> usize {
        self.open().len()
    }

    /// Whether nothing is running.
    pub fn is_empty(&self) -> bool {
        self.open().is_empty()
    }

    /// **What the status band says, in one line.**
    ///
    /// Nothing running says nothing — a band that said "0 jobs" is a row a reader stops seeing, and it
    /// is the state the editor is in almost all the time (the rule `chrome::reload_readout` keeps). One
    /// job says all of it. More say the first and how many others there are, because the band is one
    /// line and the popover is where the rest belongs.
    pub fn line(&self) -> String {
        let open = self.open();
        let mut jobs = open.values();
        let Some(first) = jobs.next() else {
            return String::new();
        };
        let one = said(first);
        match jobs.count() {
            0 => one,
            n => format!("{one} +{n}"),
        }
    }

    /// **The fraction the band's bar draws**, or `None` when it has no measurable length. `None` for
    /// no jobs as well: nothing is drawn, and a bar asked for nothing stays empty.
    pub fn band_progress(&self) -> Option<f32> {
        let open = self.open();
        match open.len() {
            1 => open.values().next().and_then(|j| j.progress),
            _ => None,
        }
    }
}

/// One job's words: the title, and the detail after it when it has one.
fn said(job: &Job) -> String {
    if job.detail.is_empty() {
        job.title.clone()
    } else {
        format!("{} {}", job.title, job.detail)
    }
}

// ── what a person sees ───────────────────────────────────────────────────────────────────────────

/// The band's job item: hidden while nothing runs, so it neither takes room nor answers a click.
#[derive(Component)]
pub struct JobsItem;

/// The popover listing every running job. Present only while it is open.
#[derive(Component)]
pub struct JobsPopover;

/// What the open popover's rows were built from, so they are rebuilt when the answer moves and not
/// when nothing did. On the popover itself rather than in a `Local`, so a second popover (there is
/// one, but the shape should not depend on that) cannot read the first one's memory.
#[derive(Component, Clone, PartialEq)]
struct JobsShown(String);

/// Adds the job record and the two surfaces that show it.
///
/// **The band's item is spawned here, not in `chrome::spawn_status_band`**, because everything a job
/// is — its words, its bar, its popover, its stop button — is one module's, and a band that grew a row
/// per feature would put the second such feature in a second place.
pub struct JobsPlugin;

impl Plugin for JobsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Jobs>()
            .add_systems(
                OnEnter(crate::screen::Screen::Editor),
                spawn_band_item.after(crate::chrome::FrameSystems),
            )
            .add_systems(
                Update,
                (refresh_band.before(bevy_gooey::GooeySystems::Style), refresh_popover).run_if(in_state(crate::screen::Screen::Editor)),
            );
    }
}

/// The band's item, appended to the status band the frame just built.
fn spawn_band_item(mut commands: Commands, frame: Res<crate::chrome::Frame>) {
    let item = commands
        .spawn((
            // **Not a `bevy::ui_widgets::Button`**, deliberately: this editor has twenty-four global `Activate` observers and a `Button` fires `Activate` for every one of them.
            // `quiet_row` documents the day that took the application down; gooey's `status_item_quiet` is a status item that stays off the bus, hidden through `Shown` until
            // a job runs. A job in progress is plain state (an `Info` kind), so the mark is not drawn.
            bevy_gooey::status_item_quiet(String::new()),
            bevy_gooey::Shown(false),
            bevy::picking::Pickable::default(),
            JobsItem,
        ))
        .observe(open_on_click)
        .id();
    // **`ControlText`, not `ButtonText`**: the item is deliberately not on the `Activate` bus, so its words must not claim to be a button's.
    crate::chrome::the_words_of_a_quiet_item_are_a_control(&mut commands, item, ());
    commands.entity(frame.status).add_child(item);
}

/// One line in the band: the words, and whether the item is there at all. Both compared before being
/// written, so a band with nothing running writes nothing on any frame.
fn refresh_band(jobs: Option<Res<Jobs>>, mut items: Query<(&mut bevy_gooey::Shown, &Children), With<JobsItem>>, mut texts: Query<&mut Text>) {
    let Some(jobs) = jobs else { return };
    let said = jobs.line();
    for (mut shown, children) in &mut items {
        let on = !said.is_empty();
        if shown.0 != on {
            shown.0 = on;
        }
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child)
                && text.0 != said
            {
                text.0 = said.clone();
            }
        }
    }
}

/// A click on the band's item opens the list of jobs — or closes it, because a press on the item that
/// opened a popover is that popover's off switch everywhere else in this editor.
fn open_on_click(
    click: On<PointerClick>,
    mut commands: Commands,
    jobs: Option<Res<Jobs>>,
    open: Query<(Entity, &bevy_gooey::PopoverPanel), With<JobsPopover>>,
) {
    // **One popover, closed by the same press that opens it.** gooey closes a popover on a press
    // *outside* it and deliberately not on its own anchor (a game's toggle needs one rule), so the
    // toggle is the host's: the popover is despawned here and gooey's own closing is left for the
    // press-anywhere case.
    for (entity, panel) in &open {
        if panel.anchor == click.entity {
            commands.entity(entity).try_despawn();
            return;
        }
    }
    let Some(jobs) = jobs else { return };
    let snapshot = jobs.snapshot();
    if snapshot.is_empty() {
        return;
    }
    let anchor = click.entity;
    commands
        .spawn((
            bevy_gooey::popover(anchor, Placement::Above),
            JobsPopover,
            JobsShown(signature(&snapshot)),
        ))
        .with_children(|panel| {
            panel.spawn(bevy_gooey::heading("Jobs"));
            rows(panel, &snapshot);
        });
}

/// The popover's body: one row per job, and the one verb a person has on it.
fn rows(panel: &mut ChildSpawnerCommands, snapshot: &[(&'static str, Job)]) {
    for (key, job) in snapshot {
        panel
            .spawn((
                bevy_gooey::plain_button(format!("Stop: {}", said(job))),
                bevy::picking::Pickable::default(),
                CancelJob(key),
            ))
            .observe(stop_this_job);
    }
}

/// The job one popover row stops.
#[derive(Component, Clone, Copy)]
pub struct CancelJob(pub &'static str);

/// Ask the job to stop. It stops at its own next seam, and its row leaves when it says so.
fn stop_this_job(click: On<PointerClick>, row: Query<&CancelJob>, jobs: Option<Res<Jobs>>) {
    let (Ok(row), Some(jobs)) = (row.get(click.entity), jobs) else {
        return;
    };
    jobs.request_cancel(row.0);
}

/// **Rebuild the open popover's rows when the answer moved**, and not when it did not.
///
/// The signature is the whole visible content, so a bake ticking from 3 of 40 to 4 of 40 rebuilds once
/// and a frame in which nothing changed rebuilds nothing. Rebuilding rather than editing in place is
/// the honest choice: the list can gain or lose a row between two frames, which no fixed set of
/// pre-spawned rows could follow.
fn refresh_popover(
    mut commands: Commands,
    jobs: Option<Res<Jobs>>,
    mut popovers: Query<(Entity, &mut JobsShown), With<JobsPopover>>,
    stale: Query<(Entity, &ChildOf), With<CancelJob>>,
) {
    let Some(jobs) = jobs else { return };
    let snapshot = jobs.snapshot();
    let now = signature(&snapshot);
    for (popover, mut shown) in &mut popovers {
        if shown.0 == now {
            continue;
        }
        shown.0 = now.clone();
        if snapshot.is_empty() {
            // The last job finished: there is nothing left to list, so the list goes.
            commands.entity(popover).try_despawn();
            continue;
        }
        // Only this popover's rows, and only the rows: its heading says the same thing at every size.
        for (row, parent) in &stale {
            if parent.parent() == popover {
                commands.entity(row).try_despawn();
            }
        }
        commands.entity(popover).with_children(|panel| rows(panel, &snapshot));
    }
}

/// Every word the popover shows, in one string, so "did anything change" is one comparison.
fn signature(snapshot: &[(&'static str, Job)]) -> String {
    let mut out = String::new();
    for (key, job) in snapshot {
        out.push_str(key);
        out.push('\u{1}');
        out.push_str(&said(job));
        out.push('\u{2}');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_says_what_it_is_and_how_far_and_a_second_one_is_counted() {
        let jobs = Jobs::default();
        assert_eq!(jobs.line(), "", "nothing running says nothing, never `0 jobs`");
        jobs.set("thumbs", "Baking previews", "3 of 40", Some(0.075));
        assert_eq!(jobs.line(), "Baking previews 3 of 40");
        jobs.set("thumbs", "Baking previews", "4 of 40", Some(0.1));
        assert_eq!(jobs.line(), "Baking previews 4 of 40", "a moved job is one row, not two");
        jobs.set("label", "Labelling", "", None);
        assert!(
            jobs.line().ends_with("+1"),
            "a second job is counted rather than dropped: {}",
            jobs.line()
        );
    }

    /// **The cancel flag survives the reports between the request and the producer's poll.** The first
    /// draft re-made the `Arc` on every `set`, so a request was forgotten on the next frame and the
    /// producer never saw it — a Cancel button that did nothing, with no failing assertion anywhere.
    #[test]
    fn a_cancel_request_outlives_the_next_report() {
        let jobs = Jobs::default();
        jobs.set("thumbs", "Baking previews", "3 of 40", Some(0.075));
        jobs.request_cancel("thumbs");
        jobs.set("thumbs", "Baking previews", "4 of 40", Some(0.1));
        assert!(jobs.cancel_requested("thumbs"), "the request is the producer's to read, frames later");
        assert!(!jobs.cancel_requested("label"), "a job that is not running answers `carry on`");
    }

    #[test]
    fn a_progress_outside_the_track_is_clamped_rather_than_drawn_past_it() {
        let jobs = Jobs::default();
        jobs.set("thumbs", "Baking previews", "", Some(1.7));
        assert_eq!(jobs.band_progress(), Some(1.0));
        jobs.set("thumbs", "Baking previews", "", Some(f32::NAN));
        assert_eq!(jobs.band_progress(), Some(0.0));
        jobs.set("label", "Labelling", "", None);
        assert_eq!(jobs.band_progress(), None, "two jobs have no one fraction, so the bar is a spinner");
    }
}
