//! **Which agent session a project had running when Cauldron was last open**, so the next launch can resume it
//! (`feedback-1794148425925`: *"If there was an agent in process, Cauldron should remember it and restart there."*).
//!
//! One file for the person, `<data_dir>/agents/resume.json` ([`RESUME_FILE`] under [`crate::terminal_agent::AgentSetup::files`]),
//! holding at most one entry per project: the harness that was running and the session it said it was in. An entry is
//! written when an agent tab reports its session, struck out when the person ends the agent (closes its tab, or leaves
//! the harness), and **left alone when the editor quits**, which is what makes it a record of what was running when
//! the editor closed.
//!
//! Only omp is remembered: it is the one harness whose sessions Cauldron can name (`omp --resume <id>`); a custom command
//! has no resume syntax the editor knows.
//!
//! Every function takes the folder, so a test owns its own and nothing here reads the real data folder.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The file under the agents folder.
pub const RESUME_FILE: &str = "resume.json";
/// The file's own version; one this build does not know is refused rather than guessed at.
const VERSION: u32 = 1;
/// The longest session id taken. omp mints UUIDs (36 characters).
const SESSION_ID_MAX: usize = 64;

/// **One project's remembered agent.**
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The project's root, as the editor held it when the agent was started.
    pub project: PathBuf,
    /// The harness that was running: `omp`.
    pub harness: String,
    /// The session the harness said it was in.
    pub session: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    agents: Vec<Entry>,
}

/// **Is `id` a session id the editor will type into a shell?** Letters, digits and `-`, as omp's UUIDs are: the id
/// is typed at a prompt, so anything else is refused, not quoted around.
pub fn is_session_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= SESSION_ID_MAX && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn read(files: &Path) -> Result<File, String> {
    let path = files.join(RESUME_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(File { version: VERSION, agents: Vec::new() }),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let file: File = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.version != VERSION {
        return Err(format!("{}: version {} is not one this build reads ({VERSION})", path.display(), file.version));
    }
    Ok(file)
}

fn write(files: &Path, file: &File) -> Result<(), String> {
    let path = files.join(RESUME_FILE);
    let text = serde_json::to_string_pretty(file).map_err(|e| format!("{}: {e}", path.display()))? + "\n";
    std::fs::create_dir_all(files).map_err(|e| format!("{}: {e}", files.display()))?;
    bevy_grimoire_core::atomic_file::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))
}

/// **Write down that `harness` was running in `project`, in `session`.** Replaces the project's entry; the other projects'
/// are kept. A malformed file is refused, not overwritten: it is the person's.
pub fn remember(files: &Path, project: &Path, harness: &str, session: &str) -> Result<(), String> {
    if !is_session_id(session) {
        return Err(format!("{session:?} is not a session id"));
    }
    let mut file = read(files)?;
    let entry = Entry { project: project.to_path_buf(), harness: harness.to_owned(), session: session.to_owned() };
    match file.agents.iter_mut().find(|e| e.project == project) {
        Some(held) if *held == entry => return Ok(()),
        Some(held) => *held = entry,
        None => file.agents.push(entry),
    }
    write(files, &file)
}

/// **Strike out `project`'s entry**, if it has one.
pub fn forget(files: &Path, project: &Path) -> Result<(), String> {
    let mut file = read(files)?;
    let before = file.agents.len();
    file.agents.retain(|e| e.project != project);
    if file.agents.len() == before {
        return Ok(());
    }
    write(files, &file)
}

/// **What `project` had running**, if anything.
pub fn recall(files: &Path, project: &Path) -> Result<Option<Entry>, String> {
    Ok(read(files)?.agents.into_iter().find(|e| e.project == project))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("agent_resume_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_project_gets_back_the_session_it_had_and_another_project_is_untouched() {
        let files = folder("round");
        let (a, b) = (Path::new("/p/a"), Path::new("/p/b"));
        assert_eq!(recall(&files, a), Ok(None), "nothing is remembered before anything is written");
        remember(&files, a, "omp", "0197-aaa").unwrap_or_else(|e| panic!("{e}"));
        remember(&files, b, "omp", "0197-bbb").unwrap_or_else(|e| panic!("{e}"));
        remember(&files, a, "omp", "0197-ccc").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(recall(&files, a).map(|e| e.map(|e| e.session)), Ok(Some("0197-ccc".to_owned())), "a newer session replaces the project's");
        assert_eq!(recall(&files, b).map(|e| e.map(|e| e.session)), Ok(Some("0197-bbb".to_owned())));
        forget(&files, a).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(recall(&files, a), Ok(None));
        assert!(recall(&files, b).is_ok_and(|e| e.is_some()), "striking one project out keeps the other");
        let _ = std::fs::remove_dir_all(&files);
    }

    #[test]
    fn an_id_that_is_not_a_session_id_is_refused_and_a_damaged_file_is_not_overwritten() {
        let files = folder("refuse");
        for bad in ["", "a b", "x;rm", "it's", &"a".repeat(SESSION_ID_MAX + 1)] {
            assert!(remember(&files, Path::new("/p"), "omp", bad).is_err(), "{bad:?} is typed into a shell, so it is refused");
        }
        assert!(!files.join(RESUME_FILE).exists(), "nothing was written for a refused id");
        std::fs::create_dir_all(&files).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(files.join(RESUME_FILE), "{ not json").unwrap_or_else(|e| panic!("{e}"));
        assert!(recall(&files, Path::new("/p")).is_err(), "a damaged file is said so, not read as empty");
        assert!(remember(&files, Path::new("/p"), "omp", "0197-aaa").is_err());
        assert_eq!(std::fs::read_to_string(files.join(RESUME_FILE)).unwrap_or_default(), "{ not json", "and it is left as it was");
        let _ = std::fs::remove_dir_all(&files);
    }
}
