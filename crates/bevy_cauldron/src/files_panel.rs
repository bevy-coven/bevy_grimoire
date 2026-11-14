//! **The project search panel: find, preview and replace across the project's files** (GD-13, #375).
//!
//! `Cmd`/`Ctrl`+`Shift`+`F` in the Files workspace opens a **SEARCH** block under the file tree: a query, the
//! three options the find bar has (case, whole word, regular expression), a path filter, the results, a
//! replacement, and the buttons. The engine is [`crate::files_search`] (the find bar's own matcher, per file);
//! this module is the panel and nothing else, built from `bevy_gooey`'s widgets.
//!
//! # A search is a background job, and the results stream in
//!
//! [`start_the_search`] runs the search on a thread of its own and says so in the status band
//! ([`crate::jobs::Jobs`], UI-22), where **Cancel** stops it (the producer's own seam: the thread looks at its
//! flag between files). [`drain_the_search`] takes what the thread has found a little at a time, each frame, and
//! adds it to the results tree, so the first matches are on screen while the rest are still being read.
//!
//! # A replace is previewed, then applied as one operation
//!
//! **Preview** plans the replace over every file and shows each change as a diff (`- old`, `+ new`), or the
//! refusal naming the file and line when the replace would break a file. **Replace all** applies the previewed
//! files through the operation `files_replace` ([`crate::ops_files`]): one log line, one undo (`Cmd+Z`), every
//! file written atomically or none. Pressed with no preview on screen it makes one first.
//!
//! Spawned on demand and despawned with its workspace, never hidden: the editor's protected tests count every
//! button and take the first search field of the whole world.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::ui_widgets::{Activate, ValueChange};
use bevy_gooey::{CodeFind, Modifiers, SelectMode, Shortcut, Table, TableSelection, Tree, TreeNode, button, column, label, observe, row, text_input, toggle, tree};

use crate::editor::EditorState;
use crate::files::{self, FilesPanel, Files};
use crate::files_search::{self, FileHits, Plan, Query as SearchQuery, Refused};
use crate::jobs::Jobs;
use crate::project::Project;
use crate::workspace::Workspace;

/// The status band's key for the search job.
pub const JOB_KEY: &str = "files_search";

/// How many messages the panel takes from the search thread in one frame: the rest wait a frame, so a big
/// project's results never stall one.
pub const DRAIN_PER_FRAME: usize = 64;

/// The longest a result row's text is, in characters: a line of minified data is not a row.
pub const ROW_CHARS: usize = 160;

/// What the search thread says.
enum Msg {
    Hits(FileHits),
    Progress(usize, usize),
    Done(Result<(), String>),
    Planned(Result<Plan, Refused>),
}

/// A search or a preview in flight: what it says, and the flag that stops it.
struct Run {
    rx: Mutex<Receiver<Msg>>,
    cancel: Arc<AtomicBool>,
}

/// What a result row opens: a file at a line and a span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub rel: String,
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

/// **The search block**: what is asked, what is shown and what is running.
#[derive(Component, Default)]
pub struct SearchPanel {
    pub query: SearchQuery,
    pub replacement: String,
    run: Option<Run>,
    /// The plan on screen, when the person previewed.
    pub preview: Option<Plan>,
    /// What each result row opens, by the row's key.
    pub targets: HashMap<u64, Target>,
    /// How many files and matches have been found so far.
    pub files: usize,
    pub matches: usize,
    /// The sentence under the buttons.
    pub status: String,
    /// Whether the search has finished (or been stopped).
    pub done: bool,
}

/// The query field.
#[derive(Component)]
pub struct QueryField;
/// The path filter field.
#[derive(Component)]
pub struct PathsField;
/// The replacement field.
#[derive(Component)]
pub struct ReplaceField;
/// One of the three option toggles.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum OptionToggle {
    Case,
    Word,
    Regex,
}
/// The buttons.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SearchButton {
    Search,
    Cancel,
    Preview,
    ReplaceAll,
    Close,
}
/// The sentence under the buttons.
#[derive(Component)]
pub struct SearchStatus;
/// The results tree.
#[derive(Component)]
pub struct SearchResults;

/// The search panel's systems.
pub struct FilesPanelPlugin;

impl Plugin for FilesPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (open_the_search, drain_the_search, keep_the_status)
                .chain()
                .run_if(in_state(crate::screen::Screen::Editor))
                .run_if(|workspace: Option<Res<Workspace>>| workspace.is_some_and(|w| *w == Workspace::Files)),
        );
    }
}

fn row_key(rel: &str, line: usize, tag: &str) -> u64 {
    files::key_of(&format!("{rel}\0{line}\0{tag}"))
}

/// A result row's text: trimmed, one line, and not longer than [`ROW_CHARS`].
fn clip(text: &str) -> String {
    let one_line = text.trim().replace('\t', " ");
    if one_line.chars().count() <= ROW_CHARS {
        return one_line;
    }
    let mut out: String = one_line.chars().take(ROW_CHARS).collect();
    out.push('\u{2026}');
    out
}

// ── Opening ──────────────────────────────────────────────────────────────────

/// **`Cmd`/`Ctrl`+`Shift`+`F` opens the SEARCH block** (and puts the keyboard in its query), seeded with what the
/// editor's own find bar held: the same words, a step wider.
fn open_the_search(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    panels: Query<Entity, With<SearchPanel>>,
    sides: Query<Entity, With<FilesPanel>>,
    mut finds: Query<&mut CodeFind>,
    fields: Query<Entity, With<QueryField>>,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
) {
    if !Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, KeyCode::KeyF).just_pressed(&keys) {
        return;
    }
    // The editor's own bar opened on the same chord (it reads Cmd+F whatever else is held): this one is the wider
    // search, so the bar's words move here and the bar goes.
    let mut seed = (String::new(), Default::default());
    for mut find in &mut finds {
        if find.open {
            seed = (find.query.clone(), find.options);
            find.open = false;
        }
    }
    if let Ok(entity) = fields.single() {
        focus.set(entity, bevy::input_focus::FocusCause::Navigated);
        return;
    }
    if !panels.is_empty() {
        return;
    }
    let Ok(side) = sides.single() else { return };
    let panel = SearchPanel {
        query: SearchQuery { text: seed.0.clone(), case_sensitive: seed.1.case_sensitive, whole_word: seed.1.whole_word, regex: seed.1.regex, paths: String::new() },
        ..SearchPanel::default()
    };
    let query = seed.0;
    let options = (panel.query.case_sensitive, panel.query.whole_word, panel.query.regex);
    let block = commands
        .spawn((column(0.5), panel))
        .with_children(|block| {
            block.spawn(label("SEARCH"));
            block.spawn((text_input("Find in files", query), QueryField, observe(on_query)));
            block.spawn((
                row(0.5),
                children![
                    (toggle("Aa", options.0), OptionToggle::Case, observe(on_toggle)),
                    (toggle("Word", options.1), OptionToggle::Word, observe(on_toggle)),
                    (toggle("Regex", options.2), OptionToggle::Regex, observe(on_toggle)),
                ],
            ));
            block.spawn((text_input("Paths (*.yaml, assets/)", ""), PathsField, observe(on_paths)));
            block.spawn((text_input("Replace with", ""), ReplaceField, observe(on_replacement)));
            block.spawn((
                row(0.5),
                children![
                    (button("Search"), SearchButton::Search, observe(on_button)),
                    (button("Cancel"), SearchButton::Cancel, observe(on_button)),
                    (button("Preview"), SearchButton::Preview, observe(on_button)),
                    (button("Replace all"), SearchButton::ReplaceAll, observe(on_button)),
                    (button("\u{00D7}"), SearchButton::Close, observe(on_button)),
                ],
            ));
            block.spawn((label(String::new()), SearchStatus));
            block
                .spawn(bevy_gooey::fill_region())
                .with_children(|results| {
                    results.spawn((tree(Vec::<TreeNode>::new()), SearchResults, SelectMode::Single, observe(on_result)));
                });
        })
        .id();
    // The block takes twice the room of its siblings and may shrink to nothing: a fill region that stands a row gap between its parts.
    commands.entity(block).insert((
        bevy_gooey::fill_region_with(crate::chrome::GAP_ROW_STEPS),
        bevy_gooey::TextFlow { grow: Some(2.0), ..default() },
    ));
    commands.entity(side).add_child(block);
}

fn panel_of<'a>(entity: Entity, parents: &Query<&ChildOf>, panels: &'a mut Query<&mut SearchPanel>) -> Option<Mut<'a, SearchPanel>> {
    let mut at = entity;
    loop {
        if panels.contains(at) {
            return panels.get_mut(at).ok();
        }
        at = parents.get(at).ok()?.parent();
    }
}

fn on_query(change: On<ValueChange<String>>, parents: Query<&ChildOf>, mut panels: Query<&mut SearchPanel>, mut commands: Commands) {
    if let Some(mut panel) = panel_of(change.source, &parents, &mut panels) {
        panel.query.text = change.value.clone();
        // **A preview is of one query**: any change to it takes the preview away, so Replace all never applies another.
        panel.preview = None;
    }
    // Enter in the query field searches, as it does in a find bar.
    if change.is_final {
        commands.queue(start_the_search);
    }
}

fn on_paths(change: On<ValueChange<String>>, parents: Query<&ChildOf>, mut panels: Query<&mut SearchPanel>) {
    if let Some(mut panel) = panel_of(change.source, &parents, &mut panels) {
        panel.query.paths = change.value.clone();
        panel.preview = None;
    }
}

fn on_replacement(change: On<ValueChange<String>>, parents: Query<&ChildOf>, mut panels: Query<&mut SearchPanel>) {
    if let Some(mut panel) = panel_of(change.source, &parents, &mut panels) {
        panel.replacement = change.value.clone();
        panel.preview = None;
    }
}

fn on_toggle(change: On<ValueChange<bool>>, toggles: Query<&OptionToggle>, parents: Query<&ChildOf>, mut panels: Query<&mut SearchPanel>) {
    let (Ok(which), Some(mut panel)) = (toggles.get(change.source), panel_of(change.source, &parents, &mut panels)) else { return };
    match which {
        OptionToggle::Case => panel.query.case_sensitive = change.value,
        OptionToggle::Word => panel.query.whole_word = change.value,
        OptionToggle::Regex => panel.query.regex = change.value,
    }
    panel.preview = None;
}

fn on_button(activate: On<Activate>, buttons: Query<&SearchButton>, mut commands: Commands) {
    let Ok(button) = buttons.get(activate.entity) else { return };
    match button {
        SearchButton::Search => commands.queue(start_the_search),
        SearchButton::Cancel => commands.queue(stop_the_search),
        SearchButton::Preview => commands.queue(start_the_preview),
        SearchButton::ReplaceAll => commands.queue(replace_all),
        SearchButton::Close => commands.queue(close_the_search),
    }
}

// ── Running ──────────────────────────────────────────────────────────────────

fn the_panel(world: &mut World) -> Option<Entity> {
    world.query_filtered::<Entity, With<SearchPanel>>().iter(world).next()
}

fn the_results(world: &mut World) -> Option<Entity> {
    world.query_filtered::<Entity, With<SearchResults>>().iter(world).next()
}

/// Empty the results tree, and what its rows open.
fn clear_results(world: &mut World) {
    if let Some(results) = the_results(world)
        && let Some(mut table) = world.get_mut::<Table>(results)
        && let Some(tree) = table.source_mut::<Tree>()
    {
        tree.set_nodes(Vec::<TreeNode>::new());
    }
    if let Some(panel) = the_panel(world)
        && let Some(mut panel) = world.get_mut::<SearchPanel>(panel)
    {
        panel.targets.clear();
        panel.files = 0;
        panel.matches = 0;
    }
}

fn stop_run(panel: &mut SearchPanel) {
    if let Some(run) = panel.run.take() {
        run.cancel.store(true, Ordering::Release);
    }
}

/// **Start a search** of the project for what the query says, on a thread of its own.
pub fn start_the_search(world: &mut World) {
    let (Some(panel_entity), Some(root)) = (the_panel(world), world.get_resource::<Project>().map(|p| p.root.clone())) else { return };
    clear_results(world);
    let Some(mut panel) = world.get_mut::<SearchPanel>(panel_entity) else { return };
    stop_run(&mut panel);
    panel.preview = None;
    panel.done = false;
    if panel.query.text.is_empty() {
        panel.status = "Type what to find.".to_owned();
        return;
    }
    let query = panel.query.clone();
    let (tx, rx): (Sender<Msg>, Receiver<Msg>) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    std::thread::spawn(move || {
        let done = files_search::search(
            &root,
            &query,
            &flag,
            |found| {
                let _ = tx.send(Msg::Hits(found));
            },
            |read, of| {
                let _ = tx.send(Msg::Progress(read, of));
            },
        );
        let _ = tx.send(Msg::Done(done.map_err(|e| e.to_string())));
    });
    panel.run = Some(Run { rx: Mutex::new(rx), cancel });
    panel.status = "Searching\u{2026}".to_owned();
    if let Some(jobs) = world.get_resource::<Jobs>() {
        jobs.set(JOB_KEY, "Searching the project", "", None);
    }
}

/// **Stop the search** the band's Cancel or this panel's asked to stop.
pub fn stop_the_search(world: &mut World) {
    if let Some(panel) = the_panel(world)
        && let Some(mut panel) = world.get_mut::<SearchPanel>(panel)
    {
        stop_run(&mut panel);
        panel.done = true;
        panel.status = format!("Stopped: {} match(es) in {} file(s) so far.", panel.matches, panel.files);
    }
    if let Some(jobs) = world.get_resource::<Jobs>() {
        jobs.finish(JOB_KEY);
    }
}

/// **Plan the replace** over every file, on a thread, and show it as a diff when it comes back.
fn start_the_preview(world: &mut World) {
    let (Some(panel_entity), Some(root)) = (the_panel(world), world.get_resource::<Project>().map(|p| p.root.clone())) else { return };
    clear_results(world);
    let Some(mut panel) = world.get_mut::<SearchPanel>(panel_entity) else { return };
    stop_run(&mut panel);
    panel.preview = None;
    panel.done = false;
    if panel.query.text.is_empty() {
        panel.status = "Type what to find, and what to replace it with.".to_owned();
        return;
    }
    let (query, replacement) = (panel.query.clone(), panel.replacement.clone());
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    std::thread::spawn(move || {
        let _ = tx.send(Msg::Planned(files_search::plan(&root, &query, &replacement, None)));
    });
    panel.run = Some(Run { rx: Mutex::new(rx), cancel });
    panel.status = "Planning the replace\u{2026}".to_owned();
}

/// **Apply the previewed replace as the one operation `files_replace`**, and put it on the undo stack. With no
/// preview on screen it makes one, and writes nothing.
fn replace_all(world: &mut World) {
    let Some(panel_entity) = the_panel(world) else { return };
    let (query, replacement, preview) = {
        let Some(panel) = world.get::<SearchPanel>(panel_entity) else { return };
        (panel.query.clone(), panel.replacement.clone(), panel.preview.clone())
    };
    let Some(plan) = preview else {
        start_the_preview(world);
        if let Some(mut panel) = world.get_mut::<SearchPanel>(panel_entity) {
            panel.status = "Review the preview, then press Replace all again.".to_owned();
        }
        return;
    };
    let Some(project) = world.get_resource::<Project>().cloned() else { return };
    let mut ctx = crate::ops::Ctx { project: Some(project), source: crate::ops::Source::Human, session: crate::ops::session_id(), ..crate::ops::Ctx::default() };
    let params = serde_json::json!({
        "query": query.text,
        "replacement": replacement,
        "case_sensitive": query.case_sensitive,
        "whole_word": query.whole_word,
        "regex": query.regex,
        "paths": query.paths,
        "only": plan.files.iter().map(|f| f.rel.clone()).collect::<Vec<_>>(),
    });
    let said = match crate::ops::dispatch(&mut ctx, "files_replace", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            if let Some(mut state) = world.get_resource_mut::<EditorState>() {
                state.status.note(outcome.summary.clone());
                state.record_logged(operation_id, outcome.summary.clone(), false);
            }
            format!("{}. It is one step in the history: Cmd+Z on the Map takes it back whole (Undo is the Map's key).", outcome.summary)
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            if let Some(mut state) = world.get_resource_mut::<EditorState>() {
                state.status.problem(e.0.clone());
            }
            e.0
        }
    };
    clear_results(world);
    if let Some(mut panel) = world.get_mut::<SearchPanel>(panel_entity) {
        panel.preview = None;
        panel.status = said;
    }
}

fn close_the_search(world: &mut World) {
    stop_the_search(world);
    if let Some(panel) = the_panel(world) {
        world.entity_mut(panel).despawn();
    }
}

/// **Take what the thread found**, a little each frame, into the results tree.
fn drain_the_search(mut panels: Query<&mut SearchPanel>, mut results: Query<&mut Table, With<SearchResults>>, jobs: Option<Res<Jobs>>) {
    let Ok(mut panel) = panels.single_mut() else { return };
    let (Ok(mut table), true) = (results.single_mut(), panel.run.is_some()) else { return };
    // The band's Cancel is a request: the thread's own flag is what it reads.
    if let (Some(jobs), Some(run)) = (jobs.as_deref(), panel.run.as_ref())
        && jobs.cancel_requested(JOB_KEY)
    {
        run.cancel.store(true, Ordering::Release);
    }
    let mut taken: Vec<Msg> = Vec::new();
    if let Some(run) = &panel.run
        && let Ok(rx) = run.rx.lock()
    {
        while taken.len() < DRAIN_PER_FRAME {
            match rx.try_recv() {
                Ok(msg) => taken.push(msg),
                Err(_) => break,
            }
        }
    }
    for msg in taken {
        match msg {
            Msg::Hits(found) => {
                let FileHits { rel, hits, more } = found;
                let Some(tree) = table.source_mut::<Tree>() else { continue };
                let mut children: Vec<TreeNode> = Vec::new();
                for hit in &hits {
                    let key = row_key(&rel, hit.line, "hit");
                    let mut node = TreeNode::new(key, format!("{:>5}  {}", hit.line + 1, clip(&hit.text))).expanded();
                    for (n, before) in hit.before.iter().enumerate() {
                        let at = hit.line - hit.before.len() + n;
                        let ck = row_key(&rel, at, "ctx");
                        panel.targets.insert(ck, Target { rel: rel.clone(), line: at, col: 0, len: 0 });
                        node = node.child(TreeNode::new(ck, format!("{:>5}  {}", at + 1, clip(before))));
                    }
                    for (n, after) in hit.after.iter().enumerate() {
                        let at = hit.line + 1 + n;
                        let ck = row_key(&rel, at, "ctx");
                        panel.targets.insert(ck, Target { rel: rel.clone(), line: at, col: 0, len: 0 });
                        node = node.child(TreeNode::new(ck, format!("{:>5}  {}", at + 1, clip(after))));
                    }
                    panel.targets.insert(key, Target { rel: rel.clone(), line: hit.line, col: hit.col, len: hit.len });
                    children.push(node);
                }
                let file_key = files::key_of(&rel);
                panel.targets.insert(file_key, Target { rel: rel.clone(), line: 0, col: 0, len: 0 });
                let label = format!("{rel}  ({}{})", hits.len(), if more { "+" } else { "" });
                tree.insert(None, TreeNode::new(file_key, label).expanded().children(children));
                panel.files += 1;
                panel.matches += hits.len();
                panel.status = format!("{} match(es) in {} file(s)\u{2026}", panel.matches, panel.files);
            }
            Msg::Progress(read, of) => {
                if let Some(jobs) = jobs.as_deref() {
                    jobs.set(JOB_KEY, "Searching the project", format!("{read} of {of} files"), Some(read as f32 / of.max(1) as f32));
                }
            }
            Msg::Done(done) => {
                panel.run = None;
                panel.done = true;
                if let Some(jobs) = jobs.as_deref() {
                    jobs.finish(JOB_KEY);
                }
                panel.status = match done {
                    Ok(()) if panel.matches == 0 => "No matches.".to_owned(),
                    Ok(()) => format!("{} match(es) in {} file(s).", panel.matches, panel.files),
                    Err(why) => format!("This pattern cannot be searched: {why}"),
                };
            }
            Msg::Planned(planned) => {
                panel.run = None;
                panel.done = true;
                match planned {
                    Err(refusal) => panel.status = refusal.to_string(),
                    Ok(plan) => {
                        let Some(tree) = table.source_mut::<Tree>() else { continue };
                        for file in &plan.files {
                            let mut children: Vec<TreeNode> = Vec::new();
                            for change in &file.changes {
                                let (old_key, new_key) = (row_key(&file.rel, change.line, "-"), row_key(&file.rel, change.line, "+"));
                                let target = Target { rel: file.rel.clone(), line: change.line, col: 0, len: 0 };
                                panel.targets.insert(old_key, target.clone());
                                panel.targets.insert(new_key, target);
                                children.push(TreeNode::new(old_key, format!("-{:>5}  {}", change.line + 1, clip(&change.old))));
                                children.push(TreeNode::new(new_key, format!("+{:>5}  {}", change.line + 1, clip(&change.new))));
                            }
                            let file_key = files::key_of(&file.rel);
                            panel.targets.insert(file_key, Target { rel: file.rel.clone(), line: 0, col: 0, len: 0 });
                            tree.insert(None, TreeNode::new(file_key, format!("{}  ({})", file.rel, file.replacements)).expanded().children(children));
                        }
                        let skipped = if plan.skipped.is_empty() { String::new() } else { format!("; {} read-only file(s) left alone", plan.skipped.len()) };
                        panel.status = format!("Would replace {} match(es) in {} file(s){skipped}. Replace all applies it as one step.", plan.replacements(), plan.files.len());
                        panel.preview = Some(plan);
                    }
                }
            }
        }
    }
}

/// **The sentence under the buttons** follows the panel's state.
fn keep_the_status(panels: Query<&SearchPanel>, mut statuses: Query<&mut Text, With<SearchStatus>>) {
    let (Ok(panel), Ok(mut text)) = (panels.single(), statuses.single_mut()) else { return };
    if text.0 != panel.status {
        text.0 = panel.status.clone();
    }
}

// ── A result row ─────────────────────────────────────────────────────────────

/// **A click on a result opens its file at the match.**
fn on_result(picked: On<ValueChange<TableSelection>>, panels: Query<&SearchPanel>, mut commands: Commands) {
    let (Some(key), Ok(panel)) = (picked.value.iter().next(), panels.single()) else { return };
    let Some(target) = panel.targets.get(&key).cloned() else { return };
    commands.queue(move |world: &mut World| open_at(world, &target));
}

/// **Open `target.rel` in the Files workspace and bring the match into view**, selected.
pub fn open_at(world: &mut World, target: &Target) {
    if let Err(why) = files::open_file(world, &target.rel) {
        if let Some(mut state) = world.get_resource_mut::<EditorState>() {
            state.status.problem(format!("NOT OPENED: {why}"));
        }
        return;
    }
    if let Some(mut files) = world.get_resource_mut::<Files>()
        && let Some(file) = files.get_mut(&target.rel)
    {
        file.reveal = (target.len > 0 || target.line > 0).then_some((target.line, target.col, target.len));
    }
}
