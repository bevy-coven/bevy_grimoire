//! **Every format the editor writes carries a version, and a file of another version is read or refused
//! with a verdict** (#202, H5 step 4).
//!
//! A format that carries no version cannot change shape without breaking the projects already on disk:
//! a reader has no way to tell an old file from a damaged one. So each kind of
//! `bevy_grimoire_core::reload::KINDS` is in exactly one of four places:
//!
//! - **versioned**: the type this crate parses has a root `version`, and the test below proves that a
//!   file one version ahead of what this build reads is refused, naming the version the build reads, and
//!   that a file from before the first version is read or refused the same way (a cache is the one
//!   exception: a file of another version is stale, and made again);
//! - **versioned above**: the type lives in a layer above this crate (the editor, the decision crate), so
//!   this crate cannot parse a sample; the entry names the test over there that proves the same two
//!   things, and a test here fails when that file is gone or does not name the kind;
//! - **unversioned**: the debt. The list may shrink and never silently grow, and a kind comes off it in
//!   the commit that gives its type a `version` (with `#[serde(default)]` so the files already on disk,
//!   frozen in `tests/old_data/`, keep reading);
//! - **owned elsewhere**: the file's format is somebody else's to version (an image's own header, a
//!   game's own RON, a family crate's own settings), and the entry says whose.

use std::collections::BTreeSet;
use std::path::Path;

use bevy_grimoire_core::destructible::{Bake, Chunk, DESTRUCTIBLE_VERSION, Marks};
use bevy_grimoire_core::heightfield::Heightfield;
use bevy_grimoire_core::map::{MAP_VERSION, Map};
use bevy_grimoire_core::notes::Notes;
use bevy_grimoire_core::plan::Plan;
use bevy_grimoire_core::repetition::RepetitionRules;
use bevy_grimoire_core::references::References;
use bevy_grimoire_core::relations::Relations;
use bevy_grimoire_core::reload::{self, KINDS};
use bevy_grimoire_core::reload_check::{self, Check};
use bevy_grimoire_core::standards::Standards;
use bevy_grimoire_core::vocab::Vocabularies;
use bevy_grimoire_core::volume::Volume;
use bevy_grimoire_core::water::{WATER_FIELD_VERSION, WaterField, WaterPlan};

/// **The kinds whose format has no version today**, each with what is left to do. Checked in: a kind
/// leaves this list in the commit that versions it.
const UNVERSIONED: &[(&str, &str)] = &[];

/// **A literal, not `UNVERSIONED.len()`**: a number that is its own measure can never fail. Lower it with
/// the list; raising it is a decision a reviewer must see.
const UNVERSIONED_MAX: usize = 0;

/// **Kinds whose type lives above this crate and carries a version there**: the kind, the test that proves
/// it (a path from the workspace root) and what the type is. The proof reads a file one version ahead and
/// one from before the first the way this file does for the kinds core parses.
const VERSIONED_ABOVE: &[(&str, &str, &str)] = &[
    ("forest", "crates/bevy_cauldron/tests/a_forest_people_or_scripts_file_of_another_version_is_refused.rs", "the editor's `Forest`, a map's planted trees"),
    ("people", "crates/bevy_cauldron/tests/a_forest_people_or_scripts_file_of_another_version_is_refused.rs", "the editor's `People`, a map's people"),
    ("scripts", "crates/bevy_cauldron/tests/a_forest_people_or_scripts_file_of_another_version_is_refused.rs", "the editor's `Scripts`, a map's triggers"),
    ("checks", "crates/bevy_oracle/tests/a_checks_file_of_another_version_is_refused.rs", "the decision crate's `Checks`"),
    ("oplog", "crates/bevy_cauldron/tests/an_oplog_line_of_another_version_is_refused.rs", "the editor's operation log entry, one JSON line each"),
    ("editor_settings", "crates/bevy_cauldron/tests/a_key_set_in_both_settings_layers_takes_the_projects.rs", "the editor's `SettingsLayer`, one settings file"),
];

/// **Kinds whose format is not the editor's to version**, and whose it is.
const OWNED_ELSEWHERE: &[(&str, &str)] = &[
    ("reference_images", "an image file: the image format's own header says what it is"),
    ("game_data", "the game's own RON: the game owns the shape, `data.ron` only lists the files"),
    ("family", "one file per family crate, in that crate's own serde type: the crate versions it"),
    ("trees", "a family crate's own species type: the crate versions it"),
];

/// **Versioned kinds that are a cache**: a file of another version is stale, never damaged, and the
/// editor makes it again, so the reader takes it and the program that uses it compares the version.
const CACHES: &[(&str, &str)] = &[("destructible_bake", "a bake of another version is stale and baked again (`ops_destructible`), so nothing is lost by reading it")];

fn fixture(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/grimoire").join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// **One valid file of each versioned kind this crate parses**, at the version this build reads.
fn versioned() -> Vec<(&'static str, &'static str, String)> {
    let s = |id, file, text: String| (id, file, text);
    vec![
        s("kits", "kits.ron", fixture("kits.ron")),
        s("library", "site/library.ron", fixture("site/library.ron")),
        s("policy", "site/project.ron", fixture("site/project.ron")),
        s("rules", "rules.ron", format!("(version: {}, blocks: [])", bevy_grimoire_core::rules::RULES_VERSION)),
        s("compositions", "compositions.ron", fixture("compositions.ron")),
        s("standards", "standards.ron", Standards::default().to_ron().unwrap()),
        s("rigs", "rigs.ron", fixture("rigs.ron")),
        s("map", "maps/yard.map.ron", Map { version: MAP_VERSION, name: "yard".to_owned(), bounds: (10.0, 4.0, 10.0), ..Map::default() }.to_ron().unwrap()),
        s("notes", "maps/yard.notes.ron", Notes::default().to_ron().unwrap()),
        s("plan", "maps/yard.plan.ron", Plan::default().to_ron().unwrap()),
        s("references", "maps/yard.references.ron", References::default().to_ron().unwrap()),
        s("relations", "maps/yard.relations.ron", Relations::default().to_ron().unwrap()),
        s("data_registry", "site/data.ron", format!("(version: {}, files: [])", bevy_grimoire_core::data_files::REGISTRY_VERSION)),
        s("family_list", "family.ron", format!("(version: {}, crates: [])", bevy_grimoire_core::family::FAMILY_VERSION)),
        s("agent", "agent.ron", format!("(version: {})", bevy_grimoire_core::note_agent::AGENT_VERSION)),
        s("play", "play.ron", format!("(version: {}, command: [\"game\"])", bevy_grimoire_core::play::PLAY_VERSION)),
        s("destructible_marks", "destructible.ron", ron::to_string(&Marks { version: DESTRUCTIBLE_VERSION, pieces: vec!["site/cart".to_owned()] }).unwrap()),
        s(
            "destructible_bake",
            "destructible/site__cart.bake.ron",
            ron::to_string(&Bake { version: DESTRUCTIBLE_VERSION, key: 7, chunks: vec![Chunk { points: vec![[0.0, 0.0, 0.0]], faces: vec![vec![0, 0, 0]] }] }).unwrap(),
        ),
        // The six kinds core parses that carried no version until #202 slice 2: each is written by the
        // type itself, so the sample is what this build writes, and `version:` is the root's first field.
        s("vocab", "vocab.ron", ron::to_string(&Vocabularies::default()).unwrap()),
        s("repetition", "repetition.ron", ron::to_string(&RepetitionRules::default()).unwrap()),
        s("terrain", "site/terrain/yard.ron", ron::to_string(&Heightfield::flat((0.0, 0.0), 1.0, 2, 2, 0.0)).unwrap()),
        s("water_plan", "site/water/yard.ron", ron::to_string(&WaterPlan::default()).unwrap()),
        s(
            "water_field",
            "site/water/yard.field.ron",
            ron::to_string(&WaterField { version: WATER_FIELD_VERSION, origin: (0.0, 0.0), cell_m: 1.0, width: 2, depth: 2, depth_m: vec![0.0; 4], settled: true, steps: 1 }).unwrap(),
        ),
        s("volume", "site/volume/yard.ron", ron::to_string(&Volume::default()).unwrap()),
    ]
}

/// The span of the digits of the root struct's `version: N`, found as the first `version:` whose
/// previous token is the root's opening parenthesis (so a comment that says "version" is not it, and
/// neither is a nested struct's own).
fn version_span(text: &str) -> Option<(usize, usize)> {
    let mut from = 0;
    while let Some(found) = text[from..].find("version:") {
        let at = from + found;
        if text[..at].trim_end().ends_with('(') {
            let digits = at + "version:".len();
            let start = digits + text[digits..].len() - text[digits..].trim_start().len();
            let end = start + text[start..].chars().take_while(char::is_ascii_digit).count();
            return (end > start).then_some((start, end));
        }
        from = at + 1;
    }
    None
}

fn version_of(text: &str) -> u32 {
    let (a, b) = version_span(text).unwrap_or_else(|| panic!("no root `version:` in\n{text}"));
    text[a..b].parse().unwrap()
}

fn with_version(text: &str, version: u32) -> String {
    let (a, b) = version_span(text).unwrap_or_else(|| panic!("no root `version:` in\n{text}"));
    format!("{}{version}{}", &text[..a], &text[b..])
}

/// Whether `reason` says the number `n` as a number of its own, and not as a digit of another or of a
/// byte offset: a refusal at version 2 of a build that reads 1 must name the 1 as a word.
fn names_number(reason: &str, n: u32) -> bool {
    reason.split(|c: char| !c.is_ascii_digit()).any(|word| word == n.to_string())
}

/// **Each kind is in exactly one place**, so a new kind has to say how its format is versioned.
#[test]
fn every_kind_is_versioned_unversioned_or_owned_elsewhere() {
    let versioned: Vec<&str> = versioned().iter().map(|v| v.0).collect();
    let above: Vec<&str> = VERSIONED_ABOVE.iter().map(|a| a.0).collect();
    let unversioned: Vec<&str> = UNVERSIONED.iter().map(|u| u.0).collect();
    let elsewhere: Vec<&str> = OWNED_ELSEWHERE.iter().map(|u| u.0).collect();
    let mut seen = BTreeSet::new();
    for id in versioned.iter().chain(&above).chain(&unversioned).chain(&elsewhere) {
        assert!(seen.insert(*id), "`{id}` is listed twice: a kind is versioned, versioned above, unversioned or owned elsewhere, one of them");
    }
    let all: BTreeSet<&str> = KINDS.iter().map(|k| k.id).collect();
    assert_eq!(
        seen,
        all,
        "every kind of bevy_grimoire_core::reload::KINDS says how its format is versioned: add the new kind to `versioned()` (when its type has a `version`), to VERSIONED_ABOVE (when its type lives above this crate and has one), to UNVERSIONED (and say why not) or to OWNED_ELSEWHERE (and say whose it is)"
    );
    for (id, why) in UNVERSIONED.iter().chain(OWNED_ELSEWHERE) {
        assert!(why.split_whitespace().count() >= 4, "`{id}` says too little about why: `{why}`");
    }
    for (id, _, what) in VERSIONED_ABOVE {
        assert!(what.split_whitespace().count() >= 3, "`{id}` says too little about what the type is: `{what}`");
    }
}

/// **A kind versioned above this crate has its proof, and the proof is about that kind.** The proof is a
/// test in the layer that owns the type; this crate only checks that the file is there and names the kind
/// and the version, so an entry cannot outlive its test.
#[test]
fn a_kind_versioned_above_this_crate_names_a_proof_that_is_there() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (id, proof, _) in VERSIONED_ABOVE {
        let text = std::fs::read_to_string(root.join(proof)).unwrap_or_else(|e| panic!("`{id}`: its proof `{proof}` is not there: {e}"));
        assert!(text.contains("version"), "`{id}`: `{proof}` does not say anything about a version");
        let named = text.split(|c: char| !c.is_alphanumeric() && c != '_').any(|word| word == *id);
        assert!(named, "`{id}`: `{proof}` never names the kind, so it cannot be its proof");
    }
}

/// **The debt only shrinks.**
#[test]
fn the_unversioned_list_only_shrinks() {
    assert!(
        UNVERSIONED.len() == UNVERSIONED_MAX,
        "the checked-in unversioned list has {} kinds and the ratchet says {UNVERSIONED_MAX}: the two move together. Lower both when a kind gains a `version`; raising either is a decision a reviewer must see.",
        UNVERSIONED.len()
    );
}

/// **A file from before the first version is read or refused naming the version.** Every kind is tried and
/// every violation is reported, so one run shows the whole list.
#[test]
fn a_version_older_than_the_first_is_read_or_refused_naming_the_version() {
    let mut broken = Vec::new();
    for (id, file, text) in versioned() {
        let kind = reload::kind_by_id(id).unwrap();
        let current = version_of(&text);
        assert!(current >= 1, "`{id}`: the current version is {current}, and the first is 1");
        assert_eq!(reload_check::check(kind, file, &text), Check::Valid, "`{id}`: the sample is not valid at its own version:\n{text}");
        match reload_check::check(kind, file, &with_version(&text, 0)) {
            Check::Valid => {}
            Check::Refused(r) if r.reason.contains("version") => {}
            Check::Refused(r) => broken.push(format!("`{id}` refused version 0 without saying it is the version: {}", r.reason)),
            Check::Unchecked => panic!("`{id}` is not checked, so it cannot be a sample here"),
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

/// **A file written by a newer build is refused, and the refusal says which version this build reads**,
/// so an author knows the project came from a newer release and not that it is damaged.
#[test]
fn a_newer_version_is_refused_and_the_version_this_build_reads_is_named() {
    let mut broken = Vec::new();
    for (id, file, text) in versioned() {
        let kind = reload::kind_by_id(id).unwrap();
        let current = version_of(&text);
        let ahead = current + 1;
        let checked = reload_check::check(kind, file, &with_version(&text, ahead));
        if CACHES.iter().any(|c| c.0 == id) {
            if checked != Check::Valid {
                broken.push(format!("`{id}` is a cache and a file of version {ahead} is stale, not refused: {checked:?}"));
            }
            continue;
        }
        match checked {
            Check::Refused(r) if r.reason.contains("version") && names_number(&r.reason, current) => {}
            Check::Refused(r) => broken.push(format!(
                "`{id}` refused version {ahead} without naming the version {current} it reads: {}",
                r.reason
            )),
            other => broken.push(format!(
                "`{id}`: a file at version {ahead} was not refused (a build that reads {current} took it as its own): {other:?}"
            )),
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}
