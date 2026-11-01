//! **A format that gains a `version` keeps reading the files written before it, and every reader refuses a
//! file of another one, not only `parse`** (#202, H5 step 4, slice 2).
//!
//! Six formats this crate parses carried no version: `vocab.ron`, `repetition.ron`, a terrain file, a water
//! plan, a water field and a volume. Each has one now (`bevy_grimoire_core::format_version`), and two
//! things must hold for every one:
//!
//! - **A file with no `version` is the first version**, so the projects already on disk (frozen in
//!   `tests/old_data/`) read unchanged, and what the type writes carries the version it was missing.
//! - **The refusal is in the type, not in its `parse`.** An editor operation reads these files with a plain
//!   `ron::from_str` and writes them back; a refusal that only `parse` raised left it writing a newer file
//!   at this build's version (what `destructible.ron` did until slice 1). Here the plain reader is the
//!   one tried, and `reload_check::read`, the reader the editor's views use, must name the `version` field.

use bevy_grimoire_core::heightfield::Heightfield;
use bevy_grimoire_core::reload_check;
use bevy_grimoire_core::repetition::RepetitionRules;
use bevy_grimoire_core::vocab::Vocabularies;
use bevy_grimoire_core::volume::Volume;
use bevy_grimoire_core::water::{WATER_FIELD_VERSION, WaterField, WaterPlan};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

fn prove<T: Serialize + DeserializeOwned + Debug + PartialEq>(id: &'static str, sample: T, version: fn(&T) -> u32) {
    let written = ron::to_string(&sample).unwrap();
    assert!(written.starts_with("(version:1,"), "`{id}`: what the type writes starts with its version:\n{written}");
    assert_eq!(version(&sample), 1, "`{id}`: the type's own value is at version 1");

    // A file an earlier release wrote has no `version`: it is the first one, and the value is the same.
    let before_versions = written.replacen("version:1,", "", 1);
    let read: T = ron::from_str(&before_versions).unwrap_or_else(|e| panic!("`{id}`: a file with no version is refused: {e}\n{before_versions}"));
    assert_eq!(version(&read), 1, "`{id}`: a file with no version reads as the first");
    assert_eq!(read, sample, "`{id}`: and as the same value");

    // A plain reader, the one an operation uses, refuses a newer file and a version zero, naming the one this build reads.
    for other in [2u32, 0, 7] {
        let text = written.replacen("version:1,", &format!("version:{other},"), 1);
        let said = ron::from_str::<T>(&text).expect_err(&format!("`{id}`: version {other} was read")).to_string();
        assert!(said.contains(&format!("version {other} is not the 1 this build reads")), "`{id}` at version {other}: {said}");

        // The reader the editor's views use says which field.
        let refusal = reload_check::read::<T>(id, "a/file.ron", &text).expect_err("refused");
        assert_eq!(refusal.field.as_deref(), Some("version"), "`{id}` at version {other}: {refusal:?}");
        assert_eq!(refusal.kind, id);
    }
}

#[test]
fn every_format_that_gained_a_version_reads_the_files_from_before_it_and_refuses_another_at_every_reader() {
    prove("vocab", Vocabularies::default(), |v| v.version);
    prove("repetition", RepetitionRules::default(), |v| v.version);
    prove("terrain", Heightfield::flat((1.0, 2.0), 0.5, 3, 2, 4.0), |v| v.version);
    prove("water_plan", WaterPlan::default(), |v| v.version);
    prove(
        "water_field",
        WaterField { version: WATER_FIELD_VERSION, origin: (0.0, 0.0), cell_m: 1.0, width: 2, depth: 2, depth_m: vec![0.0, 0.5, 0.0, 0.0], settled: true, steps: 3 },
        |v| v.version,
    );
    prove("volume", Volume::default(), |v| v.version);
}

/// **`parse` refuses too**, with the type's own prefix, so the editor's status line and the game's verdict
/// say the same sentence (`Standards::parse` and the kinds that had a version already do).
#[test]
fn parse_names_the_file_kind_and_the_version_this_build_reads() {
    for (what, said) in [
        ("vocab", Vocabularies::parse("(version: 2, kind: [], effects: [], look: [], surfaces: [])").map(drop)),
        ("repetition", RepetitionRules::parse("(version: 2)").map(drop)),
        ("terrain", Heightfield::parse("(version: 2, origin: (0.0, 0.0), cell_m: 1.0, width: 2, depth: 2, heights: [0.0, 0.0, 0.0, 0.0])").map(drop)),
        ("water plan", WaterPlan::parse("(version: 2)").map(drop)),
        ("water field", WaterField::parse("(version: 2, origin: (0.0, 0.0), cell_m: 1.0, width: 2, depth: 2, depth_m: [0.0, 0.0, 0.0, 0.0], settled: true, steps: 1)").map(drop)),
        ("volume", Volume::parse("(version: 2)").map(drop)),
    ] {
        let said = said.expect_err(what);
        assert!(said.starts_with(&format!("{what}: ")) && said.contains("version 2 is not the 1 this build reads"), "{what}: {said}");
    }
}
