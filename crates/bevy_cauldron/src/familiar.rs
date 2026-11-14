//! **The Familiar's switch: View ▸ Pause agent writes** (UI-60, #246).
//!
//! An agent drives the editor through `grimoire/operation_apply`, and every change it makes is already
//! attributed (the history row says "the Familiar", the INSPECTOR's LAST line says `agent:`) and takes back
//! with one Cmd+Z. What a person lacked was a way to say *not now*: to stop the agent changing the map while
//! they look at it, think, or take over, without disconnecting it or losing its place.
//!
//! [`AgentPause`] is that switch, one boolean for the whole editor. While it is on:
//!
//! - **`grimoire/operation_apply` and `grimoire/operation_undo` refuse every write**, with the editor's own
//!   sentence ([`refusal`]) in the answer (`outcome: "refused"`) and on the status line as a problem, and change
//!   nothing and log nothing. The agent can read what the sentence says and wait, which is what a person asking
//!   a colleague to hold on would expect.
//! - **A preview is not a write.** `dry_run` answers as it always does, so an agent can keep planning.
//! - **A person's own edits are untouched.** The switch is about the agent's writes, not the editor's.
//!
//! The row in the View menu is ticked while the switch is on, and takes the tick off when it is turned off by
//! any other means. Nothing is saved: a new session starts unpaused, because a pause a person forgot should
//! not outlive the thing they were looking at.
//!
//! # The AGENT block
//!
//! View ▸ Show agent puts a block at the top of the Map door's INSPECTOR panel that answers the three things a
//! person beside an agent wants to know without opening a log (Amershi's G1, G2 and G7):
//!
//! - **When the editor last heard from it**, and by which call: `Last heard: operation_apply, 3 s ago`. This is
//!   what the editor can know, not a claim of a connection: an agent that has asked nothing is not heard.
//! - **Whose the newest change is**, in words: `the agent's: turned table@0 to 90 deg` or `yours: ...`, from the
//!   same history the History panel lists.
//! - **A way to take the agent's last change back, and to stop it.** The Undo button presses Undo's own chord, so
//!   it is exactly Cmd+Z and lands in the history as any undo does; it is greyed unless the newest change is the
//!   agent's, because a button that undid *your* edit under the agent's name would be a lie. The switch is the
//!   View row's twin.
//!
//! The block is not there until it is asked for, for the reason the outliner's is not: screens that count every
//! button in the world would find these ones.

use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy_gooey::{Activate, Checked, ValueChange, button, observe, toggle};

use crate::commands;
use crate::editor::{EditorState, MapControlsPane};
use crate::project::OpenMap;
use crate::screen::Screen;

/// **Whether the agent's writes are paused.**
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AgentPause(pub bool);

/// The View ▸ Pause agent writes row.
#[derive(Component)]
pub struct PauseItem;

/// **View ▸ Follow the agent** (AF-04, #360). The tick is `EditorSettings::follow_agent`, so the row
/// and the settings file cannot say different things.
#[derive(Component)]
pub struct FollowItem;

/// **Whether hand-backs are answered in the AGENT block** (UI-60b, #340). Off, `grimoire/present` asks in the modal as it
/// always did; on, and with the block open, it asks in the block, beside what the agent did. A person's choice for the
/// sitting, never saved.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HandBackHere(pub bool);

/// What the editor last heard from an agent: the one fact it can know about it. Session state, never saved.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct AgentActivity {
    heard: u32,
    last: Option<Heard>,
}

/// One call from an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    /// The method, without its `grimoire/` prefix.
    pub method: &'static str,
    /// Seconds on the editor's real clock.
    pub at_s: f64,
}

impl AgentActivity {
    /// An agent called `method` at `at_s`.
    pub fn hear(&mut self, method: &'static str, at_s: f64) {
        self.heard += 1;
        self.last = Some(Heard { method, at_s });
    }

    /// How many calls have been heard.
    pub fn heard(&self) -> u32 {
        self.heard
    }

    /// The newest call, if any was heard.
    pub fn last(&self) -> Option<&Heard> {
        self.last.as_ref()
    }

    /// The line the AGENT block says it in.
    pub fn sentence(&self, now_s: f64) -> String {
        match &self.last {
            None => "not heard this session".to_owned(),
            Some(Heard { method, at_s }) => {
                let ago = (now_s - at_s).max(0.0).round() as u64;
                let calls = if self.heard == 1 { "1 call".to_owned() } else { format!("{} calls", self.heard) };
                format!("{method}, {ago} s ago ({calls})")
            }
        }
    }
}

pub struct FamiliarPlugin;

impl Plugin for FamiliarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AgentPause>().init_resource::<AgentActivity>().init_resource::<HandBackHere>().add_systems(
            Update,
            (
                tick_the_menu_row,
                tick_the_follow_row,
                tick_the_familiar_row,
                follow_the_agent,
                follow_the_plan,
                follow_the_pause,
                follow_the_hand_back_switch,
                follow_the_hand_back,
            )
                .run_if(in_state(Screen::Editor)),
        );
    }
}

/// The View ▸ Show agent row.
#[derive(Component)]
pub struct FamiliarItem;

/// The AGENT block in the INSPECTOR panel.
#[derive(Component)]
pub struct Familiar;

/// One of the block's lines of text.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FamiliarLine {
    /// When the agent was last heard.
    Heard,
    /// Whose the newest change is, and what it was.
    Newest,
    /// What the agent handed back, and what the person answered.
    HandBack,
}

/// **The block's PLAN row** (UI-60b, #340): the open map's plan as the agent declared it with `plan_set`, each step with its
/// status. It is not a [`FamiliarLine`]: the order of those is pinned, and this row reads a file rather than the editor's state.
#[derive(Component)]
pub struct FamiliarPlanLine;

/// How often the PLAN row looks at the plan file again, in seconds: a `plan_set`, its undo and a hand edit all show.
const PLAN_READ_SECS: f32 = 0.5;

/// The block's pause switch.
#[derive(Component)]
pub struct FamiliarSwitch;

/// The block's Undo button.
#[derive(Component)]
pub struct FamiliarUndo;

/// **The block's Look button** (AF-04, #360): show the change the Familiar is waiting on, now. The
/// issue names two places for Look — the chip over the viewport and this block's newest-change line —
/// and both call [`crate::follow::Following::look_now`], so a click and a button cannot wait on
/// different things.
#[derive(Component)]
pub struct AgentLookButton;


/// The block's "Answer hand-backs here" switch.
#[derive(Component)]
pub struct HandBackSwitch;

/// **The hand-back card**: what the agent handed back with `grimoire_present`, and the three answers, while the block is
/// open and the question is waiting in it.
#[derive(Component)]
pub struct HandBackCard;

/// One of the card's answers.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandBackAnswer(pub crate::present::Reply);

/// **A followed style line's link on the hand-back card** (SG-09, #408): a press opens the page on the Design workspace, on that line.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct FollowedLink {
    pub path: String,
    pub line: u32,
}

/// **Open the followed line's page** in the Design workspace on its line, the one way a page is selected (`design_view::show_file`).
fn open_the_followed_line(activate: On<Activate>, links: Query<&FollowedLink>, mut commands: Commands) {
    let Ok(link) = links.get(activate.entity) else { return };
    let (path, line) = (link.path.clone(), link.line as usize);
    commands.queue(move |world: &mut World| crate::design_view::show_file(world, &path, Some(line)));
}

/// **View ▸ Show agent puts the AGENT block into the Map door's INSPECTOR panel, first among its rows, and takes it
/// out again.**
pub fn toggle_the_familiar(
    change: On<ValueChange<bool>>,
    panes: Query<Entity, With<MapControlsPane>>,
    blocks: Query<Entity, With<Familiar>>,
    pause: Res<AgentPause>,
    here: Res<HandBackHere>,
    mut commands: Commands,
) {
    if !change.value {
        for block in &blocks {
            commands.entity(block).despawn();
        }
        return;
    }
    if !blocks.is_empty() {
        return;
    }
    let Ok(pane) = panes.single() else { return };
    let block = spawn_the_block(&mut commands, pause.0, here.0);
    commands.entity(pane).insert_children(0, &[block]);
}

/// **The AGENT block, spawned and not yet placed**: the one builder the Map's INSPECTOR toggle and the Agent workspace
/// (NV-10, #387) both use, so the block is the same block wherever a dock holds it.
pub fn spawn_the_block(commands: &mut Commands, pause: bool, here: bool) -> Entity {
    commands
        .spawn((
            Familiar,
            Name::new("agent"),
            // **Never squeezed**: the INSPECTOR's rows scroll, and a flex item that may shrink is made shorter than its text when
            // the panel's rows are taller than the panel, which draws one line over the next (found in a capture, where the
            // headless layout, whose panel is not full, shows nothing wrong).
            bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }),
        ))
        .with_children(|block| {
            crate::chrome::list_heading(block, "AGENT");
            // **The INSPECTOR's own row**: a label column and a value column that wraps beside it. A bare gooey `label` here
            // was measured one line short of what it drew once it wrapped at this panel's width, and the next line was
            // drawn over it (found in a capture; the headless layout does not show it); the rows beside this block are the
            // builders that have been drawn at this width since the readout began, so these are theirs.
            for (line, name) in [(FamiliarLine::Heard, "HEARD"), (FamiliarLine::Newest, "NEWEST"), (FamiliarLine::HandBack, "HAND-BACK")] {
                crate::chrome::rigid_row(block, 0.0, AlignItems::FlexStart).with_children(|row| {
                    crate::chrome::row_label(row, crate::editor::READOUT_LABEL_W, name);
                    row.spawn(bevy_gooey::fill_column(0.0)).with_children(|col| {
                        crate::chrome::row_value(col, "", crate::chrome::ink().text, line);
                    });
                });
            }
            crate::chrome::rigid_row(block, 0.0, AlignItems::FlexStart).with_children(|row| {
                crate::chrome::row_label(row, crate::editor::READOUT_LABEL_W, "PLAN");
                row.spawn(bevy_gooey::fill_column(0.0)).with_children(|col| {
                    crate::chrome::row_value(col, "", crate::chrome::ink().text, FamiliarPlanLine);
                });
            });
            block.spawn((toggle("Pause agent writes", pause), FamiliarSwitch, observe(toggle_the_pause)));
            block.spawn((
                toggle("Answer hand-backs here", here),
                HandBackSwitch,
                observe(|change: On<ValueChange<bool>>, mut here: ResMut<HandBackHere>| {
                    if here.0 != change.value {
                        here.0 = change.value;
                    }
                }),
            ));
            block.spawn((button("Undo the agent's change"), FamiliarUndo, observe(undo_the_agents_change)));
            // **Look**, beside Undo: the block says what the agent did, so "show me" belongs under it
            // (AF-04, #360). It calls the same `Following::look_now` the chip's button does.
            block.spawn((
                button("Look at the agent's change"),
                AgentLookButton,
                observe(|activate: On<Activate>, buttons: Query<(), With<AgentLookButton>>, mut following: Option<ResMut<crate::follow::Following>>, mut state: Option<ResMut<EditorState>>| {
                    if !buttons.contains(activate.entity) {
                        return;
                    }
                    let Some(following) = following.as_deref_mut() else { return };
                    following.look_now();
                    if let Some(state) = state.as_deref_mut() {
                        state.status.note("looking at what the Familiar changed");
                    }
                }),
            ));
            // **The shared list of rows** (#354): one per waiting wish today, one per lesson of
            // AG-06's tomorrow. Its children are `wish_view`'s, so a row that is a wish does not
            // teach this block what a wish is.
            block.spawn((
                crate::wish_view::WishRows,
                Name::new("agent rows"),
                bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }),
            ));
        })
        .id()
}

/// **The PLAN row follows the plan file**, read at most every [`PLAN_READ_SECS`]: the count, then each step with its status, or `no plan`.
/// The same words the inspector's PLAN section prints ([`crate::ops_plan::plan_text`]).
fn follow_the_plan(
    time: Res<Time<Real>>,
    open: Option<Res<OpenMap>>,
    mut lines: Query<&mut Text, With<FamiliarPlanLine>>,
    mut read_at: Local<Option<f32>>,
) {
    if lines.is_empty() {
        *read_at = None;
        return;
    }
    let now = time.elapsed_secs();
    if read_at.is_some_and(|at| now - at < PLAN_READ_SECS) {
        return;
    }
    let Some(open) = open else { return };
    *read_at = Some(now);
    let plan = bevy_grimoire_core::plan::Plan::load(&bevy_grimoire_core::plan::path_for_map(&open.map_path)).unwrap_or_default();
    let said = crate::ops_plan::plan_text(&plan).join("\n");
    for mut text in &mut lines {
        if text.0 != said {
            text.0.clone_from(&said);
        }
    }
}

/// **The Undo button presses Undo's chord** when the newest change is the agent's, and otherwise says why not.
fn undo_the_agents_change(
    activate: On<Activate>,
    buttons: Query<(), With<FamiliarUndo>>,
    mut chords: ResMut<crate::command_run::Chords>,
    live: Res<crate::keys::Live>,
    mut state: Option<ResMut<EditorState>>,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    let Some(state) = state.as_deref_mut() else { return };
    if !newest_is_the_agents(state) {
        state.status.note("the newest change is yours, not the agent's: nothing was undone".to_owned());
        return;
    }
    let Some(command) = commands::by_id("edit.undo") else { return };
    chords.queue(&command, *live);
    state.status.note("asked for Undo from the AGENT block".to_owned());
}

/// Is the newest change on the undo stack the agent's?
pub fn newest_is_the_agents(state: &EditorState) -> bool {
    let depth = state.undo_depth();
    state.history_window(depth, depth).first().is_some_and(|row| row.agent)
}

/// What the block says about the newest change: whose it is and what it was.
fn newest_sentence(state: &EditorState, map: &bevy_grimoire_core::map::Map) -> String {
    match state.undo_next(map) {
        None => "none yet".to_owned(),
        Some(what) if newest_is_the_agents(state) => format!("the agent's: {what}"),
        Some(what) => format!("yours: {what}"),
    }
}

/// What the block says about the agent's last hand-back (`grimoire/present`): waiting on the person, or how they answered.
/// `none yet` when nothing has been handed back this session.
fn hand_back_sentence(presenting: &crate::present::Presenting) -> String {
    match &presenting.0 {
        None => "none yet".to_owned(),
        Some(presented) => match &presented.answer {
            None => format!("{}: waiting for your answer", presented.title),
            Some(answer) => format!("{}: you answered {answer}", presented.title),
        },
    }
}

/// **The block follows the editor**: its lines are rewritten when the words change, and the Undo button is greyed
/// unless pressing it would undo the agent's change.
#[allow(clippy::too_many_arguments)]
fn follow_the_agent(
    activity: Res<AgentActivity>,
    time: Res<Time<Real>>,
    state: Option<Res<EditorState>>,
    open: Option<Res<OpenMap>>,
    presenting: Option<Res<crate::present::Presenting>>,
    mut lines: Query<(&mut Text, &FamiliarLine)>,
    undo: Query<(Entity, Has<InteractionDisabled>), With<FamiliarUndo>>,
    mut commands: Commands,
) {
    for (mut text, line) in &mut lines {
        let said = match line {
            FamiliarLine::Heard => activity.sentence(time.elapsed_secs_f64()),
            FamiliarLine::Newest => match (&state, &open) {
                (Some(state), Some(open)) => newest_sentence(state, &open.map),
                _ => continue,
            },
            FamiliarLine::HandBack => presenting.as_deref().map(hand_back_sentence).unwrap_or_default(),
        };
        if text.0 != said {
            text.0 = said;
        }
    }
    let Some(state) = state else { return };
    let can = newest_is_the_agents(&state);
    for (button, disabled) in &undo {
        match (can, disabled) {
            (true, true) => {
                commands.entity(button).remove::<InteractionDisabled>();
            }
            (false, false) => {
                commands.entity(button).insert(InteractionDisabled);
            }
            _ => {}
        }
    }
}

/// The block's switch says what the switch says, whichever way it was set.
fn follow_the_pause(pause: Res<AgentPause>, switches: Query<(Entity, Has<Checked>), With<FamiliarSwitch>>, mut commands: Commands) {
    for (switch, ticked) in &switches {
        match (pause.0, ticked) {
            (true, false) => {
                commands.entity(switch).insert(Checked);
            }
            (false, true) => {
                commands.entity(switch).remove::<Checked>();
            }
            _ => {}
        }
    }
}

/// The "Answer hand-backs here" switch says what the setting says.
fn follow_the_hand_back_switch(here: Res<HandBackHere>, switches: Query<(Entity, Has<Checked>), With<HandBackSwitch>>, mut commands: Commands) {
    for (switch, ticked) in &switches {
        match (here.0, ticked) {
            (true, false) => {
                commands.entity(switch).insert(Checked);
            }
            (false, true) => {
                commands.entity(switch).remove::<Checked>();
            }
            _ => {}
        }
    }
}

/// The tick on View ▸ Show agent says whether the block is there.
fn tick_the_familiar_row(items: Query<(Entity, Has<Checked>), With<FamiliarItem>>, blocks: Query<(), With<Familiar>>, mut commands: Commands) {
    let present = !blocks.is_empty();
    for (item, ticked) in &items {
        match (present, ticked) {
            (true, false) => {
                commands.entity(item).insert(Checked);
            }
            (false, true) => {
                commands.entity(item).remove::<Checked>();
            }
            _ => {}
        }
    }
}

/// What a refused write says, to the agent in its answer and to the person on the status line.
pub fn refusal(operation: &str) -> String {
    format!(
        "the agent's writes are paused, so `{operation}` changed nothing. A person turned them off with View > Pause agent writes; \
         turn that off to let the agent write again. Previews still work."
    )
}

/// The row was chosen: the switch follows the tick.
pub fn toggle_the_pause(change: On<ValueChange<bool>>, mut pause: ResMut<AgentPause>) {
    if pause.0 != change.value {
        pause.0 = change.value;
    }
}

/// **The View ▸ Follow the agent row is ticked while the view follows**, read from the running
/// settings so a settings-screen change and a menu click cannot disagree.
fn tick_the_follow_row(
    settings: Option<Res<crate::settings::EditorSettings>>,
    items: Query<(Entity, Has<Checked>), With<FollowItem>>,
    mut commands: Commands,
) {
    let on = settings.is_none_or(|s| s.follow_agent);
    for (item, ticked) in &items {
        match (on, ticked) {
            (true, false) => {
                commands.entity(item).insert(Checked);
            }
            (false, true) => {
                commands.entity(item).remove::<Checked>();
            }
            _ => {}
        }
    }
}

/// **The row was chosen: the setting follows the tick.** Written through the settings screen's own
/// `write_key`, so the file, the running editor and the row are one change rather than three.
///
/// **The user's layer**, which is where `settings.rs`'s own doc puts this key: *"it is how a person
/// wants to watch an agent, not a fact about one project"*. The first version wrote the project's
/// `editor.yaml`, which would have made one person's preference a file every other person opening
/// that project inherits — the opposite of what the doc says, and the disagreement is what the review
/// caught.
///
/// **A missing user file is said, not worked around.** A machine with no config directory has nowhere
/// to keep a preference; writing the project's instead would let the project decide for its authors,
/// so the refusal is shown and nothing is written anywhere.
pub fn toggle_the_follow(
    change: On<ValueChange<bool>>,
    file: Option<Res<crate::settings::UserSettingsFile>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let Some(file) = file else {
        if let Some(state) = state.as_deref_mut() {
            state.status.problem(format!(
                "no personal settings file is available on this machine, so Follow the agent was not \
                 saved. It is {} until this session ends.",
                if change.value { "on" } else { "off" }
            ));
        }
        return;
    };
    if let Err(why) = crate::setting_catalogue::write_key(
        &file.0,
        "follow_agent",
        Some(crate::setting_catalogue::Value::On(change.value)),
    ) && let Some(state) = state.as_deref_mut()
    {
        state.status.problem(format!("could not save Follow the agent: {why}"));
    }
}

/// The tick says what the switch says, however it was set.
fn tick_the_menu_row(pause: Res<AgentPause>, items: Query<(Entity, Has<Checked>), With<PauseItem>>, mut commands: Commands) {
    for (item, ticked) in &items {
        match (pause.0, ticked) {
            (true, false) => {
                commands.entity(item).insert(Checked);
            }
            (false, true) => {
                commands.entity(item).remove::<Checked>();
            }
            _ => {}
        }
    }
}

/// **The card is there while a hand-back waits in the block, and not otherwise.** It says what was made, what the person
/// said, what the agent assumed and where to look, and offers Accept (keep it), the undo of every operation it covers, and
/// Dismiss (keep it, unread). The answer goes the way the modal's goes ([`crate::present::PresentationReply`]).
fn follow_the_hand_back(
    presenting: Res<crate::present::Presenting>,
    blocks: Query<Entity, With<Familiar>>,
    cards: Query<Entity, With<HandBackCard>>,
    mut commands: Commands,
) {
    let waiting = presenting.0.as_ref().filter(|presented| presented.inline && presented.answer.is_none());
    match (waiting, cards.is_empty()) {
        (Some(presented), true) => {
            for block in &blocks {
                let card = commands
                    .spawn((
                        HandBackCard,
                        Name::new("agent hand-back"),
                        bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }),
                    ))
                    .with_children(|card| {
                        crate::chrome::wrapping_row(card, crate::chrome::GAP_TIGHT_STEPS, crate::chrome::NO_MARGIN, AlignItems::Stretch)
                            .with_children(|answers| {
                                for (text, reply) in [
                                    ("Accept".to_owned(), crate::present::Reply::Accept),
                                    (crate::present::undo_label(presented.operation_ids.len()), crate::present::Reply::Undo),
                                    ("Dismiss".to_owned(), crate::present::Reply::Dismiss),
                                ] {
                                    answers.spawn((
                                        button(text),
                                        HandBackAnswer(reply),
                                        observe(move |_: On<Activate>, mut answer: ResMut<crate::present::PresentationReply>| {
                                            answer.0 = Some(reply);
                                        }),
                                    ));
                                }
                            });
                        for (name, text) in [
                            ("MADE", presented.summary.clone()),
                            ("SAID", presented.specified.join("; ")),
                            ("ASSUMED", presented.inferred.join("; ")),
                            ("CHECK", presented.check.join("; ")),
                        ] {
                            if text.is_empty() {
                                continue;
                            }
                            crate::chrome::rigid_row(card, 0.0, AlignItems::FlexStart)
                                .with_children(|row| {
                                    crate::chrome::row_label(row, crate::editor::READOUT_LABEL_W, name);
                                    row.spawn(bevy_gooey::fill_column(0.0))
                                        .with_children(|col| {
                                            crate::chrome::row_value(col, text, crate::chrome::ink().text, ());
                                        });
                                });
                        }
                        // **The style lines the work followed** (SG-09, #408): at most `FOLLOWED_SHOWN_MAX` with each line's text and a link that opens it on the
                        // Design page, and a count of the rest. The text is the person's to read here; the state carries only the entries.
                        if !presented.followed.is_empty() {
                            crate::chrome::rigid_row(card, 0.0, AlignItems::FlexStart)
                                .with_children(|row| {
                                    crate::chrome::row_label(row, crate::editor::READOUT_LABEL_W, "FOLLOWED");
                                    row.spawn(bevy_gooey::fill_column(crate::chrome::GAP_TIGHT_STEPS))
                                        .with_children(|col| {
                                            for followed in presented.followed.iter().take(crate::style_followed::FOLLOWED_SHOWN_MAX) {
                                                crate::chrome::wrapping_row(col, crate::chrome::GAP_TIGHT_STEPS, crate::chrome::NO_MARGIN, AlignItems::Center)
                                                    .with_children(|line| {
                                                        line.spawn((
                                                            button(format!("{}:{}", followed.path, followed.line)),
                                                            FollowedLink { path: followed.path.clone(), line: followed.line },
                                                            observe(open_the_followed_line),
                                                        ));
                                                        crate::chrome::row_value(line, followed.text.trim().to_owned(), crate::chrome::ink().text, ());
                                                    });
                                            }
                                            if let Some(more) = crate::style_followed::more_says(presented.followed.len()) {
                                                crate::chrome::row_value(col, more, crate::chrome::ink().dim, ());
                                            }
                                        });
                                });
                        }
                    })
                    .id();
                // Right under the HAND-BACK line (the heading and the three lines are the block's first four children), where the
                // answers are on screen without scrolling the panel.
                commands.entity(block).insert_children(4, &[card]);
            }
        }
        (None, false) => {
            for card in &cards {
                commands.entity(card).despawn();
            }
        }
        _ => {}
    }
}
