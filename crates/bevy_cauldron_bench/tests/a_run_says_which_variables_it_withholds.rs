//! **A run says, once, which environment variables the omp agent is not given** (#434).
//!
//! `withhold_credentials` removes credential-looking variables (#433), and nothing said which: the smoke could only apply
//! the rule to its own environment to learn the list was empty. The runner now prints one line with the NAMES (never a
//! value) before its first trial.

use std::ffi::OsString;

use bevy_cauldron_bench::run::{credential_variables, withheld_line};

fn names(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

/// **The line names what was withheld, sorted, and holds no value.**
#[test]
fn the_line_names_the_withheld_variables() {
    let withheld = credential_variables(names(&["PATH", "OPENAI_API_KEY", "GITHUB_TOKEN", "HOME"]));
    let line = withheld_line(&withheld);
    assert!(line.contains("GITHUB_TOKEN, OPENAI_API_KEY"), "both names, sorted: {line}");
    assert!(line.contains("(2)"), "and how many: {line}");
    assert!(!line.contains("PATH") && !line.contains("HOME"), "an inherited variable is not named: {line}");
    assert!(!line.contains('='), "names only, never a `NAME=value`: {line}");
}

/// **An environment with nothing to withhold says so**, so silence never means "the filter did not run".
#[test]
fn an_empty_list_is_said() {
    let line = withheld_line(&credential_variables(names(&["PATH", "HOME", "LANG"])));
    assert!(line.contains("no environment variable is withheld"), "{line}");
}
