//! **One text grant can be withdrawn, and only that one** (SG-07, #406 review).
//!
//! `TrustStore` had no way to take an allowance back. `revoke_text` withdraws the grant for an exact text of a project's file and leaves every
//! other grant, and a project-wide trust, as they were.

use bevy_grimoire_core::trust::{Decision, TrustStore};

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    dir
}

#[test]
fn revoking_a_text_withdraws_that_grant_and_no_other() {
    let root = scratch("revoke_text");
    let mut store = TrustStore::default();
    store.trust_text(&root, "design/style.md", "one").expect("allowed");
    store.trust_text(&root, "design/style.md", "two").expect("allowed");
    store.trust_text(&root, "design/other.md", "one").expect("allowed");

    assert!(store.revoke_text(&root, "design/style.md", "one").expect("resolves"), "there was a grant to withdraw");
    assert_eq!(store.text_decision(&root, "design/style.md", "one").expect("resolves"), Decision::Untrusted);
    assert!(matches!(store.text_decision(&root, "design/style.md", "two").expect("resolves"), Decision::Trusted(_)), "another text of the file stays allowed");
    assert!(matches!(store.text_decision(&root, "design/other.md", "one").expect("resolves"), Decision::Trusted(_)), "the same text of another file stays allowed");
    assert!(!store.revoke_text(&root, "design/style.md", "one").expect("resolves"), "a second revoke finds nothing");
    std::fs::remove_dir_all(&root).expect("cleaned");
}

#[test]
fn a_project_wide_trust_is_not_a_text_grant_and_stays() {
    let root = scratch("revoke_project");
    let mut store = TrustStore::default();
    store.trust_project(&root).expect("trusted");
    assert!(!store.revoke_text(&root, "design/style.md", "anything").expect("resolves"));
    assert!(matches!(store.text_decision(&root, "design/style.md", "anything").expect("resolves"), Decision::Trusted(_)));
    std::fs::remove_dir_all(&root).expect("cleaned");
}
