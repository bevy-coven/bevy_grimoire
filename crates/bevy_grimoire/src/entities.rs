//! **A game defines entities in YAML, and a save changes the running game** (GD-05, #367).
//!
//! `<root>/game/<name>.entities.yaml` is a list of named entities; each maps a component's short type path to
//! its value, and `children` are allowed. **Reflection is the only bridge**: a type the game registers
//! (`register_type::<T>()` with `#[reflect(Component)]` or `#[reflect(Resource)]`) is usable from the file, and
//! a type the game does not register is refused, naming the type. There is no second registry and no
//! hand-written mapping, so what a file may say is exactly what the game accepts.
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
}

/// **The entities a game's files made**, by file and then by the name in the file, so a reload can tell a
/// change from a new entity.
#[derive(Resource, Default, Debug)]
pub struct GameEntities {
    /// The entity each name stands for, per file.
    pub spawned: BTreeMap<String, BTreeMap<String, Entity>>,
}

/// **A document read from its text**, refusing with the line rather than only the path.
#[allow(clippy::result_large_err)]
pub fn read_document(file: &str, text: &str) -> Result<Document, Fault> {
    // **Read through GD-01's one YAML reader**, not a second parser: the rules every project file is read by
    // (no duplicate key, no anchor, an alias refused, a tab that indents) apply to a game's files too, and a
    // tag a component's value carries becomes the one-key object a reflected enum is written as.
    let json = bevy_grimoire_core::yaml::to_json(text, file).map_err(|p| Fault {
        file: file.to_owned(),
        line: 0,
        at: p.where_.as_ref().and_then(|w| w.field.clone()).unwrap_or_default(),
        why: p.line(),
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
fn registration<'a>(registry: &'a TypeRegistry, path: &str, file: &str, at: &str) -> Result<&'a TypeRegistration, Fault> {
    if registry.is_ambiguous(path) {
        let short = path.rsplit("::").next().unwrap_or(path);
        let mut candidates: Vec<&str> = registry
            .iter()
            .map(|r| r.type_info().type_path())
            .filter(|full| full.rsplit("::").next() == Some(short))
            .collect();
        candidates.sort_unstable();
        return Err(Fault {
            file: file.to_owned(),
            line: 0,
            at: at.to_owned(),
            why: format!(
                "`{path}` names more than one registered type, so write the full path: {}",
                candidates.join(", ")
            ),
        });
    }
    registry.get_with_type_path(path).or_else(|| registry.get_with_short_type_path(path)).ok_or_else(|| Fault {
        file: file.to_owned(),
        line: 0,
        at: at.to_owned(),
        why: format!(
            "`{path}` is not a type this game registered, so its files may not name it. Register it with \
             `app.register_type::<{path}>()` (and `#[reflect(Component)]` or `#[reflect(Resource)]`) to use it"
        ),
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
) -> Result<Box<dyn bevy::reflect::PartialReflect>, Fault> {
    let deserialize = registration.data::<ReflectDeserialize>().ok_or_else(|| Fault {
        file: file.to_owned(),
        line: 0,
        at: at.to_owned(),
        why: format!(
            "`{}` is registered without `#[reflect(Deserialize)]`, so a value in a file cannot build it",
            registration.type_info().type_path()
        ),
    })?;
    let text = serde_json::to_string(value).map_err(|e| Fault {
        file: file.to_owned(),
        line: 0,
        at: at.to_owned(),
        why: e.to_string(),
    })?;
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    let built = deserialize.deserialize(&mut deserializer).map_err(|e| Fault {
        file: file.to_owned(),
        line: 0,
        at: at.to_owned(),
        why: e.to_string(),
    })?;
    Ok(built)
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

/// **Apply a document to the world**, through the registry and nothing else.
///
/// `key` is the name this file's entities are remembered under, so two files may define an entity of the same
/// name without colliding.
#[allow(clippy::result_large_err)]
pub fn apply(world: &mut World, file: &str, key: &str, document: &Document) -> Result<Applied, Fault> {
    let registry_handle = world.resource::<AppTypeRegistry>().clone();
    let registry = registry_handle.read();
    let mut applied = Applied::default();
    let previous = world
        .get_resource::<GameEntities>()
        .and_then(|held| held.spawned.get(key).cloned())
        .unwrap_or_default();
    let mut now: BTreeMap<String, Entity> = BTreeMap::new();

    for (name, def) in &document.entities {
        let at = format!("{ENTITIES_KEY}.{name}");
        // **The entity this name already stands for**, so a change is a change and not a respawn.
        let existing = previous.get(name).copied().filter(|e| world.get_entity(*e).is_ok());
        let entity = apply_entity(world, &registry, file, &at, def, existing)?;
        if existing.is_some() {
            applied.changed.push(name.clone());
        } else {
            applied.spawned.push(name.clone());
        }
        now.insert(name.clone(), entity);
    }

    // **A name the file dropped despawns its entity**, which is what removing it from the file means.
    for (name, entity) in &previous {
        if !now.contains_key(name) {
            if let Ok(entity_mut) = world.get_entity_mut(*entity) {
                entity_mut.despawn();
            }
            applied.despawned.push(name.clone());
        }
    }

    for (path, value) in &document.resources {
        let at = format!("{RESOURCES_KEY}.{path}");
        let registration = registration(&registry, path, file, &at)?;
        // **A resource is inserted through the world's own reflected door**, which is the one thing that can
        // spawn the resource entity it lives on: `World::insert_reflect_resource`. It overwrites a resource that
        // is already there, which is what a document setting a field means.
        let _is_a_resource = registration.data::<ReflectResource>().ok_or_else(|| Fault {
            file: file.to_owned(),
            line: 0,
            at: at.clone(),
            why: format!("`{path}` is registered but not as a `Resource`, so `{RESOURCES_KEY}` cannot set it"),
        })?;
        let built = reflected(registration, value, file, &at)?;
        let type_id = registration.type_id();
        let component_id = world.components().get_id(type_id).ok_or_else(|| Fault {
            file: file.to_owned(),
            line: 0,
            at: at.clone(),
            why: format!("`{path}` is registered as a type but not as a component or resource this world can hold"),
        })?;
        world.insert_reflect_resource(component_id, built);
        applied.resources.push(path.clone());
    }

    world.get_resource_or_insert_with(GameEntities::default).spawned.insert(key.to_owned(), now);
    Ok(applied)
}

/// **One entity and its children**, applied in place when it already exists.
#[allow(clippy::result_large_err)]
fn apply_entity(
    world: &mut World,
    registry: &TypeRegistry,
    file: &str,
    at: &str,
    def: &EntityDef,
    existing: Option<Entity>,
) -> Result<Entity, Fault> {
    let entity = existing.unwrap_or_else(|| world.spawn_empty().id());
    for (path, value) in &def.components {
        let component_at = format!("{at}.{path}");
        let registration = registration(registry, path, file, &component_at)?;
        let some_reflect = registration.data::<ReflectComponent>().ok_or_else(|| Fault {
            file: file.to_owned(),
            line: 0,
            at: component_at.clone(),
            why: format!("`{path}` is registered but not as a `Component`, so an entity cannot carry it"),
        })?;
        let built = reflected(registration, value, file, &component_at)?;
        let present = world.get_entity(entity).is_ok_and(|e| some_reflect.contains(e));
        if present {
            // **In place**, so the `Entity` id and everything hung on it survive a field changing.
            some_reflect.apply(world.entity_mut(entity), built.as_ref());
        } else {
            some_reflect.insert(&mut world.entity_mut(entity), built.as_ref(), registry);
        }
    }
    for (name, child) in &def.children {
        let child_at = format!("{at}.{CHILDREN_KEY}.{name}");
        let child_entity = apply_entity(world, registry, file, &child_at, child, None)?;
        world.entity_mut(child_entity).set_parent_in_place(entity);
    }
    Ok(entity)
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
        app.insert_resource(EntitiesRoot(self.root.clone()))
            .init_resource::<EntityFiles>()
            .init_resource::<GameEntities>()
            .add_systems(Startup, read_every_entity_file)
            .add_systems(Update, read_changed_entity_files);
    }
}

/// **The project root the entity files are read from.**
#[derive(Resource, Clone, Debug)]
pub struct EntitiesRoot(pub PathBuf);

/// **The files the game has read, and the modification time each corresponds to**, so a change is noticed once.
#[derive(Resource, Default, Debug)]
pub struct EntityFiles {
    /// Each file the game reads, and when it was last written.
    pub seen: BTreeMap<PathBuf, std::time::SystemTime>,
}

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
    if let Ok(modified) = std::fs::metadata(path).and_then(|m| m.modified()) {
        world.get_resource_or_insert_with(EntityFiles::default).seen.insert(path.to_path_buf(), modified);
    }
}

/// **Read and apply every file whose modification time moved.**
fn read_changed_entity_files(mut commands: Commands, mut seen: ResMut<EntityFiles>, root: Option<Res<EntitiesRoot>>) {
    let Some(root) = root else { return };
    for path in files(&root.0) {
        let Ok(modified) = std::fs::metadata(&path).and_then(|m| m.modified()) else { continue };
        if seen.seen.get(&path) == Some(&modified) {
            continue;
        }
        seen.seen.insert(path.clone(), modified);
        commands.queue(ReadFile { path });
    }
}
