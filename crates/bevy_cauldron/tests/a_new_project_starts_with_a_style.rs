//! **A new project starts with a style, and each topic starts from a template** (SG-08, #407): the wizard writes `design/style.md` with
//! one H1 and nine H2s in a fixed order, each holding one italic prompt line; a topic of the game starts from its own template; a lower
//! place is offered only the game's topics; and a project the editor did not create gets no file.

use bevy_cauldron::design_templates::{self, STYLE_H2S, TOPICS};
use bevy_cauldron::new_project::{self, NewProject, Template};
use bevy_grimoire_core::design::{self, Tree};

use crate::fixtures;

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("a_new_project_starts_with_a_style_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    dir
}

/// The lines under each H2 of `text`, up to the next heading, with blanks dropped.
fn under_each_h2(text: &str) -> Vec<(String, Vec<String>)> {
    let headings = design::headings(text);
    let lines: Vec<&str> = text.lines().collect();
    headings
        .iter()
        .enumerate()
        .filter(|(_, h)| h.level == 2)
        .map(|(i, h)| {
            let end = headings.get(i + 1).map_or(lines.len(), |n| n.line);
            (h.title.clone(), lines[h.line + 1..end].iter().filter(|l| !l.trim().is_empty()).map(|l| (*l).to_owned()).collect())
        })
        .collect()
}

#[test]
fn the_wizard_writes_a_style_with_nine_h2s_in_order_each_holding_one_italic_line() {
    let place = scratch("wizard");
    let dir = new_project::create(&NewProject { name: "Salt Harbour".to_owned(), location: place.to_string_lossy().into_owned(), template: Template::Empty })
        .unwrap_or_else(|e| panic!("the wizard refused: {e}"));
    let text = std::fs::read_to_string(dir.join("design/style.md")).expect("the new project has design/style.md");

    let headings = design::headings(&text);
    let h1: Vec<&str> = headings.iter().filter(|h| h.level == 1).map(|h| h.title.as_str()).collect();
    assert_eq!(h1, ["Salt Harbour"], "one H1, the place's name");
    assert_eq!(headings.iter().filter(|h| h.level == 2).map(|h| h.title.as_str()).collect::<Vec<_>>(), STYLE_H2S, "the nine H2s, in order");
    assert_eq!(headings.len(), 1 + STYLE_H2S.len(), "and no other heading");
    for (title, body) in under_each_h2(&text) {
        assert_eq!(body.len(), 1, "`{title}` holds one line: {body:?}");
        assert!(body[0].starts_with('*') && body[0].ends_with('*') && !body[0].starts_with("**"), "`{title}`'s line is italic prompt text: {}", body[0]);
    }
    assert!(text.contains("never varnished") && text.contains("0.10 m"), "Do and Don't ask for statements a screenshot can confirm or refute");
    assert!(text.contains("what to take from it"), "Pictures asks for alt text that says what to take");

    // The file is the game's `style` topic, and the tree reads it with nothing wrong.
    let tree = Tree::read(&dir);
    assert_eq!(tree.place("design").and_then(|p| p.topic("style")).map(|t| t.file.as_str()), Some("design/style.md"));
    assert!(tree.problems.is_empty(), "{:?}", tree.problems.iter().map(|p| p.line()).collect::<Vec<_>>());
    assert!(!text.contains("{place}"), "the place token is filled");
    std::fs::remove_dir_all(&place).unwrap();
}

#[test]
fn every_template_has_one_h1_and_short_h2s_with_one_italic_line_each() {
    for (name, _) in TOPICS {
        let text = design_templates::topic_text(name, "the yard").unwrap_or_else(|| panic!("`{name}` has no template"));
        let headings = design::headings(&text);
        assert_eq!(headings.iter().filter(|h| h.level == 1).count(), 1, "{name}: one H1");
        assert!(headings.iter().any(|h| h.level == 2), "{name}: it has H2s");
        assert!(headings.iter().all(|h| h.level <= 2), "{name}: a fixed, shallow shape");
        for (title, body) in under_each_h2(&text) {
            assert_eq!(body.len(), 1, "{name}: `{title}` holds one line");
            assert!(body[0].starts_with('*') && body[0].ends_with('*'), "{name}: `{title}`: {}", body[0]);
        }
        assert!(!text.contains("{place}"), "{name}: the token is filled");
    }
    assert_eq!(TOPICS.map(|(n, _)| n), ["dialogue", "interaction", "sound", "ui", "names"], "the five topics, named in one place");
}

#[test]
fn a_new_topic_of_the_game_is_written_from_its_template_and_a_lower_place_is_offered_only_the_games() {
    let root = scratch("topics");
    std::fs::create_dir_all(root.join("design/levels/yard")).unwrap();
    // The game offers every topic it does not have yet.
    assert_eq!(design_templates::topics_offered(&Tree::read(&root), "design"), ["dialogue", "interaction", "sound", "ui", "names"]);
    let file = design_templates::create_topic(&root, "design", "dialogue", "Salt Harbour").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(file, root.join("design/dialogue.md"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), design_templates::topic_text("dialogue", "Salt Harbour").unwrap());
    assert!(design_templates::create_topic(&root, "design", "dialogue", "Salt Harbour").is_err(), "a topic that is there is never overwritten");
    assert!(design_templates::create_topic(&root, "design", "weather", "Salt Harbour").is_err(), "the game offers the five templates and no other");

    // A level is offered the game's topics, not the five.
    std::fs::write(root.join("design/sound.md"), "# Salt Harbour: sound\n").unwrap();
    let tree = Tree::read(&root);
    assert_eq!(design_templates::topics_offered(&tree, "design"), ["interaction", "ui", "names"]);
    assert_eq!(design_templates::topics_offered(&tree, "design/levels/yard"), ["dialogue", "sound"], "only the game's topics");
    design_templates::create_topic(&root, "design/levels/yard", "sound", "yard").unwrap_or_else(|e| panic!("{e}"));
    assert!(root.join("design/levels/yard/sound.md").is_file());
    assert_eq!(design_templates::topics_offered(&Tree::read(&root), "design/levels/yard"), ["dialogue"], "and not one it has");
    assert!(design_templates::create_topic(&root, "design/levels/yard", "ui", "yard").is_err(), "a topic the game does not have is refused below it");
    assert!(design_templates::create_topic(&root, "assets/grimoire", "dialogue", "x").is_err(), "not a place");
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn create_style_writes_the_template_once_and_opening_a_project_writes_no_file() {
    let root = fixtures::Fixture::new(&format!("a_style_existing_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let files = |root: &std::path::Path| -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    out.push(path.strip_prefix(root).unwrap().to_string_lossy().into_owned());
                }
            }
        }
        out.sort();
        out
    };
    let before = files(&root);
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    let after: Vec<String> = files(&root).into_iter().filter(|f| !f.starts_with(".dark_factory/") && !f.ends_with("oplog.jsonl")).collect();
    let before: Vec<String> = before.into_iter().filter(|f| !f.starts_with(".dark_factory/")).collect();
    assert!(!root.join("design").exists(), "opening a project the editor did not make writes no design folder");
    assert_eq!(before, after, "and no other file");

    // Create style in the empty Design workspace is the one act that writes it, for the game's name.
    let file = design_templates::write_style(&root, "design", "Yard Game").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(file, root.join("design/style.md"));
    assert!(std::fs::read_to_string(&file).unwrap().starts_with("# Yard Game\n"));
    assert!(design_templates::write_style(&root, "design", "Yard Game").is_err(), "an existing style is never overwritten");
}
