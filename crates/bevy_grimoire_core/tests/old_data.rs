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

use bevy_grimoire_core::reload::{self, KINDS};
use bevy_grimoire_core::reload_check::{self, Check};

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
    frozen!("kits", "kits.ron", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/kits.ron"),
    frozen!("vocab", "vocab.ron", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/vocab.ron"),
    frozen!("library", "scp/library.ron", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/library.ron"),
    frozen!("library", "site/library.ron", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/library_site.ron"),
    frozen!("policy", "site/project.ron", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/policy.ron"),
    frozen!("policy", "furniture/project.ron", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/policy_furniture.ron"),
    frozen!("compositions", "compositions.ron", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/compositions.ron"),
    frozen!("rigs", "rigs.ron", "bevy_emerge_core 0.1.0", "old_data/core-0.1.0/rigs.ron"),
    frozen!("rules", "rules.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/rules.ron"),
    frozen!("standards", "standards.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/standards.ron"),
    frozen!("repetition", "repetition.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/repetition.ron"),
    frozen!("map", "maps/yard.map.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/map.ron"),
    frozen!("notes", "maps/yard.notes.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/notes.ron"),
    frozen!("plan", "maps/yard.plan.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/plan.ron"),
    frozen!("references", "maps/yard.references.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/references.ron"),
    frozen!("relations", "maps/yard.relations.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/relations.ron"),
    frozen!("terrain", "site/terrain/yard.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/terrain.ron"),
    frozen!("water_plan", "site/water/yard.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/water_plan.ron"),
    frozen!("water_field", "site/water/yard.field.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/water_field.ron"),
    frozen!("volume", "site/volume/yard.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/volume.ron"),
    frozen!("data_registry", "site/data.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/data_registry.ron"),
    frozen!("family_list", "family.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/family_list.ron"),
    frozen!("agent", "agent.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/agent.ron"),
    frozen!("play", "play.ron", "bevy_grimoire_core 0.3.0", "old_data/core-0.3.0/play.ron"),
    frozen!("destructible_marks", "destructible.ron", "the editor at 3f4084c", "old_data/editor-3f4084c/destructible.ron"),
    frozen!("destructible_bake", "destructible/site__cart.bake.ron", "the editor at 3f4084c", "old_data/editor-3f4084c/site__cart.bake.ron"),
];

/// **A literal, not `FROZEN.len()`**: a number that is its own measure can never fail. Frozen files
/// are added as releases write new shapes; this floor stops one being dropped to make a type pass.
const FROZEN_MIN: usize = 26;

#[test]
fn data_an_earlier_release_wrote_still_reads() {
    let mut broken = Vec::new();
    for f in FROZEN {
        let kind = reload::kind_by_id(f.kind).unwrap_or_else(|| panic!("`{}` is not a kind of the registry", f.kind));
        if let Check::Refused(r) = reload_check::check(kind, f.file, f.text) {
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
        assert!(kind.matches(f.file), "`{}` is not a path of kind `{}`", f.file, f.kind);
        assert!(!f.text.trim().is_empty(), "`{}` is empty", f.file);
    }
}

#[test]
fn a_frozen_file_is_never_dropped() {
    assert!(FROZEN.len() >= FROZEN_MIN, "the list has {} files and the floor is {FROZEN_MIN}: a frozen file is never removed", FROZEN.len());
}
