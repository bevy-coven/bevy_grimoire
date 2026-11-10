//! **A game's own reflected types, written where an outside editor looks** (GD-04, #366).
//!
//! `<root>/.dark_factory/schema/game.schema.json` describes what the game's own files accept, and only the
//! game knows that: its components are the types it registered. An outside editor's extension reads the file
//! to complete a component's fields, so it has to be **the game's registry's own answer** and not a second
//! description of the same types.
//!
//! # Where the answer comes from
//!
//! Bevy's `registry.schema` remote method is that answer, and it is the game's to make: a game that runs
//! `RemotePlugin` (a debugger, a `bevy_fairy` session) can ask its own registry for the document, and this
//! module puts it where an outside editor looks. **`bevy_grimoire` does not depend on `bevy_remote`**: it is
//! not in Bevy's default features, and a game that ships would gain a listening server for a file it writes
//! once at start. The text arrives through [`GameSchema`], exactly as
//! [`bevy_grimoire_core::schema::write_game`] takes it, and a game with no remote at all writes nothing and
//! says so once.
//!
//! # What a game does
//!
//! ```ignore
//! app.add_plugins(bevy_grimoire::game_schema::GameSchemaPlugin {
//!     root: project_root.clone(),
//!     // Whatever the game's own registry answers: `registry.schema`'s result, or a document of the same
//!     // shape. A game whose components are the point of the file hands over the real one.
//!     schema: Box::new(|| bevy::remote::schemas::…),
//! });
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::prelude::*;

/// **The document a game's registry answers**, as the JSON text `registry.schema` produces.
///
/// A boxed closure rather than a string because the registry is complete only once the game is: a document
/// taken at construction would describe whatever had been registered before the plugin was added.
pub type RegistrySchema = Arc<dyn Fn(&World) -> Option<String> + Send + Sync>;

/// **The name a game's schema file carries**, without the folder: `game.schema.json`.
pub const GAME_SCHEMA_KIND: &str = "game";

/// **Writes a game's reflected schema into its project when the game starts.**
///
/// `root` is the game's project root — the folder holding `assets/`, the one `.dark_factory/` sits under —
/// because that is where an outside editor looks for every kind. A game that names no root writes nothing.
///
/// **When**: `Startup`, so every type the game registered is in the document. A type registered after the
/// first frame is in the file on the next start; the file records nothing about that, because the registry
/// is the game's and this crate cannot see when it changes.
///
/// **A failure never stops the game.** An unwritable folder and an unreadable document are both a warning:
/// the schema is what completion is built from, not what the game is. A person's own `game.schema.json` is
/// refused by [`bevy_grimoire_core::schema::write_game`] and reported the same way.
pub struct GameSchemaPlugin {
    /// The project root, the folder holding `assets/`.
    pub root: PathBuf,
    /// **What the game's registry answers.** `None` for a game that runs no remote: nothing is written, and
    /// the plugin says so once at start.
    pub schema: Option<RegistrySchema>,
}

impl Plugin for GameSchemaPlugin {
    fn build(&self, app: &mut App) {
        let root = self.root.clone();
        let schema = self.schema.clone();
        app.add_systems(Startup, move |world: &mut World| {
            let Some(schema) = &schema else {
                debug!(
                    "no game schema is written: this game runs no remote, so its registry was never asked \
                     for one. Add a `schema` to GameSchemaPlugin to have one at .dark_factory/schema/{}",
                    bevy_grimoire_core::schema::SCHEMA_EXT
                );
                return;
            };
            let Some(text) = schema(world) else {
                warn!("the game's registry answered no schema, so .dark_factory/schema/ has none for it");
                return;
            };
            match bevy_grimoire_core::schema::write_game(&root, &text) {
                Ok(path) => trace!("the game's schema is at {}", path.display()),
                Err(e) => warn!("{}", e.line()),
            }
        });
    }
}

/// **Write a game's schema into the project at `root` right now**, from the document the registry answered.
///
/// The same write [`GameSchemaPlugin`] makes at start, for a game that wants it at another moment — after a
/// mod registered a type, say. A file that cannot be written, or a person's own file at that path, is the
/// [`bevy_grimoire_core::problem::Problem`] `write_game` answers.
pub fn write_now(root: &Path, text: &str) -> Result<PathBuf, bevy_grimoire_core::problem::Problem> {
    bevy_grimoire_core::schema::write_game(root, text)
}
