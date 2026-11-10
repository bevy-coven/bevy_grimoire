//! **A game defines entities in YAML, and a save changes the running game** (GD-05, #367).
//!
//! `<root>/game/<name>.entities.yaml` is a list of named entities; each maps a component's short type path to
//! its value, and `children` are allowed. **Reflection is the only bridge**: a type the game registers
//! (`register_type::<T>()` with `#[reflect(Component)]` or `#[reflect(Resource)]`) is usable from the file, and
//! a type the game does not register is refused, naming the type. There is no second registry and no
//! hand-written mapping, so what a file may say is exactly what the game accepts.
//!
//! **"Registered" is the engine's whole registry, not only the game's own types.** `App::register_type` adds
//! Bevy's built-ins as plugins build, so a file can name `Window`, `Camera3d` or `ChildOf` — and a component
//! that needs `#[reflect(Deserialize)]` to be built from a value, or that is immutable, is refused or inserted
//! rather than applied (see [`apply_components`]). An entity file is therefore as powerful as a scene file, by
//! the engine's own rules rather than by a list this module keeps.
//!
//! ```yaml
//! entities:
//!   door_north:
//!     Health: { current: 10, max: 10 }
//!     Spin: { degrees_per_second: 45.0 }
//!     children:
//!       hinge:
//!         Spin: { degrees_per_second: 90.0 }
//! resources:
//!   Clock: { elapsed: 0.0 }
//! ```
//!
//! # What a save does
//!
//! - **A component's changed fields are applied in place**, so an entity keeps its `Entity` id and everything
//!   a game hung on it (an animation, a booking, a camera target).
//! - **A new entity spawns, and one removed from the file despawns.**
//! - **Nothing restarts**, and no other state is lost. A file that does not read keeps the last good state
//!   live: nothing is despawned from a document that was refused.
//!
//! **An entity's identity across reloads is its `name` in the file**, which is the map's key. Renaming one is a
//! despawn and a spawn, and the log says so rather than reporting a change nobody asked for.
//!
//! Map placements stay as they are: this is game-defined state beside them, not a second way to place pieces.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::ecs::reflect::{ReflectComponent, ReflectResource};
use bevy::prelude::*;
use bevy::reflect::{ReflectDeserialize, TypeRegistration, TypeRegistry};
use bevy_grimoire_core::live::FileWatch;

/// **The folder a game's entity files live in**, under the project root.
pub const ENTITIES_DIR: &str = "game";

/// **The file extension an entity file has**: `<name>.entities.yaml`.
pub const ENTITIES_EXT: &str = ".entities.yaml";

/// **The key a document's entity list sits under.**
pub const ENTITIES_KEY: &str = "entities";

/// **The key a document's resources sit under.**
pub const RESOURCES_KEY: &str = "resources";

/// **The key a document's children sit under**, inside an entity.
pub const CHILDREN_KEY: &str = "children";

/// **One thing wrong with a document**, in the shape GD-01's problems have so the editor shows it as any
/// other file's fault. **The line is named** because a person fixing a file goes to a line, not a path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    /// The file it was found in, as the caller named it.
    pub file: String,
    /// The line, counted from 1 as an editor shows it. `0` for the document's shape rather than a place in it.
    pub line: usize,
    /// The dotted path to what is wrong (`entities.door_north.Health.current`).
    pub at: String,
    /// What is wrong, in a sentence a person can act on.
    pub why: String,
}

impl Fault {
    /// The sentence the editor and the game log show.
    pub fn line(&self) -> String {
        format!("{}: line {}: {}: {}", self.file, self.line, self.at, self.why)
    }
}

/// **A component's or resource's value, as the file wrote it** — JSON, because a YAML value is JSON with tags.
pub type Value = serde_json::Value;

/// **One entity the file defines**: its components by type path, and its children by name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntityDef {
    /// The entity's own components, keyed by the path the file wrote (short or full).
    pub components: BTreeMap<String, Value>,
    /// Its children, by their own names.
    pub children: BTreeMap<String, EntityDef>,
}

/// **A file a game's entities are read from**, and the resources it sets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    /// The entities, by the name the file gives them — which is what identity across reloads means.
    pub entities: BTreeMap<String, EntityDef>,
    /// Resources, by type path.
    pub resources: BTreeMap<String, Value>,
    /// **The line each path stands on**, so a fault found while applying names a place in the file and not
    /// only a path: `entities.door_north.Mana` is what is wrong, and line 3 is where a person goes.
    pub lines: BTreeMap<String, usize>,
}

impl Document {
    /// **The line `at` stands on**, searching the nearest prefix when the fault is about a path the file does
    /// not have a node for (a type nobody registered has no node of its own; its key does).
    ///
    /// A path the file has nothing for answers `0`, which a fault renders as no line rather than a wrong one.
    fn line_of(&self, at: &str) -> usize {
        let mut probe = at.to_owned();
        loop {
            if let Some(line) = self.lines.get(&probe) {
                return *line;
            }
            match probe.rfind('.') {
                Some(dot) => probe.truncate(dot),
                None => return 0,
            }
        }
    }
}

/// **The entities a game's files made**, by file and then by the name in the file, so a reload can tell a
/// change from a new entity.
#[derive(Resource, Default, Debug)]
pub struct GameEntities {
    /// The entity each name stands for, per file — a child under `parent.children.name`.
    pub spawned: BTreeMap<String, BTreeMap<String, Entity>>,
    /// **The component paths each entity carried last time**, under `<file>:<name>`, so a save can tell a
    /// component the file dropped from one it never had.
    pub carried: BTreeMap<String, Vec<String>>,
}

/// **The line a core `Problem` names**, or `0` when its text has none: GD-01's refusal spells it
/// `line 4, column 3: …` inside `why`, so this reads the number back rather than a second reader inventing one.
fn problem_line(problem: &bevy_grimoire_core::problem::Problem) -> usize {
    let why = problem.why.clone().unwrap_or_default();
    why.split("line ").nth(1).and_then(|rest| rest.split([',', ':']).next()).and_then(|n| n.trim().parse().ok()).unwrap_or(0)
}

/// **A document read from its text**, refusing with the line rather than only the path.
#[allow(clippy::result_large_err)]
pub fn read_document(file: &str, text: &str) -> Result<Document, Fault> {
    // **Read through GD-01's one YAML reader**, not a second parser: the rules every project file is read by
    // (no duplicate key, no anchor, an alias refused, a tab that indents) apply to a game's files too, and a
    // tag a component's value carries becomes the one-key object a reflected enum is written as.
    let lines = bevy_grimoire_core::yaml::lines(text);
    let json = bevy_grimoire_core::yaml::to_json(text, file).map_err(|p| {
        let at = p.where_.as_ref().and_then(|w| w.field.clone()).unwrap_or_default();
        // **The line the reader itself named**, which its own refusal carries: a fault that told a person
        // `line 0` would be a path with no place to go.
        Fault { file: file.to_owned(), line: problem_line(&p), at, why: p.line() }
    })?;
    let root = json.as_object().ok_or_else(|| Fault {
        file: file.to_owned(),
        line: 0,
        at: String::new(),
        why: format!("a document is a mapping of `{ENTITIES_KEY}` and `{RESOURCES_KEY}`"),
    })?;
    let mut document = Document::default();
    for (key, value) in root {
        match key.as_str() {
            ENTITIES_KEY => document.entities = read_entities(file, "", value)?,
            RESOURCES_KEY => {
                let map = value.as_object().ok_or_else(|| Fault {
                    file: file.to_owned(),
                    line: 0,
                    at: RESOURCES_KEY.to_owned(),
                    why: "a resource's section is a mapping of a type path to its value".to_owned(),
                })?;
                document.resources = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            }
            other => {
                return Err(Fault {
                    file: file.to_owned(),
                    line: 0,
                    at: other.to_owned(),
                    why: format!("a document says `{ENTITIES_KEY}` and `{RESOURCES_KEY}` and nothing else"),
                });
            }
        }
    }
    document.lines = lines;
    Ok(document)
}

/// The entities of one level: the `entities` section, or a `children` mapping inside one.
#[allow(clippy::result_large_err)]
fn read_entities(file: &str, prefix: &str, value: &Value) -> Result<BTreeMap<String, EntityDef>, Fault> {
    let map = value.as_object().ok_or_else(|| Fault {
        file: file.to_owned(),
        line: 0,
        at: prefix.to_owned(),
        why: "entities are a mapping of a name to that entity's components".to_owned(),
    })?;
    let mut out = BTreeMap::new();
    for (name, body) in map {
        let at = if prefix.is_empty() { format!("{ENTITIES_KEY}.{name}") } else { format!("{prefix}.{name}") };
        let fields = body.as_object().ok_or_else(|| Fault {
            file: file.to_owned(),
            line: 0,
            at: at.clone(),
            why: "an entity is a mapping of a type path to its value".to_owned(),
        })?;
        let mut def = EntityDef::default();
        for (key, value) in fields {
            if key == CHILDREN_KEY {
                def.children = read_entities(file, &format!("{at}.{CHILDREN_KEY}"), value)?;
            } else {
                def.components.insert(key.clone(), value.clone());
            }
        }
        out.insert(name.clone(), def);
    }
    Ok(out)
}

/// **The registration a type path names**, refusing when the game did not register it or the short path is
/// ambiguous.
///
/// **Short paths resolve through the registry's own lookup**, so `Health` means the type whose short path is
/// `Health` and `my_game::Health` means exactly that one. An ambiguous short path is refused **listing the full
/// paths**, because "ambiguous" alone leaves a person to guess which of two types the file meant.
#[allow(clippy::result_large_err)]
fn registration<'a>(registry: &'a TypeRegistry, path: &str, file: &str, at: &str, document: &Document) -> Result<&'a TypeRegistration, Fault> {
    if registry.is_ambiguous(path) {
        // **The short path the registry itself compared**, which `TypePath` computes: slicing on `::` splits a
        // generic (`Foo<Bar>` has none), so the registry's own table is what the candidates are matched on.
        let mut candidates: Vec<&str> = registry
            .iter()
            .map(|r| r.type_info())
            .filter(|info| info.type_path().rsplit("::").next() == Some(path) || info.type_path() == path)
            .map(|info| info.type_path())
            .collect();
        candidates.sort_unstable();
        return Err(fault(
            file,
            document.line_of(at),
            at.to_owned(),
            format!("`{path}` names more than one registered type, so write the full path: {}", candidates.join(", ")),
        ));
    }
    registry.get_with_type_path(path).or_else(|| registry.get_with_short_type_path(path)).ok_or_else(|| {
        fault(
            file,
            document.line_of(at),
            at.to_owned(),
            format!(
                "`{path}` is not a type this game registered, so its files may not name it. Register it with \
                 `app.register_type::<{path}>()` (and `#[reflect(Component)]` or `#[reflect(Resource)]`) to use it"
            ),
        )
    })
}

/// **A reflected value built from the JSON a file wrote**, through the type's own `Deserialize`.
///
/// The registration is what decides how a value is built: a type with `#[reflect(Deserialize)]` reads its own
/// `Deserialize`, and one without is refused rather than guessed at. This is the same door Bevy's scene format
/// uses, which is why a type a scene can write is one a file can set.
#[allow(clippy::result_large_err)]
fn reflected(
    registration: &TypeRegistration,
    value: &Value,
    file: &str,
    at: &str,
    document: &Document,
) -> Result<Box<dyn bevy::reflect::Reflect>, Fault> {
    let line = document.line_of(at);
    let deserialize = registration.data::<ReflectDeserialize>().ok_or_else(|| {
        fault(
            file,
            line,
            at.to_owned(),
            format!(
                "`{}` is registered without `#[reflect(Deserialize)]`, so a value in a file cannot build it",
                registration.type_info().type_path()
            ),
        )
    })?;
    let text = serde_json::to_string(value).map_err(|e| fault(file, line, at.to_owned(), e.to_string()))?;
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    deserialize.deserialize(&mut deserializer).map_err(|e| fault(file, line, at.to_owned(), e.to_string()))
}

/// **What applying a document changed**, so a caller can say what a save did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// Entities the document made that were not there.
    pub spawned: Vec<String>,
    /// Entities the document changed in place, keeping their `Entity`.
    pub changed: Vec<String>,
    /// Entities the document no longer names, which were despawned.
    pub despawned: Vec<String>,
    /// Resources set.
    pub resources: Vec<String>,
}

/// **One component of one entity, checked and built**, before anything in the world changes.
struct Planned<'a> {
    /// The path in the file, for a refusal.
    at: String,
    /// The component's type path as the file spelled it.
    path: String,
    registration: &'a TypeRegistration,
    built: Box<dyn bevy::reflect::Reflect>,
}

/// **One entity's plan**: its own components, and each child's.
#[derive(Default)]
struct EntityPlan<'a> {
    components: Vec<Planned<'a>>,
    children: BTreeMap<String, EntityPlan<'a>>,
}

/// **A whole document, checked and built.**
struct Plan<'a> {
    entities: BTreeMap<String, EntityPlan<'a>>,
    resources: Vec<Planned<'a>>,
}

/// **Build every registration and every value a document names**, refusing at the first thing that is wrong.
#[allow(clippy::result_large_err)]
fn plan<'a>(registry: &'a TypeRegistry, file: &str, document: &Document) -> Result<Plan<'a>, Fault> {
    let mut entities = BTreeMap::new();
    for (name, def) in &document.entities {
        entities.insert(name.clone(), plan_entity(registry, file, &format!("{ENTITIES_KEY}.{name}"), def, document)?);
    }
    let mut resources = Vec::new();
    for (path, value) in &document.resources {
        let at = format!("{RESOURCES_KEY}.{path}");
        let registration = registration(registry, path, file, &at, document)?;
        registration.data::<ReflectResource>().ok_or_else(|| {
            fault(file, document.line_of(&at), at.clone(), format!("`{path}` is registered but not as a `Resource`, so `{RESOURCES_KEY}` cannot set it"))
        })?;
        resources.push(Planned {
            at: at.clone(),
            path: path.clone(),
            registration,
            built: reflected(registration, value, file, &at, document)?,
        });
    }
    Ok(Plan { entities, resources })
}

/// **One entity and its children, checked and built**, without touching the world.
#[allow(clippy::result_large_err)]
fn plan_entity<'a>(registry: &'a TypeRegistry, file: &str, at: &str, def: &EntityDef, document: &Document) -> Result<EntityPlan<'a>, Fault> {
    let mut components = Vec::new();
    for (path, value) in &def.components {
        let component_at = format!("{at}.{path}");
        let registration = registration(registry, path, file, &component_at, document)?;
        registration.data::<ReflectComponent>().ok_or_else(|| {
            fault(file, document.line_of(&component_at), component_at.clone(), format!("`{path}` is registered but not as a `Component`, so an entity cannot carry it"))
        })?;
        components.push(Planned {
            at: component_at.clone(),
            path: path.clone(),
            registration,
            built: reflected(registration, value, file, &component_at, document)?,
        });
    }
    let mut children = BTreeMap::new();
    for (name, child) in &def.children {
        children.insert(name.clone(), plan_entity(registry, file, &format!("{at}.{CHILDREN_KEY}.{name}"), child, document)?);
    }
    Ok(EntityPlan { components, children })
}

/// **Apply a document to the world**, through the registry and nothing else.
///
/// `key` is the name this file's entities are remembered under, so two files may define an entity of the same
/// name without colliding.
///
/// **Checked and built first, applied second** (M2): every registration and every value is resolved before one
/// entity is spawned or one component written, so a refusal leaves the world exactly as it was — no entity, no
/// changed field, and nothing for the next good save to duplicate.
#[allow(clippy::result_large_err)]
pub fn apply(world: &mut World, file: &str, key: &str, document: &Document) -> Result<Applied, Fault> {
    let registry_handle = world.resource::<AppTypeRegistry>().clone();
    let registry = registry_handle.read();
    let plan = plan(&registry, file, document)?;

    let mut applied = Applied::default();
    let previous = world
        .get_resource::<GameEntities>()
        .and_then(|held| held.spawned.get(key).cloned())
        .unwrap_or_default();
    let mut now: BTreeMap<String, Entity> = BTreeMap::new();
    let mut carried: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (name, entity_plan) in &plan.entities {
        // **The entity this name already stands for**, so a change is a change and not a respawn.
        let existing = previous.get(name).copied().filter(|e| world.get_entity(*e).is_ok());
        let entity = existing.unwrap_or_else(|| world.spawn_empty().id());
        let at = format!("{ENTITIES_KEY}.{name}");
        // **A component the file stopped naming comes off the entity**, which is what dropping it means.
        let drop_list: Vec<String> = world
            .get_resource::<GameEntities>()
            .and_then(|held| held.carried.get(&format!("{key}:{name}")).cloned())
            .unwrap_or_default();
        let mut changed = false;
        for path in &drop_list {
            if entity_plan.components.iter().any(|planned| &planned.path == path) {
                continue;
            }
            if let Ok(registration) = registration(&registry, path, file, &at, document)
                && let Some(some_reflect) = registration.data::<ReflectComponent>()
                && let Ok(mut entity_mut) = world.get_entity_mut(entity)
            {
                some_reflect.remove(&mut entity_mut);
                changed = true;
            }
        }
        changed |= apply_components(world, &registry, entity, &entity_plan.components)?;
        changed |= apply_children(world, &registry, entity, name, &entity_plan.children, &previous, &mut now)?;
        carried.insert(format!("{key}:{name}"), entity_plan.components.iter().map(|planned| planned.path.clone()).collect());
        // **Only what actually changed is reported** (NIT): a save nobody altered is not every entity changed.
        if existing.is_none() {
            applied.spawned.push(name.clone());
        } else if changed {
            applied.changed.push(name.clone());
        }
        now.insert(name.clone(), entity);
    }

    // **A name the file dropped despawns its entity**, and a child whose parent's list no longer has it.
    for (name, entity) in &previous {
        if !now.contains_key(name) {
            if let Ok(entity_mut) = world.get_entity_mut(*entity) {
                entity_mut.despawn();
            }
            applied.despawned.push(name.clone());
        }
    }

    for planned in &plan.resources {
        let component_id = world.components().get_id(planned.registration.type_id()).ok_or_else(|| {
            fault(
                file,
                document.line_of(&planned.at),
                planned.at.clone(),
                format!("`{}` is registered as a type but not as a component or resource this world can hold", planned.path),
            )
        })?;
        let value = planned.built.as_partial_reflect().to_dynamic().map_err(|e| {
            fault(file, document.line_of(&planned.at), planned.at.clone(), format!("the value did not clone: {e:?}"))
        })?;
        world.insert_reflect_resource(component_id, value);
        applied.resources.push(planned.path.clone());
    }

    let mut held = world.get_resource_or_insert_with(GameEntities::default);
    held.spawned.insert(key.to_owned(), now);
    for (entity_key, paths) in carried {
        held.carried.insert(entity_key, paths);
    }
    Ok(applied)
}

/// **Write one entity's planned components**, inserting what is absent and applying what is there, and answer
/// whether anything changed.
///
/// **An immutable component is inserted, never applied** (M4): `ReflectComponent::apply` panics on one, and
/// `ChildOf` — registered as a component, and immutable — is exactly the shape a file can name, so the
/// mutability is asked of the engine and a value that cannot be changed is replaced instead.
#[allow(clippy::result_large_err)]
fn apply_components(world: &mut World, registry: &TypeRegistry, entity: Entity, planned: &[Planned]) -> Result<bool, Fault> {
    let mut changed = false;
    for one in planned {
        let Some(some_reflect) = one.registration.data::<ReflectComponent>() else { continue };
        let mutable = world
            .components()
            .get_id(one.registration.type_id())
            .and_then(|id| world.components().get_info(id))
            .is_some_and(|info| info.mutable());
        let present = world.get_entity(entity).is_ok_and(|e| some_reflect.contains(e));
        if present && mutable {
            // **Compared before it is written**: a component that is already the file's value is not a change,
            // and reporting one would make every reload look like an edit.
            let same = world
                .get_entity(entity)
                .ok()
                .and_then(|e| some_reflect.reflect(e))
                .is_some_and(|now| now.reflect_partial_eq(one.built.as_partial_reflect()) == Some(true));
            if !same {
                some_reflect.apply(world.entity_mut(entity), one.built.as_partial_reflect());
                changed = true;
            }
        } else {
            some_reflect.insert(&mut world.entity_mut(entity), one.built.as_partial_reflect(), registry);
            changed = true;
        }
    }
    Ok(changed)
}

/// **Write one entity's planned children**, each tracked by `parent.children.name` so a child is made once and
/// a child the file drops is despawned.
#[allow(clippy::result_large_err)]
fn apply_children(
    world: &mut World,
    registry: &TypeRegistry,
    parent: Entity,
    parent_key: &str,
    children: &BTreeMap<String, EntityPlan>,
    previous: &BTreeMap<String, Entity>,
    now: &mut BTreeMap<String, Entity>,
) -> Result<bool, Fault> {
    let mut changed = false;
    for (name, plan) in children {
        let key = child_key(parent_key, name);
        let existing = previous.get(&key).copied().filter(|e| world.get_entity(*e).is_ok());
        let child = existing.unwrap_or_else(|| world.spawn_empty().id());
        changed |= existing.is_none();
        changed |= apply_components(world, registry, child, &plan.components)?;
        changed |= apply_children(world, registry, child, &key, &plan.children, previous, now)?;
        world.entity_mut(child).set_parent_in_place(parent);
        now.insert(key, child);
    }
    Ok(changed)
}

/// **The key a child's entity is remembered under**: its parent's key and the name in the file, so two parents
/// may each have a `hinge` and one parent's `hinge` is one entity across saves.
fn child_key(parent: &str, name: &str) -> String {
    format!("{parent}.{CHILDREN_KEY}.{name}")
}

/// **A fault at a line the document knows**, so every refusal names where to go.
fn fault(file: &str, line: usize, at: String, why: String) -> Fault {
    Fault { file: file.to_owned(), line, at, why }
}

/// **The path of every entity file under `root`**, sorted, so a game and a test read the same set.
pub fn files(root: &Path) -> Vec<PathBuf> {
    let dir = root.join(ENTITIES_DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(ENTITIES_EXT))
        .collect();
    out.sort();
    out
}

/// **Read one entity file** into a [`Document`].
#[allow(clippy::result_large_err)]
pub fn load(path: &Path) -> Result<Document, Fault> {
    let file = path.display().to_string();
    let text = std::fs::read_to_string(path).map_err(|e| Fault {
        file: file.clone(),
        line: 0,
        at: String::new(),
        why: e.to_string(),
    })?;
    read_document(&file, &text)
}

/// **A game's entity files, loaded and applied when one changes.**
///
/// `root` is the project root, the folder holding `game/`. The files are read at `Startup` and again when a
/// file's modification time moves, through the same [`apply`], so the first read and a reload are one path.
///
/// **A file that does not read keeps the last good state live**: nothing is despawned from a document that was
/// refused, and the fault is logged where a person and an agent both read it.
#[derive(Clone, Debug)]
pub struct GameEntitiesPlugin {
    /// The project root, the folder holding `game/`.
    pub root: PathBuf,
}

impl Plugin for GameEntitiesPlugin {
    fn build(&self, app: &mut App) {
        // **Stat BEFORE the first load** (`FileWatch::new` does), so a write between the two is a change the
        // first poll sees, and poll at the project's own half second with its settle time: a half-written save
        // is not read, and a run of writes is one reload.
        let watch = FileWatch::new(&self.root.join(ENTITIES_DIR), &["yaml"]);
        app.insert_resource(EntitiesRoot(self.root.clone()))
            .insert_resource(EntityFiles(watch))
            .init_resource::<GameEntities>()
            .add_systems(Startup, read_every_entity_file)
            .add_systems(Update, read_changed_entity_files);
    }
}

/// **The project root the entity files are read from.**
#[derive(Resource, Clone, Debug)]
pub struct EntitiesRoot(pub PathBuf);

/// **The watched folder**, through the same `FileWatch` the project files use (`bevy_grimoire_core::live`):
/// one poll interval, one settle time, hidden temp files skipped, and a file set that may grow or shrink.
#[derive(Resource)]
pub struct EntityFiles(pub FileWatch);

fn read_every_entity_file(mut commands: Commands, root: Option<Res<EntitiesRoot>>) {
    let Some(root) = root else { return };
    for path in files(&root.0) {
        commands.queue(ReadFile { path });
    }
}

/// **One file to read and apply**, queued so the work has `&mut World` and one file's refusal cannot stop
/// another's.
struct ReadFile {
    path: PathBuf,
}

impl Command for ReadFile {
    type Out = ();

    fn apply(self, world: &mut World) -> Self::Out {
        read_and_apply(world, &self.path);
    }
}

/// Read one file and apply it, reporting a refusal instead of stopping.
pub fn read_and_apply(world: &mut World, path: &Path) {
    let file = path.display().to_string();
    match load(path) {
        Ok(document) => match apply(world, &file, &file, &document) {
            Ok(applied) => {
                if !applied.spawned.is_empty() || !applied.changed.is_empty() || !applied.despawned.is_empty() {
                    info!(
                        "game entities: {file}: spawned {:?}, changed {:?}, despawned {:?}",
                        applied.spawned, applied.changed, applied.despawned
                    );
                }
            }
            Err(fault) => warn!("{}", fault.line()),
        },
        Err(fault) => warn!("{}", fault.line()),
    }
}

/// **Read and apply every file the watch says changed**, about a second after its last write, and despawn what
/// a file that has gone made.
fn read_changed_entity_files(
    mut commands: Commands,
    mut watch: ResMut<EntityFiles>,
    // **No clock, no polling**: `Time` is in every game `DefaultPlugins` builds, and a bare `App` in a test
    // has none — a missing clock must mean "nothing to time", not a system that panics every frame.
    time: Option<Res<Time>>,
    root: Option<Res<EntitiesRoot>>,
) {
    let (Some(root), Some(time)) = (root, time) else { return };
    if !watch.0.poll(time.elapsed_secs_f64()) {
        return;
    }
    for path in files(&root.0) {
        commands.queue(ReadFile { path });
    }
    // **A deleted file's entities are despawned**, which is what deleting it means: the file set is the
    // authority, so anything tracked for a file that is no longer there goes.
    commands.queue(ForgetMissingFiles { root: root.0.clone() });
}

/// **Forget the entities of a file that is no longer on disk**, despawning them.
struct ForgetMissingFiles {
    root: PathBuf,
}

impl Command for ForgetMissingFiles {
    type Out = ();

    fn apply(self, world: &mut World) -> Self::Out {
        let present: std::collections::BTreeSet<String> =
            files(&self.root).iter().map(|path| path.display().to_string()).collect();
        let gone: Vec<String> = world
            .get_resource::<GameEntities>()
            .map(|held| held.spawned.keys().filter(|key| !present.contains(*key)).cloned().collect())
            .unwrap_or_default();
        for key in gone {
            if let Some(by_name) = world.get_resource_mut::<GameEntities>().and_then(|mut held| held.spawned.remove(&key)) {
                for entity in by_name.values() {
                    if let Ok(entity_mut) = world.get_entity_mut(*entity) {
                        entity_mut.despawn();
                    }
                }
                info!("game entities: {key} is gone, so its entities were despawned");
            }
        }
    }
}
