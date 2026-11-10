//! `play.ron`: how a project runs its game (#97).
//!
//! The editor never links the game. A project says how to start it, and the editor starts that
//! command as a child process on the open map. Engine-free: this is the file's schema and the
//! substitution of the map and camera into the command.

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::path::{Path, PathBuf};

/// **The version this build writes.** **2** added `build`: the command that must succeed before the
/// game can show a change that needs Rust (GD-09, #371). A version 1 file still reads — its project has
/// no build command, and `game_build` says so rather than building nothing.
pub const PLAY_VERSION: u32 = 2;

/// **The oldest version this build still reads.** A file of this version has no `build` command, which
/// is a project that may never need one.
pub const PLAY_VERSION_READS_FROM: u32 = 1;

/// The port `bevy_fairy` serves BRP on unless told otherwise.
pub const DEFAULT_PORT: u16 = 15702;

fn default_port() -> u16 {
    DEFAULT_PORT
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlayConfig {
    pub version: u32,
    /// The program and its arguments. `{map}`, `{root}` and `{camera}` inside an argument are
    /// replaced by the map's name, the project root and `x,y,z`.
    pub command: Vec<String>,
    /// The directory the command runs in, relative to the project root. Default: the root.
    #[serde(default)]
    pub cwd: Option<String>,
    /// The port the game's `bevy_fairy` answers on (BRP over HTTP, localhost).
    #[serde(default = "default_port")]
    pub port: u16,
    /// **How the game is rebuilt when a change needs Rust** (GD-09, #371): the program and its
    /// arguments, run with cargo's own JSON messages. `{root}` is replaced by the project root.
    ///
    /// **Empty in a version 1 file, and in a project that never needs one**: `game_build` refuses with
    /// a sentence naming this field rather than reporting a build it did not run. A list rather than
    /// an `Option`, because a list with no entries reads and writes as `[]` in RON and an absent
    /// `Option` needs `None` spelled out in every file that has a game and no build.
    #[serde(default)]
    pub build: Vec<String>,
}

/// `<project>/play.ron`.
pub fn path_for_project(root: &Path) -> PathBuf {
    root.join("play.ron")
}

impl PlayConfig {
    pub fn from_ron(text: &str) -> Result<PlayConfig, String> {
        let c: PlayConfig = ron::from_str(text).map_err(|e| format!("play: parse: {e}"))?;
        if c.version < PLAY_VERSION_READS_FROM || c.version > PLAY_VERSION {
            return Err(format!(
                "play: version {} is not one of the versions this build reads ({PLAY_VERSION_READS_FROM}-{PLAY_VERSION})",
                c.version
            ));
        }
        if c.command.first().is_none_or(|p| p.trim().is_empty()) {
            return Err("play: `command` is empty, so there is no game to start".to_owned());
        }
        Ok(c)
    }

    /// Reads the project's `play.ron`. A project without one has no way to play, and says so.
    pub fn load(root: &Path) -> Result<PlayConfig, String> {
        let path = path_for_project(root);
        match std::fs::read_to_string(&path) {
            Ok(text) => PlayConfig::from_ron(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(format!(
                "{} does not exist. Write `(version: 1, command: [\"cargo\", \"run\", \"--\", \"{{map}}\"])` there: how to run this game.",
                path.display()
            )),
            Err(e) => Err(format!("play: {}: {e}", path.display())),
        }
    }

    /// The command with `{map}`, `{root}` and `{camera}` filled in.
    pub fn argv(&self, root: &Path, map: &str, camera: Option<[f32; 3]>) -> Vec<String> {
        let cam = camera.map(|c| format!("{},{},{}", c[0], c[1], c[2])).unwrap_or_default();
        self.command
            .iter()
            .map(|a| {
                a.replace("{map}", map).replace("{root}", &root.to_string_lossy()).replace("{camera}", &cam)
            })
            .collect()
    }

    /// The command as a person reads and trusts it: the template, with `{map}`, `{root}` and `{camera}`
    /// unfilled, and the working directory as the file writes it.
    pub fn spec(&self) -> crate::trust::CommandSpec {
        crate::trust::CommandSpec {
            kind: crate::trust::CommandKind::Play,
            program: self.command.first().cloned().unwrap_or_default(),
            args: self.command.iter().skip(1).cloned().collect(),
            cwd: self.cwd.clone(),
        }
    }

    /// **The build command with `{root}` filled in**, or why this project has none.
    ///
    /// The one place the command is read, so a `play.ron` that names an empty build is refused the
    /// same way everywhere it is asked.
    pub fn build_argv(&self, root: &Path) -> Result<Vec<String>, String> {
        // **One refusal for both empty cases**: a `build: []` and a `build` the file never wrote mean
        // the same thing to a person, and two sentences for one fact would read as two problems.
        if self.build.first().is_none_or(|p| p.trim().is_empty()) {
            return Err(format!(
                "{} names no `build` command, so there is nothing to rebuild. Add one, e.g. \
                 `build: [\"cargo\", \"build\", \"--message-format=json-render-diagnostics\"]`, \
                 and a change that needs Rust can be built from here.",
                path_for_project(root).display()
            ));
        }
        Ok(self.build.iter().map(|a| a.replace("{root}", &root.to_string_lossy())).collect())
    }

    /// **The build command as a person reads and trusts it**, for the trust gate: the template, with
    /// `{root}` unfilled, because what is allowed is the command a person read.
    pub fn build_spec(&self) -> Option<crate::trust::CommandSpec> {
        let build = &self.build;
        build.first()?;
        Some(crate::trust::CommandSpec {
            kind: crate::trust::CommandKind::Build,
            program: build.first().cloned().unwrap_or_default(),
            args: build.iter().skip(1).cloned().collect(),
            cwd: self.cwd.clone(),
        })
    }

    pub fn dir(&self, root: &Path) -> PathBuf {
        match &self.cwd {
            Some(c) => root.join(c),
            None => root.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_takes_the_map_and_camera() {
        let c = PlayConfig::from_ron(r#"(version: 1, command: ["g", "--map", "{map}", "--at={camera}"])"#).unwrap();
        assert_eq!(c.port, DEFAULT_PORT);
        assert_eq!(c.argv(Path::new("/p"), "town", Some([1.0, 2.5, -3.0])), ["g", "--map", "town", "--at=1,2.5,-3"]);
    }

    #[test]
    fn a_project_with_no_build_command_says_so_and_a_version_1_file_still_reads() {
        let c = PlayConfig::from_ron(r#"(version: 1, command: ["g", "{map}"])"#).unwrap();
        assert!(c.build.is_empty(), "a version 1 file has no build command");
        let refused = c.build_argv(Path::new("/p")).expect_err("a project with no build says so");
        assert!(refused.contains("build") && refused.contains("play.ron"), "{refused}");
        assert!(c.build_spec().is_none(), "a project with no build names nothing to allow");
    }

    #[test]
    fn a_build_command_takes_the_root_and_is_what_a_person_allows() {
        let c = PlayConfig::from_ron(r#"(version: 2, command: ["g"], build: ["cargo", "build", "--manifest-path={root}/Cargo.toml"])"#).unwrap();
        assert_eq!(c.build_argv(Path::new("/p")).unwrap(), ["cargo", "build", "--manifest-path=/p/Cargo.toml"]);
        let spec = c.build_spec().expect("a build command is allowed like the game's");
        assert_eq!(spec.program, "cargo");
        assert_eq!(spec.args, ["build", "--manifest-path={root}/Cargo.toml"]);
    }

    #[test]
    fn an_empty_command_is_refused() {
        assert!(PlayConfig::from_ron("(version: 1, command: [])").is_err());
    }
}
