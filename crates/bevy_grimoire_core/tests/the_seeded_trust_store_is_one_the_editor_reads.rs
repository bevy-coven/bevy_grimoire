//! **The trust store the style measurement seeds by hand is one the editor reads** (SG-10, #409).
//!
//! A trial has no person to allow the sample game's style, so the registered commands write `$XDG_DATA_HOME/Grimoire/trusted.yaml` naming every
//! trial project. It is written by a shell, so nothing but this test keeps it in step with the store's own version: the file's `version` is the
//! one the registration says, it is the constant `TrustStore` reads, and a store of that shape trusts the projects it lists and no others.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::trust::{Decision, Scope, TrustStore, TRUST_VERSION};

/// The page a style is served from, and what it says: the decision does not depend on either when the project is trusted.
const PAGE: &str = "design/style.md";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-seeded-trust-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch folder is made");
    // The store names a project by its resolved path, so a symlinked temp folder is named by where it points.
    dir.canonicalize().expect("the scratch folder resolves")
}

/// **The registration's seeding command writes the version the trust store reads**, not a number typed twice.
#[test]
fn the_registered_seed_writes_the_trust_stores_own_version() {
    let doc = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/research/2026-11-14-style-helps-an-agent.md");
    let text = std::fs::read_to_string(&doc).unwrap_or_else(|e| panic!("{}: {e}", doc.display()));
    let wanted = format!("echo \"version: {TRUST_VERSION}\"");
    assert!(text.contains(&wanted), "the pre-registration's seeding command does not write `{wanted}`, the version `TrustStore` reads");
}

/// **A store written the way the command writes it trusts the projects it lists, and only those.**
#[test]
fn a_seeded_store_trusts_the_listed_projects_and_no_other() {
    let listed = scratch("listed");
    let other = scratch("other");
    let store = scratch("store").join("trusted.yaml");
    std::fs::write(&store, format!("version: {TRUST_VERSION}\nprojects:\n- {}\ncommands: []\ntexts: []\n", listed.display())).expect("the store is written");
    let trusted = TrustStore::load(&store).unwrap_or_else(|e| panic!("the seeded store does not read: {e}"));
    assert_eq!(trusted.text_decision(&listed, PAGE, "any text").expect("the project resolves"), Decision::Trusted(Scope::Project));
    assert_eq!(trusted.text_decision(&other, PAGE, "any text").expect("the project resolves"), Decision::Untrusted, "a project the store does not list is not trusted");
}
