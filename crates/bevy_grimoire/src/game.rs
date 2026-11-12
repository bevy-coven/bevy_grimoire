//! **A whole game, from a folder of data** (GD-10, #372).
//!
//! `App::new().add_plugins((DefaultPlugins, GamePlugin)).run()` is a game: the plugin reads the project at
//! [`ROOT_ENV`] (the working directory when it is not set), registers the behaviours ([`BehaviourPlugin`]), reads
//! `game/*.entities.yaml` ([`GameEntitiesPlugin`]) and runs the Rhai scripts the entities name
//! ([`ScriptPlugin`]), and puts the project's first map on the ground ([`GrimoirePlugin`]). **There is no
//! game-specific Rust to write**: a behaviour a game needs and cannot express is a component of
//! [`crate::behaviours`], or a gap in the script API, and is filed as one.
//!
//! # The data half is a plugin of its own
//!
//! [`GameDataPlugin`] is everything but the ground: no renderer, no asset server. A headless test of a game plays it
//! (`games/sample/tests/`), and a server could run it. [`GamePlugin`] is that plus the map.
//!
//! # A release reads its bake
//!
//! With the `baked` feature [`GamePlugin`] has this plugin open the bake in [`BAKED_DIR`] under the root before it reads anything
//! ([`bevy_grimoire_core::load::use_baked`]), and from then on every project file and entity file is read from it.
//! **A bake that is missing, or that was made from YAML that has changed, stops the game before its first frame**,
//! naming the file: the plugin logs the problem and asks the app to exit with an error. It never falls back to
//! the YAML.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_grimoire_rhai::{ScriptEvent, ScriptPlugin, TRIGGER_EVENT};

use crate::behaviours::{BehaviourPlugin, BehaviourSystems, Triggered};
use crate::entities::GameEntitiesPlugin;

/// **Whether this build is a release that reads only its bake** (the `baked` cargo feature).
pub const BAKED_BUILD: bool = cfg!(feature = "baked");

/// **The environment variable that names a game's project folder.** The working directory when it is not set.
pub const ROOT_ENV: &str = "GRIMOIRE_ROOT";

/// **The folder of a game's bake**, under its project root (`cauldron bake <root> --out <root>/baked`).
pub const BAKED_DIR: &str = "baked";

/// **Where a game's project data lives under its root**: the folder holding `kits.yaml`.
pub const GRIMOIRE_DIR: &str = "assets/grimoire";

/// **The project folder this process reads**: [`ROOT_ENV`], or the working directory.
pub fn root_from_env() -> PathBuf {
    std::env::var_os(ROOT_ENV).map_or_else(|| PathBuf::from("."), PathBuf::from)
}

/// **Why a game could not start**, kept so the app can say it and exit.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct StartRefused(pub String);

/// **The data half of a game**: behaviours, entity files and scripts, with the bake first in a release.
#[derive(Clone, Debug)]
pub struct GameDataPlugin {
    /// The project root: the folder holding `game/`, `scripts/` and `assets/grimoire/`.
    pub root: PathBuf,
    /// **The bake to read instead of the YAML**, for a release ([`GamePlugin`] sets it in a build with the `baked` feature).
    /// A bake that cannot be opened stops the game before its first frame.
    pub bake: Option<PathBuf>,
}

impl Plugin for GameDataPlugin {
    fn build(&self, app: &mut App) {
        if let Some(dir) = &self.bake
            && let Err(problem) = bevy_grimoire_core::load::use_baked(dir, &self.root)
        {
            let sentence = problem.line();
            error!("{sentence}");
            app.insert_resource(StartRefused(sentence)).add_systems(Startup, refuse_to_start);
            return;
        }
        app.add_plugins((BehaviourPlugin, GameEntitiesPlugin { root: self.root.clone() }, ScriptPlugin { root: self.root.clone() }))

            .add_systems(PostUpdate, tell_scripts_of_crossings.after(BehaviourSystems::Produces));
    }
}

/// **A release that cannot trust its data exits with an error before the first frame.**
fn refuse_to_start(mut exit: MessageWriter<AppExit>) {
    exit.write(AppExit::error());
}

/// **A volume crossed is an event for the script on the volume**, in the one vocabulary scripts have
/// ([`TRIGGER_EVENT`]): `on_trigger(other)` runs on the volume's own script with the body's `Name`.
///
/// Both directions are one event with the body's name; a script that cares about the way out reads its own state.
fn tell_scripts_of_crossings(
    mut crossings: MessageReader<Triggered>,
    names: Query<&Name>,
    mut events: MessageWriter<ScriptEvent>,
) {
    for crossing in crossings.read() {
        if !crossing.entered {
            continue;
        }
        let (Ok(volume), Ok(body)) = (names.get(crossing.trigger), names.get(crossing.body)) else { continue };
        events.write(ScriptEvent { name: TRIGGER_EVENT.to_owned(), data: body.as_str().to_owned(), to: volume.as_str().to_owned() });
    }
}

/// **A whole game**: [`GameDataPlugin`] at [`root_from_env`], and the project's first map on the ground.
#[derive(Clone, Copy, Debug, Default)]
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        let root = root_from_env();
        app.add_plugins(GameDataPlugin { bake: BAKED_BUILD.then(|| root.join(BAKED_DIR)), root: root.clone() });
        if app.world().contains_resource::<StartRefused>() {
            return;
        }
        app.add_plugins(crate::GrimoirePlugin);
        match world_of(&root) {
            Ok(Some(world)) => {
                app.insert_resource(world);
            }
            Ok(None) => {}
            Err(sentence) => {
                error!("{sentence}");
                app.insert_resource(StartRefused(sentence)).add_systems(Startup, exit_with_error);
            }
        }
    }
}

fn exit_with_error(mut exit: MessageWriter<AppExit>) {
    exit.write(AppExit::error());
}

/// **The project's first map, as the world the ground is spawned from**, or `None` for a game with no project data.
///
/// The first map by name: a game that wants another says so by having only that one. Refused with the file's own
/// sentence when the project does not load.
pub fn world_of(root: &Path) -> Result<Option<crate::GrimoireWorld>, String> {
    let dir = root.join(GRIMOIRE_DIR);
    if !bevy_grimoire_core::load::exists(&dir.join(bevy_grimoire_core::kits::KITS_FILE)) {
        return Ok(None);
    }
    let Some(map) = first_map_named(root)? else { return Ok(None) };
    let data = bevy_grimoire_core::project_data::load(&dir, &[map.as_str()])?;
    let Some(map) = data.maps.get(&map).cloned() else { return Ok(None) };
    crate::GrimoireWorld::with_compositions(
        data.bound.library,
        map,
        data.vocab,
        &data.compositions.compositions,
        data.kits.lattice,
    )
    .map(Some)
}

/// **The name of the first map the project holds**, in name order: through the same listing a release reads its manifest by.
fn first_map_named(root: &Path) -> Result<Option<String>, String> {
    let suffix = ".map.yaml";
    let mut names: Vec<String> = bevy_grimoire_core::load::files_of_kind(root, "map")
        .map_err(|p| p.line())?
        .iter()
        .filter_map(|path| path.file_name()?.to_str()?.strip_suffix(suffix).map(str::to_owned))
        .collect();
    names.sort();
    Ok(names.into_iter().next())
}
