//! **A rename is not a change** (#201): a placement whose id changed and whose row did not is the
//! same piece under another name, so a reload that diffs by id must say so rather than report one
//! piece removed and another added, which would throw away whatever a game hung on it.

use bevy_grimoire_core::live::diff_maps;
use bevy_grimoire_core::map::{Map, Placed};

fn row(id: &str, descriptor: &str, at: (f32, f32)) -> Placed {
    Placed { id: id.to_owned(), descriptor: descriptor.to_owned(), at, ..Placed::default() }
}

fn rider(id: &str, at: (f32, f32), host: &str) -> Placed {
    Placed { on: Some(host.to_owned()), ..row(id, "kit/crate", at) }
}

fn map(placements: Vec<Placed>) -> Map {
    Map { name: "yard".to_owned(), bounds: (100.0, 10.0, 100.0), placements, ..Map::default() }
}

fn pair(from: &str, to: &str) -> (String, String) {
    (from.to_owned(), to.to_owned())
}

#[test]
fn a_placement_with_a_new_id_and_the_same_row_is_renamed() {
    let old = map(vec![row("well", "kit/well", (1.0, 1.0)), row("crate_b", "kit/crate", (4.0, 1.0))]);
    let new = map(vec![row("well", "kit/well", (1.0, 1.0)), row("crate_beta", "kit/crate", (4.0, 1.0))]);
    let diff = diff_maps(&old, &new);
    assert_eq!(diff.renamed, vec![pair("crate_b", "crate_beta")]);
    assert!(diff.added.is_empty() && diff.removed.is_empty() && diff.changed.is_empty(), "{diff:?}");
    assert!(!diff.is_empty(), "a rename is an edit");
    assert_eq!(diff.summary(), "0 added, 0 removed, 0 changed, 1 renamed: crate_b→crate_beta");
}

#[test]
fn a_rename_that_also_changes_the_row_is_a_removal_and_an_addition() {
    let old = map(vec![row("crate_b", "kit/crate", (4.0, 1.0))]);
    for new_row in [
        row("crate_beta", "kit/crate", (6.0, 1.0)),
        row("crate_beta", "kit/table", (4.0, 1.0)),
        Placed { yaw: 90.0, ..row("crate_beta", "kit/crate", (4.0, 1.0)) },
        Placed { paint: 2, ..row("crate_beta", "kit/crate", (4.0, 1.0)) },
        Placed { lift: 0.5, ..row("crate_beta", "kit/crate", (4.0, 1.0)) },
        Placed { tip: (1, 0), ..row("crate_beta", "kit/crate", (4.0, 1.0)) },
    ] {
        let diff = diff_maps(&old, &map(vec![new_row.clone()]));
        assert!(diff.renamed.is_empty(), "{new_row:?}");
        assert_eq!((diff.added.as_slice(), diff.removed.as_slice()), (&["crate_beta".to_owned()][..], &["crate_b".to_owned()][..]));
    }
}

#[test]
fn identical_rows_are_not_guessed_between() {
    // Two stacked placements with the same row, both renamed: nothing says which is which, so a
    // game keeps neither one's state rather than swapping them.
    let old = map(vec![row("a", "kit/crate", (4.0, 1.0)), row("b", "kit/crate", (4.0, 1.0))]);
    let new = map(vec![row("c", "kit/crate", (4.0, 1.0)), row("d", "kit/crate", (4.0, 1.0))]);
    let diff = diff_maps(&old, &new);
    assert!(diff.renamed.is_empty(), "{diff:?}");
    assert_eq!((diff.added.len(), diff.removed.len()), (2, 2));

    // One removed and two added with the row of the removed one: still ambiguous.
    let diff = diff_maps(&map(vec![row("a", "kit/crate", (4.0, 1.0))]), &map(vec![row("c", "kit/crate", (4.0, 1.0)), row("d", "kit/crate", (4.0, 1.0))]));
    assert!(diff.renamed.is_empty(), "{diff:?}");
}

#[test]
fn many_renames_in_one_edit_each_follow_their_own_row() {
    let old = map((0..5).map(|i| row(&format!("crate_{i}"), "kit/crate", (i as f32 * 2.0, 1.0))).collect());
    let new = map((0..5).map(|i| row(&format!("box_{i}"), "kit/crate", (i as f32 * 2.0, 1.0))).collect());
    let diff = diff_maps(&old, &new);
    let want: Vec<_> = (0..5).map(|i| pair(&format!("crate_{i}"), &format!("box_{i}"))).collect();
    assert_eq!(diff.renamed, want);
    assert!(diff.added.is_empty() && diff.removed.is_empty() && diff.changed.is_empty());
}

#[test]
fn two_ids_that_swap_rows_are_changed_not_renamed() {
    let old = map(vec![row("a", "kit/crate", (1.0, 1.0)), row("b", "kit/crate", (4.0, 1.0))]);
    let new = map(vec![row("a", "kit/crate", (4.0, 1.0)), row("b", "kit/crate", (1.0, 1.0))]);
    let diff = diff_maps(&old, &new);
    assert!(diff.renamed.is_empty(), "both ids are still there");
    assert_eq!(diff.changed, vec!["a".to_owned(), "b".to_owned()]);
}

#[test]
fn what_rests_on_a_renamed_host_is_not_changed_by_naming_it_again() {
    let old = map(vec![row("table_a", "kit/table", (5.0, 1.0)), rider("lamp", (5.0, 1.0), "table_a")]);
    let new = map(vec![row("table_b", "kit/table", (5.0, 1.0)), rider("lamp", (5.0, 1.0), "table_b")]);
    let diff = diff_maps(&old, &new);
    assert_eq!(diff.renamed, vec![pair("table_a", "table_b")]);
    assert!(diff.changed.is_empty(), "the rider's row only names its host's new id: {diff:?}");

    // Resting on a different piece is a change.
    let moved = map(vec![row("table_b", "kit/table", (5.0, 1.0)), rider("lamp", (5.0, 1.0), "shelf")]);
    let diff = diff_maps(&old, &moved);
    assert_eq!(diff.changed, vec!["lamp".to_owned()], "{diff:?}");
}
