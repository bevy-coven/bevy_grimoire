//! **The style measurement's suite scores every line of the style once, and never says the style in a prompt** (SG-10, #409).
//!
//! `games/sample/bench/tasks/` holds three tasks and the two arms of `docs/research/2026-11-14-style-helps-an-agent.md`. This reads what a person can
//! read in those files, with no editor and no model, and fails on the mistakes a pre-registration cannot afford:
//!
//! - a line of the style that no check scores, or that two checks score, or a check that names a line the style does not state;
//! - a prompt that quotes the style, which would hand the without-style arm what its tool is withheld to keep from it;
//! - an arm that does not differ from the other by exactly the one tool and the two pages it is about.
//!
//! That the tasks *load* (an operation and a check this build knows) and that each fails undone and passes answered is what the benchmark runner proves
//! on a real editor, `cauldron-bench run --agent nothing` and `--agent oracle`, which the pre-registration states the result of. It is a runner of
//! an editor, so it is not a test of this engine-free crate; this reads the same files for what the runner cannot know.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

/// **How many consecutive words of a style line may appear in a prompt**: a prompt may say `market stalls`, and may not say a line.
const QUOTED_WORDS: usize = 4;

/// The two pages of the style, relative to the sample game's root.
const GAME_PAGE: &str = "design/style.md";
const LEVEL_PAGE: &str = "design/levels/courtyard/style.md";

/// **The suite's directory**, relative to the sample game's root: the runner looks for the arms in `<root>/bench/tasks/variants.yaml`.
const TASKS: &str = "bench/tasks";

/// **Every line the style states, by the id the tasks' comments give it**: the page, the heading, the 1-based place in that list, and a word the
/// line must carry, so an id cannot drift to another line when a page is edited.
const LINES: &[(&str, &str, &str, usize, &str)] = &[
    ("G-Do1", GAME_PAGE, "Do", 1, "1800 K"),
    ("G-Do2", GAME_PAGE, "Do", 2, "flicker"),
    ("G-Do3", GAME_PAGE, "Do", 3, "2 m and 5 m"),
    ("G-Do4", GAME_PAGE, "Do", 4, "toll"),
    ("G-Do5", GAME_PAGE, "Do", 5, "lantern_oil"),
    ("G-Do6", GAME_PAGE, "Do", 6, "awning_canvas"),
    ("G-Dn1", GAME_PAGE, "Don't", 1, "neon_sign"),
    ("G-Dn2", GAME_PAGE, "Don't", 2, "plastic_chair"),
    ("G-Dn3", GAME_PAGE, "Don't", 3, "traffic_cone"),
    ("G-Dn4", GAME_PAGE, "Don't", 4, "vending_machine"),
    ("G-Dn5", GAME_PAGE, "Don't", 5, "exclamation"),
    ("G-Dn6", GAME_PAGE, "Don't", 6, "tint"),
    ("L-Do1", LEVEL_PAGE, "Do", 1, "two lanterns"),
    ("L-Do2", LEVEL_PAGE, "Do", 2, "400 lm"),
    ("L-Do3", LEVEL_PAGE, "Do", 3, "z = 8.5"),
    ("L-Do4", LEVEL_PAGE, "Do", 4, "yaw 180"),
    ("L-Do5", LEVEL_PAGE, "Do", 5, "friend"),
    ("L-Do6", LEVEL_PAGE, "Do", 6, "dusk"),
    ("L-Dn1", LEVEL_PAGE, "Don't", 1, "6 Hz"),
    ("L-Dn2", LEVEL_PAGE, "Don't", 2, "5 lux"),
    ("L-Dn3", LEVEL_PAGE, "Don't", 3, "2.5 m"),
    ("L-Dn4", LEVEL_PAGE, "Don't", 4, "welcome"),
    ("L-Dn5", LEVEL_PAGE, "Don't", 5, "crown"),
    ("L-Dn6", LEVEL_PAGE, "Don't", 6, "pennant_bright"),
];

fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../games/sample")
}

/// **One task as the file states it**: its directory name, its text (whose comments name the line each check scores) and its parsed value.
struct TaskFile {
    id: String,
    text: String,
    value: Value,
}

/// **The three tasks**, in id order, read through the workspace's one YAML reader.
fn tasks() -> Vec<TaskFile> {
    let dir = sample().join(TASKS);
    let mut ids: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    ids.sort();
    ids.into_iter()
        .map(|id| {
            let path = dir.join(&id).join("task.yaml");
            let shown = path.display().to_string();
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{shown}: {e}"));
            let value: Value = bevy_grimoire_core::yaml::from_str(&text, &shown).unwrap_or_else(|p| panic!("{}", p.line()));
            TaskFile { id, text, value }
        })
        .collect()
}

/// **The bullets under one `## <heading>`** of a page, each without its `- `.
fn bullets(rel: &str, heading: &str) -> Vec<String> {
    let text = std::fs::read_to_string(sample().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
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

/// **The ids of the lines a task scores, in the order its checks are written, and how many checks are guards.**
fn comments_of(task: &TaskFile) -> (Vec<String>, usize) {
    let (mut ids, mut guards) = (Vec::new(), 0);
    for row in task.text.lines() {
        let Some(comment) = row.strip_prefix("  # ") else { continue };
        if let Some(rest) = comment.strip_prefix("line ") {
            ids.push(rest.split_whitespace().next().unwrap_or_default().to_owned());
        } else if comment.starts_with("guard") {
            guards += 1;
        }
    }
    (ids, guards)
}

/// **The three tasks of #409 are there**, each named for its directory.
#[test]
fn the_three_tasks_are_there() {
    let found: Vec<(String, String)> =
        tasks().into_iter().map(|t| (t.id.clone(), t.value["id"].as_str().unwrap_or_default().to_owned())).collect();
    let ids = ["dress-the-market-area", "light-the-gate-at-night", "write-the-gatekeepers-greeting"];
    assert_eq!(found, ids.map(|i| (i.to_owned(), i.to_owned())), "the suite is the three tasks of #409, each with its own directory's name for an id");
}

/// **Every line of the style is scored by exactly one check, and every check comment names a line the style states.**
#[test]
fn every_line_of_the_style_is_scored_once() {
    let mut scored: BTreeMap<String, String> = BTreeMap::new();
    for task in tasks() {
        let (ids, guards) = comments_of(&task);
        let checks = task.value["checks"].as_array().map_or(0, Vec::len);
        assert_eq!(checks, ids.len() + guards, "{}: every check is a `# line <id>` or a `# guard`, so the position of a check names its line", task.id);
        for id in ids {
            if let Some(other) = scored.insert(id.clone(), task.id.clone()) {
                panic!("line {id} is scored by {other} and by {}", task.id);
            }
        }
    }
    let wanted: BTreeSet<&str> = LINES.iter().map(|l| l.0).collect();
    let got: BTreeSet<&str> = scored.keys().map(String::as_str).collect();
    assert_eq!(got, wanted, "the lines the suite scores are the lines the style states, 24 of them, each once");
}

/// **An id names the line it says**: the nth `Do` or `Don't` bullet of its page, carrying its word.
#[test]
fn every_id_names_the_line_it_says() {
    for (id, page, heading, nth, word) in LINES {
        let list = bullets(page, heading);
        let line = list.get(nth - 1).unwrap_or_else(|| panic!("{id}: {page} has no {nth}th `{heading}` line: {list:?}"));
        assert!(line.contains(word), "{id}: the {nth}th `{heading}` line of {page} is `{line}`, which does not carry `{word}`");
    }
}

/// **A prompt gives the task and none of the style**: no run of [`QUOTED_WORDS`] words of a style line is in it, and it never says `style`.
#[test]
fn no_prompt_quotes_the_style() {
    let words = |text: &str| -> Vec<String> {
        text.to_lowercase().split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).map(str::to_owned).collect()
    };
    let shingles = |text: &str| -> BTreeSet<Vec<String>> { words(text).windows(QUOTED_WORDS).map(<[String]>::to_vec).collect() };
    for task in tasks() {
        let prompt_text = task.value["prompt"].as_str().unwrap_or_else(|| panic!("{}: no prompt", task.id)).to_owned();
        let prompt = shingles(&prompt_text);
        assert!(!words(&prompt_text).iter().any(|w| w == "style"), "{}: the prompt says `style`", task.id);
        for (id, page, heading, _, _) in LINES {
            for line in bullets(page, heading) {
                let shared: Vec<_> = shingles(&line).intersection(&prompt).cloned().collect();
                assert!(shared.is_empty(), "{}: the prompt quotes {id} (`{line}`): {shared:?}", task.id);
            }
        }
    }
}

/// **The two arms differ by the one tool and the two pages**, and by nothing else.
#[test]
fn the_arms_differ_by_the_tool_and_the_pages() {
    let path = sample().join(TASKS).join("variants.yaml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let file: Value = bevy_grimoire_core::yaml::from_str(&text, &path.display().to_string()).unwrap_or_else(|p| panic!("{}", p.line()));
    let arms = file["variants"].as_array().unwrap_or_else(|| panic!("no variants in {file}"));
    let list = |arm: &Value, key: &str| -> Vec<String> {
        arm[key].as_array().map(|v| v.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect()).unwrap_or_default()
    };
    let names: Vec<&str> = arms.iter().filter_map(|a| a["name"].as_str()).collect();
    assert_eq!(names, ["with-style", "without-style"]);
    let (with, without) = (&arms[0], &arms[1]);
    assert!(list(with, "hide_tools").is_empty(), "the with-style arm is offered every tool: {with}");
    assert_eq!(list(with, "overlay"), [GAME_PAGE, LEVEL_PAGE], "the with-style arm has the two pages on its copy");
    for page in [GAME_PAGE, LEVEL_PAGE] {
        assert!(sample().join(page).is_file(), "{page} is laid over the copy, so it has to be a file of the sample game");
    }
    assert_eq!(list(without, "hide_tools"), ["grimoire_style"], "the without-style arm is not offered `grimoire_style`");
    assert!(list(without, "overlay").is_empty(), "the without-style arm has no page on its disk: {without}");
}
