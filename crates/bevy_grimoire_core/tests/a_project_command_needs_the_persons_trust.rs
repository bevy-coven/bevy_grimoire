//! **A project file cannot start a program until a person has trusted it.**
//!
//! `agent.ron` and `play.ron` name programs. Before this, the editor started whatever they named the
//! moment a person pressed Send or Play, so opening a cloned project was one key from running its author's
//! command (the review of 2026-11-02, risk 1). These tests pin the model: nothing runs untrusted; a person
//! can trust **the exact command** (an edit to it asks again) or **the whole project**; the store is the
//! person's own file and a damaged one is refused, never read as "nothing is trusted" or "all is".

use bevy_grimoire_core::note_agent::{AgentSettings, LaunchError};
use bevy_grimoire_core::trust::{CommandKind, CommandSpec, Decision, Scope, TrustStore};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A fresh directory under the system temp dir, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Dir {
        let p = std::env::temp_dir().join(format!("grimoire-trust-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        std::fs::create_dir_all(&p).unwrap();
        Dir(p)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn spec(program: &str, args: &[&str]) -> CommandSpec {
    CommandSpec { kind: CommandKind::Play, program: program.to_owned(), args: args.iter().map(|a| (*a).to_owned()).collect(), cwd: None }
}

#[test]
fn nothing_is_trusted_until_a_person_says_so() {
    let project = Dir::new("none");
    let store = TrustStore::default();
    assert_eq!(store.decision(&project.0, &spec("game", &["{map}"])).unwrap(), Decision::Untrusted);
}

#[test]
fn trusting_the_exact_command_covers_that_command_and_nothing_an_edit_could_reach() {
    let project = Dir::new("cmd");
    let mut store = TrustStore::default();
    let granted = spec("cargo", &["run", "--", "{map}"]);
    store.trust_command(&project.0, &granted).unwrap();

    assert_eq!(store.decision(&project.0, &granted).unwrap(), Decision::Trusted(Scope::Command));
    // Every way a planted edit could change what runs asks again.
    assert_eq!(store.decision(&project.0, &spec("sh", &["run", "--", "{map}"])).unwrap(), Decision::Untrusted, "another program");
    assert_eq!(store.decision(&project.0, &spec("cargo", &["run", "--", "{map}", "--evil"])).unwrap(), Decision::Untrusted, "another argument");
    let mut elsewhere = granted.clone();
    elsewhere.cwd = Some("tools".to_owned());
    assert_eq!(store.decision(&project.0, &elsewhere).unwrap(), Decision::Untrusted, "another working directory");
    let mut notes = granted.clone();
    notes.kind = CommandKind::Notes;
    assert_eq!(store.decision(&project.0, &notes).unwrap(), Decision::Untrusted, "the same words as the other kind of command");
}

#[test]
fn trusting_the_project_covers_every_command_in_it_and_no_other_project() {
    let mine = Dir::new("mine");
    let other = Dir::new("other");
    let mut store = TrustStore::default();
    store.trust_project(&mine.0).unwrap();

    assert_eq!(store.decision(&mine.0, &spec("anything", &["at", "all"])).unwrap(), Decision::Trusted(Scope::Project));
    assert_eq!(store.decision(&other.0, &spec("anything", &["at", "all"])).unwrap(), Decision::Untrusted);
}

#[test]
fn a_grant_belongs_to_the_project_it_was_made_for() {
    let mine = Dir::new("a");
    let other = Dir::new("b");
    let mut store = TrustStore::default();
    let granted = spec("game", &["{map}"]);
    store.trust_command(&mine.0, &granted).unwrap();
    assert_eq!(store.decision(&other.0, &granted).unwrap(), Decision::Untrusted, "a clone of the same command elsewhere is not trusted");
}

#[cfg(unix)]
#[test]
fn a_symlink_to_a_trusted_project_is_that_project_and_a_look_alike_is_not() {
    let real = Dir::new("real");
    let holder = Dir::new("holder");
    let link = holder.0.join("alias");
    std::os::unix::fs::symlink(&real.0, &link).unwrap();
    let lookalike = holder.0.join("real");
    std::fs::create_dir_all(&lookalike).unwrap();

    let mut store = TrustStore::default();
    store.trust_project(&real.0).unwrap();
    assert_eq!(store.decision(&link, &spec("g", &[])).unwrap(), Decision::Trusted(Scope::Project), "same directory, reached by a link");
    assert_eq!(store.decision(&lookalike, &spec("g", &[])).unwrap(), Decision::Untrusted, "a different directory with the same name");
}

#[test]
fn a_project_that_cannot_be_resolved_is_refused_not_guessed() {
    let store = TrustStore::default();
    let gone = std::env::temp_dir().join(format!("grimoire-trust-gone-{}", std::process::id()));
    let err = store.decision(&gone, &spec("g", &[])).unwrap_err();
    assert!(err.contains(&gone.display().to_string()), "the refusal names the path: {err}");
}

#[test]
fn the_store_survives_a_round_trip_and_a_missing_file_is_empty() {
    let home = Dir::new("home");
    let project = Dir::new("proj");
    let file = home.0.join("Grimoire").join("trusted.yaml");
    assert_eq!(TrustStore::load(&file).unwrap(), TrustStore::default(), "no file yet: nothing is trusted");

    let mut store = TrustStore::default();
    let granted = spec("cargo", &["run"]);
    store.trust_command(&project.0, &granted).unwrap();
    store.trust_project(&home.0).unwrap();
    store.save(&file).unwrap();

    let back = TrustStore::load(&file).unwrap();
    assert_eq!(back, store);
    assert_eq!(back.decision(&project.0, &granted).unwrap(), Decision::Trusted(Scope::Command));
}

#[test]
fn a_damaged_store_is_refused_and_never_read_as_empty_or_as_everything() {
    let home = Dir::new("bad");
    let file = home.0.join("trusted.yaml");
    for (text, why) in [
        ("(version: 1, projects: [", "cut off"),
        ("(version: 99, projects: [], commands: [])", "a version this build does not read"),
        ("(version: 1, projects: [], commands: [], also: true)", "a field it does not know"),
        ("", "empty"),
    ] {
        std::fs::write(&file, text).unwrap();
        let err = TrustStore::load(&file).expect_err(why);
        assert!(err.contains("trusted.yaml"), "{why}: the refusal names the file: {err}");
    }
}

#[test]
fn the_sentence_for_an_untrusted_command_shows_it_and_says_who_can_trust_it() {
    let project = Dir::new("say");
    let s = TrustStore::untrusted_sentence(&project.0, &spec("cargo", &["run", "--", "{map}"]));
    assert!(s.contains("cargo run -- {map}"), "it shows the exact command: {s}");
    assert!(s.contains(&project.0.display().to_string()), "it names the project: {s}");
    assert!(s.contains("person"), "it says a person trusts it, in the editor: {s}");
}

// ---- the launch itself ----

fn agent_in(marker: &Path) -> AgentSettings {
    AgentSettings::parse(&format!(
        "version: 1\ncommand: /usr/bin/touch\nargs: [\"{}\"]\n",
        marker.display()
    ))
    .unwrap()
}

#[cfg(unix)]
#[test]
fn an_untrusted_agent_command_is_not_started() {
    let project = Dir::new("launch");
    let marker = project.0.join("ran.marker");
    let settings = agent_in(&marker);

    let err = settings.launch(&project.0, &TrustStore::default()).unwrap_err();
    assert!(matches!(err, LaunchError::Untrusted(_)), "refused as untrusted, not as a failure to start: {err}");
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!marker.exists(), "the program must not have started");
}

#[cfg(unix)]
#[test]
fn a_trusted_agent_command_is_started_and_an_edited_one_is_not() {
    let project = Dir::new("launch2");
    let marker = project.0.join("ran.marker");
    let settings = agent_in(&marker);

    let mut store = TrustStore::default();
    store.trust_command(&project.0, &settings.spec().unwrap()).unwrap();
    settings.launch(&project.0, &store).unwrap().expect("a process id");
    for _ in 0..50 {
        if marker.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(marker.exists(), "the trusted command ran");

    // The same trust does not follow an edit of the file.
    let other = project.0.join("other.marker");
    let edited = agent_in(&other);
    let err = edited.launch(&project.0, &store).unwrap_err();
    assert!(matches!(err, LaunchError::Untrusted(_)), "{err}");
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!other.exists());
}

#[test]
fn no_command_means_nothing_to_trust_and_nothing_started() {
    let project = Dir::new("nocmd");
    let settings = AgentSettings::default();
    assert!(settings.spec().is_none());
    assert_eq!(settings.launch(&project.0, &TrustStore::default()).unwrap(), None);
}
