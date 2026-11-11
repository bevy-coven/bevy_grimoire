//! **The five ways a script could take the game down, or act on the wrong entity** (the review of #368).
//!
//! Each test here was a defect in the first version, found by reading rather than by running, and each is written
//! so that the defect is what fails:
//!
//! - **M1**: `on_trigger` cast its argument with `Dynamic::cast`, which **panics** on a different type, so an event
//!   whose data is a number killed the game once a script defined the hook.
//! - **M2**: a component write called `ReflectComponent::apply`, which **panics** on an entity that does not have the
//!   component and on an immutable one, so `world_set("guard", "Door", …)` against a door-less guard crashed — and
//!   the docs claimed a write to an unknown property changed nothing.
//! - **M3**: `on_start` ran twice, because the attach never recorded the file's stamp and the first poll saw every
//!   script as changed.
//! - **M4**: a timer's event was broadcast, so two doors running one script both closed.
//! - **M5**: nothing bounded a string, an array or a map, so `let s = "x"; loop { s += s; }` ate gigabytes.
//!
//! Every test writes a real `.rhai` file into a scratch project and steps a headless world, as the crate's other
//! tests do.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use bevy_grimoire_rhai::{
    HOOK_START, SCRIPTS_DIR, SCRIPT_SETTLE_S, SCRIPT_STEP_LIMIT, Script, ScriptPlugin, ScriptProblem, Scripts,
};

/// **A component the test game registers**, mutable, so `this.<Name>` has something to write.
#[derive(Component, Reflect, Default, Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize)]
struct Health {
    current: i32,
    max: i32,
}

/// **A second mutable one**, for the write-through-another-entity tests.
#[derive(Component, Reflect, Default, Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize)]
struct Door {
    open: bool,
}

/// **An immutable component**, which `ReflectComponent::apply` panics on: the engine refuses to change one through
/// reflection, so a write at one has to be refused rather than crash.
#[derive(Component, Reflect, Default, Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[component(immutable)]
#[reflect(Component, Deserialize, Serialize)]
struct Stamped {
    value: i32,
}

fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(SCRIPTS_DIR)).expect("the script folder is made");
    dir
}

fn write_script(root: &Path, name: &str, text: &str) -> PathBuf {
    let path = root.join(SCRIPTS_DIR).join(format!("{name}.rhai"));
    std::fs::write(&path, text).expect("the script is written");
    path
}

fn game(root: &Path) -> App {
    let mut app = App::new();
    app.init_resource::<AppTypeRegistry>().init_resource::<Time>();
    app.register_type::<Health>().register_type::<Door>().register_type::<Stamped>();
    app.add_plugins(ScriptPlugin { root: root.to_path_buf() });
    app
}

fn frame(app: &mut App) {
    app.world_mut().resource_mut::<Time>().advance_by(Duration::from_millis(100));
    app.update();
    app.finish();
    app.cleanup();
}

fn frames(app: &mut App, count: usize) {
    for _ in 0..count {
        frame(app);
    }
}

/// **Every problem raised across a run of frames**, collected per frame because Bevy keeps a message for two.
fn problems_over(app: &mut App, count: usize) -> Vec<ScriptProblem> {
    let mut problems = Vec::new();
    for _ in 0..count {
        frame(app);
        problems.extend(app.world_mut().resource_mut::<Messages<ScriptProblem>>().drain());
    }
    problems
}

fn settle_frames() -> usize {
    ((SCRIPT_SETTLE_S / 0.1).ceil() as usize) + 4
}

fn health(app: &App, entity: Entity) -> Health {
    app.world().get::<Health>(entity).cloned().expect("the entity has a `Health`")
}

fn door_is_open(app: &App, entity: Entity) -> bool {
    app.world().get::<Door>(entity).cloned().expect("the entity has a `Door`").open
}

/// **M1: an event whose data is not a string does not kill a script that defines `on_trigger`.**
///
/// `emit("trigger", 5)` sends data a trigger hook's `other` is not: the first version cast it with
/// `Dynamic::cast::<String>()`, which panics. The game must keep running and the hook must see the data as text.
#[test]
fn a_trigger_hook_takes_data_that_is_not_a_string_without_panicking() {
    let root = project("rhai_review_trigger_data");
    write_script(
        &root,
        "watcher",
        "fn on_start() {\n    emit(\"trigger\", 5);\n    emit(\"trigger\");\n}\nfn on_trigger(other) {\n    this.state.seen = other;\n    this.Health.current = 1;\n}\n",
    );
    let mut app = game(&root);
    let entity = app.world_mut().spawn((Script { file: format!("{SCRIPTS_DIR}/watcher.rhai") }, Health { current: 10, max: 10 })).id();

    // **The frame returns**, which is the claim: a numeric event does not panic through a trigger hook.
    frames(&mut app, 3);
    assert_eq!(health(&app, entity).current, 1, "the trigger hook ran, so it was called with a number and with none");
    let seen = app.world().resource::<Scripts>().state_of(entity).and_then(|state| state.get("seen").cloned());
    assert!(seen.is_some(), "and `other` reached the script as a value: {seen:?}");

    let _ = std::fs::remove_dir_all(&root);
}

/// **M2a: `world_set` on a component the target does not have is refused, not a panic**, and the refusal is a
/// problem naming the file.
///
/// The docs promised "a write to an unknown property changes nothing"; the first version panicked, because
/// `ReflectComponent::apply` panics when the component is absent.
#[test]
fn a_write_to_a_component_the_target_does_not_have_is_a_problem_not_a_panic() {
    let root = project("rhai_review_write_absent");
    write_script(&root, "writer", "fn on_update(dt) {\n    world_set(\"guard\", \"Door\", #{ open: true });\n}\n");
    let mut app = game(&root);
    // **The guard has no `Door`**: the shape the old code panicked on.
    let guard = app.world_mut().spawn((Name::new("guard"), Health { current: 5, max: 5 })).id();
    app.world_mut().spawn((Script { file: format!("{SCRIPTS_DIR}/writer.rhai") }, Health { current: 1, max: 1 }));

    let problems = problems_over(&mut app, 2);
    assert_eq!(problems.len(), 1, "the refused write is reported once, not once a frame: {problems:?}");
    let line = problems[0].line();
    assert!(line.contains("writer.rhai"), "the problem names the script: {line}");
    assert!(line.contains("Door"), "and the component it could not write: {line}");
    assert!(app.world().get::<Door>(guard).is_none(), "and no `Door` was invented on the guard");

    let _ = std::fs::remove_dir_all(&root);
}

/// **M2b: a write at an immutable component is refused with a problem**, rather than panicking inside the engine.
#[test]
fn a_write_at_an_immutable_component_is_a_problem() {
    let root = project("rhai_review_write_immutable");
    write_script(&root, "stamp", "fn on_update(dt) {\n    this.Stamped.value = 2;\n}\n");
    let mut app = game(&root);
    let entity = app.world_mut().spawn((Script { file: format!("{SCRIPTS_DIR}/stamp.rhai") }, Stamped { value: 1 })).id();

    let problems = problems_over(&mut app, 2);
    assert!(!problems.is_empty(), "the immutable write is refused with a problem");
    assert_eq!(app.world().get::<Stamped>(entity).map(|s| s.value), Some(1), "and the value is untouched");

    let _ = std::fs::remove_dir_all(&root);
}

/// **M2c: `this.<Component>` a script invents changes nothing, and is refused once by name.**
///
/// The first version's docs promised silence and its code raised rhai's raw `No writable property 'count' … a
/// setter is not registered for type '()'` — every frame, in a shape a person cannot act on. **Rhai cannot make a
/// write into `()` a no-op**, so the honest single path is a refusal: nothing on the entity changes, and one
/// `ScriptProblem` names the script and the component, once per edit rather than once per frame.
#[test]
fn a_write_to_a_component_the_scripted_entity_does_not_have_changes_nothing_and_is_named_once() {
    let root = project("rhai_review_write_unknown_this");
    write_script(&root, "invent", "fn on_update(dt) {\n    this.Nowhere.count = 3;\n}\n");
    let mut app = game(&root);
    let entity = app.world_mut().spawn((Script { file: format!("{SCRIPTS_DIR}/invent.rhai") }, Health { current: 7, max: 7 })).id();

    let problems = problems_over(&mut app, 5);
    assert_eq!(problems.len(), 1, "one fault, reported once rather than once a frame: {problems:?}");
    let line = problems[0].line();
    assert!(line.contains("invent.rhai"), "the problem names the script: {line}");
    assert_eq!(health(&app, entity).current, 7, "and nothing on the entity changed");

    let _ = std::fs::remove_dir_all(&root);
}

/// **M3: `on_start` runs exactly once**, over frames well past the first poll.
///
/// The first version recorded no stamp when it attached a script, so the first poll counted every file as changed,
/// recompiled it and reset `started` — the hook ran twice and a counter a script reset in it reset twice.
#[test]
fn on_start_runs_once_over_several_frames_and_a_missing_file_is_reported_once() {
    let root = project("rhai_review_start_once");
    write_script(&root, "once", "fn on_start() {\n    this.state.starts = (this.state.starts ?? 0) + 1;\n}\nfn on_update(dt) {\n    this.Health.current += 0;\n}\n");
    let mut app = game(&root);
    let entity = app.world_mut().spawn((Script { file: format!("{SCRIPTS_DIR}/once.rhai") }, Health { current: 1, max: 1 })).id();
    // **A second script, on a file nobody wrote**: its problem is one event, reported once rather than every poll.
    let missing = app.world_mut().spawn(Script { file: format!("{SCRIPTS_DIR}/absent.rhai") }).id();

    let problems = problems_over(&mut app, settle_frames() + 2);
    let starts = app
        .world()
        .resource::<Scripts>()
        .state_of(entity)
        .and_then(|state| state.get("starts").cloned())
        .and_then(|value| value.as_int().ok());
    assert_eq!(starts, Some(1), "`on_start` ran exactly once");
    let absent = problems.iter().filter(|problem| problem.line().contains("absent.rhai")).count();
    assert_eq!(absent, 1, "a missing script is reported once, not once a poll: {problems:?}");
    assert!(app.world().get::<Script>(missing).is_some(), "and the entity is still there to fix");

    let _ = std::fs::remove_dir_all(&root);
}

/// **M4: a timer's event goes to the entity that started it**, so two doors running one script do not both close.
///
/// The first version wrote `to: String::new()`, which is "every script", so a timer one door started closed both.
#[test]
fn a_timer_fires_for_the_entity_that_started_it_only() {
    let root = project("rhai_review_timer_recipient");
    write_script(
        &root,
        "door",
        "fn on_start() {\n    if this.Door.open { timer(\"close\", 0.3); }\n}\nfn on_event(name, data) {\n    if name == \"timer\" && data == \"close\" { this.Door.open = false; }\n}\n",
    );
    let mut app = game(&root);
    let path = format!("{SCRIPTS_DIR}/door.rhai");
    // **Only the first door is open at its start**, so only it starts a timer. The second is shut then, and opened
    // before the timer rings: a broadcast would shut it, and an addressed event leaves it open.
    let timer_owner = app.world_mut().spawn((Name::new("owner"), Script { file: path.clone() }, Door { open: true })).id();
    let bystander = app.world_mut().spawn((Name::new("bystander"), Script { file: path }, Door { open: false })).id();
    frames(&mut app, 2);
    app.world_mut().get_mut::<Door>(bystander).expect("a door").open = true;
    frames(&mut app, 8);

    assert!(!door_is_open(&app, timer_owner), "the door that started the timer closed");
    assert!(door_is_open(&app, bystander), "and the other door, which started none, is still open");

    let _ = std::fs::remove_dir_all(&root);
}

/// **M5: a string that grows without limit is stopped**, with a problem naming the file, rather than eating memory.
///
/// `let s = "x"; loop { s += s; }` doubles a string every iteration; without a cap it takes gigabytes in
/// milliseconds. The step limit does not catch it, because the operations are few and the values are huge.
#[test]
fn a_string_that_grows_without_limit_is_stopped() {
    let root = project("rhai_review_string_growth");
    write_script(&root, "grow", "fn on_update(dt) {\n    let s = \"xxxxxxxxxxxxxxxx\";\n    loop { s += s; }\n}\n");
    let mut app = game(&root);
    app.world_mut().spawn((Script { file: format!("{SCRIPTS_DIR}/grow.rhai") }, Health { current: 1, max: 1 }));

    let problems = problems_over(&mut app, 2);
    assert!(!problems.is_empty(), "the runaway string was stopped and reported");
    let line = problems[0].line();
    assert!(line.contains("grow.rhai"), "the problem names the file: {line}");
    // **The game is alive**: the world still steps, which is what "stopped" has to mean.
    frame(&mut app);
    let _ = SCRIPT_STEP_LIMIT;

    let _ = std::fs::remove_dir_all(&root);
}

/// **S7: removing `Script` stops its script**, and a replaced `Script` is the new file's.
///
/// The first version kept `Scripts.running` forever: a despawned entity's script was still recompiled when its file
/// was saved, and a `Script` whose file changed reached nothing, because `Added` does not fire on a replace.
#[test]
fn removing_a_script_stops_it_and_replacing_one_loads_the_new_file() {
    let root = project("rhai_review_lifecycle");
    write_script(&root, "one", "fn on_update(dt) {\n    this.Health.current = 1;\n}\n");
    write_script(&root, "two", "fn on_update(dt) {\n    this.Health.current = 2;\n}\n");
    let mut app = game(&root);
    let entity = app.world_mut().spawn((Script { file: format!("{SCRIPTS_DIR}/one.rhai") }, Health { current: 0, max: 0 })).id();
    frames(&mut app, 2);
    assert_eq!(health(&app, entity).current, 1, "the first script ran");

    // **A replace is the second script**, which `Added` does not see.
    app.world_mut().entity_mut(entity).insert(Script { file: format!("{SCRIPTS_DIR}/two.rhai") });
    frames(&mut app, 2);
    assert_eq!(health(&app, entity).current, 2, "the replaced script's file is the one running");

    // **A removal stops it**, so the component stops moving.
    app.world_mut().entity_mut(entity).remove::<Script>();
    app.world_mut().entity_mut(entity).insert(Health { current: 9, max: 9 });
    frames(&mut app, 3);
    assert_eq!(health(&app, entity).current, 9, "nothing runs on the entity any more");
    assert!(app.world().resource::<Scripts>().state_of(entity).is_none(), "and the script's state is gone");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A script's `import` resolves nothing**, so it cannot read a file the game did not give it.
#[test]
fn an_import_resolves_nothing() {
    let root = project("rhai_review_import");
    write_script(&root, "importer", "fn on_update(dt) {\n    import \"other\" as other;\n}\n");
    let mut app = game(&root);
    app.world_mut().spawn((Script { file: format!("{SCRIPTS_DIR}/importer.rhai") }, Health { current: 1, max: 1 }));
    let problems = problems_over(&mut app, 2);
    assert!(!problems.is_empty(), "the import was refused: {problems:?}");
    assert!(problems[0].line().contains("importer.rhai"), "and named: {}", problems[0].line());
    let _ = std::fs::remove_dir_all(&root);
}

/// **`HOOK_START` is the name the plugin reads**, stated so a rename of the constant is caught by a test rather than
/// only by a script that stops starting.
#[test]
fn the_hook_names_are_the_ones_a_script_writes() {
    assert_eq!(HOOK_START, "on_start");
    assert_eq!(SCRIPTS_DIR, "scripts");
}
