//! **A trial carries one provider's credential and leaves none of it behind** (#432, D6 and D7).
//!
//! The first real-agent run found `seed_credentials` importing *every* stored credential (an OAuth refresh
//! token included) into `<trial>/home/.omp/agent/agent.db`, and nothing deleting that `HOME`: each trial kept
//! a copy of the host's whole login, plus the 128 MB native library omp extracts there, about 200 MB a trial
//! (28 GB for a 140-trial baseline). A failed import also returned before the carrying file was removed, so
//! `auth-carried.json` kept the keys in plain text.
//!
//! These tests run the real functions against a stand-in `omp`, a shell script that prints a planted export
//! and imports it the way omp does (into `$HOME/.omp/agent/agent.db`), plus a fake native library. No model
//! is called and no editor starts; what is checked is the files left under a finished trial's directory.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use bevy_cauldron_bench::run::{credentials_for, model_provider, seed_credentials, TrialSecrets, OMP_PROGRAM};

/// The secret of the provider the trial's model runs on, and of another the host is also logged in to.
const OWN_SECRET: &str = "SECRET-OWN-6f1c0b7e";
const OTHER_SECRET: &str = "SECRET-OTHER-93d2a1c4";
/// The model every case runs, and the provider it names.
const MODEL: &str = "anthropic/claude-sonnet-5-5";

/// **A scratch directory this test owns**, named with the test's name and the process id.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bench-secrets-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **A stand-in `omp`**, in `<dir>/bin` (never under the run's own directory, which is what is scanned for secrets, and the
/// script holds them): `auth export` prints two providers' credentials; `auth import <file>` copies the file
/// to `$HOME/.omp/agent/agent.db` and extracts a fake native library beside it, as omp does; a file called
/// `refuse-import` beside the script makes the import fail after it has read the file.
fn stand_in(dir: &Path) -> PathBuf {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap_or_else(|e| panic!("{}: {e}", bin.display()));
    let path = bin.join("omp");
    let script = format!(
        r#"#!/bin/sh
here="$(dirname "$0")"
if [ "$1 $2" = "auth export" ]; then
  cat <<'JSON'
{{"version":1,"credentials":[{{"provider":"anthropic","credential":{{"type":"oauth","access":"{OWN_SECRET}"}}}},{{"provider":"openrouter","credential":{{"type":"api_key","key":"{OTHER_SECRET}"}}}}]}}
JSON
  exit 0
fi
if [ "$1 $2" = "auth import" ]; then
  mkdir -p "$HOME/.omp/agent" "$HOME/.omp/natives/18.18.9"
  cp "$3" "$HOME/.omp/agent/agent.db"
  head -c 300000 /dev/zero > "$HOME/.omp/natives/18.18.9/pi_natives.node"
  if [ -e "$here/refuse-import" ]; then echo "import refused" >&2; exit 1; fi
  exit 0
fi
echo "unexpected: $*" >&2
exit 2
"#
    );
    std::fs::write(&path, script).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    path
}

/// **Every file under `dir` that holds `needle`**, by path.
fn holding(dir: &Path, needle: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(holding(&path, needle));
        } else if std::fs::read(&path).is_ok_and(|bytes| bytes.windows(needle.len()).any(|w| w == needle.as_bytes())) {
            found.push(path);
        }
    }
    found
}

/// **The model's provider is the part before the first `/`**, and a bare name names none.
#[test]
fn a_model_names_its_provider() {
    assert_eq!(model_provider("anthropic/claude-sonnet-5-5"), Some("anthropic"));
    assert_eq!(model_provider("ollama-cloud/deepseek-v4.1-flash:1017"), Some("ollama-cloud"));
    assert_eq!(model_provider("sonnet"), None);
    assert_eq!(model_provider("/odd"), None);
}

/// **Only the model's provider's credential is kept**, and a host with none for it is told which to log in to.
#[test]
fn only_the_models_provider_is_kept() {
    let export: serde_json::Value = serde_json::from_str(&format!(
        r#"{{"version":1,"credentials":[{{"provider":"anthropic","credential":{{"k":"{OWN_SECRET}"}}}},{{"provider":"openrouter","credential":{{"k":"{OTHER_SECRET}"}}}}]}}"#
    ))
    .unwrap_or_else(|e| panic!("{e}"));
    let kept = credentials_for(&export, Some("anthropic"), MODEL).unwrap_or_else(|e| panic!("{e}"));
    let text = kept.to_string();
    assert!(text.contains(OWN_SECRET), "the model's own credential is kept: {text}");
    assert!(!text.contains(OTHER_SECRET), "another provider's credential is not carried: {text}");
    assert_eq!(kept.get("version"), export.get("version"), "the export's own shape is kept");
    let none = credentials_for(&export, Some("ollama-cloud"), "ollama-cloud/x").expect_err("no credential for the provider");
    assert!(none.contains("ollama-cloud") && none.contains("anthropic"), "the refusal names the provider wanted and those held: {none}");
    assert!(!none.contains(OWN_SECRET) && !none.contains(OTHER_SECRET), "and no secret: {none}");
    let bare = credentials_for(&export, None, "sonnet").unwrap_or_else(|e| panic!("{e}"));
    assert!(bare.to_string().contains(OTHER_SECRET), "a bare model name cannot be narrowed, so every credential is kept");
}

/// **A trial's directory holds no credential and no natives once its agent is done.**
#[test]
fn a_finished_trial_holds_no_credential() {
    let dir = scratch("finished");
    let omp = stand_in(&dir);
    let runs = dir.join("run");
    let trial = runs.join("trial-1");
    let home = trial.join("home");
    {
        let _secrets = TrialSecrets::new([home.clone()]);
        seed_credentials(&omp.to_string_lossy(), &home, MODEL).unwrap_or_else(|e| panic!("the import works: {e}"));
        // **While the agent runs the trial has what it needs**: the model's credential, and not the other one.
        assert_eq!(holding(&home, OWN_SECRET).len(), 1, "the agent's HOME holds its own credential");
        assert!(holding(&home, OTHER_SECRET).is_empty(), "and never another provider's: {:?}", holding(&home, OTHER_SECRET));
        assert!(!home.join("auth-carried.json").exists(), "the carrying file is gone once the import ran");
        assert!(home.join(".omp/natives/18.18.9/pi_natives.node").is_file(), "the natives are there to be deleted");
    }
    // The agent is done; the trial's records stay.
    std::fs::write(trial.join("trial.json"), "{}\n").unwrap_or_else(|e| panic!("{e}"));
    for secret in [OWN_SECRET, OTHER_SECRET] {
        assert!(holding(&runs, secret).is_empty(), "`{secret}` is nowhere under the finished trial: {:?}", holding(&runs, secret));
    }
    assert!(!home.exists(), "the trial's HOME, natives included, is not kept");
    assert!(trial.join("trial.json").is_file(), "while the trial's own record is");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **A failed import leaves no credential either**: the carrying file is removed on the error path, and the guard
/// removes what the import wrote before it failed.
#[test]
fn a_failed_import_leaves_no_credential() {
    let dir = scratch("failed");
    let omp = stand_in(&dir);
    std::fs::write(dir.join("bin/refuse-import"), "").unwrap_or_else(|e| panic!("{e}"));
    let runs = dir.join("run");
    let trial = runs.join("trial-1");
    let home = trial.join("home");
    {
        let _secrets = TrialSecrets::new([home.clone()]);
        let said = seed_credentials(&omp.to_string_lossy(), &home, MODEL).expect_err("the import was refused");
        assert!(said.contains("import refused"), "the refusal is named: {said}");
        assert!(!home.join("auth-carried.json").exists(), "the plain-text file is removed on the error path, before anything else is");
    }
    for secret in [OWN_SECRET, OTHER_SECRET] {
        assert!(holding(&runs, secret).is_empty(), "`{secret}` is nowhere under the failed trial: {:?}", holding(&runs, secret));
    }
    assert!(!home.exists(), "and the HOME is gone");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The program a trial runs is `omp`**, found on `PATH`: the parameter exists for these tests and the run does not change it.
#[test]
fn the_runner_runs_omp() {
    assert_eq!(OMP_PROGRAM, "omp");
}
