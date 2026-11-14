//! **A string in a project file is checked by the words it holds** (SG-10, #409).
//!
//! The style measurement scores a greeting a run wrote. A check on the hand-back's words would score what the agent *said about* the
//! greeting, which differs between an agent that was told the forbidden words and one that was not; so `file_value` reads the saved leaf
//! and asks whether the string `contains` or `lacks` a word, without case. Each case states a passing project and the one change that fails
//! it, so a check that said `true` for everything would be caught by the second half.

use bevy_cauldron_bench::check::{grade, Check};
use bevy_cauldron_bench::{Handback, Snapshot};

/// **A project this test owns**, named with the test's name and the process id, holding one data file.
fn project(test: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("grimoire-bench-string-leaf-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let dir = root.join("assets/grimoire/data");
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    std::fs::write(
        dir.join("gatekeeper.yaml"),
        "gatekeeper:\n  greeting:\n    text: Evening, Friend. The toll is two coins.\n  coins: 2\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let map = root.join("assets/grimoire/maps");
    std::fs::create_dir_all(&map).unwrap_or_else(|e| panic!("{}: {e}", map.display()));
    std::fs::write(
        map.join("m.map.yaml"),
        "version: 9\nname: m\nnote: null\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 8.0\n- 3.0\n- 8.0\nbash: null\nplacements: []\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    root
}

/// **The verdict of one check, written the way a task writes it**, on a project, against itself as the setup.
fn verdict(root: &std::path::Path, text: &str) -> bevy_cauldron_bench::Verdict {
    let check = bevy_grimoire_core::yaml::from_str::<Check>(text, "the test's check")
        .unwrap_or_else(|p| panic!("the check `{text}` reads: {}", p.line()));
    let snap = Snapshot::read(root, "m", Vec::<Handback>::new(), Default::default());
    grade(&check, &snap, &snap)
}

const LEAF: &str = "file: data/gatekeeper.yaml, path: gatekeeper.greeting.text";

/// **`contains` holds when the saved string has the word, whatever its case, and says what it found when it has not.**
#[test]
fn a_string_leaf_contains_a_word_without_case() {
    let root = project("contains");
    for word in ["toll", "friend", "FRIEND"] {
        let v = verdict(&root, &format!("file_value: {{{LEAF}, contains: {word}}}"));
        assert!(v.passed, "the greeting contains `{word}`: {}", v.measured);
    }
    let v = verdict(&root, &format!("file_value: {{{LEAF}, contains: dusk}}"));
    assert!(!v.passed, "the greeting does not contain `dusk`: {}", v.measured);
    assert!(v.measured.contains("Evening"), "the failure says the string it found: {}", v.measured);
    let _ = std::fs::remove_dir_all(&root);
}

/// **`lacks` holds when the saved string has not the word, and fails when it has.**
#[test]
fn a_string_leaf_lacks_a_word_without_case() {
    let root = project("lacks");
    for word in ["welcome", "crown", "!"] {
        let v = verdict(&root, &format!("file_value: {{{LEAF}, lacks: \"{word}\"}}"));
        assert!(v.passed, "the greeting lacks `{word}`: {}", v.measured);
    }
    for word in ["toll", "TOLL", "Evening"] {
        let v = verdict(&root, &format!("file_value: {{{LEAF}, lacks: {word}}}"));
        assert!(!v.passed, "the greeting has `{word}`, so it does not lack it: {}", v.measured);
        assert!(v.measured.contains("lack"), "the failure says what was asked: {}", v.measured);
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// **A leaf that is not a string, a path that is not there and an empty word are each a failure that says so**, never a pass.
#[test]
fn a_string_check_refuses_what_it_cannot_read() {
    let root = project("refuses");
    let v = verdict(&root, "file_value: {file: data/gatekeeper.yaml, path: gatekeeper.coins, contains: \"2\"}");
    assert!(!v.passed && v.measured.contains("not a string"), "a number is not a string: {}", v.measured);
    let v = verdict(&root, "file_value: {file: data/gatekeeper.yaml, path: gatekeeper.farewell, lacks: welcome}");
    assert!(!v.passed, "nothing is at that path, so it lacks nothing: {}", v.measured);
    let v = verdict(&root, &format!("file_value: {{{LEAF}, contains: \"\"}}"));
    assert!(!v.passed && v.measured.contains("empty"), "an empty word is in every string, so it is refused: {}", v.measured);
    let v = verdict(&root, &format!("file_value: {{{LEAF}, lacks: \"\"}}"));
    assert!(!v.passed && v.measured.contains("empty"), "an empty word is in every string, so it is refused: {}", v.measured);
    let _ = std::fs::remove_dir_all(&root);
}

/// **`contains` and `lacks` are two questions of one leaf**, and both have to hold.
#[test]
fn contains_and_lacks_both_have_to_hold() {
    let root = project("both");
    let v = verdict(&root, &format!("file_value: {{{LEAF}, contains: toll, lacks: crown}}"));
    assert!(v.passed, "{}", v.measured);
    let v = verdict(&root, &format!("file_value: {{{LEAF}, contains: toll, lacks: friend}}"));
    assert!(!v.passed, "{}", v.measured);
    let _ = std::fs::remove_dir_all(&root);
}
