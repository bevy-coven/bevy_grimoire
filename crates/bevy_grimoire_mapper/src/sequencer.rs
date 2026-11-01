//! **Capturing a sequence** — the editor takes the frames, so frame 0 is the state before.
//!
//! # Why this is on the editor's side and not the MCP server's
//!
//! **The brief's rule is "frame 0 is the state before it"**, and only the editor can promise
//! that. The screenshot reads the *rendered* frame, not the ECS state — so an operation applied
//! in the same frame a capture is spawned would show in frame 0, and the sequence would begin
//! with the thing it was supposed to show the absence of.
//!
//! **So the editor takes the frames**, times them itself, and applies the operation **after
//! frame 0's observer has fired**. The MCP server asks for a sequence and polls for its result.
//!
//! # What it writes
//!
//! One PNG per frame, named `f{i:03}_t{t:.2}s.png`, **and never over an existing file** — a
//! capture that silently replaced the last run's frames would make two runs indistinguishable.
//! Beside them, a `sequence.json` with the state at each frame, so a frame is never a picture
//! without a state dump.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::surface::Surface;

/// **The slowest rate a sequence may ask for**, in frames a second: two, for a long slow operation.
pub const MIN_FPS: u32 = 2;
/// **The fastest rate a sequence may ask for**, in frames a second. Set from a measurement on a
/// real editor (see the commit). A higher rate is refused, not clamped: a caller must never get
/// frames at a rate it did not ask for.
pub const MAX_FPS: u32 = 15;

/// **How a capture sequence is asked for.**
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SequenceParams {
    /// **The sequence's name** — a slash-separated snake_case id. It names the directory the
    /// frames land in, and it is refused if it is not an id.
    pub label: String,
    /// **Where the frames go.** An absolute path; the editor writes there and nowhere else.
    pub out_dir: String,
    /// **How long the sequence runs**, in seconds.
    #[serde(default = "two_seconds")]
    pub seconds: f32,
    /// **How many frames a second.** Four by default; any rate from `MIN_FPS` to `MAX_FPS`.
    #[serde(default = "four_fps")]
    pub fps: u32,
    /// **The operation to apply, once frame 0 is in.** Optional: without it the sequence is the
    /// settle alone, which is what a reviewer wants when they are checking that nothing moved.
    #[serde(default)]
    pub operation: Option<Operation>,
}

/// **The operation a sequence applies.**
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    /// **The operation's name**, as `grimoire/operation_list` gives it.
    pub name: String,
    /// **Its parameters.**
    #[serde(default)]
    pub parameters: Value,
}

/// **Two seconds, which is one contact sheet at four frames a second.**
fn two_seconds() -> f32 {
    2.0
}

/// **Four frames a second, the brief's default.**
fn four_fps() -> u32 {
    4
}

/// **Whether this editor can write a frame at all.**
///
/// **Settled once, at startup.** A headless editor has no render world, so a screenshot never
/// produces a readback, and a sequence there can promise a *schedule* but not a *picture*. The
/// run says so rather than finishing as though the pictures were there.
#[derive(Clone, Copy, Debug, Resource)]
pub struct CanRender(pub bool);

/// **What a sequence is doing right now.**
#[derive(Resource, Default)]
pub struct Sequencer {
    /// **The run in flight**, if there is one.
    run: Option<Run>,
    /// **The runs that have finished**, newest last, so a caller can ask about one by id.
    finished: Vec<Finished>,
}

/// **How long one capture may take to land** before the run gives up on it. A readback lands
/// within a frame or two, so five seconds is a stalled editor, and a run that waits longer must
/// say so rather than hang.
const CAPTURE_TIMEOUT_S: f32 = 5.0;

/// **How many captures may be on their way to disk at once.** Bevy's duplicate check is per
/// extraction frame, not per run: one screenshot per render target per frame, and a capture that is
/// already reading back is not counted against a new one. So the sequencer asks for one new frame per
/// update and lets the earlier PNGs land behind it. The cap bounds how many readbacks are pending.
const MAX_IN_FLIGHT: usize = 4;

/// **A capture on its way to disk**: the frame it is for, when it was asked for, and the
/// screenshot entity carrying it. A timeout must despawn that entity, or its readback can still land.
struct InFlight {
    index: u32,
    since: f32,
    entity: Entity,
}

/// **A run in flight.**
struct Run {
    id: u64,
    label: String,
    out_dir: std::path::PathBuf,
    /// **When it started**, in seconds since the editor started.
    started: f32,
    seconds: f32,
    fps: u32,
    /// **The next frame's index.**
    next: u32,
    /// **The operation, applied once frame 0 is in.**
    operation: Option<Operation>,
    /// **Whether the operation has been applied yet.**
    applied: bool,
    /// **When it was applied**, in seconds from the start.
    applied_at: Option<f32>,
    /// **What it said.**
    operation_result: Option<Value>,
    /// **The frames asked for, and when.** The manifest's list.
    asked: Vec<Frame>,
    /// **The frames whose PNG has landed.** The run is finished when this has caught up.
    written: Vec<String>,
    /// **The captures on their way to disk**, at most `MAX_IN_FLIGHT`. See `drive_sequencer`.
    in_flight: Vec<InFlight>,
}

/// **One frame, written.**
#[derive(Debug, Clone, Serialize)]
pub struct Frame {
    /// **Its index.**
    pub index: u32,
    /// **When it was scheduled, in seconds from the start.**
    pub t_s: f32,
    /// **When it was actually taken**, in seconds from the start. The two differ: the editor
    /// renders at whatever rate it renders at, and the schedule is the *intent*.
    pub actual_t_s: f32,
    /// **Where it landed.**
    pub path: String,
}

/// **A run that has finished.**
#[derive(Debug, Clone, Serialize)]
pub struct Finished {
    pub id: u64,
    pub label: String,
    pub out_dir: String,
    pub frames: Vec<Frame>,
    /// **When the operation was applied**, in seconds from the start. `None` when there was none.
    pub operation_applied_t_s: Option<f32>,
    /// **What the operation said.**
    pub operation_result: Option<Value>,
    /// **Whether it failed, and why.**
    pub error: Option<String>,
}

/// **The parameters of a status query.**
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusParams {
    /// **Which run.**
    pub id: u64,
}

impl Sequencer {
    /// **Start a sequence, and return its id.**
    ///
    /// **A second sequence while one is running is refused.** Two sequences writing frames at
    /// once would interleave their file names, and a reader could not tell which frame belongs
    /// to which.
    pub fn start(&mut self, p: &SequenceParams, now: f32) -> Result<u64, String> {
        if self.run.is_some() {
            return Err("a sequence is already running. Wait for it to finish.".to_owned());
        }
        if !bevy_grimoire_core::naming::is_id(&p.label) {
            return Err(format!(
                "`{}` is not a sequence name. Names are snake_case, with `/` between the parts.",
                p.label
            ));
        }
        if !(0.0..=10.0).contains(&p.seconds) || p.seconds <= 0.0 {
            return Err(format!(
                "a sequence runs for between a moment and ten seconds; `{}` is neither.",
                p.seconds
            ));
        }
        if !(MIN_FPS..=MAX_FPS).contains(&p.fps) {
            return Err(format!(
                "a sequence is captured between {MIN_FPS} and {MAX_FPS} frames a second; `{}` is outside that.",
                p.fps
            ));
        }
        let out_dir = std::path::PathBuf::from(&p.out_dir);
        std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
        let id = self.finished.len() as u64 + 1;
        self.run = Some(Run {
            id,
            label: p.label.clone(),
            out_dir,
            started: now,
            seconds: p.seconds,
            fps: p.fps,
            next: 0,
            operation: p.operation.clone(),
            applied: false,
            applied_at: None,
            operation_result: None,
            asked: Vec::new(),
            written: Vec::new(),
            in_flight: Vec::new(),
        });
        Ok(id)
    }

    /// **What is happening, in one value.**
    pub fn status(&self, id: u64) -> Value {
        if let Some(run) = &self.run
            && run.id == id
        {
            return json!({
                "status": "running",
                "id": run.id,
                "label": run.label,
                "frames": run.asked.len(),
                "written": run.written.len(),
                "expected_frames": (run.seconds * run.fps as f32) as u32,
            });
        }
        match self.finished.iter().find(|f| f.id == id) {
            Some(f) => json!({
                "status": if f.error.is_some() { "failed" } else { "done" },
                "id": f.id,
                "label": f.label,
                "out_dir": f.out_dir,
                "frames": f.frames,
                "operation_applied_t_s": f.operation_applied_t_s,
                "operation_result": f.operation_result,
                "error": f.error,
            }),
            None => json!({"status": "unknown", "id": id}),
        }
    }
}

/// **The plugin that runs the sequencer.**
pub struct SequencerPlugin;

impl Plugin for SequencerPlugin {
    fn build(&self, app: &mut App) {
        // **Whether a frame can be written at all is settled here, once.**
        //
        // **A headless editor has no render world** — `RenderPlugin` creates the `RenderApp`
        // sub-app only when a backend was available — and so a screenshot cannot produce a
        // readback. **The sub-app's presence is the fact**, checked at startup rather than
        // guessed at every frame, and a run that waited for a frame that cannot exist would
        // wait forever.
        let can_render = app.get_sub_app(bevy::render::RenderApp).is_some();
        app.init_resource::<Sequencer>()
            .insert_resource(CanRender(can_render))
            .add_systems(Update, drive_sequencer.in_set(crate::keys::Phase::Sense));
    }
}

/// **What the sequencer reads and writes, bundled to stay under the parameter limit.**
///
/// **The redraw flag is here** because a turn applied inside a sequence must reach the screen the
/// same way an agent's turn does.
#[derive(bevy::ecs::system::SystemParam)]
struct Operated<'w> {
    project: Option<ResMut<'w, crate::project::Project>>,
    open: Option<ResMut<'w, crate::project::OpenMap>>,
    due: Option<ResMut<'w, crate::watch::Due>>,
    state: Option<ResMut<'w, crate::editor::EditorState>>,
    confirm: Option<Res<'w, crate::confirm::Confirm>>,
    import: Option<ResMut<'w, crate::tiles::ImportState>>,
    door: Option<Res<'w, crate::tiles::Door>>,
    mode: Option<Res<'w, crate::tiles::Mode>>,
    filters: Option<Res<'w, crate::filter::Filters>>,
}

/// **Take the frames that are due, and finish the run when the last one is in.**
///
/// **The editor's own clock**, and not the caller's — the caller asked for a *duration* and the
/// editor knows what a frame costs.
fn drive_sequencer(
    time: Res<Time>,
    mut sequencer: ResMut<Sequencer>,
    surface: Option<Res<Surface>>,
    mut commands: Commands,
    mut operated: Operated,
    can_render: Option<Res<CanRender>>,
) {
    let Some(run) = sequencer.run.as_mut() else {
        return;
    };
    // **A headless editor has no surface, and so no renderer and no readback.** The frames are
    // still *asked for* -- the schedule is the sequencer's own and does not need a GPU -- and
    // the run still finishes. **What the headless case cannot do is produce a PNG**, and that is
    // said in the test rather than papered over.
    let surface = surface.as_ref();
    let now = time.elapsed_secs() - run.started;
    let total = (run.seconds * run.fps as f32) as u32;

    // **And a run that waited for a frame that cannot exist would wait forever**, so the
    // headless editor ends the run and says why.
    let can_render = can_render.map(|c| c.0).unwrap_or(false);

    // **A capture that never lands ends the run with its reason**, rather than leaving the run
    // waiting for a PNG that is not coming.
    if let Some(stale) = run
        .in_flight
        .iter()
        .find(|f| now - f.since > CAPTURE_TIMEOUT_S)
    {
        let index = stale.index;
        // **Every capture still out is despawned with the run.** Each entity carries the
        // `save_to_disk` observer, so left alive, a readback that lands after the run has failed
        // would write a PNG into that run's directory.
        for flight in &run.in_flight {
            commands.entity(flight.entity).try_despawn();
        }
        let error = format!(
            "frame {index} did not land within {CAPTURE_TIMEOUT_S} s, so the run stopped rather than wait for it"
        );
        finish_run(&mut sequencer, Some(error));
        return;
    }

    // **The operation goes in AFTER frame 0 has been asked for.**
    //
    // **This is the whole reason the sequencer is on the editor's side.** The screenshot reads
    // the *rendered* frame, so an operation applied before frame 0's readback would be *in*
    // frame 0 — and the sequence would begin with the thing it was supposed to show the absence
    // of. **So the operation waits for frame 0**, and the delay is recorded in the manifest
    // rather than hidden.
    if !run.applied && run.next >= 1 {
        run.applied = true;
        if let Some(op) = run.operation.clone() {
            let mut ctx = crate::ops_brp::take(&operated.project, &operated.open);
            // **What the turn broke, measured as `grimoire/operation_apply` measures it (E-074).** The
            // two front ends answer the same question with the same words, so a sequence's frames and
            // its answer cannot disagree about what the change introduced.
            let checked = crate::checks::wanted(&op.name);
            let before = crate::checks::before(&op.name, &ctx);
            // **Through the registry, and not around it.** The sequencer is a way of *when* an
            // operation happens, and it must not be a second way of *what* happens.
            // **A door is refused before its plan, as `grimoire/operation_apply` refuses it (cycle 8).**
            // A question on screen, a leaving question or a door already waiting owns the next frame.
            // Before this, the sequence refused nothing and answered `applied` for a door it never
            // crossed to.
            let asking = operated.confirm.as_deref().is_some_and(|c| c.is_open());
            let door_refusal = (op.name == "door_open")
                .then(|| operated.state.as_deref().and_then(|s| crate::editor::door_refusal(s, asking)))
                .flatten();
            // **A piece is selected only from the Meshes shelf, as the shelf shows it (E-044)**: the
            // refusal `grimoire/operation_apply` gives, and a refusal changes nothing.
            let piece_refusal = (op.name == "piece_select")
                .then(|| {
                    let id = op.parameters.get("id").and_then(Value::as_str).unwrap_or_default();
                    crate::ops_brp::piece_select_refusal(
                        operated.door.as_deref().copied(),
                        operated.mode.as_deref().copied(),
                        operated.project.as_deref(),
                        operated.filters.as_deref(),
                        id,
                    )
                })
                .flatten();
            let outcome = match door_refusal.map(str::to_string).or(piece_refusal) {
                Some(why) => Ok(crate::ops::Applied::Refused(crate::ops::Refusal::new(why))),
                None => crate::ops::dispatch(&mut ctx, &op.name, &op.parameters),
            };
            let checks = (checked && matches!(outcome, Ok(crate::ops::Applied::Done { .. })))
                .then(|| {
                    crate::checks::find(&ctx)
                        .map(|after| crate::checks::report(before.as_ref(), &after))
                })
                .flatten();
            // **Redrawn, as `grimoire/operation_apply` is.** The write-back is the watcher's own
            // write and the watcher ignores it, so a saved turn would stay off the screen until
            // some unrelated redraw. The sequence's frames are the proof, and they would show
            // nothing.
            if let (Some(due), Ok(crate::ops::Applied::Done { outcome, .. })) =
                (operated.due.as_mut(), &outcome)
            {
                due.map_replaced |= outcome.saved;
            }
            // **A piece the sequence selects is the row a click would be (E-044)**, as the apply holds it.
            if let (Ok(crate::ops::Applied::Done { outcome, .. }), Some(import)) =
                (&outcome, operated.import.as_deref_mut())
                && op.name == "piece_select"
            {
                crate::editor::apply_selected(import, outcome.after.as_ref());
            }
            // **The agent's change joins the person's one undo history (B-019)**, as it does through
            // `grimoire/operation_apply`: a turn a sequence applies is an agent's turn. An operation that
            // changes no document (`Undo::None`) has nothing to take back, so it stays out (gap 4).
            if let Ok(crate::ops::Applied::Done {
                outcome,
                operation_id,
                ..
            }) = &outcome
                && crate::ops::find(&op.name).is_some_and(|o| o.undo() != crate::ops::Undo::None)
            {
                let writes = crate::ops::find(&op.name).is_some_and(|o| o.writes_library());
                crate::ops_brp::join_history(
                    writes,
                    operated.state.as_deref_mut(),
                    operated.import.as_deref_mut(),
                    operation_id,
                    &outcome.summary,
                );
            }
            // **A door the operation answered with is stored for the editor to cross (cycle 8).** The
            // BRP path stores it the same way, and `editor::apply_pending_door` carries it out in the
            // next frame, so the door the sequence opened is the door the person is on.
            if let Ok(crate::ops::Applied::Done { outcome, .. }) = &outcome
                && op.name == "door_open"
                && let Some(state) = operated.state.as_mut()
            {
                crate::editor::request_door(state, outcome.after.as_ref());
            }
            // **The person's receipt says what the sequence applied (B-020)**, as the apply does.
            if let Some(state) = operated.state.as_mut() {
                match &outcome {
                    Ok(crate::ops::Applied::Done { outcome, .. }) => {
                        state.receipt_agent_change(&op.name, &outcome.summary);
                    }
                    Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
                        state.problem_agent_problem(&op.name, e.problem());
                    }
                }
            }
            run.applied_at = Some(now);
            run.operation_result = Some(match outcome {
                Ok(crate::ops::Applied::Done { outcome, .. }) => {
                    // **A door the editor will ask about is not yet open**, as `grimoire/operation_apply`
                    // answers it: `waiting`, and the word that names what it waits for.
                    let waiting = outcome
                        .after
                        .as_ref()
                        .and_then(|after| after["waiting"].as_str())
                        .filter(|_| op.name == "door_open");
                    json!({"outcome": if waiting.is_some() { "waiting" } else { "applied" }, "waiting_on": waiting, "summary": outcome.summary, "saved": outcome.saved, "checks": checks})
                }
                Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
                    json!({"outcome": "refused", "refusal": e.0, "problem": e.problem().to_json()})
                }
            });
            // **The map and the library go back, changed or not** (gap 4). The same write-back
            // `grimoire/operation_apply` makes, so a label a sequence sets is on the screen the person sees.
            crate::ops_brp::put_back(&mut operated.project, &mut operated.open, ctx);
        }
    }

    // **A frame is due when its scheduled time has passed.** The schedule is the intent; the
    // editor renders when it renders, and the manifest records both.
    while run.next < total {
        let t = run.next as f32 / run.fps as f32;
        if now < t {
            break;
        }
        let index = run.next;
        let path = run.out_dir.join(format!("f{index:03}_t{t:.2}s.png"));
        let actual = now;
        let Some(handle) = surface.map(|s| s.image.clone()).filter(|_| can_render) else {
            // **No renderer: the frame is asked for and nothing more.** The schedule still
            // advances, so the run finishes and the manifest still says when each frame was due.
            run.next += 1;
            run.asked.push(Frame {
                index,
                t_s: t,
                actual_t_s: actual,
                path: path.to_string_lossy().into_owned(),
            });
            continue;
        };
        // **Up to `MAX_IN_FLIGHT` captures are on their way to disk.** A due frame waits only when
        // the cap is reached, not for the PNG before it: the old gate held each frame until the
        // previous readback landed, so the readback's latency set the capture rate (B-040).
        if run.in_flight.len() >= MAX_IN_FLIGHT {
            break;
        }
        run.next += 1;
        // **The frame is written by the observer, when the readback lands.**
        //
        // The screenshot is spawned here and the PNG is written when the capture completes,
        // because the bytes do not exist until then — and a frame written from a spawn would be
        // the frame *before* the one that was asked for.
        let out = path.clone();
        let written_path = out.to_string_lossy().into_owned();
        let run_id = run.id;
        let shot = commands
            .spawn(bevy::render::view::screenshot::Screenshot::image(handle))
            .observe(
                // **Bevy's own writer, and not a hand-rolled encoder.** `save_to_disk` is the
                // crate's PNG writer and it is what every other capture in this editor uses, so
                // a frame written here is a frame written the same way as one written by the
                // `Ctrl+P` path. **And it refuses to overwrite**, which is the property that
                // matters here: a capture that silently replaced the last run's frames would
                // make two runs indistinguishable.
                bevy::render::view::screenshot::save_to_disk(out.clone()),
            )
            // **And the write is recorded, so the run knows the frame has landed.**
            .observe(
                move |_t: On<bevy::render::view::screenshot::ScreenshotCaptured>,
                      mut sequencer: ResMut<Sequencer>| {
                    let Some(run) = sequencer.run.as_mut() else {
                        return;
                    };
                    // **A capture that outlived its run** (it timed out, or a new run started) has
                    // nothing to land in, so its PNG is not counted.
                    if run.id != run_id {
                        return;
                    }
                    run.written.push(written_path.clone());
                    run.in_flight.retain(|f| f.index != index);
                },
            )
            .id();
        // **The capture is in flight until its PNG lands.**
        run.in_flight.push(InFlight { index, since: now, entity: shot });
        // **The frame is counted when it is ASKED for**, and the run is finished when it is
        // WRITTEN. The two are different moments — the readback lands a frame or two after the
        // spawn — and a run that finished on the ask would report itself done while its last
        // PNG was still being written. **A caller that then read the files would find one
        // missing**, and it would be missing for no reason the caller could see.
        run.asked.push(Frame {
            index,
            t_s: t,
            actual_t_s: actual,
            path: out.to_string_lossy().into_owned(),
        });
        // **One new screenshot per update.** Bevy's duplicate check is per frame, so a second spawn
        // in this update would be despawned without a PNG. The next due frame waits for the next update.
        break;
    }

    // **The run is finished when every frame has been WRITTEN**, and not when they were asked
    // for. The readback lands a frame or two after the spawn, so a run that finished on the ask
    // would report itself done while its last PNG was still on its way — and a caller that read
    // the files then would find one missing, for no reason it could see.
    //
    // **And not while its operation is still pending (E-065).** The gate above waits for
    // `next >= 1`, so a run whose first tick is already past the last frame's time asks for every
    // frame in that one tick with `next == 0`. Finishing here would close the run with
    // `operation_result: None` and the operation could never apply: the caller is told the sequence
    // is done and the change it asked for silently never happened. On the next tick `next >= 1` and
    // the gate applies it, so the only thing missing was one more update.
    let all_asked = run.asked.len() as u32 >= total;
    let all_written = run.written.len() as u32 >= total;
    let operation_pending = run.operation.is_some() && !run.applied;
    if all_asked && (all_written || !can_render) && !operation_pending {
        // **And a headless run says why there is no picture**, rather than finishing as though the
        // frames were there. A caller that is told "done" and then finds no file is owed an
        // answer; a caller that is told "no render world" knows exactly what happened.
        let error = if can_render {
            None
        } else {
            Some("this editor has no render world, so no frame could be written".to_owned())
        };
        finish_run(&mut sequencer, error);
    }
}

/// **Close the run and put it on the finished list.**
///
/// **One place, so the two ways a run ends — every frame written, or no frame possible — leave
/// the same shape behind.**
fn finish_run(sequencer: &mut Sequencer, error: Option<String>) {
    let Some(run) = sequencer.run.take() else {
        return;
    };
    let finished = Finished {
        id: run.id,
        label: run.label,
        out_dir: run.out_dir.to_string_lossy().into_owned(),
        frames: run.asked,
        operation_applied_t_s: run.applied_at,
        operation_result: run.operation_result.clone(),
        error,
    };
    sequencer.finished.push(finished);
}
