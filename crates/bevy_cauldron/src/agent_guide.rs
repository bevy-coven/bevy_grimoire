//! **The layers of an agent's guide the editor composes** (#347, AG-02).
//!
//! `bevy_grimoire_core::agent_guide` reads one file and refuses what it cannot serve whole; this module
//! knows **where** the two files are, which is the editor's knowledge and nobody else's: the person's
//! file is in their OS config directory, and the project's is inside the project it has open. So the MCP
//! server puts its own shipped layer first and reads no file itself, and `grimoire/agent_guide` returns
//! what this module read.
//!
//! **Both files are read on each call.** An agent asks at the start of a task, and a rule a person wrote
//! ten seconds ago is the one worth following; holding a copy would be a cache with no invalidation
//! anything can see.
//!
//! **A problem reaches the person as well as the agent**, because a person who wrote a guide that is not
//! being served is the only one who can fix it.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_grimoire_core::agent_guide::{self, Guide};
use bevy_grimoire_core::problem::Problem;

use crate::project::Project;

/// **Where the person's guide is**, when the binary has said. A test points it at a file of its own, as
/// `settings::UserSettingsFile` does, so no test reads or writes the real one.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct UserGuideFile(pub PathBuf);

/// `<config_dir>/Grimoire/AGENTS.md`, or `None` on a platform with no config directory.
pub fn user_file() -> Option<PathBuf> {
    bevy_grimoire_core::user_dirs::config_dir().map(|dir| dir.join(agent_guide::USER_FILE))
}

/// `<root>/.dark_factory/AGENTS.md`.
pub fn project_file(root: &std::path::Path) -> PathBuf {
    root.join(agent_guide::PROJECT_PATH)
}

/// **The whole guide the editor can add to the shipped one**: the person's layer, then the project's.
pub fn read(project: Option<&Project>, user: Option<&UserGuideFile>) -> Guide {
    let user_file = user.map(|f| f.0.clone());
    let project_file = project.map(|p| project_file(&p.root));
    agent_guide::read(user_file.as_deref(), project_file.as_deref())
}

/// **The guide's `layers` and `problems`, as `grimoire/agent_guide` answers**: each layer with the word an
/// agent routes on and the path it was read from, never the heading (which the shipped guide's own text
/// supplies the agent, and which a caller may want to place differently).
pub fn json(guide: &Guide) -> serde_json::Value {
    serde_json::json!({
        "layers": guide
            .layers
            .iter()
            .map(|layer| serde_json::json!({
                "source": layer.source.word(),
                "path": layer.path.display().to_string(),
                "text": layer.text,
            }))
            .collect::<Vec<_>>(),
        "problems": guide.problems.iter().map(Problem::to_json).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_projects_guide_is_the_dark_factory_folder_inside_the_project() {
        let root = std::path::Path::new("/a/project");
        assert_eq!(project_file(root), PathBuf::from("/a/project/.dark_factory/AGENTS.md"));
    }

    #[test]
    fn a_guide_with_no_project_and_no_file_is_empty_and_says_nothing() {
        let guide = read(None, None);
        assert!(guide.layers.is_empty());
        assert!(guide.problems.is_empty());
        assert_eq!(json(&guide), serde_json::json!({"layers": [], "problems": []}));
    }
}
