//! **Rhai scripts on entities, reloaded live** (GD-06, #368).
//!
//! Each test writes a real `.rhai` file into a scratch project, spawns an entity carrying `Script`, and then
//! either steps the world or **rewrites the file on disk** and checks that the running game followed — because
//! "saving a script recompiles it in the running game, and `this.state` survives" is the whole of the issue.
//!
//! The world is stepped by hand with a clock that moves a fixed step per frame, so a timer of two seconds is a
//! countable number of frames rather than a sleep. `std::thread::sleep` is what a reload test must not use: the
//! settle rule is measured in the fake clock here and in real seconds in a game.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use bevy_grimoire_rhai::{
    HOOK_EVENT, SCRIPTS_DIR, SCRIPT_SETTLE_S, SCRIPT_STEP_LIMIT, Script, ScriptEvent, ScriptPlugin, ScriptProblem, Scripts,
};

/// **A component the test game registers**, so a script has something of its own to read and write.
#[derive(Component, Reflect, Default, Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize)]
struct Health {
    current: i32,
    max: i32,
}

/// **A second one**, so "a script writes a component" is not satisfied by one lucky type.
#[derive(Component, Reflect, Default, Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize)]
struct Door {
    open: bool,
}

/// **A scratch project of this test's own**: its name and the process id, which the tests beside it cannot
/// collide with.
fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(SCRIPTS_DIR)).expect("the script folder is made");
    dir
}

/// **Write one script.** `scripts/<name>.rhai`, the path a `Script` names.
fn write_script(root: &Path, name: &str, text: &str) -> PathBuf {
    let path = root.join(SCRIPTS_DIR).join(format!("{name}.rhai"));
    std::fs::write(&path, text).expect("the script is written");
    path
}

/// **A headless game with the two components registered and the script host on.**
fn game(root: &Path) -> App {
    let mut app = App::new();
    app.init_resource::<AppTypeRegistry>();
    // A bare `App` has no `Time`; the hooks and the watcher both read one, so the test's game has a clock it
    // advances itself rather than waiting on a wall clock.
    app.init_resource::<Time>();
    app.register_type::<Health>().register_type::<Door>();
    app.add_plugins(ScriptPlugin { root: root.to_path_buf() });
    app
}

/// **One frame**, with the clock moved a tenth of a second: a timer of one second is ten frames, and the
/// watcher's poll and settle are a tenth each.
fn frame(app: &mut App) {
    app.world_mut().resource_mut::<Time>().advance_by(Duration::from_millis(100));
    app.update();
    app.finish();
    app.cleanup();
}

/// **Several frames**, which is how long a save takes to be noticed and settle.
fn frames(app: &mut App, count: usize) {
    for _ in 0..count {
        frame(app);
    }
}

/// **How many frames a save needs to be read**: the poll interval plus the settle time, in this test's clock,
/// with one frame to spare so the boundary is not the test's own arithmetic.
fn settle_frames() -> usize {
    ((SCRIPT_SETTLE_S / 0.1).ceil() as usize) + 4
}

/// **The components of the one entity a test made**, as the world holds them.
fn health(app: &App, entity: Entity) -> Health {
    app.world().get::<Health>(entity).cloned().expect("the entity has a `Health`")
}

fn door_is_open(app: &App, entity: Entity) -> bool {
    app.world().get::<Door>(entity).cloned().expect("the entity has a `Door`").open
}

/// **The messages of one type this frame**, drained so a later frame's count is its own.
fn sent<M: Message + Clone>(app: &mut App) -> Vec<M> {
    app.world_mut().resource_mut::<Messages<M>>().drain().collect()
}

/// **Every problem raised across a run of frames**, collected per frame because Bevy keeps a message for two
/// frames: draining at the end would let a problem from the first frame of a wait expire unseen.
fn problems_over(app: &mut App, count: usize) -> Vec<ScriptProblem> {
    let mut problems = Vec::new();
    for _ in 0..count {
        frame(app);
        problems.extend(sent::<ScriptProblem>(app));
    }
    problems
}

/// **`this.state.count` of one entity**, which is the map the acceptance says survives a reload.
fn state_count(app: &App, entity: Entity) -> Option<i64> {
    app.world()
        .resource::<Scripts>()
        .state_of(entity)
        .and_then(|state| state.get("count").cloned())
        .and_then(|value| value.as_int().ok())
}

/// **The acceptance's central claim**: a script counts frames in `this.state`, the file is edited on disk, the
/// new code runs, and the count carries on from where it was.
#[test]
fn a_saved_script_runs_and_its_state_carries_on_across_the_reload() {
    let root = project("rhai_reload_state");
    let path = write_script(
        &root,
        "counter",
        // **The first version counts up**, one per frame.
        "fn on_update(dt) {\n    this.state.count = (this.state.count ?? 0) + 1;\n}\n",
    );
    let mut app = game(&root);
    let entity = app.world_mut().spawn((Script { file: format!("{SCRIPTS_DIR}/counter.rhai") }, Health { current: 10, max: 10 })).id();
    frames(&mut app, 4);
    let before = state_count(&app, entity).expect("the script kept a count");
    assert!(before >= 3, "the script counted the frames it ran in: {before}");

    // **The edit**: the script now stops counting and starts writing a component. The new code is what runs.
    std::thread::sleep(Duration::from_millis(1100));
    std::fs::write(&path, "fn on_update(dt) {\n    this.Health.current = 1;\n}\n").expect("the script is saved");
    frames(&mut app, settle_frames());

    assert_eq!(health(&app, entity).current, 1, "the new code ran: it wrote `Health`");
    // **The state carried across**: the new version never mentions `count`, so a count that is still there is a
    // map this crate kept rather than one the new code made.
    assert!(state_count(&app, entity).is_some(), "`this.state` survived the reload: {:?}", app.world().resource::<Scripts>().state_of(entity));

    let _ = std::fs::remove_dir_all(&root);
}

/// **A save with a syntax error leaves the old version running**, and the problem names the file **and the line**.
#[test]
fn a_script_with_a_syntax_error_leaves_the_old_version_running_and_names_the_file_and_line() {
    let root = project("rhai_syntax_error");
    let path = write_script(&root, "door", "fn on_update(dt) {\n    this.Door.open = true;\n}\n");
    let mut app = game(&root);
    let entity = app.world_mut().spawn((Script { file: format!("{SCRIPTS_DIR}/door.rhai") }, Door { open: false })).id();
    frames(&mut app, 2);
    assert!(door_is_open(&app, entity), "the good version opened the door");
    let _ = sent::<ScriptProblem>(&mut app);

    // **The edit that does not compile.** The bad line is the third, which is what the problem has to name.
    std::thread::sleep(Duration::from_millis(1100));
    std::fs::write(&path, "fn on_update(dt) {\n    let x = 1;\n    x.+\n}\n").expect("the broken script is saved");

    let problems = problems_over(&mut app, settle_frames());
    assert_eq!(problems.len(), 1, "one save, one problem: {problems:?}");
    let problem = &problems[0];
    let line = problem.line();
    assert!(line.contains("door.rhai"), "the problem names the file: {line}");
    assert_eq!(problem.problem.where_.as_ref().and_then(|where_| where_.line), Some(3), "and the line it is wrong at: {line}");

    // **The old version is still the running one**, which is the point: what the old code did it still does.
    app.world_mut().entity_mut(entity).insert(Door { open: false });
    frame(&mut app);
    frame(&mut app);
    assert!(door_is_open(&app, entity), "the last good version is still running, so it still opens the door");

    let _ = std::fs::remove_dir_all(&root);
}

/// **An endless loop is stopped at the step limit**, with a problem naming the file, and the game keeps running.
#[test]
fn an_endless_loop_is_stopped_at_the_step_limit() {
    let root = project("rhai_endless_loop");
    write_script(&root, "spin", "fn on_update(dt) {\n    let n = 0;\n    loop {\n        n += 1;\n    }\n}\n");
    let mut app = game(&root);
    app.world_mut().spawn((Script { file: format!("{SCRIPTS_DIR}/spin.rhai") }, Health { current: 10, max: 10 }));

    // **The frame returns**, which is the acceptance: a runaway script costs a frame, not the game.
    frame(&mut app);
    let problems = sent::<ScriptProblem>(&mut app);
    assert_eq!(problems.len(), 1, "the runaway was stopped and reported: {problems:?}");
    let line = problems[0].line();
    assert!(line.contains("spin.rhai"), "the problem names the file: {line}");
    // **`SCRIPT_STEP_LIMIT` is what stopped it**, which is worth saying rather than only that something did.
    assert!(
        line.to_lowercase().contains("operation"),
        "the problem says the step budget was spent, at {SCRIPT_STEP_LIMIT}: {line}"
    );

    // And the game is alive: the next frame runs, and the script is stopped again rather than never run.
    frame(&mut app);
    assert_eq!(sent::<ScriptProblem>(&mut app).len(), 1, "the next frame is stopped the same way, not frozen");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A script reads and writes its own reflected components by name**, which is what "reflection is the only
/// bridge" means for a script as much as for a file.
#[test]
fn a_script_reads_and_writes_the_components_the_game_registered() {
    let root = project("rhai_components");
    write_script(&root, "hurt", "fn on_update(dt) {\n    this.Health.current -= 1;\n}\n");
    let mut app = game(&root);
    let entity = app.world_mut().spawn((Script { file: format!("{SCRIPTS_DIR}/hurt.rhai") }, Health { current: 10, max: 10 })).id();
    frames(&mut app, 3);
    assert_eq!(health(&app, entity).current, 7, "one point a frame, off the component the game registered");
    assert_eq!(health(&app, entity).max, 10, "and the field it did not touch is untouched");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A script finds another entity by name and writes one of its components**, which is the `find`/`world_set`
/// pair the module documents.
#[test]
fn a_script_finds_another_entity_by_name_and_writes_it() {
    let root = project("rhai_find");
    write_script(&root, "reader", "fn on_update(dt) {\n    world_set(\"gate\", \"Door\", #{ open: true });\n    this.Health.current = find(\"gate\").Health.current;\n}\n");
    let mut app = game(&root);
    let gate = app.world_mut().spawn((Name::new("gate"), Health { current: 42, max: 42 }, Door { open: false })).id();
    let reader = app.world_mut().spawn((Name::new("reader"), Script { file: format!("{SCRIPTS_DIR}/reader.rhai") }, Health { current: 0, max: 42 })).id();
    frames(&mut app, 2);
    assert!(door_is_open(&app, gate), "the script opened the other entity's door");
    assert_eq!(health(&app, reader).current, 42, "and read its `Health` back into its own");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A script sends an event and another script receives it**, which is the pair the issue asks for.
#[test]
fn a_script_sends_an_event_another_script_receives() {
    let root = project("rhai_events");
    write_script(&root, "bell", "fn on_start() {\n    emit(\"pressed\");\n}\n");
    write_script(&root, "lamp", "fn on_event(name, data) {\n    if name == \"pressed\" { this.Door.open = true; }\n}\n");
    let mut app = game(&root);
    let lamp = app.world_mut().spawn((Name::new("lamp"), Script { file: format!("{SCRIPTS_DIR}/lamp.rhai") }, Door { open: false })).id();
    app.world_mut().spawn((Name::new("bell"), Script { file: format!("{SCRIPTS_DIR}/bell.rhai") }));
    frames(&mut app, 3);
    assert!(door_is_open(&app, lamp), "the event the bell sent opened the lamp's door");
    let _ = HOOK_EVENT;

    let _ = std::fs::remove_dir_all(&root);
}

/// **A script starts a timer, and the timer fires once as an event**, which is the fourth thing the issue names.
#[test]
fn a_script_starts_a_timer_that_fires_once() {
    let root = project("rhai_timer");
    write_script(
        &root,
        "fuse",
        "fn on_start() {\n    timer(\"bang\", 1.0);\n}\nfn on_event(name, data) {\n    if name == \"timer\" && data == \"bang\" { this.Health.current = 0; }\n}\n",
    );
    let mut app = game(&root);
    let fuse = app.world_mut().spawn((Script { file: format!("{SCRIPTS_DIR}/fuse.rhai") }, Health { current: 10, max: 10 })).id();
    // **Half the timer's length**: it has not fired yet, so the component is untouched.
    frames(&mut app, 4);
    assert_eq!(health(&app, fuse).current, 10, "a second has not passed");
    // **Past it**: the timer fired and the event it carried ran the hook.
    frames(&mut app, 8);
    assert_eq!(health(&app, fuse).current, 0, "the timer fired once, and the hook it names ran");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A file a `Script` names but nobody wrote is a problem naming the path**, not an idle script: a typo and a
/// script whose hooks all do nothing look identical from outside otherwise.
#[test]
fn a_script_file_that_is_not_there_is_a_problem_naming_its_path() {
    let root = project("rhai_missing");
    let mut app = game(&root);
    app.world_mut().spawn(Script { file: format!("{SCRIPTS_DIR}/nowhere.rhai") });
    frame(&mut app);
    let problems = sent::<ScriptProblem>(&mut app);
    assert_eq!(problems.len(), 1, "one missing file, one problem: {problems:?}");
    let line = problems[0].line();
    assert!(line.contains("nowhere.rhai"), "the problem names the path a person has to fix: {line}");
    assert_eq!(app.world().resource::<Scripts>().compiled(), 0, "and nothing claims to be running");
    let _ = std::fs::remove_dir_all(&root);
}

/// **`Script` is a component a game's own entity file may set**, which is what makes a script configurable from
/// YAML rather than from Rust.
#[test]
fn a_script_is_a_registered_component_a_file_can_name() {
    let app = game(Path::new("/nonexistent"));
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let registration = registry
        .get_with_short_type_path("Script")
        .unwrap_or_else(|| panic!("`Script` is registered under its short path"));
    assert!(registration.data::<ReflectComponent>().is_some(), "and as a component");
    assert!(registration.data::<bevy::reflect::ReflectDeserialize>().is_some(), "with a value a file can write");
    assert_eq!(registration.type_info().type_path_table().short_path(), "Script");
}

/// **`ScriptEvent` a game writes reaches a script**, which is how a game's own input presses a script's button
/// without going through a script to send it.
#[test]
fn an_event_a_game_writes_reaches_the_script_it_addresses() {
    let root = project("rhai_addressed");
    write_script(&root, "lamp", "fn on_event(name, data) {\n    if name == \"pressed\" { this.Door.open = true; }\n}\n");
    let mut app = game(&root);
    let lamp = app.world_mut().spawn((Name::new("lamp"), Script { file: format!("{SCRIPTS_DIR}/lamp.rhai") }, Door { open: false })).id();
    frame(&mut app);

    // **Addressed to another name, so nothing happens**: the recipient is read, not ignored.
    app.world_mut().write_message(ScriptEvent { name: "pressed".to_owned(), data: String::new(), to: "elsewhere".to_owned() });
    frame(&mut app);
    assert!(!door_is_open(&app, lamp), "an event addressed elsewhere is not this script's");

    app.world_mut().write_message(ScriptEvent { name: "pressed".to_owned(), data: String::new(), to: "lamp".to_owned() });
    frame(&mut app);
    assert!(door_is_open(&app, lamp), "an event addressed to this script is");

    let _ = std::fs::remove_dir_all(&root);
}
