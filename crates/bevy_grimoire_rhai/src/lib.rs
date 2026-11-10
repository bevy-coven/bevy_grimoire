//! **Rhai scripts on entities, reloaded live** (GD-06, #368).
//!
//! A [`Script`] is a component naming a `.rhai` file. The file defines four hooks, and this crate calls the ones
//! it finds:
//!
//! ```rhai
//! // scripts/door.rhai
//! fn on_start()             { this.state.open = false; }
//! fn on_update(dt)          { this.Health.current -= 0; this.state.frames += 1; }
//! fn on_event(name, data)   { if name == "pressed" { this.Door.open = true; } }
//! fn on_trigger(other)      { print(other); }
//! ```
//!
//! # What a script can do
//!
//! - **Read and write the entity's reflected components by name**: `this.Health.current -= 1`. The map is built
//!   from the components the game registered that this entity actually carries, so a script can touch exactly
//!   what the game registered and nothing else — the same set GD-04's `game.schema.json` documents.
//! - **Find another entity by its `Name`**: `find("guard")` answers that entity's components as a value, and
//!   `world_set("guard", "Health", data)` writes one back. **It is two calls rather than one handle** because of
//!   a measurement, not a taste: rhai writes through a `Scope` table and through `this`, but **a write to a map
//!   a native function returned lands on a copy** (measured against rhai 1.27), so `find("g").Health.current -= 1`
//!   would silently change nothing. Two calls that both work beat one that looks better and loses the write.
//! - **Send and receive events**: `emit("door_open")` writes a [`ScriptEvent`] every scripted entity may see,
//!   and `on_event(name, data)` is called for each whose `to` is this entity or empty.
//! - **Start a timer**: `timer("slow", 2.5)` makes `on_event("timer", "slow")` fire once, `2.5` s later.
//!
//! # State survives a reload
//!
//! `this.state` is a map this crate owns, per scripted entity. **Saving the script recompiles it and carries the
//! map across**, so a counter keeps counting. That is the whole of "a save changes the running game" for a
//! script: the code changes, the state stays.
//!
//! # A bad script never takes the game down
//!
//! - **A file that does not compile leaves the old version running** and raises a [`ScriptProblem`] naming the
//!   file and the line — GD-01's [`Problem`] shape, so a game reports it where it reports any other fault.
//! - **A call that runs past [`SCRIPT_STEP_LIMIT`] operations is stopped**, with a problem naming the file. A
//!   runaway script costs one call and never freezes the game.
//! - **A runtime error becomes a problem naming the file and the line** of the statement that raised it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_grimoire_core::problem::{Detail, Location, Problem};
use rhai::{Dynamic, Engine, Map, Scope};

/// **The folder a game's scripts live in**, under the project root.
pub const SCRIPTS_DIR: &str = "scripts";

/// **The extension a script file has.**
pub const SCRIPTS_EXT: &str = "rhai";

/// **How many rhai operations one hook call may run** before it is stopped (GD-06's `SCRIPT_STEP_LIMIT`).
///
/// A named constant rather than a number in the engine's setup, so a game that wants a bigger budget for one
/// heavy script can say so and a reader can find the limit by its name. Rhai counts every statement, operator
/// and function call as one operation.
pub const SCRIPT_STEP_LIMIT: u64 = 200_000;

/// **The hook a script defines to run once, when it loads and after every reload.**
pub const HOOK_START: &str = "on_start";

/// **The hook a script defines to run every frame, with the frame's `dt` in seconds.**
pub const HOOK_UPDATE: &str = "on_update";

/// **The hook a script defines to receive an event, with its name and its data.**
pub const HOOK_EVENT: &str = "on_event";

/// **The hook a script defines to be told another entity came near it.**
pub const HOOK_TRIGGER: &str = "on_trigger";

/// **The event name a timer fires under.**
pub const TIMER_EVENT: &str = "timer";

/// **The event name a near miss is reported under**, which is what makes `on_trigger(other)` run.
///
/// A trigger volume is `bevy_grimoire::behaviours`'s `Triggered`, and that message is the behaviours' business, not
/// this crate's: a game that wants a script told about a crossing writes one [`ScriptEvent`] named this, with the
/// other entity's `Name` as its data. One vocabulary for both hooks — an event is an event — rather than a second
/// way to reach a script.
pub const TRIGGER_EVENT: &str = "trigger";

/// **How often a script file is stat'd**, in seconds.
pub const SCRIPT_POLL_S: f64 = 0.1;

/// **How long a script file must stay as it is before it is read**, in seconds: long enough that an editor's
/// write-then-rename lands as one save rather than a truncated compile.
pub const SCRIPT_SETTLE_S: f64 = 0.1;

/// **The one component a script is put on an entity by.**
///
/// ```yaml
/// entities:
///   door:
///     Script: { file: "scripts/door.rhai" }
/// ```
///
/// The path is relative to the project root, so `scripts/door.rhai` is `<root>/scripts/door.rhai`. **A file that
/// is not there is a problem naming the path**, not a silently idle script: a typo in a path and a script whose
/// hooks all do nothing look identical from outside otherwise.
#[derive(Component, Reflect, Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Script {
    /// The `.rhai` file, relative to the project root.
    pub file: String,
}

/// **An event a script sent, or one a game sent to a script.**
///
/// A script writes one with `emit(name)`, which every scripted entity may see. A game writes one with a `to` to
/// name a single recipient; an empty `to` goes to every script.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct ScriptEvent {
    /// The name the receiver compares against.
    pub name: String,
    /// What rode along, as JSON text; empty when nothing did.
    pub data: String,
    /// The `Name` of the one entity it is for, or empty for every scripted entity.
    pub to: String,
}

/// **Something a script did wrong, in the shape every other fault in this project has** (GD-01).
///
/// A game that shows a project file's refusal in its log shows this one the same way: the file, the line, why.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct ScriptProblem {
    /// The problem, with the file and the line in its location.
    pub problem: Problem,
}

impl ScriptProblem {
    /// The sentence a game logs.
    pub fn line(&self) -> String {
        self.problem.line()
    }
}

/// **Marks an entity this crate attached a script to**, so a script's `find` sees exactly the entities the
/// scripts run on.
#[derive(Component, Debug, Clone, Copy)]
pub struct Scripted;

/// **A timer a script started**: the name it will fire under, and how long it has left.
#[derive(Clone, Debug, PartialEq)]
struct Pending {
    /// The event name it fires as.
    name: String,
    /// Seconds still to run.
    remaining_s: f32,
}

/// **Which of the four hooks a compiled script defines**, read once when it compiles rather than searched for on
/// every frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Hooks {
    start: bool,
    update: bool,
    event: bool,
    trigger: bool,
}

impl Hooks {
    fn of(ast: &rhai::AST) -> Hooks {
        let mut hooks = Hooks::default();
        for function in ast.iter_functions() {
            match function.name {
                HOOK_START => hooks.start = true,
                HOOK_UPDATE => hooks.update = true,
                HOOK_EVENT => hooks.event = true,
                HOOK_TRIGGER => hooks.trigger = true,
                _ => {}
            }
        }
        hooks
    }
}

/// **One scripted entity's live state**: its compiled script, the map it keeps, and its timers.
struct Running {
    /// The file it was compiled from.
    file: String,
    /// The compiled script, or `None` when its last load was refused and nothing has ever run.
    ast: Option<rhai::AST>,
    /// **The map a script keeps across a reload**: `this.state`.
    state: Map,
    /// The timers it started, in the order it started them.
    timers: Vec<Pending>,
    /// Whether `on_start` has run for the script as it stands now.
    started: bool,
    /// Which hooks this file defines.
    hooks: Hooks,
}

/// **Every scripted entity's running state**, keyed by the entity.
#[derive(Resource, Default)]
pub struct Scripts {
    running: BTreeMap<Entity, Running>,
}

impl Scripts {
    /// **The `this.state` map of one entity**, so a game or a test can read what a script kept. `None` for an
    /// entity with no script.
    pub fn state_of(&self, entity: Entity) -> Option<&Map> {
        self.running.get(&entity).map(|running| &running.state)
    }

    /// **How many entities are running a script whose last load succeeded**, which is what a game's status line
    /// reports.
    pub fn compiled(&self) -> usize {
        self.running.values().filter(|running| running.ast.is_some()).count()
    }

    /// **The entities with a script**, in entity order.
    pub fn scripted(&self) -> Vec<Entity> {
        self.running.keys().copied().collect()
    }

    /// **Whether `on_start` has run** for one entity's script as it stands now.
    fn started(&self, entity: Entity) -> bool {
        self.running.get(&entity).is_some_and(|one| one.started)
    }

    /// **Mark one entity's setup as done**, which is what lets its other hooks run.
    fn mark_started(&mut self, entity: Entity) {
        if let Some(one) = self.running.get_mut(&entity) {
            one.started = true;
        }
    }

    /// **Add the timers a hook started**, in the order it started them.
    fn push_timers(&mut self, entity: Entity, timers: Vec<(String, f32)>) {
        if let Some(one) = self.running.get_mut(&entity) {
            for (name, seconds) in timers {
                one.timers.push(Pending { name, remaining_s: seconds });
            }
        }
    }
}

/// **The scripts, their hooks, and the watcher that recompiles them when they are saved.**
///
/// `root` is the project root, the folder holding `scripts/`.
///
/// # What it needs, and does not add
///
/// **`Time`**, for `on_update(dt)`, for the timers and for the file watcher. A game without one runs none of
/// them rather than panicking: the same rule `bevy_grimoire::entities` states for its own reload. Every game
/// `DefaultPlugins` builds has one.
#[derive(Clone, Debug)]
pub struct ScriptPlugin {
    /// The project root, the folder holding `scripts/`.
    pub root: PathBuf,
}

impl Plugin for ScriptPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Script>()
            .insert_resource(ScriptsRoot(self.root.clone()))
            .init_resource::<ScriptStats>()
            .init_resource::<Scripts>()
            .init_resource::<ScriptEngine>()
            .add_message::<ScriptEvent>()
            .add_message::<ScriptProblem>()
            // **`First`**, so a recompile lands before the frame's hooks run: a save that is compiled this frame
            // is the code this frame executes.
            .add_systems(First, reload_changed_scripts)
            // **`PostUpdate`**, after the world has moved: a hook that reads a `Transform` sees this frame's.
            .add_systems(PostUpdate, (attach_scripts, settle_timers, run_hooks).chain());
    }
}

/// **The project root the scripts are read from.**
#[derive(Resource, Clone, Debug)]
pub struct ScriptsRoot(pub PathBuf);

/// **The one rhai engine**, with the functions a script may call on it.
///
/// **One engine, not one per script**: an `Engine` carries the registered function set, which *is* the script
/// API. Building it per entity would give two entities two APIs that could drift.
#[derive(Resource)]
pub struct ScriptEngine {
    engine: Engine,
}

impl Default for ScriptEngine {
    fn default() -> Self {
        ScriptEngine { engine: host() }
    }
}

impl ScriptEngine {
    /// **The engine**, for a game that wants to register a function of its own on the same host rather than
    /// building a second one.
    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }
}

/// **The host**: rhai's engine with this crate's functions on it and the budget set.
///
/// **An unknown map property reads as `()`, and that is right here.** A script writes `this.Health.current`, and
/// the map carries `Health` only when the entity has it. With rhai's default, reading an unknown property is an
/// error, so guarding with `if this.Switch != ()` would raise on every entity without one — the natural spelling
/// of "if I have a switch" would be a refusal. **A write to an unknown property still changes nothing**, because
/// the map is rebuilt from the entity's real components and only components the script changed are written back.
fn host() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_operations(SCRIPT_STEP_LIMIT);
    engine.set_fail_on_invalid_map_property(false);
    register_script_functions(&mut engine);
    engine
}

/// **The functions a script may call.**
///
/// **They answer from the call's tag, not from a `&mut World`**, because a rhai native function is not given
/// one. The tag is the [`Outbox`]: a snapshot of every scripted entity's components, plus the lists a script's
/// own calls append to. `find` and `world_get` read the snapshot; `world_set`, `emit` and `timer` record, and the
/// call applies what was recorded once the hook returns. That is why `find("guard").Health.current -= 1` is not
/// the API: a write through a returned map lands on a copy, measured, while `world_set` is one recorded write
/// that lands on the entity.
fn register_script_functions(engine: &mut Engine) {
    engine.register_type_with_name::<Outbox>("Outbox");

    // **`find(name)`**: every component the named entity carries, or `()` when nothing has that name.
    engine.register_fn("find", |context: rhai::NativeCallContext, name: String| -> Dynamic {
        match outbox_of(&context) {
            Some(outbox) => outbox.entity(&name),
            None => Dynamic::UNIT,
        }
    });

    // **`world_get(name, component)`**, for the one field a script wants rather than a whole entity.
    engine.register_fn("world_get", |context: rhai::NativeCallContext, name: String, component: String| -> Dynamic {
        match outbox_of(&context) {
            Some(outbox) => {
                let entity = outbox.entity(&name);
                match entity.read_lock::<Map>().and_then(|map| map.get(component.as_str()).cloned()) {
                    Some(value) => value,
                    None => Dynamic::UNIT,
                }
            }
            None => Dynamic::UNIT,
        }
    });

    // **`world_set(name, component, value)`**: write one component of another entity, applied after the hook.
    engine.register_fn("world_set", |context: rhai::NativeCallContext, name: String, component: String, value: Dynamic| {
        if let Some(outbox) = outbox_of(&context) {
            outbox.set(&name, &component, &value);
        }
    });

    // **`emit(name)`** and **`emit(name, data)`**, which every scripted entity may see.
    engine.register_fn("emit", |context: rhai::NativeCallContext, name: String| {
        if let Some(outbox) = outbox_of(&context) {
            outbox.emit(&name, "");
        }
    });
    engine.register_fn("emit", |context: rhai::NativeCallContext, name: String, data: Dynamic| {
        if let Some(outbox) = outbox_of(&context) {
            outbox.emit(&name, &dynamic_to_json(&data).to_string());
        }
    });

    // **`timer(name, seconds)`**: an event called `name`, fired once after `seconds`.
    engine.register_fn("timer", |context: rhai::NativeCallContext, name: String, seconds: f64| {
        if let Some(outbox) = outbox_of(&context) {
            outbox.timer(&name, seconds);
        }
    });
}

/// **The call's [`Outbox`]**, taken out of the tag.
fn outbox_of(context: &rhai::NativeCallContext) -> Option<Outbox> {
    context.tag().cloned()?.try_cast::<Outbox>()
}

/// **Give every entity with a [`Script`] and no running state its state.**
fn attach_scripts(
    mut commands: Commands,
    scripts: Query<(Entity, &Script), Added<Script>>,
    root: Option<Res<ScriptsRoot>>,
    mut running: ResMut<Scripts>,
    mut problems: MessageWriter<ScriptProblem>,
) {
    let Some(root) = root else { return };
    for (entity, script) in &scripts {
        let path = root.0.join(&script.file);
        let (ast, hooks) = match compile(&path) {
            Ok(ast) => {
                let hooks = Hooks::of(&ast);
                (Some(ast), hooks)
            }
            Err(problem) => {
                problems.write(ScriptProblem { problem });
                (None, Hooks::default())
            }
        };
        // **A refused script still gets an entry**, so a later save of the same path reaches it through the same
        // place rather than through a second path.
        running.running.insert(
            entity,
            Running { file: script.file.clone(), ast, state: Map::new(), timers: Vec::new(), started: false, hooks },
        );
        commands.entity(entity).insert(Scripted);
    }
}

/// **Compile one script file**, or say where it is wrong.
// `Problem` is the workspace's one refusal shape; boxing a copy of it here would make this crate's answer a
// different type from the editor's, which is the whole point of sharing it.
#[allow(clippy::result_large_err)]
fn compile(path: &Path) -> Result<rhai::AST, Problem> {
    let file = path.display().to_string();
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            return Err(Problem::with(
                "a script could not be read".to_owned(),
                Detail::default().at(Location::file(file)).because(e.to_string()),
            ));
        }
    };
    // **The same host the run uses**, so a compile that passed here cannot fail on a missing function later.
    host().compile(&text).map_err(|e| script_problem(&file, e.position().line(), e.to_string()))
}

/// **A problem naming the file and the line**, in GD-01's shape.
fn script_problem(file: &str, line: Option<usize>, why: String) -> Problem {
    let location = match line {
        Some(line) => Location::file(file).line(line),
        None => Location::file(file),
    };
    Problem::with(format!("the script `{file}` did not run"), Detail::default().at(location).because(why))
}

/// **Recompile every script whose file changed on disk**, carrying `this.state` across.
///
/// **A refused save changes nothing**: the entity keeps the script it was running, because a half-typed edit is
/// not a reason to stop a door opening. The problem names the file and the line, and the next good save
/// recompiles again.
///
/// **The files are stat'd here rather than through `FileWatch`**, which answers only *whether* something in its
/// directory moved: a recompile needs *which*, or one script's save is every script's recompile. The poll and
/// settle rule is the same one: a file is read once its modification time has stopped moving for
/// [`SCRIPT_SETTLE_S`].
fn reload_changed_scripts(
    mut running: ResMut<Scripts>,
    mut stat: ResMut<ScriptStats>,
    mut problems: MessageWriter<ScriptProblem>,
    root: Option<Res<ScriptsRoot>>,
    time: Option<Res<Time>>,
) {
    let (Some(root), Some(time)) = (root, time) else { return };
    let now = time.elapsed_secs_f64();
    if stat.last_poll.is_some_and(|last| now - last < SCRIPT_POLL_S) {
        return;
    }
    stat.last_poll = Some(now);

    // **Which files moved since the last poll**, so only those scripts are recompiled.
    let present: Vec<PathBuf> = running
        .running
        .values()
        .map(|one| root.0.join(&one.file))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut moved: Vec<PathBuf> = Vec::new();
    for path in &present {
        let stamp = std::fs::metadata(path).and_then(|meta| meta.modified()).ok();
        if stat.seen.get(path).copied() != Some(stamp) {
            moved.push(path.clone());
            stat.stable.insert(path.clone(), now);
        }
        stat.seen.insert(path.clone(), stamp);
    }
    // **Read what has stopped moving**, which is the files waiting from an earlier poll as well as this one: a
    // save is noticed on one poll and settles on a later one, so a loop over only this poll's movers would leave
    // every save waiting for a second edit that never comes.
    let ready: Vec<PathBuf> = stat
        .stable
        .iter()
        .filter(|(_, since)| now - *since >= SCRIPT_SETTLE_S)
        .map(|(path, _)| path.clone())
        .collect();
    for path in &ready {
        stat.stable.remove(path);
    }
    if ready.is_empty() {
        return;
    }

    for one in running.running.values_mut() {
        let path = root.0.join(&one.file);
        if !ready.iter().any(|ready| ready == &path) {
            continue;
        }
        match compile(&path) {
            Ok(ast) => {
                one.hooks = Hooks::of(&ast);
                one.ast = Some(ast);
                // **`started` goes back to false**, so the new code's `on_start` runs: a save that adds a setup
                // line expects it to have run once the save lands.
                one.started = false;
                info!("the script `{}` recompiled; its state is carried across", path.display());
            }
            // **The old AST stays**, and the problem names the file and its line.
            Err(problem) => {
                problems.write(ScriptProblem { problem });
            }
        }
    }
}

/// **When each script file was last seen with which modification time**, and when it stopped moving — the state
/// a recompile needs and a bare `FileWatch` does not keep.
#[derive(Resource, Default)]
pub struct ScriptStats {
    /// The modification time each file had at the last poll.
    seen: BTreeMap<PathBuf, Option<std::time::SystemTime>>,
    /// When each moved file last changed, so it is read once it has stopped.
    stable: BTreeMap<PathBuf, f64>,
    /// The last poll, so a script file is not stat'd more often than [`SCRIPT_POLL_S`].
    last_poll: Option<f64>,
}

/// **Advance every running timer** and fire the ones that reach zero as a [`ScriptEvent`].
fn settle_timers(mut running: ResMut<Scripts>, time: Option<Res<Time>>, mut events: MessageWriter<ScriptEvent>) {
    let Some(time) = time else { return };
    let delta_s = time.delta_secs();
    let mut fired: Vec<String> = Vec::new();
    for one in running.running.values_mut() {
        for timer in &mut one.timers {
            timer.remaining_s -= delta_s;
            if timer.remaining_s <= 0.0 {
                fired.push(timer.name.clone());
            }
        }
        one.timers.retain(|timer| timer.remaining_s > 0.0);
    }
    for name in fired {
        events.write(ScriptEvent { name: TIMER_EVENT.to_owned(), data: name, to: String::new() });
    }
}

/// **Run every scripted entity's hooks for this frame**: `on_start` once, then `on_update(dt)`.
fn run_hooks(world: &mut World) {
    let delta_s = world.get_resource::<Time>().map(|time| time.delta_secs()).unwrap_or(0.0);
    let entities: Vec<Entity> = world.get_resource::<Scripts>().map(Scripts::scripted).unwrap_or_default();
    for entity in entities {
        // **`on_start` first, always**: a script's frame code assumes what its setup did.
        run_one(world, entity, HOOK_START, Hook::Start, vec![]);
        run_one(world, entity, HOOK_UPDATE, Hook::Update, vec![Dynamic::from_float(f64::from(delta_s))]);
    }
    run_events(world);
}

/// **Which hook a call is for**, so its arity is the compiler's business and not a `match` on a string.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Hook {
    Start,
    Update,
    Event,
    Trigger,
}

impl Hook {
    fn defined(self, hooks: &Hooks) -> bool {
        match self {
            Hook::Start => hooks.start,
            Hook::Update => hooks.update,
            Hook::Event => hooks.event,
            Hook::Trigger => hooks.trigger,
        }
    }
}

/// **Call one hook of one entity when its script defines it and its setup has run.**
///
/// **`on_start` counts as done whether or not a script defines it**, which is what makes a script with only
/// `on_update` run at all: a hook that does not exist has nothing to wait for, so waiting for it would idle the
/// script forever.
fn run_one(world: &mut World, entity: Entity, name: &str, which: Hook, args: Vec<Dynamic>) {
    // **`on_start` runs once**, so a script that resets a counter there does not reset it every frame. `started`
    // is the flag, and it is read here rather than in `ready_hook` because a start that has already happened is
    // not a hook waiting to be called — it is a hook that is done.
    if which == Hook::Start && world.get_resource::<Scripts>().is_some_and(|scripts| scripts.started(entity)) {
        return;
    }
    match ready_hook(world, entity, which) {
        Some(ast) => call(world, entity, name, which, ast, args),
        // **The start is spent rather than skipped**: a script without `on_start` has nothing to set up, so its
        // frame code is ready on its first frame.
        None if which == Hook::Start => {}
        None => return,
    }
    if which == Hook::Start
        && let Some(mut scripts) = world.get_resource_mut::<Scripts>()
    {
        scripts.mark_started(entity);
    }
}

/// **Run `on_event(name, data)` for every script that should see this frame's events.**
fn run_events(world: &mut World) {
    // **Every event of this frame, taken once**, so a script that sends one during its own hook cannot make the
    // loop run forever: what it sends this frame is read on the next.
    let events: Vec<ScriptEvent> =
        world.get_resource_mut::<Messages<ScriptEvent>>().map(|mut messages| messages.drain().collect()).unwrap_or_default();
    if events.is_empty() {
        return;
    }
    for entity in world.get_resource::<Scripts>().map(Scripts::scripted).unwrap_or_default() {
        // **No name is not no recipient**: a broadcast goes to every script, so an entity that was never given a
        // `Name` still hears it. Only an addressed event needs one, and an entity without a name cannot be its
        // addressee — which is the honest answer rather than dropping the event for everybody else.
        let name = name_of(world, entity).unwrap_or_default();
        for event in &events {
            // **Addressed or broadcast**: an empty `to` goes to every script, a named one to its own.
            if !event.to.is_empty() && event.to != name {
                continue;
            }
            // **`on_trigger(other)` reads the same event**, for the one name that means a near miss, so a script
            // declares the hook its job actually is. A script that defines both gets both, which is what writing
            // both means.
            if event.name == TRIGGER_EVENT
                && let Some(ast) = ready_hook(world, entity, Hook::Trigger)
            {
                call(world, entity, HOOK_TRIGGER, Hook::Trigger, ast, vec![json_to_dynamic_str(&event.data)]);
            }
            let Some(ast) = ready_hook(world, entity, Hook::Event) else { continue };
            let data = json_to_dynamic_str(&event.data);
            call(world, entity, HOOK_EVENT, Hook::Event, ast, vec![Dynamic::from(event.name.clone()), data]);
        }
    }
}

/// **The compiled script of one entity, when it defines `which` and is ready to be called.**
fn ready_hook(world: &World, entity: Entity, which: Hook) -> Option<rhai::AST> {
    let running = world.get_resource::<Scripts>()?;
    let one = running.running.get(&entity)?;
    if !which.defined(&one.hooks) {
        return None;
    }
    // **Every hook but `on_start` waits for it**, so a script's setup is not skipped because its first frame ran
    // before the load did. A script with no `on_start` is marked started by `run_one` on its first frame.
    if which != Hook::Start && !one.started {
        return None;
    }
    one.ast.clone()
}

/// **What a script asked the world while its hook ran.** A rhai function has no `&mut World`, so a script asks
/// through the call's tag and the code beside it acts once the hook returns.
///
/// The snapshot is every scripted entity's components by `Name`, which is what `find` and `world_get` answer
/// from; the three lists are what `world_set`, `emit` and `timer` record. **A clone of this struct shares the
/// lists**, because the tag holds one copy and the caller holds the other.
#[derive(Clone, Default)]
struct Outbox {
    /// Every scripted entity's components, by its name.
    entities: Arc<Mutex<BTreeMap<String, Map>>>,
    /// Components a script asked to write, as `(entity name, component, value)`.
    writes: Arc<Mutex<Vec<(String, String, Dynamic)>>>,
    /// Events a script asked to send, as `(name, data)`.
    events: Arc<Mutex<Vec<(String, String)>>>,
    /// Timers a script started, as `(name, seconds)`.
    timers: Arc<Mutex<Vec<(String, f32)>>>,
}

impl Outbox {
    /// **Build the snapshot** a hook's `find` answers from.
    fn with_entities(entities: BTreeMap<String, Map>) -> Outbox {
        Outbox { entities: Arc::new(Mutex::new(entities)), ..Outbox::default() }
    }

    /// **One entity's components**, or `()` when nothing is called `name`.
    fn entity(&self, name: &str) -> Dynamic {
        let entities = match self.entities.lock() {
            Ok(entities) => entities,
            Err(_) => return Dynamic::UNIT,
        };
        match entities.get(name) {
            Some(map) => Dynamic::from_map(map.clone()),
            None => Dynamic::UNIT,
        }
    }

    fn set(&self, name: &str, component: &str, value: &Dynamic) {
        if let Ok(mut writes) = self.writes.lock() {
            writes.push((name.to_owned(), component.to_owned(), value.clone()));
        }
    }

    fn emit(&self, name: &str, data: &str) {
        if let Ok(mut events) = self.events.lock() {
            events.push((name.to_owned(), data.to_owned()));
        }
    }

    fn timer(&self, name: &str, seconds: f64) {
        if let Ok(mut timers) = self.timers.lock() {
            timers.push((name.to_owned(), seconds as f32));
        }
    }
}

/// **Call one hook of one script**, binding `this` to a map built from the entity's real components plus its
/// kept state, then writing back what the script changed.
fn call(world: &mut World, entity: Entity, name: &str, which: Hook, ast: rhai::AST, args: Vec<Dynamic>) {
    let Some(map) = this_map(world, entity) else { return };
    let before = map.clone();
    // **`this` is a `Dynamic` holding the map**, because that is what rhai binds: a write is `this.Health.current`,
    // which lands in the map inside it, and the map is read back out once the call returns.
    let mut then = Dynamic::from_map(map);
    let file = world.get_resource::<Scripts>().and_then(|s| s.running.get(&entity)).map(|one| one.file.clone()).unwrap_or_default();
    // **Every scripted entity's picture**, which is what `find` answers from: the same map `this` is, under each
    // entity's `Name`. Built once per call rather than once per `find`, because a script that looks up five
    // entities should cost one walk of the world.
    let outbox = Outbox::with_entities(snapshot(world));

    let mut scope = Scope::new();
    let options = {
        let mut options = rhai::CallFnOptions::new();
        options.tag = Some(Dynamic::from(outbox.clone()));
        options.bind_this_ptr(&mut then)
    };
    // **The engine is borrowed for the call and not moved out**, because an `Engine` owns its function set and
    // is not `Clone`: a copy per call would be the second host `AGENTS.md` forbids.
    let result = world.resource_scope(|world, engine: Mut<ScriptEngine>| {
        let _ = world;
        with_args(&engine.engine, &mut scope, options, &ast, name, which, args)
    });

    // **The world is written back after the call**, so the hook ran against one consistent picture of it.
    match result {
        Ok(()) => {}
        Err(error) => {
            let problem = script_problem(&file, error.position().line(), error.to_string());
            if let Some(mut messages) = world.get_resource_mut::<Messages<ScriptProblem>>() {
                messages.write(ScriptProblem { problem });
            }
        }
    }
    // **What the script left `this` as**, which is the map after its writes rather than the one it began from.
    let after = then.clone().try_cast::<Map>().unwrap_or_default();
    write_back(world, entity, &before, &after);
    keep_state(world, entity, &after);

    // **The writes a script asked for**, each one applied to the entity its name resolves to.
    let writes = outbox.writes.lock().map(|writes| writes.clone()).unwrap_or_default();
    for (target, component, value) in writes {
        let Some(target) = entity_named(world, &target) else { continue };
        set_component(world, target, &component, &value);
    }

    let timers = outbox.timers.lock().map(|t| t.clone()).unwrap_or_default();
    if !timers.is_empty()
        && let Some(mut scripts) = world.get_resource_mut::<Scripts>()
    {
        scripts.push_timers(entity, timers);
    }
    let events = outbox.events.lock().map(|e| e.clone()).unwrap_or_default();
    if !events.is_empty()
        && let Some(mut messages) = world.get_resource_mut::<Messages<ScriptEvent>>()
    {
        for (event_name, data) in events {
            messages.write(ScriptEvent { name: event_name, data, to: String::new() });
        }
    }
}

/// **Every named entity's components, by its `Name`**, which is what a script's `find` answers from.
///
/// **Every named entity, not only the scripted ones**: `find("gate")` is a script reaching a door the game made,
/// which is the common case — a door does not need a script to be found by one.
fn snapshot(world: &mut World) -> BTreeMap<String, Map> {
    let mut query = world.query::<(Entity, &Name)>();
    let named: Vec<(Entity, String)> = query.iter(world).map(|(entity, name)| (entity, name.as_str().to_owned())).collect();
    let mut out = BTreeMap::new();
    for (entity, name) in named {
        if let Some(map) = components_of(world, entity) {
            out.insert(name, map);
        }
    }
    out
}

/// **The entity a script's `find` names**, through the engine's own `Name` and nothing else.
fn entity_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut query = world.query::<(Entity, &Name)>();
    query.iter(world).find(|(_, candidate)| candidate.as_str() == name).map(|(entity, _)| entity)
}

/// **Write one component of one entity**, through the type's own `Deserialize` — the door GD-05's file reader
/// uses, so a script and a YAML file cannot disagree about what a component's data is.
fn set_component(world: &mut World, entity: Entity, component: &str, value: &Dynamic) {
    let Some(registry_handle) = world.get_resource::<AppTypeRegistry>().cloned() else { return };
    let registry = registry_handle.read();
    let Some(registration) = registry.get_with_short_type_path(component).or_else(|| registry.get_with_type_path(component))
    else {
        return;
    };
    // **Cloned out of the registry**, which is `Clone`, so the registry's read borrow ends before the world is
    // written: a refusal here is no write at all rather than a half-applied one.
    let Some(reflect_component) = registration.data::<ReflectComponent>().cloned() else { return };
    let Some(deserialize) = registration.data::<bevy::reflect::ReflectDeserialize>() else { return };
    let text = serde_json::to_string(&dynamic_to_json(value)).unwrap_or_default();
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    let Ok(built) = deserialize.deserialize(&mut deserializer) else { return };
    drop(registry);
    let Ok(mut entity_mut) = world.get_entity_mut(entity) else { return };
    reflect_component.apply(&mut entity_mut, built.as_partial_reflect());
}

/// **Call a hook with the arguments its signature takes**, which is the one place the four arities live.
fn with_args(
    engine: &Engine,
    scope: &mut Scope,
    options: rhai::CallFnOptions,
    ast: &rhai::AST,
    name: &str,
    which: Hook,
    args: Vec<Dynamic>,
) -> Result<(), Box<rhai::EvalAltResult>> {
    let mut args = args.into_iter();
    match which {
        Hook::Start => engine.call_fn_with_options::<()>(options, scope, ast, name, ()),
        Hook::Update => {
            let dt = args.next().and_then(|v| v.as_float().ok()).unwrap_or(0.0);
            engine.call_fn_with_options::<()>(options, scope, ast, name, (dt,))
        }
        Hook::Event => {
            let event = args.next().map(|v| v.cast::<String>()).unwrap_or_default();
            let data = args.next().unwrap_or(Dynamic::UNIT);
            engine.call_fn_with_options::<()>(options, scope, ast, name, (event, data))
        }
        Hook::Trigger => {
            let other = args.next().map(|v| v.cast::<String>()).unwrap_or_default();
            engine.call_fn_with_options::<()>(options, scope, ast, name, (other,))
        }
    }
}

/// **A script's `this` map**: each component the entity carries, by its short type path, plus `state`.
///
/// **Built from the registry and the entity's real components**, so what a script may touch is exactly what the
/// game registered and this entity carries — the property GD-04's schema relies on. A component whose value
/// cannot be written as data is left out rather than guessed at.
fn this_map(world: &World, entity: Entity) -> Option<Map> {
    let mut map = components_of(world, entity)?;
    // **`state` is this crate's own**, and the one part a reload carries across.
    let state = world.get_resource::<Scripts>().and_then(|s| s.running.get(&entity)).map(|one| one.state.clone()).unwrap_or_default();
    map.insert("state".into(), Dynamic::from_map(state));
    Some(map)
}

/// **The components one entity carries, by short type path**, without `state`: what `find` answers with, and
/// what `this` is built from.
fn components_of(world: &World, entity: Entity) -> Option<Map> {
    let registry = world.get_resource::<AppTypeRegistry>()?.clone();
    let registry = registry.read();
    let mut map = Map::new();
    let ids: Vec<Option<std::any::TypeId>> = world.inspect_entity(entity).ok()?.map(|(_, info)| info.type_id()).collect();
    for type_id in ids.into_iter().flatten() {
        let Some(registration) = registry.get(type_id) else { continue };
        let Some(reflect_component) = registration.data::<ReflectComponent>() else { continue };
        let Some(value) = world.get_entity(entity).ok().and_then(|e| reflect_component.reflect(e)) else { continue };
        let Ok(json) = serde_json::to_value(bevy::reflect::serde::TypedReflectSerializer::new(value, &registry)) else { continue };
        map.insert(registration.type_info().type_path_table().short_path().into(), json_to_dynamic(&json));
    }
    Some(map)
}

/// **Write `this` back to the entity**: every component whose entry the script changed, by name.
fn write_back(world: &mut World, entity: Entity, before: &Map, after: &Map) {
    // **Compared as the JSON they would be written as**, because `Dynamic` has no `PartialEq`: two maps are the
    // same component when they serialise the same, which is also what the write would send.
    let changed: Vec<(String, Dynamic)> = after
        .iter()
        .filter(|(key, value)| {
            key.as_str() != "state" && before.get(*key).map(dynamic_to_json) != Some(dynamic_to_json(value))
        })
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect();
    for (component, value) in changed {
        set_component(world, entity, &component, &value);
    }
}

/// **Keep what a script left in `this.state`**, so the next frame and a reload both see it.
fn keep_state(world: &mut World, entity: Entity, then: &Map) {
    let Some(state) = then.get("state").and_then(|value| value.read_lock::<Map>()).map(|state| state.clone()) else {
        return;
    };
    if let Some(mut scripts) = world.get_resource_mut::<Scripts>()
        && let Some(one) = scripts.running.get_mut(&entity)
    {
        one.state = state;
    }
}

/// **The `Name` of an entity**, which is how a script names another one and how an addressed event finds its
/// recipient.
fn name_of(world: &World, entity: Entity) -> Option<String> {
    world.get::<Name>(entity).map(|name| name.as_str().to_owned())
}

/// **A float as JSON**, or `null` for one JSON cannot hold (`NaN`, an infinity).
fn f64_json(f: f64) -> serde_json::Value {
    serde_json::Number::from_f64(f).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null)
}

/// **A JSON value as a rhai value**, so a reflected component crosses into a script as data rather than as a
/// string it would have to parse.
fn json_to_dynamic(value: &serde_json::Value) -> Dynamic {
    match value {
        serde_json::Value::Null => Dynamic::UNIT,
        serde_json::Value::Bool(b) => Dynamic::from_bool(*b),
        // **An integer stays one**, because `Health.current` is an `i32` in Rust and a float in the script would
        // make `-= 1` a type change on the way back through the type's own `Deserialize`.
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(i) => Dynamic::from_int(i),
            None => Dynamic::from_float(n.as_f64().unwrap_or(0.0)),
        },
        serde_json::Value::String(s) => Dynamic::from(s.clone()),
        serde_json::Value::Array(items) => Dynamic::from_array(items.iter().map(json_to_dynamic).collect()),
        serde_json::Value::Object(fields) => {
            let mut map = Map::new();
            for (key, value) in fields {
                map.insert(key.as_str().into(), json_to_dynamic(value));
            }
            Dynamic::from_map(map)
        }
    }
}

/// **A script's `data` argument as a value**: whatever its JSON says, or the string itself when it is not JSON.
fn json_to_dynamic_str(text: &str) -> Dynamic {
    if text.is_empty() {
        return Dynamic::UNIT;
    }
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => json_to_dynamic(&value),
        Err(_) => Dynamic::from(text.to_owned()),
    }
}

/// **A rhai value as JSON**, so it can go through the type's own `Deserialize` on the way back.
fn dynamic_to_json(value: &Dynamic) -> serde_json::Value {
    if value.is_unit() {
        return serde_json::Value::Null;
    }
    if let Ok(b) = value.as_bool() {
        return serde_json::Value::Bool(b);
    }
    // **The integer check comes first**, because rhai's `Dynamic::from(1_i32)` and `from_int(1)` both answer
    // `as_int`, and a `f32` field written from an integer must not become `1.0`'s text with a fraction.
    // **`as_int` answers only rhai's own `INT`**, so a value that came in as an `i32` (which is what a reflected
    // `Health.current` is) is asked for by its own type. Both are integers to JSON.
    if let Ok(i) = value.as_int() {
        return serde_json::Value::from(i);
    }
    if let Some(i) = value.clone().try_cast::<i32>() {
        return serde_json::Value::from(i);
    }
    if let Ok(f) = value.as_float() {
        return f64_json(f);
    }
    if let Some(f) = value.clone().try_cast::<f32>() {
        return f64_json(f64::from(f));
    }
    if let Some(s) = value.clone().try_cast::<String>() {
        return serde_json::Value::String(s);
    }
    if value.is_array() {
        let items: rhai::Array = value.clone().cast();
        return serde_json::Value::Array(items.iter().map(dynamic_to_json).collect());
    }
    if let Some(map) = value.read_lock::<Map>() {
        let mut out = serde_json::Map::new();
        for (key, item) in map.iter() {
            out.insert(key.to_string(), dynamic_to_json(item));
        }
        return serde_json::Value::Object(out);
    }
    serde_json::Value::Null
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_json_number_crosses_into_a_script_as_the_integer_it_was() {
        assert_eq!(json_to_dynamic(&serde_json::json!(3)).as_int(), Ok(3));
        assert_eq!(json_to_dynamic(&serde_json::json!(2.5)).as_float(), Ok(2.5));
    }

    #[test]
    fn a_scripts_integer_comes_back_as_a_json_integer() {
        assert_eq!(dynamic_to_json(&Dynamic::from_int(7)), serde_json::json!(7));
        assert_eq!(dynamic_to_json(&Dynamic::from(7_i32)), serde_json::json!(7));
        assert_eq!(dynamic_to_json(&Dynamic::from_bool(true)), serde_json::json!(true));
    }

    #[test]
    fn the_hooks_a_script_defines_are_read_once() {
        let ast = host().compile("fn on_start() {}\nfn on_event(n, d) {}\n").expect("it compiles");
        let hooks = Hooks::of(&ast);
        assert!(hooks.start && hooks.event, "{hooks:?}");
        assert!(!hooks.update && !hooks.trigger, "{hooks:?}");
    }
}
