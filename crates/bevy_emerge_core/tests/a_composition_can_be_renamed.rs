//! **A composition can be renamed, and every map that stamps it follows.**
//!
//! The regression tests for the 2026-10-20-06 run. What they pin:
//!
//! - **The rename edits the file and does not reformat it.** A hand-written note survives, and the
//!   rest of the file is byte-identical. This is the test that fails if the writer reaches for
//!   `to_ron`.
//! - **Every map that stamps the composition follows**, and a map that does not is **untouched** —
//!   byte for byte, not "reformatted the same way".
//! - **A name that is already taken is refused, and nothing is written.**
//! - **The members are untouched.** A rename changes a name and nothing else.
//! - **A round trip.** Rename A→B then B→A gives back the original text.

use bevy_emerge_core::composition::{rename_composition, Compositions};

/// A `compositions.ron` with a hand-written note, two compositions, and a member each.
const COMPOSITIONS: &str = r#"(
    version: 1,
    note: Some("a hand-written note that must survive a rename"),
    compositions: [
        (
            id: "site/house_a",
            envelope: Anchored,
            members: [
                (
                    id: "p00_wall",
                    body: Descriptor(
                        id: "wall",
                        tip: (0, 0),
                        on: None,
                        patch: None,
                    ),
                    at: (0.0, 0.0),
                    yaw: 0.0,
                    lift: 0.0,
                    of_fingerprint: None,
                    note: Some("the wall's own note"),
                ),
            ],
            locations: [],
            note: Some("the house's own note"),
        ),
        (
            id: "site/house_b",
            envelope: Anchored,
            members: [],
            locations: [],
            note: None,
        ),
    ],
)
"#;

/// A map with two stamps of `site/house_a` and one of `site/house_b`.
const MAP: &str = r#"(
    version: 6,
    name: "m",
    origin: (0.0, 0.0, 0.0),
    bounds: (10.0, 10.0, 10.0),
    bash: None,
    placements: [],
    stamps: [
        (
            id: "house_a@1",
            of: "site/house_a",
            at: (0.0, 0.0),
            yaw: 0.0,
            overrides: [],
            of_fingerprint: None,
            owned: false,
            owned_because: None,
            note: None,
        ),
        (
            id: "house_b@1",
            of: "site/house_b",
            at: (1.0, 1.0),
            yaw: 0.0,
            overrides: [],
            of_fingerprint: None,
            owned: false,
            owned_because: None,
            note: None,
        ),
        (
            id: "house_a@2",
            of: "site/house_a",
            at: (2.0, 2.0),
            yaw: 0.0,
            overrides: [],
            of_fingerprint: None,
            owned: false,
            owned_because: None,
            note: None,
        ),
    ],
    locations: [],
)
"#;

/// **The rename edits the file, and does not reformat it.**
///
/// The note is the proof. It is a *field*, so a serializer keeps it — but the **rest of the file
/// must be byte-identical**, and that is the part a rewrite loses. A hand-written file is the
/// author's, and a rename that reformats it is a rename that buries the change.
#[test]
fn the_rename_edits_the_file_and_does_not_reformat_it() {
    let renamed = rename_composition(COMPOSITIONS, &[("m".to_owned(), MAP.to_owned())], "site/house_a", "site/house_b2")
        .unwrap_or_else(|e| panic!("the rename must succeed: {e}"));

    // **The note survived.**
    assert!(
        renamed.compositions.contains("a hand-written note that must survive a rename"),
        "the hand-written note was lost. The file is the author's."
    );
    assert!(
        renamed.compositions.contains("the house's own note"),
        "the composition's own note was lost. A rename changes a name, not prose."
    );
    assert!(
        renamed.compositions.contains("the wall's own note"),
        "a member's note was lost. A rename does not reach the members."
    );

    // **And the rest is byte-identical.** The strongest form of "the file was not reformatted".
    let before: Vec<&str> = COMPOSITIONS.lines().collect();
    let after: Vec<&str> = renamed.compositions.lines().collect();
    assert_eq!(
        before.len(),
        after.len(),
        "the file's line count changed: {} -> {}. A rename edits one line; it does not reformat.",
        before.len(),
        after.len()
    );
    let differing: Vec<usize> = before
        .iter()
        .zip(after.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        differing,
        vec![5],
        "exactly one line should have changed — the composition's id — and {} did: {:?}",
        differing.len(),
        differing
    );
}

/// **Every map that stamps the composition follows, and the rest of the map is untouched.**
#[test]
fn every_map_that_stamps_the_composition_follows() {
    let renamed = rename_composition(COMPOSITIONS, &[("m".to_owned(), MAP.to_owned())], "site/house_a", "site/house_b2")
        .unwrap_or_else(|e| panic!("the rename must succeed: {e}"));

    let (_, map) = &renamed.maps[0];
    assert_eq!(
        map.matches("\"site/house_b2\"").count(),
        2,
        "the two stamps that named the old id must name the new one. The map names it:\n{map}"
    );
    assert_eq!(
        map.matches("\"site/house_a\"").count(),
        0,
        "a stamp still names the old id, so the map has a dangling reference and will not open."
    );
    // **The stamp of the other composition is untouched.** Counted on the *quoted* form, because
    // `site/house_b2` contains `site/house_b` and a substring count would say three.
    assert_eq!(
        map.matches("\"site/house_b\"").count(),
        1,
        "the stamp of the composition that was not renamed was changed. A rename reaches only what \
         names it."
    );

    // **And the map was edited, not rewritten.**
    let before: Vec<&str> = MAP.lines().collect();
    let after: Vec<&str> = map.lines().collect();
    assert_eq!(
        before.len(),
        after.len(),
        "the map's line count changed: {} -> {}. A rename edits one line per stamp; it does not \
         reformat.",
        before.len(),
        after.len()
    );
}

/// **A map that does not stamp the composition is left alone, byte for byte.**
#[test]
fn a_map_that_does_not_stamp_the_composition_is_untouched() {
    let other = MAP.replace("site/house_a", "site/something_else");
    let renamed = rename_composition(COMPOSITIONS, &[("m".to_owned(), other.clone())], "site/house_a", "site/house_b2")
        .unwrap_or_else(|e| panic!("the rename must succeed: {e}"));
    assert_eq!(
        renamed.maps[0].1, other,
        "a map that stamps nothing was rewritten. It had nothing to follow, so it must be byte for \
         byte what it was."
    );
}

/// **A name that is already taken is refused, and nothing is written.**
#[test]
fn a_name_that_is_already_taken_is_refused() {
    let refused = rename_composition(COMPOSITIONS, &[], "site/house_a", "site/house_b");
    let said = refused.expect_err("`site/house_b` is already a composition, so the rename must be refused");
    assert!(
        said.contains("site/house_b"),
        "the refusal must name the composition that is in the way, so the author knows what to \
         choose instead. It said `{said}`"
    );
}

/// **A name that is not a legal id is refused.**
#[test]
fn a_name_that_is_not_a_legal_id_is_refused() {
    for bad in ["", "Site/House", "site/house_", "site/__house", "site/house name"] {
        let refused = rename_composition(COMPOSITIONS, &[], "site/house_a", bad);
        assert!(
            refused.is_err(),
            "`{bad}` is not a usable id and the rename must be refused"
        );
    }
}

/// **A composition that does not exist is refused.**
#[test]
fn a_composition_that_does_not_exist_is_refused() {
    let refused = rename_composition(COMPOSITIONS, &[], "site/house_z", "site/house_b2");
    assert!(
        refused.is_err(),
        "renaming a composition that is not there must be refused, not silently accepted"
    );
}

/// **The members are untouched.** A rename changes a name and nothing else.
#[test]
fn the_members_are_untouched() {
    let renamed = rename_composition(COMPOSITIONS, &[], "site/house_a", "site/house_b2")
        .unwrap_or_else(|e| panic!("the rename must succeed: {e}"));
    let set = Compositions::parse(&renamed.compositions)
        .unwrap_or_else(|e| panic!("the renamed file must still parse: {e}"));
    let comp = set
        .compositions
        .iter()
        .find(|c| c.id == "site/house_b2")
        .expect("the renamed composition is there");
    assert_eq!(comp.members.len(), 1, "the members are still there");
    assert_eq!(comp.members[0].id, "p00_wall", "the member's id did not change");
    assert_eq!(
        comp.members[0].note.as_deref(),
        Some("the wall's own note"),
        "the member's note did not change"
    );
}

/// **A round trip.** Rename A→B then B→A gives back the original text.
///
/// The metamorphic property: **the rename is reversible**, which is what "it only edits the name"
/// means when it is stated as a test.
#[test]
fn the_rename_is_reversible() {
    let there = rename_composition(COMPOSITIONS, &[("m".to_owned(), MAP.to_owned())], "site/house_a", "site/house_b2")
        .unwrap_or_else(|e| panic!("the rename must succeed: {e}"));
    let back = rename_composition(
        &there.compositions,
        &there.maps,
        "site/house_b2",
        "site/house_a",
    )
    .unwrap_or_else(|e| panic!("the rename back must succeed: {e}"));
    assert_eq!(
        back.compositions, COMPOSITIONS,
        "renaming there and back did not give back the file. A rename edits the name and nothing \
         else, so it is reversible."
    );
    assert_eq!(
        back.maps[0].1, MAP,
        "renaming there and back did not give back the map."
    );
}

/// **The renamed file still parses, and the composition is under its new name.**
#[test]
fn the_renamed_file_still_parses() {
    let renamed = rename_composition(COMPOSITIONS, &[], "site/house_a", "site/house_b2")
        .unwrap_or_else(|e| panic!("the rename must succeed: {e}"));
    let set = Compositions::parse(&renamed.compositions)
        .unwrap_or_else(|e| panic!("the renamed file must still parse: {e}"));
    assert!(
        set.compositions.iter().any(|c| c.id == "site/house_b2"),
        "the composition is not under its new name"
    );
    assert!(
        !set.compositions.iter().any(|c| c.id == "site/house_a"),
        "the composition is still under its old name, so the rename did not rename it"
    );
}
