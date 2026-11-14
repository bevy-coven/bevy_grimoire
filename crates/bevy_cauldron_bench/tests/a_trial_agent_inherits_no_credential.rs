//! **The omp agent inherits no credential from the runner's environment** (#433 review).
//!
//! The agent's shell can run `env`, and the runner's environment held whatever the person who started it had exported
//! (`ANTHROPIC_API_KEY`, `GITHUB_TOKEN`, `AWS_SECRET_ACCESS_KEY`): none of it is scrubbed from the transcript, because
//! the runner never imported it. So the omp agent (and the pre-flight turn) is started with those variables removed.
//! Everything else is inherited as before: the editor's port, `PATH`, and the marker a test puts on a run's processes.
#![cfg(unix)]

use std::ffi::OsString;
use std::process::Command;

use bevy_cauldron_bench::run::{credential_variables, withhold_from};

fn names(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

/// **A name that looks like a credential is withheld, and an ordinary one is not.**
#[test]
fn credential_looking_names_are_withheld() {
    let withheld = credential_variables(names(&[
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GITHUB_TOKEN",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_ACCESS_KEY_ID",
        "DB_PASSWORD",
        "GOOGLE_APPLICATION_CREDENTIALS",
        "SSH_AUTH_SOCK",
        "my_service_token",
        "PATH",
        "HOME",
        "LANG",
        "DISPLAY",
        "BEVY_BRP_PORT",
        "CAULDRON_BENCH_TEST_MARK",
        "KEYBOARD_LAYOUT",
        "TERM",
    ]));
    let withheld: Vec<String> = withheld.iter().map(|n| n.to_string_lossy().into_owned()).collect();
    for name in [
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GITHUB_TOKEN",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_ACCESS_KEY_ID",
        "DB_PASSWORD",
        "GOOGLE_APPLICATION_CREDENTIALS",
        "SSH_AUTH_SOCK",
        "my_service_token",
    ] {
        assert!(withheld.iter().any(|w| w == name), "`{name}` is withheld: {withheld:?}");
    }
    for name in ["PATH", "HOME", "LANG", "DISPLAY", "BEVY_BRP_PORT", "CAULDRON_BENCH_TEST_MARK", "KEYBOARD_LAYOUT", "TERM"] {
        assert!(!withheld.iter().any(|w| w == name), "`{name}` is inherited: {withheld:?}");
    }
}

/// **A child started with them withheld cannot read them, and still reads the rest.**
#[test]
fn a_withheld_variable_is_not_in_the_childs_environment() {
    let mut command = Command::new("sh");
    command.args(["-c", "echo \"${MY_API_KEY:-none} ${KEEP_ME:-none}\""]).env("MY_API_KEY", "sk-secret-value").env("KEEP_ME", "kept");
    withhold_from(&mut command, credential_variables(names(&["MY_API_KEY", "KEEP_ME"])));
    let out = command.output().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "none kept", "the secret is gone, the ordinary variable stays");
}
