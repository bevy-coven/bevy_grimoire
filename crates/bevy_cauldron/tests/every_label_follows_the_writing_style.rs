//! **Every label follows the writing style** (UI-07, #215; `docs/ui/vision.md` §9): the command
//! registry's titles are the words every menu, the palette, the toolbar and MCP show, so a title
//! that abbreviates, contracts or names an implementation detail is refused here once.

use bevy_cauldron::commands;

/// The abbreviations a label never uses, with the word it spells out. `docs/ui/vision.md` §9 names
/// the same list, and a test below fails when the two drift.
const ABBREVIATIONS: &[(&str, &str)] = &[
    ("verts", "vertices"),
    ("loc", "location"),
    ("rot", "rotation"),
    ("fac", "face"),
    ("obj", "object"),
    ("cfg", "configuration"),
    ("info", "details"),
];

/// Words from the code a person never needs to read in a label.
const IMPLEMENTATION_TERMS: &[&str] = &["ron", "entity", "struct"];

/// The words of a title, lower-cased, with the punctuation a title carries stripped.
fn words(title: &str) -> Vec<String> {
    title
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// What is wrong with one label, in the words a reviewer would use. Empty is the rule.
fn faults(label: &str) -> Vec<String> {
    let mut faults = Vec::new();
    for word in words(label) {
        if let Some((_, full)) = ABBREVIATIONS.iter().find(|(abbr, _)| *abbr == word) {
            faults.push(format!("`{word}` is an abbreviation; write `{full}`"));
        }
        if IMPLEMENTATION_TERMS.contains(&word.as_str()) {
            faults.push(format!("`{word}` is an implementation term; say what it is for"));
        }
        if word.contains('\'') && !word.ends_with("'s") {
            faults.push(format!("`{word}` is a contraction"));
        }
    }
    faults
}

#[test]
fn no_command_title_abbreviates_contracts_or_names_an_implementation_term() {
    let bad: Vec<String> = commands::all()
        .iter()
        .flat_map(|c| faults(c.title).into_iter().map(move |f| format!("`{}` (\"{}\"): {f}", c.id, c.title)))
        .collect();
    assert!(bad.is_empty(), "{} title(s) break the writing style:\n  {}", bad.len(), bad.join("\n  "));
}

#[test]
fn the_check_refuses_each_word_on_the_list_and_lets_plain_words_through() {
    for (abbr, full) in ABBREVIATIONS {
        assert_eq!(faults(&format!("Show the {abbr}")).len(), 1, "`{abbr}` is not refused");
        assert_eq!(faults(&format!("Show the {}", abbr.to_uppercase())).len(), 1, "`{abbr}` is refused only in lower case");
        assert!(faults(&format!("Show the {full}")).is_empty(), "`{full}` is the plain word and is refused");
    }
    // A word that merely contains an abbreviation is not one: `location`, `information`, `factory`.
    assert!(faults("Move to the location, then the factory").is_empty());
    assert_eq!(faults("Don't show the entity"), vec!["`don't` is a contraction", "`entity` is an implementation term; say what it is for"]);
    assert!(faults("Show this session's errors").is_empty(), "a possessive is not a contraction");
}

#[test]
fn the_refused_list_is_in_the_doc() {
    let doc = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/ui/vision.md")).expect("docs/ui/vision.md");
    let style = doc.split("## 9. Writing style").nth(1).and_then(|s| s.split("\n## ").next()).expect("§9 Writing style");
    for (abbr, _) in ABBREVIATIONS {
        assert!(style.contains(&format!("`{abbr}`")), "§9 does not name the refused `{abbr}`");
    }
}
