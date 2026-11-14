//! **A hand-back the person kept becomes a benchmark task draft, and a wish names it** (BM-07, #417).
//!
//! The headless editor does what a sitting does: an agent applies three operations through the bridge,
//! presents them, the person keeps the result, and the Agent workspace's row for that hand-back gets
//! **Make a benchmark task**. The press writes `.dark_factory/bench/drafts/<id>/task.yaml`, and what is measured
//! is the draft:
//!
//! 1. **its scene is the map before the first operation**, rebuilt on a copy (the open map keeps what the agent
//!    left, byte for byte), and nothing is in its `setup`;
//! 2. **its `solution` passes its checks and its start fails them**, which is BM-01's validity test applied to a
//!    task nobody wrote, with the bench crate's own loader and grader;
//! 3. **its prompt is what the person asked for**, from the hand-back's `specified` lines;
//! 4. **a wish the person keeps from that hand-back names the draft**, as a `Benchmark task:` line in the issue;
//! 5. **a hand-back with an operation that records no inverse has the button disabled**, naming the operation.
//!
//! The test lives in this crate and not the editor's because the proof needs the bench crate's loader and
//! grader, which depend on the editor: the editor cannot depend back on them.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

use bevy_cauldron::agent_view::{AgentHandBackRow, AgentMakeTask};
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::keys::Live;
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::wish_view::{WishOf, WishOpen, WishUnfold};
use bevy_cauldron::workspace::Workspace;
use bevy_cauldron_bench::{fixture, Grading, Outcome, Snapshot, Task, PRESENT_STEP};

/// Frames a press is given to settle in.
const SETTLE_FRAMES: usize = 12;
/// One frame of the test clock, in seconds.
const FRAME_S: f32 = 1.0 / 60.0;
/// The map the repository's benchmark fixture opens.
const MAP: &str = "bench_bedroom";

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// **A scratch path this test owns**, named with the test's name and the process id.
fn scratch(test: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bench-draft-test-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn repo() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|e| panic!("cannot resolve the repository root: {e}"))
}

/// **The benchmark's own project**, laid out the way a trial's is: the repository's `assets/` and the suite's fixture over it.
fn project(test: &str) -> std::path::PathBuf {
    let into = scratch(test);
    fixture::lay_out(&repo(), &into).unwrap_or_else(|e| panic!("the fixture is laid out: {e}"));
    into
}

/// Removes a scratch file when the test ends, however it ends.
struct Remove(std::path::PathBuf);

impl Drop for Remove {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// **One editor, on a project, with the agent's bridge**, and the editor not following the agent (a followed change would carry the person off
/// the workspace being measured).
fn editor(root: &std::path::Path, test: &str) -> (App, Remove) {
    let mut app = bevy_cauldron::harness::build_headless(root, MAP, None).unwrap_or_else(|e| panic!("the project opens: {e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME_S)));
    updates(&mut app, 8);
    let file = std::env::temp_dir().join(format!("bench_draft_settings_{test}_{}.yaml", std::process::id()));
    app.insert_resource(bevy_cauldron::settings::UserSettingsFile(file.clone()));
    bevy_cauldron::setting_catalogue::write_key(&file, "follow_agent", Some(bevy_cauldron::setting_catalogue::Value::On(false)))
        .unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 8);
    (app, Remove(file))
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned() else {
        panic!("`{method}` is not registered");
    };
    let answer = app
        .world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"));
    updates(app, 4);
    answer
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> String {
    let answer = call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    );
    answer["operation_id"].as_str().unwrap_or_else(|| panic!("`{operation}` names its id: {answer}")).to_owned()
}

fn switch_to(app: &mut App, workspace: Workspace) {
    let id = format!("workspace.{}", workspace.id());
    let command = commands::by_id(&id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    updates(app, SETTLE_FRAMES);
    assert_eq!(
        app.world().resource::<Workspace>(),
        &workspace,
        "`{id}` did not open the {} workspace (a question open: {}; {})",
        workspace.title(),
        app.world().resource::<bevy_cauldron::confirm::Confirm>().is_open(),
        said(app)
    );
}

fn press(app: &mut App, entity: Entity) {
    app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity });
    updates(app, SETTLE_FRAMES);
}

/// **The map file the editor holds open**, as the agent left it.
fn map_text(root: &std::path::Path) -> String {
    let path = root.join("assets/grimoire/maps").join(format!("{MAP}.map.yaml"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// **Present the operations and keep the result**, as an agent hands back and a person accepts. Answers the hand-back's id.
fn hand_back_and_keep(app: &mut App, operation_ids: &[String]) -> String {
    let presented = call(
        app,
        "grimoire/present",
        serde_json::json!({
            "title": "The bedroom, rearranged",
            "summary": "A double bed against the north wall, the chair turned about, and the drawer taken out.",
            "specified": ["a double bed flat against the north wall", "the chair turned to face the other way", "the drawer gone"],
            "operation_ids": operation_ids,
        }),
    );
    let id = presented["presentation"].as_str().unwrap_or_else(|| panic!("the present names its id: {presented}")).to_owned();
    // **The person keeps it**: the modal's No is `N`, the answer that changes nothing and keeps the result.
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyN,
            logical_key: Key::Character("n".into()),
            state,
            text: (state == ButtonState::Pressed).then(|| "n".into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
    updates(app, SETTLE_FRAMES);
    id
}

/// **The button of a kept hand-back**, when the Agent workspace draws one.
fn make_button(app: &mut App, id: &str) -> Option<Entity> {
    let world = app.world_mut();
    world.query::<(Entity, &AgentMakeTask)>().iter(world).find(|(_, b)| b.0 == id).map(|(e, _)| e)
}

/// **What the status line says**, which is where a refusal to make a task is told.
fn said(app: &App) -> String {
    let state = app.world().resource::<bevy_cauldron::editor::EditorState>();
    format!("note: {:?}; problems: {:?}", state.status.note_text(), state.status.problems().iter().map(|p| p.text.clone()).collect::<Vec<_>>())
}

/// **Every draft folder the project holds**, by name.
fn drafts(root: &std::path::Path) -> Vec<String> {
    let dir = root.join(".dark_factory/bench/drafts");
    let mut out: Vec<String> = std::fs::read_dir(&dir)
        .map(|entries| entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    out.sort();
    out
}

/// **Three operations an agent made**: a bed added, a chair turned, a drawer removed. One of each kind of change a check is drafted from.
fn three_operations(app: &mut App) -> Vec<String> {
    vec![
        apply(app, "placement_add", serde_json::json!({"piece": "bed_double", "at": [0.0, -1.39], "yaw": 0.0})),
        apply(app, "placement_set_yaw", serde_json::json!({"id": "chair", "yaw": 180.0})),
        apply(app, "placement_remove", serde_json::json!({"id": "drawer"})),
    ]
}

/// **A task, proved both ways on a fresh trial**: the start must fail a check, and the `solution` must pass every one.
///
/// BM-01's rule, made on the headless editor with the task's own fixture over the project, which is how a trial starts.
fn prove(project: &std::path::Path, task: &Task, test: &str) -> Result<(), String> {
    let into = scratch(&format!("{test}-trial"));
    fixture::lay_out_task(project, &task.dir, &into).map_err(|e| format!("the trial lays out: {e}"))?;
    let mut app = bevy_cauldron::harness::build_headless(&into, &task.map, None).map_err(|e| format!("the trial opens: {e}"))?;
    updates(&mut app, 8);
    let mut ctx = Ctx {
        project: Some(app.world().resource::<bevy_cauldron::project::Project>().clone()),
        open: Some(app.world().resource::<bevy_cauldron::project::OpenMap>().clone()),
        source: ops::Source::Agent,
        session: "bench-draft-proof".to_owned(),
        ..Ctx::default()
    };
    let untouched = Snapshot::read(&into, &task.map, Vec::new(), Default::default());
    let nothing = Grading::of(task, &untouched, &untouched);
    if nothing.outcome == Outcome::Pass {
        return Err(format!("{} — the draft's start already passes every check", nothing.sentence()));
    }
    for (n, step) in task.solution.iter().enumerate() {
        if step.op == PRESENT_STEP {
            continue;
        }
        match ops::dispatch(&mut ctx, &step.op, &step.params) {
            Ok(Applied::Done { .. }) => {}
            other => return Err(format!("solution step {n} (`{}`) did not apply: {other:?}", step.op)),
        }
    }
    let left = Snapshot::read(&into, &task.map, Vec::new(), Default::default());
    let graded = Grading::of(task, &untouched, &left);
    let _ = std::fs::remove_dir_all(&into);
    if graded.outcome != Outcome::Pass {
        return Err(format!("{} — the draft's own solution does not pass its checks", graded.sentence()));
    }
    Ok(())
}

/// **The draft's task, read by the bench crate's own loader**, which refuses what it does not read.
fn load(project: &std::path::Path, id: &str) -> Task {
    let known: Vec<&str> = ops::registry().iter().map(|op| op.name()).collect();
    Task::load(&project.join(".dark_factory/bench/drafts").join(id), project, &known)
        .unwrap_or_else(|p| panic!("the draft loads: {}", p.line()))
}

/// **1-3. A kept hand-back makes a draft whose start is the map before and whose solution passes its own checks.**
#[test]
fn a_kept_handback_makes_a_task_draft_that_proves_itself() {
    let root = project("draft");
    let (mut app, _settings) = editor(&root, "draft");
    let before = bevy_grimoire_core::map::Map::parse(&map_text(&root)).unwrap_or_else(|e| panic!("{e}"));
    let ids = three_operations(&mut app);
    let left_by_the_agent = map_text(&root);
    let handback = hand_back_and_keep(&mut app, &ids);
    switch_to(&mut app, Workspace::Agent);
    updates(&mut app, SETTLE_FRAMES);

    let button = make_button(&mut app, &handback).unwrap_or_else(|| panic!("the Agent workspace offers no Make a benchmark task for the kept hand-back `{handback}`"));
    assert!(app.world().get::<bevy::ui::InteractionDisabled>(button).is_none(), "the button is enabled: every operation recorded its inverse");
    let row = {
        let world = app.world_mut();
        world.query_filtered::<&Text, With<AgentHandBackRow>>().iter(world).map(|t| t.0.clone()).collect::<Vec<_>>()
    };
    assert!(row.iter().any(|t| t.contains("The bedroom, rearranged")), "the hand-back has a row in the history: {row:?}");
    press(&mut app, button);

    // **The open map is never touched**: the file keeps what the agent left, byte for byte.
    assert_eq!(map_text(&root), left_by_the_agent, "making a task wrote to the open map");
    let made = drafts(&root);
    assert_eq!(made.len(), 1, "one draft was written: {made:?}; {}", said(&app));
    assert!(made[0].starts_with(&format!("{handback}-")), "the draft is named for its hand-back: {made:?}");
    let task = load(&root, &made[0]);

    // **The prompt is what the person asked for.**
    for line in ["a double bed flat against the north wall", "the chair turned to face the other way", "the drawer gone"] {
        assert!(task.prompt.contains(line), "the prompt carries `{line}`: {}", task.prompt);
    }
    // **The setup is the map before the first operation**, as the task's own scene, and nothing else.
    assert!(task.setup.is_empty(), "the scene is the draft's fixture, so the setup is empty: {:?}", task.setup);
    assert_eq!(task.map, MAP);
    let scene_path = task.dir.join("fixture/assets/grimoire/maps").join(format!("{MAP}.map.yaml"));
    let scene = bevy_grimoire_core::map::Map::parse(
        &std::fs::read_to_string(&scene_path).unwrap_or_else(|e| panic!("the draft keeps the scene: {}: {e}", scene_path.display())),
    )
    .unwrap_or_else(|e| panic!("the scene reads: {e}"));
    assert_eq!(scene, before, "the draft's scene is the map before the agent's first operation");
    // **The solution is the operations**, in order, and the hand-back.
    let ops_of: Vec<&str> = task.solution.iter().map(|s| s.op.as_str()).collect();
    assert_eq!(ops_of, ["placement_add", "placement_set_yaw", "placement_remove", "grimoire_present"]);
    // **The checks are drafted from the change**: one per placement added, turned and removed, and the other map files unchanged.
    let names: Vec<&str> = task.checks.iter().map(bevy_cauldron_bench::Check::name).collect();
    assert_eq!(names.iter().filter(|n| **n == "placement_at").count(), 2, "the bed added and the chair turned: {names:?}");
    assert_eq!(names.iter().filter(|n| **n == "placement_exists").count(), 1, "the drawer removed: {names:?}");

    // **The draft proves itself**: its start fails a check, and its solution passes every one.
    prove(&root, &task, "draft").unwrap_or_else(|why| panic!("{why}"));

    // **And it opened in the Files workspace for review**, which is where a person reads it before moving it.
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Files, "the draft opens in the Files workspace");
    let _ = std::fs::remove_dir_all(&root);
}

/// **A second press shows the draft already there and writes nothing again**, because the person may have edited it.
#[test]
fn a_second_press_leaves_the_draft_as_the_person_edited_it() {
    let root = project("twice");
    let (mut app, _settings) = editor(&root, "twice");
    let ids = three_operations(&mut app);
    let handback = hand_back_and_keep(&mut app, &ids);
    switch_to(&mut app, Workspace::Agent);
    let button = make_button(&mut app, &handback).unwrap_or_else(|| panic!("no button for `{handback}`"));
    press(&mut app, button);
    let made = drafts(&root);
    let file = root.join(".dark_factory/bench/drafts").join(&made[0]).join("task.yaml");
    let mut text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{e}"));
    text.push_str("# the person's own note\n");
    std::fs::write(&file, &text).unwrap_or_else(|e| panic!("{e}"));
    switch_to(&mut app, Workspace::Agent);
    let button = make_button(&mut app, &handback).unwrap_or_else(|| panic!("no button the second time"));
    press(&mut app, button);
    assert_eq!(std::fs::read_to_string(&file).unwrap_or_default(), text, "a second press rewrote the person's draft");
    assert_eq!(drafts(&root), made, "and made no second draft");
    let _ = std::fs::remove_dir_all(&root);
}

/// **4. A wish the person keeps from that hand-back names the draft.**
#[test]
fn a_kept_wish_names_the_benchmark_task_in_its_issue() {
    let root = project("wish");
    let (mut app, _settings) = editor(&root, "wish");
    let ids = three_operations(&mut app);
    let handback = hand_back_and_keep(&mut app, &ids);
    // The agent's wish, made before the person made the task: the address is built when the wish is made.
    let made = call(
        &mut app,
        "grimoire/wish",
        serde_json::json!({
            "after": {"presentation": handback},
            "task": "rearranged the bedroom",
            "hardship": "three calls to find the piece ids of the chair and the drawer",
            "tool": "grimoire_find_piece: searches the map's placements by piece and answers their ids.",
            "example_call": "grimoire_find_piece({\"piece\": \"chair\"})",
        }),
    );
    let wish = made["id"].as_str().unwrap_or_else(|| panic!("the wish names its id: {made}")).to_owned();
    switch_to(&mut app, Workspace::Agent);
    let button = make_button(&mut app, &handback).unwrap_or_else(|| panic!("no button for `{handback}`"));
    press(&mut app, button);
    let rel = {
        let name = drafts(&root).remove(0);
        format!(".dark_factory/bench/drafts/{name}/task.yaml")
    };
    switch_to(&mut app, Workspace::Agent);
    let entity_for = |app: &mut App, part: &str| -> Entity {
        let world = app.world_mut();
        let found = match part {
            "unfold" => world.query::<(Entity, &WishUnfold, &WishOf)>().iter(world).find(|(_, _, o)| o.0 == wish).map(|(e, _, _)| e),
            _ => world.query::<(Entity, &WishOpen, &WishOf)>().iter(world).find(|(_, _, o)| o.0 == wish).map(|(e, _, _)| e),
        };
        found.unwrap_or_else(|| panic!("the Agent workspace shows no {part} for `{wish}`"))
    };
    let unfold = entity_for(&mut app, "unfold");
    press(&mut app, unfold);
    let open = entity_for(&mut app, "open");
    press(&mut app, open);
    let links = bevy_cauldron::harness::opened_links(&app);
    assert_eq!(links.len(), 1, "Open on GitHub opened one address: {links:?}");
    let named = bevy_grimoire_core::wish::percent_encode(&format!("Benchmark task: {rel}"));
    assert!(links[0].contains(&named), "the issue's context names the draft's path `{rel}`: {}", links[0]);
    let _ = std::fs::remove_dir_all(&root);
}

/// **A wish from a hand-back with no draft files the issue it always did**: no `Benchmark task:` line.
#[test]
fn a_wish_with_no_draft_names_none() {
    let root = project("no_draft");
    let (mut app, _settings) = editor(&root, "no_draft");
    let ids = three_operations(&mut app);
    let handback = hand_back_and_keep(&mut app, &ids);
    let made = call(
        &mut app,
        "grimoire/wish",
        serde_json::json!({
            "after": {"presentation": handback},
            "task": "rearranged the bedroom",
            "hardship": "three calls to find the piece ids of the chair and the drawer",
            "tool": "grimoire_find_piece: searches the map's placements by piece and answers their ids.",
            "example_call": "grimoire_find_piece({\"piece\": \"chair\"})",
        }),
    );
    let wish = made["id"].as_str().unwrap_or_default().to_owned();
    switch_to(&mut app, Workspace::Agent);
    let (unfold, open) = {
        let world = app.world_mut();
        let unfold = world.query::<(Entity, &WishUnfold, &WishOf)>().iter(world).find(|(_, _, o)| o.0 == wish).map(|(e, _, _)| e);
        (unfold, ())
    };
    let unfold = unfold.unwrap_or_else(|| panic!("no wish row"));
    press(&mut app, unfold);
    let open_button = {
        let world = app.world_mut();
        world.query::<(Entity, &WishOpen, &WishOf)>().iter(world).find(|(_, _, o)| o.0 == wish).map(|(e, _, _)| e)
    }
    .unwrap_or_else(|| panic!("no Open on GitHub"));
    let _ = open;
    press(&mut app, open_button);
    let links = bevy_cauldron::harness::opened_links(&app);
    assert_eq!(links.len(), 1);
    assert!(!links[0].contains("Benchmark"), "a wish with no draft names one: {}", links[0]);
    let _ = std::fs::remove_dir_all(&root);
}

/// **5. A hand-back with an operation that records no inverse has the button disabled, and the tooltip names the operation.**
#[test]
fn an_operation_with_no_inverse_disables_the_button_and_is_named() {
    let root = project("no_inverse");
    let (mut app, _settings) = editor(&root, "no_inverse");
    // A view verb changes what a person looks through and records no inverse: there is nothing to put back, so the scene before the
    // hand-back cannot be rebuilt by taking its operations back.
    let ids = vec![
        apply(&mut app, "placement_add", serde_json::json!({"piece": "bed_double", "at": [0.0, -1.39], "yaw": 0.0})),
        apply(&mut app, "view_cycle_grid", serde_json::json!({})),
    ];
    let handback = hand_back_and_keep(&mut app, &ids);
    switch_to(&mut app, Workspace::Agent);
    let button = make_button(&mut app, &handback).unwrap_or_else(|| panic!("a kept hand-back still has its button, disabled"));
    assert!(app.world().get::<bevy::ui::InteractionDisabled>(button).is_some(), "the button is disabled");
    let tooltip = app
        .world()
        .get::<bevy_gooey::Tooltip>(button)
        .map(|t| format!("{t:?}"))
        .unwrap_or_else(|| panic!("the disabled button carries a tooltip"));
    assert!(tooltip.contains("view_cycle_grid"), "the tooltip names the operation: {tooltip}");
    // Pressed anyway (a disabled button does not Activate, but nothing may depend on that), nothing is drafted.
    press(&mut app, button);
    assert!(drafts(&root).is_empty(), "a hand-back that cannot be rebuilt wrote a draft: {:?}", drafts(&root));
    let _ = std::fs::remove_dir_all(&root);
}

/// **A hand-back the person took back offers no button**: nothing was kept, so nothing is a result to ask for.
#[test]
fn a_hand_back_nobody_kept_offers_no_task() {
    let root = project("unkept");
    let (mut app, _settings) = editor(&root, "unkept");
    let ids = three_operations(&mut app);
    let presented = call(
        &mut app,
        "grimoire/present",
        serde_json::json!({"title": "Not kept", "summary": "taken back", "specified": ["x"], "operation_ids": ids}),
    );
    let handback = presented["presentation"].as_str().unwrap_or_default().to_owned();
    // **The person answers Y**: undo all, which keeps nothing.
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyY,
            logical_key: Key::Character("y".into()),
            state,
            text: (state == ButtonState::Pressed).then(|| "y".into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
    updates(&mut app, SETTLE_FRAMES);
    switch_to(&mut app, Workspace::Agent);
    updates(&mut app, SETTLE_FRAMES);
    assert!(make_button(&mut app, &handback).is_none(), "a hand-back the person took back has a Make a benchmark task");
    let _ = std::fs::remove_dir_all(&root);
}
