//! **The sample game's style states checkable Do and Don't lines** (SG-10, #409; the series #399).
//!
//! The measurement of whether `grimoire_style` helps an agent (`docs/research/2026-11-14-style-helps-an-agent.md`) scores a run
//! against the lines of the sample game's style, so the style has to exist, read without a problem, and state at least
//! [`LINES_MIN_PER_KIND`] Do and as many Don't lines in each of its two files: the game's and the courtyard level's. A line is one
//! bullet on one row that starts with its word, so a run can be scored against it by a check and a person can read it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use bevy_grimoire_core::design::{self, PlaceKind};
use bevy_grimoire_core::style::{self, FileState, Request};
use bevy_grimoire_core::trust::{Decision, Scope};

/// **The fewest Do lines, and the fewest Don't lines, each of the two files states** (#409: "at least six").
const LINES_MIN_PER_KIND: usize = 6;

/// **The level the measurement runs on**: the sample game's one map.
const LEVEL: &str = "courtyard";

/// The page of the whole game, and the page of the level, relative to the sample game's root.
const GAME_PAGE: &str = "design/style.md";
const LEVEL_PAGE: &str = "design/levels/courtyard/style.md";

fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../games/sample")
}

/// **The bullets under one `## <heading>`**, each without its `- `, in the order the page states them.
fn bullets(text: &str, heading: &str) -> Vec<String> {
    let mut inside = false;
    let mut out = Vec::new();
    for row in text.lines() {
        if let Some(name) = row.strip_prefix("## ") {
            inside = name.trim() == heading;
        } else if inside && let Some(line) = row.strip_prefix("- ") {
            out.push(line.trim().to_owned());
        }
    }
    out
}

fn page(tree: &design::Tree, rel: &str) -> String {
    tree.text(rel).unwrap_or_else(|| panic!("{rel} did not read: {:?}", tree.problems)).to_owned()
}

/// **The tree reads whole**: the two pages are there, and the tree states no problem.
#[test]
fn the_sample_games_design_tree_reads_without_a_problem() {
    let tree = design::Tree::read(&sample());
    assert!(tree.problems.is_empty(), "the sample game's design tree has problems: {:?}", tree.problems);
    let game = tree.place("design").expect("the game is a place");
    assert!(game.topic(design::STYLE_TOPIC).is_some(), "the game has a style.md: {game:?}");
    let level = tree
        .places
        .iter()
        .find(|p| p.kind == PlaceKind::Level && p.map.as_deref() == Some(LEVEL))
        .unwrap_or_else(|| panic!("the courtyard has a level of its own: {:?}", tree.places));
    assert!(level.topic(design::STYLE_TOPIC).is_some(), "the level has a style.md: {level:?}");
}

/// **Each page states at least six Do lines and six Don't lines**, each one row, each starting with its word and ending in a full stop.
#[test]
fn each_page_states_at_least_six_do_and_six_dont_lines() {
    let tree = design::Tree::read(&sample());
    for rel in [GAME_PAGE, LEVEL_PAGE] {
        let text = page(&tree, rel);
        for (heading, word) in [("Do", "Do "), ("Don't", "Don't ")] {
            let lines = bullets(&text, heading);
            assert!(lines.len() >= LINES_MIN_PER_KIND, "{rel} states {} `{heading}` lines, and the measurement needs {LINES_MIN_PER_KIND}: {lines:?}", lines.len());
            for line in &lines {
                assert!(line.starts_with(word), "{rel}: `{line}` is under `## {heading}` and does not start with `{}`", word.trim());
                assert!(line.ends_with('.'), "{rel}: `{line}` does not end in a full stop, so it is not one statement");
            }
            let distinct: BTreeSet<&String> = lines.iter().collect();
            assert_eq!(distinct.len(), lines.len(), "{rel}: a `{heading}` line is stated twice: {lines:?}");
        }
    }
}

/// **`grimoire_style` for the courtyard serves the game's page and then the level's**, both allowed, with every line in the answer.
#[test]
fn the_courtyards_chain_serves_the_game_and_then_the_level() {
    let tree = design::Tree::read(&sample());
    let request = Request { map: Some(LEVEL.to_owned()), at: None, topic: None };
    let mut allow = |_: &str, _: &str| Ok(Decision::Trusted(Scope::Project));
    let answer = style::compose(&tree, &request, &mut allow).unwrap_or_else(|e| panic!("the chain was refused: {e}"));
    let served: Vec<(&str, FileState)> = answer.files.iter().map(|f| (f.path.as_str(), f.state)).collect();
    assert_eq!(served, vec![(GAME_PAGE, FileState::Served), (LEVEL_PAGE, FileState::Served)], "game first and the level last: {answer:?}");
    for rel in [GAME_PAGE, LEVEL_PAGE] {
        let text = page(&tree, rel);
        for heading in ["Do", "Don't"] {
            for line in bullets(&text, heading) {
                assert!(answer.text.contains(&line), "the answer does not carry `{line}`");
            }
        }
    }
}
