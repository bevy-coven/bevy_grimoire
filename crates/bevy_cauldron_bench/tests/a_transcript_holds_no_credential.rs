//! **A trial's retained files hold no credential string** (#433, item 3e).
//!
//! The agent inside a trial can read its provider credential in its HOME (its shell and file tools reach `agent.db`), and
//! whatever it prints lands in `transcript.jsonl`, which a run keeps. The credential strings the runner imported are known
//! to it, so after the agent is done they are scrubbed from the files the trial keeps (`transcript.jsonl`, `agent.err`),
//! replaced by `[redacted]`, before they are read or reported. (The credential in HOME is deleted separately, #432.)
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use bevy_cauldron_bench::run::{credential_strings, scrub_secrets, seed_credentials, REDACTED};
use serde_json::json;

/// The provider's access and refresh tokens, and another provider's key.
const ACCESS: &str = "sk-ant-oat01-ACCESS-9f8e7d6c5b4a";
const REFRESH: &str = "sk-ant-ort01-REFRESH-1a2b3c4d5e6f";
const OTHER: &str = "or-OTHER-PROVIDER-0123456789";

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bench-scrub-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **A stand-in `omp`** exporting two providers' credentials, importing into `$HOME/.omp/agent/agent.db`.
fn stand_in(dir: &Path) -> PathBuf {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap_or_else(|e| panic!("{e}"));
    let path = bin.join("omp");
    let script = format!(
        r#"#!/bin/sh
case "$1 $2" in
  "auth export") echo '{{"version":1,"credentials":[{{"provider":"anthropic","credential":{{"type":"oauth","access":"{ACCESS}","refresh":"{REFRESH}","expires":1}}}},{{"provider":"openrouter","credential":{{"key":"{OTHER}"}}}}]}}'; exit 0 ;;
  "auth import") mkdir -p "$HOME/.omp/agent"; cp "$3" "$HOME/.omp/agent/agent.db"; exit 0 ;;
esac
exit 2
"#
    );
    std::fs::write(&path, script).unwrap_or_else(|e| panic!("{e}"));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap_or_else(|e| panic!("{e}"));
    path
}

/// **The strings of a credential are its string values**, those long enough to be a secret and not a word.
#[test]
fn the_secret_strings_are_the_long_values() {
    let export = json!({"version": 1, "credentials": [
        {"provider": "anthropic", "credential": {"type": "oauth", "access": ACCESS, "refresh": REFRESH, "expires": 12345}},
        {"provider": "x", "credential": {"nested": {"key": "abcdefgh1234"}, "short": "k"}}]});
    let found = credential_strings(&export);
    assert!(found.contains(&ACCESS.to_owned()) && found.contains(&REFRESH.to_owned()), "{found:?}");
    assert!(found.contains(&"abcdefgh1234".to_owned()), "a nested value too: {found:?}");
    assert!(!found.iter().any(|s| s == "oauth" || s == "k" || s == "anthropic"), "a short word is no secret: {found:?}");
}

/// **The import hands back what it carried**, only the model's provider's.
#[test]
fn the_import_answers_the_strings_it_carried() {
    let dir = scratch("carried");
    let omp = stand_in(&dir);
    let secrets = seed_credentials(&omp.to_string_lossy(), &dir.join("run/home"), "anthropic/claude-sonnet-5-5").unwrap_or_else(|e| panic!("{e}"));
    assert!(secrets.contains(&ACCESS.to_owned()) && secrets.contains(&REFRESH.to_owned()), "{secrets:?}");
    assert!(!secrets.contains(&OTHER.to_owned()), "another provider's key was never carried: {secrets:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **A secret the agent printed is gone from the retained files**, whether written plainly or JSON-escaped.
#[test]
fn a_printed_secret_is_scrubbed_from_what_the_trial_keeps() {
    let dir = scratch("scrub");
    let transcript = dir.join("transcript.jsonl");
    let err = dir.join("agent.err");
    let escaped = "tok/en\"with\\odd";
    let line = json!({"type": "tool_execution_end", "output": format!("cat agent.db -> {ACCESS} and {REFRESH}"), "other": escaped});
    std::fs::write(&transcript, format!("{line}\n{{\"type\":\"message_end\"}}\n")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&err, format!("warn: {REFRESH}\n")).unwrap_or_else(|e| panic!("{e}"));
    let missing = dir.join("never-written.txt");
    let secrets = vec![ACCESS.to_owned(), REFRESH.to_owned(), escaped.to_owned()];
    let scrubbed = scrub_secrets(&[transcript.clone(), err.clone(), missing], &secrets);
    assert_eq!(scrubbed, 4, "two plain secrets in the transcript, the escaped one, and one in agent.err; a missing file is skipped");
    for file in [&transcript, &err] {
        let text = std::fs::read_to_string(file).unwrap_or_default();
        for secret in &secrets {
            assert!(!text.contains(secret.as_str()), "`{secret}` is gone from {}: {text}", file.display());
            let json_form = serde_json::to_string(secret).unwrap_or_default();
            assert!(!text.contains(json_form.trim_matches('"')), "and in its JSON-escaped form: {text}");
        }
    }
    let kept = std::fs::read_to_string(&transcript).unwrap_or_default();
    assert!(kept.contains(REDACTED), "the place is marked: {kept}");
    assert!(kept.contains("message_end"), "and the rest is kept: {kept}");
    let _ = std::fs::remove_dir_all(&dir);
}
