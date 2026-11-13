//! **Data an earlier release wrote keeps reading** (#200 step 5, after Kleppmann on schema evolution).
//!
//! A settings struct may gain a field only if the field has a default, and may lose one only if no
//! file still needs it: a reader that meets an old file without a field it now requires refuses the
//! file, and so does one that meets a field it never heard of when its type denies unknown fields
//! (`#[serde(deny_unknown_fields)]`; a type without it reads the old file and drops the field, which
//! no test here can see). The game's copy of the project is such a file, so an edit to a type that
//! breaks them breaks every project already on disk.
//!
//! `tests/old_data/` freezes one file of every kind [`reload_check::check`] reads, **as a release
//! wrote it**, and this test reads each with the type's own reader. The first release's files
//! (`core-0.1.0/`) are the project files exactly as `bevy_emerge_core` 0.1.0, the first release, shipped
//! them, taken from its git tag; the others (`core-0.3.0/`) are what `bevy_grimoire_core` 0.3.0 writes,
//! for the kinds the first release shipped no sample of, each collection holding one element so that
//! the element types are read too. A type that no longer reads one of them is refused here, naming the
//! kind, the file, the field and the release that wrote it. The two destructible files (`editor-3f4084c/`)
//! are what the editor wrote at that commit, real files of a fracture baked from a box, before core had
//! a type for them.
//!
//! A frozen file is never edited. A type that has to change shape bumps its version, migrates the old
//! version and keeps the old file reading; the new version's file is added beside the old one.

use bevy_grimoire_migrate as migrate;
use bevy_grimoire_core::reload::{self, KINDS};
use bevy_grimoire_core::reload_check::{self, Check};

/// **`rel` with its `.ron` ending put to `.yaml`**, the spelling a project file has after the cutover.
/// A frozen file is never edited, so its path still says `.ron`; this is what compares it with a pattern.
fn written_by_an_earlier_release(rel: &str) -> String {
    match rel.rsplit_once('.') {
        Some((stem, migrate::RON_EXT)) => format!("{stem}.{}", migrate::YAML_EXT),
        _ => rel.to_owned(),
    }
}

/// One frozen file: the kind, the path the project names it by, the release that wrote it, its text.
struct Frozen {
    kind: &'static str,
    file: &'static str,
    since: &'static str,
    text: &'static str,
}

macro_rules! frozen {
    ($kind:literal, $file:literal, $since:literal, $path:literal) => {
        Frozen { kind: $kind, file: $file, since: $since, text: include_str!($path) }
    };
}

const FROZEN: &[Frozen] = &[
    frozen!("kits", "kits.yaml", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/kits.ron"),
    frozen!("vocab", "vocab.ron", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/vocab.ron"),
    frozen!("library", "scp/library.yaml", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/library.ron"),
    frozen!("library", "site/library.yaml", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/library_site.ron"),
    frozen!("policy", "site/project.yaml", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/policy.ron"),
    frozen!("policy", "furniture/project.yaml", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/policy_furniture.ron"),
    frozen!("compositions", "compositions.yaml", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/compositions.ron"),
    frozen!("rigs", "rigs.yaml", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/rigs.ron"),
    frozen!("rules", "rules.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/rules.ron"),
    frozen!("standards", "standards.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/standards.ron"),
    frozen!("repetition", "repetition.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/repetition.ron"),
    frozen!("map", "maps/yard.map.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/map.ron"),
    frozen!("notes", "maps/yard.notes.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/notes.ron"),
    frozen!("plan", "maps/yard.plan.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/plan.ron"),
    // Owner exception (2026-11-13): this row was kind `references`, which the dream board replaced (SG-03, #402). The file is still
    // the one a release wrote and still goes through the migration, which writes it as the level's board; so the kind is `board`.
    frozen!("board", "maps/yard.references.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/references.ron"),
    frozen!("relations", "maps/yard.relations.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/relations.ron"),
    frozen!("terrain", "site/terrain/yard.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/terrain.ron"),
    frozen!("water_plan", "site/water/yard.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/water_plan.ron"),
    frozen!("water_field", "site/water/yard.field.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/water_field.ron"),
    frozen!("volume", "site/volume/yard.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/volume.ron"),
    frozen!("data_registry", "site/data.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/data_registry.ron"),
    frozen!("family_list", "family.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/family_list.ron"),
    frozen!("agent", "agent.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/agent.ron"),
    frozen!("play", "play.yaml", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/play.ron"),
    frozen!("destructible_marks", "destructible.yaml", "the editor at 3f4084c", "old_data/editor-3f4084c/destructible.ron"),
    frozen!("destructible_bake", "destructible/site__cart.bake.yaml", "the editor at 3f4084c", "old_data/editor-3f4084c/site__cart.bake.ron"),
];

/// **A literal, not `FROZEN.len()`**: a number that is its own measure can never fail. Frozen files
/// are added as releases write new shapes; this floor stops one being dropped to make a type pass.
const FROZEN_MIN: usize = 26;

#[test]
fn data_an_earlier_release_wrote_still_reads() {
    let mut broken = Vec::new();
    for f in FROZEN {
        let kind = reload::kind_by_id(f.kind).unwrap_or_else(|| panic!("`{}` is not a kind of the registry", f.kind));
        // **A frozen file written before the cutover is read the way a project reads it now**: through the
        // migration, whose own check is that the YAML it writes holds the RON value (GD-03, #365). So this
        // keeps proving the property it always did — that a type still reads data an earlier release wrote —
        // and additionally that the migration does not lose it. A kind whose type the migration cannot read
        // (`checks`, `editor_settings`, a family crate's settings) still has its own reader checked.
        let (file, text) = if reads_yaml(f.kind) {
            match migrate::written_as_yaml(f.file, f.text, migrate::COMMAND) {
                Ok((file, text)) => (file, text),
                Err(e) => {
                    broken.push(format!("`{}` ({}) written by {} could not be migrated: {}", f.kind, f.file, f.since, e.line()));
                    continue;
                }
            }
        } else {
            (f.file.to_owned(), f.text.to_owned())
        };
        if let Check::Refused(r) = reload_check::check(kind, &file, &text) {
            broken.push(format!(
                "`{}` ({}) written by {} no longer reads{}: {}",
                r.kind,
                r.file,
                f.since,
                r.field.as_deref().map(|p| format!(", at field `{p}`")).unwrap_or_default(),
                r.reason
            ));
        }
    }
    assert!(
        broken.is_empty(),
        "an edit to a settings type refuses data already on disk. A field added with no default, or (for a type that denies unknown fields) a field removed that old files still carry, is refused here; give the field `#[serde(default)]`, keep reading the old one, or bump the version and migrate it (the frozen file stays):\n{}",
        broken.join("\n")
    );
}

#[test]
fn every_kind_the_check_reads_has_frozen_data_and_each_file_is_its_kinds() {
    let mut frozen: Vec<&str> = FROZEN.iter().map(|f| f.kind).collect();
    frozen.sort_unstable();
    frozen.dedup();
    let mut checked: Vec<&str> = KINDS.iter().filter(|k| !reload_check::unchecked().any(|u| u.id == k.id)).map(|k| k.id).collect();
    checked.sort_unstable();
    assert_eq!(frozen, checked, "a kind `check` reads needs a file frozen in `tests/old_data/`, and a frozen file needs a kind that is read");
    for f in FROZEN {
        let kind = reload::kind_by_id(f.kind).unwrap();
        // **The spelling the file is read in today**: a frozen file keeps the name it had, and the
        // pattern it is compared with is the kind's current one (`kits.yaml`) or the old one (`kits.ron`),
        // whichever the cutover has moved that kind to.
        // Owner exception (2026-11-13): a references file became a board, which sits in a place of the design tree, not beside the map;
        // the board kind's row names the game's own board, so that is the name it is compared with.
        let named = if f.kind == "board" {
            "design/board/board.yaml".to_owned()
        } else if reads_yaml(f.kind) {
            written_by_an_earlier_release(f.file)
        } else {
            f.file.to_owned()
        };
        assert!(
            kind.matches(&named),
            "`{}` is not a path of kind `{}`",
            f.file,
            f.kind
        );
        assert!(!f.text.trim().is_empty(), "`{}` is empty", f.file);
    }
}

#[test]
fn a_frozen_file_is_never_dropped() {
    assert!(FROZEN.len() >= FROZEN_MIN, "the list has {} files and the floor is {FROZEN_MIN}: a frozen file is never removed", FROZEN.len());
}

/// **Every kind converts now**, the editor's and a family crate's by value (`migrate::convert_value`), so every frozen file goes
/// through the migration. Owner exception (2026-11-12): this replaces `migrate::reads_yaml`, which the cutover deleted; no
/// frozen file is skipped and no assertion is weakened.
fn reads_yaml(_kind: &str) -> bool {
    true
}

/// **A trust store an earlier release wrote** (#401): the person's own file, outside any project, so it is not a kind of
/// the reload registry and `FROZEN` cannot hold it. Version 1 had no guide grants; version 2 had them, one per project and
/// digest with no path, and version 3 replaced them with a `TextGrant` per file. Both still read, and what they allowed is
/// still allowed. The projects are `/` and `/usr`, which exist on every machine, because a grant names a resolved path.
const FROZEN_STORES: &[(&str, &str, &str)] = &[
    ("trusted.yaml", "an early release (version 1)", include_str!("old_data/core-0.11.0/trusted_v1.yaml")),
    ("trusted.yaml", "bevy_grimoire_core 0.11.0 (version 2)", include_str!("old_data/core-0.11.0/trusted_v2.yaml")),
];

/// A literal floor, as `FROZEN_MIN` is: a stored shape is never dropped to make the type pass.
const FROZEN_STORES_MIN: usize = 2;

#[test]
fn a_trust_store_an_earlier_release_wrote_still_reads_and_still_allows_what_it_allowed() {
    use bevy_grimoire_core::trust::{CommandKind, CommandSpec, Decision, Scope, TrustStore};
    use std::path::Path;

    assert!(FROZEN_STORES.len() >= FROZEN_STORES_MIN, "a frozen store is never removed");
    for (n, (name, since, text)) in FROZEN_STORES.iter().enumerate() {
        let file = std::env::temp_dir().join(format!("old_data_store_{n}_{}_{name}", std::process::id()));
        std::fs::write(&file, text).expect("the frozen store is written to a scratch file");
        let store = TrustStore::load(&file).unwrap_or_else(|e| panic!("the store {since} wrote no longer reads: {e}"));
        let _ = std::fs::remove_file(&file);
        let spec = CommandSpec { kind: CommandKind::Play, program: "cargo".into(), args: vec!["run".into(), "--".into(), "{map}".into()], cwd: None };
        assert_eq!(
            store.decision(Path::new("/usr"), &spec).unwrap_or_else(|e| panic!("{e}")),
            Decision::Trusted(Scope::Project),
            "the project {since} trusted is still trusted"
        );
        if text.contains("guides:") {
            let guide = |text: &str| store.guide_decision(Path::new("/"), text).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(guide("hello"), Decision::Trusted(Scope::Command), "the guide {since} allowed is still allowed");
            assert_eq!(guide("hello!"), Decision::Untrusted, "and one changed byte still asks");
            assert_eq!(
                store.decision(Path::new("/"), &spec).unwrap_or_else(|e| panic!("{e}")),
                Decision::Trusted(Scope::Command),
                "the command {since} allowed is still allowed"
            );
        }
    }
}
