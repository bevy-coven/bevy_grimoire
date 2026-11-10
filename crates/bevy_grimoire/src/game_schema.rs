//! **A game's own reflected types, written where an outside editor looks** (GD-04, #366).
//!
//! `<root>/.dark_factory/schema/game.schema.json` describes what the game's own files accept, and only the
//! game's registry knows that. So the file is **Bevy's own answer**, not a second description of the same
//! types: [`bevy_remote::builtin_methods::export_registry_types`] is the function behind the
//! `registry.schema` remote method, and this calls it directly over the running `World`. That means the file
//! lists exactly the types the game registered, with their fields and their reflect data.
//!
//! # Why `bevy_remote` is a dependency, and why that adds no server
//!
//! The schema export needs `bevy_remote`, and `bevy_remote`'s default features include an HTTP transport. A
//! game that ships must not gain a listening socket for one file written at start. `bevy_internal` takes it
//! with `default-features = false, features = ["bevy_asset", "bevy_render"]` — no `http`, no port, no
//! runtime — so this crate does the same and gets only the pure functions. `RemotePlugin` is not added here,
//! so the game binds nothing.
//!
//! # What it writes
//!
//! One entry per registered type path, each carrying the type's reflect information and which of them are
//! `Component`, `Resource`, `Event` and so on — the shape `registry.schema` answers. A type registered lazily
//! after the first frame is in the file on the next start, because the registry is the game's and this crate
//! cannot see when it changes.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_remote::builtin_methods::export_registry_types;
use bevy_remote::schemas::SchemaTypesMetadata;

/// **The document a game's registry answers**, as the JSON text `registry.schema` produces.
///
/// **The metadata resource is inserted if the game has none.** `export_registry_types` reads
/// `SchemaTypesMetadata` to name each type's reflect data, and only `RemotePlugin` inserts it, so a game that
/// runs no remote would panic there. Bevy's own defaults say what `Component`, `Resource` and the rest are, so
/// inserting them is the same answer the remote method gives.
///
/// **Infallible to ask**: the export is a pure function over the `World`, so the only failure is the write.
pub fn registry_schema(world: &mut World) -> String {
    if world.get_resource::<SchemaTypesMetadata>().is_none() {
        world.insert_resource(SchemaTypesMetadata::default());
    }
    // The request that asks for every type, unfiltered: an empty set of filters means all crates, all types.
    let answer = export_registry_types(In(None), world).unwrap_or_else(|e| {
        // The export cannot fail on a world with a registry; a world without one has nothing to say, and an
        // empty document is the honest answer rather than a refusal that stops a game starting.
        warn!("the game's registry answered no schema: {e:?}");
        serde_json::Value::Object(Default::default())
    });
    serde_json::to_string(&answer).unwrap_or_else(|e| {
        warn!("the game's schema did not serialise: {e}");
        "{}".to_owned()
    })
}

/// **Write a game's schema into the project at `root` right now**, from the running world's registry.
///
/// The same write [`GameSchemaPlugin`] makes at start, for a game that wants it at another moment — after a
/// mod registered a type, say. A file that cannot be written, or a person's own file at that path, is the
/// [`bevy_grimoire_core::problem::Problem`] `bevy_grimoire_core::schema::write_game` answers.
#[allow(clippy::result_large_err)] // `Problem` is the workspace's one refusal shape; boxing a copy of it
                                     // would make this crate's refusals a different type from the editor's.
pub fn write_now(root: &Path, world: &mut World) -> Result<PathBuf, bevy_grimoire_core::problem::Problem> {
    bevy_grimoire_core::schema::write_game(root, &registry_schema(world))
}

/// **Writes a game's reflected schema into its project when the game starts.**
///
/// `root` is the game's project root — the folder holding `assets/`, the one `.dark_factory/` sits under —
/// because that is where an outside editor looks for every kind.
///
/// **When**: `Startup`, so every type the game registered is in the document. **A failure never stops the
/// game**: an unwritable folder is a warning, because the schema is what completion is built from, not what
/// the game is. A person's own `game.schema.json` is refused by
/// [`bevy_grimoire_core::schema::write_game`] and reported the same way.
pub struct GameSchemaPlugin {
    /// The project root, the folder holding `assets/`.
    pub root: PathBuf,
}

impl Plugin for GameSchemaPlugin {
    fn build(&self, app: &mut App) {
        let root = self.root.clone();
        app.add_systems(Startup, move |world: &mut World| {
            match write_now(&root, world) {
                Ok(path) => trace!("the game's schema is at {}", path.display()),
                Err(e) => warn!("{}", e.line()),
            }
        });
    }
}
