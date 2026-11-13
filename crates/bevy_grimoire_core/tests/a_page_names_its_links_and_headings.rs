//! **A page names its links, its pictures and its headings** (SG-04, #403): the reader the design tree and the file editor
//! share (`design::cites`, `design::headings`).
//!
//! The file editor flags a path that is not there on the path's own bytes, so what is measured here is where those bytes are,
//! what a destination resolves to, and what names no file of the project at all.

use bevy_grimoire_core::design::{cites, headings};

const PAGE: &str = "design/levels/yard/style.md";

#[test]
fn a_destination_is_found_where_it_is_written_and_resolved_against_its_own_folder() {
    let text = "# Yard\n\nSee [the notes](../notes.md#top) and ![fog](board/fog.png \"A title\") and ![a b](<board/a b.png>).\n";
    let found = cites(text, PAGE);
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(!found[0].image && found[1].image && found[2].image);
    assert_eq!(&text[found[0].range.clone()], "../notes.md#top");
    assert_eq!(found[0].file.as_deref(), Some("design/levels/notes.md"), "the fragment is not part of the file");
    assert_eq!(&text[found[1].range.clone()], "board/fog.png");
    assert_eq!(found[1].file.as_deref(), Some("design/levels/yard/board/fog.png"));
    assert_eq!(&text[found[2].range.clone()], "board/a b.png", "an angle-bracketed destination is found inside its brackets");
    assert!(found.iter().all(|c| c.line == 3), "the line is counted from 1: {found:?}");
}

#[test]
fn what_names_no_file_of_the_project_is_not_a_file() {
    let text = "[a](https://example.com/a.md) [b](mailto:x@y.z) [c](#section) [d](/abs.md) [e](../../../../out.md) [f](data:text/plain;base64,AAAA) [g](a%20b.md)\n";
    let found = cites(text, PAGE);
    let files: Vec<Option<&str>> = found.iter().map(|c| c.file.as_deref()).collect();
    assert_eq!(files, [None, None, None, None, None, None, Some("design/levels/yard/a b.md")], "{found:?}");
}

#[test]
fn the_headings_are_in_order_and_a_hash_in_a_fence_is_not_one() {
    let text = "# One\n\ntext\n\n## Two `code`\n\n```\n# not\n```\n\nThree\n=====\n";
    let said: Vec<(usize, String, usize)> = headings(text).into_iter().map(|h| (h.level, h.title, h.line)).collect();
    assert_eq!(said, [(1, "One".to_owned(), 0), (2, "Two code".to_owned(), 4), (1, "Three".to_owned(), 10)]);
}
