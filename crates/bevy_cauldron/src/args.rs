//! **One parser for what opens a door**, whether the menu chose it or the command line did.
//!
//! The menu produces exactly the argv `main` accepts — `[root, map?, --door <d>, --kit <k>]` — so
//! there is one thing to get wrong rather than two that agree until they do not. That mattered less
//! when the menu launched the editor as a child process and argv was literally the interface; it
//! matters more now that both live in one application (`crate::screen`) and the temptation is a
//! second, tidier struct on the inside.

use std::path::PathBuf;

use bevy::prelude::*;

use crate::project::{OpenMap, Project};
use crate::tiles::Mode;
use crate::workspace::Workspace;

/// A door, loaded and ready to insert.
pub struct Opened {
    pub project: Project,
    /// `None` on every door but Maps — see [`crate::project::OpenMap`].
    pub open_map: Option<OpenMap>,
    pub workspace: Workspace,
    /// Which of the door's panels it opens on.
    pub mode: Mode,
}

impl Opened {
    /// Put it in the World. Synchronous, because `OnEnter(Editor)` is the next schedule and
    /// everything in it assumes a loaded project.
    pub fn insert_into(self, world: &mut World) {
        // **Every kind's JSON Schema is written into the project where an open is ACCEPTED** (GD-04, #366), so
        // an outside editor's YAML extension finds a schema beside each YAML file it is pointed at, and the
        // schemas a release shipped are replaced by the ones this build's types describe. It is here and not
        // in `open_staged` because the staged load runs on a thread a person can cancel, and a cancelled open
        // leaves the project byte for byte as it was (`loading.rs`: an open only reads). The launcher, the
        // command line and an agent's open all end in this function, so one seam keeps them one behaviour.
        //
        // **A refused write is reported and does not stop the open.** A schema is what completion is built
        // from, not what the project is; the message is the `Problem`'s own line, which names the file it
        // refused and never claims a file a person wrote was replaced.
        if let Err(e) = bevy_grimoire_core::schema::write_all(&self.project.root) {
            bevy::log::warn!(target: "cauldron::schema", "{}", e.line());
        }
        world.insert_resource(self.project);
        world.insert_resource(self.workspace);
        world.insert_resource(self.mode);
        match self.open_map {
            Some(m) => world.insert_resource(m),
            // **Removed, not left stale.** A door with no map that inherited the last one would be
            // the exact failure the full teardown exists to prevent.
            None => {
                world.remove_resource::<OpenMap>();
            }
        }
    }

    /// Drop everything a door owns.
    ///
    /// Named one by one because there is no reachability rule for resources the way there is for
    /// entities — a resource nobody removes simply stays, and a stale `Project` outliving its door
    /// is the bug a full teardown is spent to make impossible.
    pub fn remove_from(world: &mut World) {
        world.remove_resource::<Project>();
        world.remove_resource::<OpenMap>();
        world.remove_resource::<Workspace>();
        world.remove_resource::<Mode>();
    }
}

/// **What the workspace needs, out of the argv both callers speak.**
///
/// `--workspace` is required for anything but the Map workspace, because a bare positional cannot say
/// which workspace it names. A missing `--workspace` with a map name is the Map workspace, which is what
/// `bevy_cauldron . site_67` always meant.
/// **What an argv says, split once.**
///
/// Every question this module answers — where the root is, whether a door was named, which door —
/// is answered off this one split. Three hand-rolled scans is how `root_of` came to treat `--kit`'s
/// *value* as the first positional: `bevy_cauldron --kit site` pointed Bevy's asset root at
/// `./site/assets` while `open` correctly resolved the project at `.`, and every mesh 404'd. The
/// module note promised "one thing to get wrong rather than two that agree until they do not"; this
/// is that one thing.
pub struct Split<'a> {
    pub positional: Vec<&'a str>,
    pub kit: Option<&'a str>,
    pub workspace: Option<&'a str>,
    /// `--live`: write every committed change to disk at once, so a running game that watches the
    /// project reloads it (see [`crate::live`]).
    pub live: bool,
}

/// Split an argv into its positionals and the three flags this editor takes: `--kit` and
/// `--workspace` and `--kit`, which take a value, and `--live`, which does not.
///
/// A flag's value is consumed with it, so it can never be mistaken for a positional.
pub fn split(args: &[String]) -> Split<'_> {
    let mut out = Split {
        positional: Vec::new(),
        kit: None,
        workspace: None,
        live: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--kit" => out.kit = it.next().map(String::as_str),
            "--workspace" | "--door" => out.workspace = it.next().map(String::as_str),
            "--live" => out.live = true,
            other => out.positional.push(other),
        }
    }
    out
}

pub fn open(args: &[String]) -> Result<Opened, String> {
    open_staged(args, &mut |_| {})
}

/// [`open`], telling `at` which [`crate::project::Stage`] it is entering, for the loading screen
/// (`loading.rs`).
pub fn open_staged(args: &[String], at: &mut dyn FnMut(crate::project::Stage)) -> Result<Opened, String> {
    let Split {
        positional,
        kit,
        workspace: workspace_flag,
        ..
    } = split(args);
    let root = root_from(&positional);

    let workspace = match workspace_flag {
        None => Workspace::default(),
        Some(name) => Workspace::from_flag(name).ok_or_else(|| {
            format!(
                "no workspace called `{name}`. The workspaces are {}.",
                Workspace::ALL
                    .iter()
                    .map(|d| d.label().to_lowercase())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?,
    };
    // **An unbuilt workspace is refused as `door_open` refuses it**, with the sentence that names the
    // issue bringing it, rather than opening an editor on a screen that has nothing to show.
    if let Some(missing) = workspace.missing() {
        return Err(format!("cannot open the {} workspace: {missing}", workspace.title().to_lowercase()));
    }

    let mut project = Project::open_staged(&root, kit, at)
        .map_err(|e| format!("cannot open {}: {e}", root.display()))?;
    at(crate::project::Stage::Maps);

    // **Only the Map workspace takes a map**, and it is the only one that needs the second positional.
    let open_map = if workspace == Workspace::Map {
        let name = positional.get(1).copied().unwrap_or("untitled_map");
        let m =
            OpenMap::open(&project, name).map_err(|e| format!("cannot open map `{name}`: {e}"))?;
        // **And the project remembers the map's name**, so switching from the Kit workspace back to
        // the Map workspace knows which map to open. See `Project::open_map_name`.
        project.open_map_name = Some(m.map.name.clone());
        Some(m)
    } else {
        None
    };

    Ok(Opened {
        mode: workspace.opens_on(),
        project,
        open_map,
        workspace,
    })
}

/// The project root out of the positionals: the first one, or the working directory.
fn root_from(positional: &[&str]) -> PathBuf {
    let root = PathBuf::from(positional.first().copied().unwrap_or("."));
    // A path that cannot be canonicalised is one that does not exist; keep it as written so
    // `Project::open` names it in the refusal rather than reporting an absolute path nobody typed.
    std::fs::canonicalize(&root).unwrap_or(root)
}

/// **What `--help` prints.** The arguments [`split`] takes, and nothing the editor does not read.
pub const USAGE: &str = "\
Cauldron: the editor that writes a grimoire, the book your game loads.

usage: bevy_cauldron [ROOT] [MAP] [--workspace <id>] [--kit NAME] [--live]

  ROOT          the project's root, the folder holding assets/grimoire (default: the current folder)
  MAP           the map to open on the Map workspace; naming one, or a --workspace, skips the launcher
  --workspace W open a workspace directly: project, map, world, kit, animation, sound, files, game or agent.
                `--door` is still read, so a recorded run keeps working.
  --kit NAME    the kit to author in, when the project binds more than one
  --live        write every committed change to disk at once, for a game that watches the project
  -h, --help    print this and exit";

/// Whether the arguments ask for [`USAGE`] rather than a window.
pub fn asks_for_help(args: &[String]) -> bool {
    args.iter().any(|a| a == "--help" || a == "-h")
}

/// Where the project is, for the asset root. `main` needs this before anything is opened.
pub fn root_of(args: &[String]) -> PathBuf {
    root_from(&split(args).positional)
}

/// **Whether these arguments name a workspace directly**, rather than asking for the menu.
///
/// A `--workspace`, or a second positional — which is the map name, and only the Map workspace takes
/// one.
/// Counted off [`split`], so `--kit site` is a flag and its value, never "a second positional".
pub fn names_a_door(args: &[String]) -> bool {
    let split = split(args);
    split.workspace.is_some() || split.positional.len() >= 2
}

/// **Whether these arguments ask for the launcher**: no project and no door named, so a person who ran the
/// editor bare is shown the projects they have opened (`launcher.rs`). A `ROOT`, a map, `--door` or `--kit`
/// names what to open: a bare `ROOT` goes to the chooser as before, and a door or a map goes through the loading
/// screen (`loading.rs`). `--live` names no project, so a bare `--live` opens the launcher, which keeps it for the hand-off.
pub fn opens_the_launcher(args: &[String]) -> bool {
    let split = split(args);
    split.positional.is_empty() && split.workspace.is_none() && split.kit.is_none()
}
