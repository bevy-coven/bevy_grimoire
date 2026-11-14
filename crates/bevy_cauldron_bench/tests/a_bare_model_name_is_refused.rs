//! **A model name with no provider is refused for `--agent omp`** (#433, item 3a).
//!
//! A trial carries only its model's provider's credential, and the provider is the part of `provider/model` before
//! its first `/`. A bare name (`sonnet`) names no provider, so the runner used to fall back to carrying EVERY stored
//! credential into the trial's HOME. It now refuses the name, before omp is asked anything, and says which form to use.
#![cfg(unix)]

use bevy_cauldron_bench::run::{check_model, check_model_form, credentials_for};
use serde_json::json;

/// **A bare name is refused, naming the form that works.**
#[test]
fn a_bare_name_is_refused_with_the_provider_form() {
    let said = check_model_form("sonnet").expect_err("a name with no provider is refused");
    assert!(said.contains("`sonnet`"), "it names what was given: {said}");
    assert!(said.contains("provider/model"), "and the form to use: {said}");
    assert!(said.contains("anthropic/claude-sonnet-5-5"), "with an example that works: {said}");
    assert!(check_model_form("/odd").is_err(), "an empty provider is no provider");
    check_model_form("anthropic/claude-sonnet-5-5").unwrap_or_else(|e| panic!("a provider/model name passes: {e}"));
    check_model_form("ollama-cloud/deepseek-v4.1-flash:1017").unwrap_or_else(|e| panic!("so does the default: {e}"));
}

/// **The refusal comes before omp is run**: a program that does not exist is never reached.
#[test]
fn the_refusal_comes_before_omp_is_asked_anything() {
    let said = check_model("/nonexistent/omp-that-must-not-run", "sonnet").expect_err("refused");
    assert!(said.contains("provider/model"), "the model's form is what is said, not that omp could not be run: {said}");
}

/// **No credential is carried for a bare name, whatever calls this**: the narrowing has no fallback to everything.
#[test]
fn a_credential_is_never_carried_for_no_provider() {
    let export = json!({"version": 1, "credentials": [{"provider": "anthropic", "credential": {"k": "x"}}]});
    let said = credentials_for(&export, None, "sonnet").expect_err("no provider, no credential");
    assert!(said.contains("provider/model"), "{said}");
}
