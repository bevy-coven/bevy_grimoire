//! **`grimoire_guide` returns the shipped guide, then the person's guide, then the project's** (#347, AG-02).
//!
//! The shipped guide says how to work in Cauldron and can know nothing about one person's habits or one
//! project's conventions. Those are two more files: `<config_dir>/Grimoire/AGENTS.md` and
//! `<root>/.dark_factory/AGENTS.md`. `grimoire/agent_guide` is the editor's half — it holds the open project
//! and the person's directories, so the editor composes the two layers and the MCP server reads no file
//! itself.
//!
//! **A later layer adds to the earlier ones.** The test reads the answer in order and never asks for a merge:
//! a project's rule and a person's habit both arrive whole, each under a heading that names its source, so
//! an agent can tell a project rule from a shipped one.
//!
//! **A missing file is an absent layer; a file that cannot be served is a problem.** Nothing is truncated,
//! decoded lossily or guessed at: a guide an agent half-reads is worse than one it is told is missing.

use bevy::prelude::*;
use bevy_cauldron::ops_brp::OpsBridgePlugin;
use bevy_cauldron::project::Project;
use bevy_grimoire_core::agent_guide::{self, GUIDE_MAX_BYTES};
use serde_json::json;

use crate::fixtures;

/// **An editor on a fixture project, with the person's guide where a test can reach it.**
fn editor(tag: &str) -> (App, std::path::PathBuf, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let home = root.join(".user-config");
    std::fs::create_dir_all(&home).expect("the person's folder is made");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(bevy_cauldron::agent_guide::UserGuideFile(home.join(agent_guide::USER_FILE)));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(OpsBridgePlugin);
    for _ in 0..6 {
        app.update();
    }
    let project = app.world().resource::<Project>().root.clone();
    (app, home, project)
}

fn call(app: &mut App, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get("grimoire/agent_guide") {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("`grimoire/agent_guide` is not registered"),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("grimoire/agent_guide: {e:?}"))
        .unwrap_or_else(|e| panic!("grimoire/agent_guide: {e:?}"))
}

/// The answer's layers, as `(source, path, text)`.
fn layers(answer: &serde_json::Value) -> Vec<(String, String, String)> {
    answer["layers"]
        .as_array()
        .unwrap_or_else(|| panic!("the answer has no layers: {answer}"))
        .iter()
        .map(|layer| {
            (
                layer["source"].as_str().unwrap_or_default().to_owned(),
                layer["path"].as_str().unwrap_or_default().to_owned(),
                layer["text"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

/// **Both files, the person's first, each with its own path.**
#[test]
fn the_persons_guide_comes_before_the_projects_and_each_names_its_path() {
    let (mut app, home, root) = editor(&format!("guide_two_{}", std::process::id()));
    let user_file = home.join(agent_guide::USER_FILE);
    std::fs::write(&user_file, "Be brief; I read the answers on a phone.").expect("the person's guide is written");
    let project_file = root.join(agent_guide::PROJECT_PATH);
    std::fs::create_dir_all(project_file.parent().expect("it has a folder")).expect("the folder is made");
    std::fs::write(&project_file, "Walls are 2.40 m; never place a doorway against a corner.").expect("the project's guide is written");

    let answer = call(&mut app, json!({}));
    let layers = layers(&answer);
    assert_eq!(layers.len(), 2, "two files, two layers: {answer}");
    assert_eq!(layers[0].0, "user", "the person's guide is first: {answer}");
    assert_eq!(layers[0].1, user_file.display().to_string(), "and it names the file it was read from: {answer}");
    assert!(layers[0].2.contains("Be brief"), "and carries its text: {answer}");
    assert_eq!(layers[1].0, "project", "the project's guide is second: {answer}");
    assert_eq!(layers[1].1, project_file.display().to_string(), "with its own path: {answer}");
    assert!(layers[1].2.contains("Walls are 2.40 m"), "and its own text: {answer}");
    assert_eq!(answer["problems"], json!([]), "nothing was refused: {answer}");
}

/// **A file that cannot be served is left out, and the problem names the path and the reason.**
#[test]
fn a_guide_over_the_limit_and_one_that_is_not_text_are_each_left_out_naming_the_path_and_the_reason() {
    let (mut app, home, root) = editor(&format!("guide_refused_{}", std::process::id()));
    let project_file = root.join(agent_guide::PROJECT_PATH);
    std::fs::create_dir_all(project_file.parent().expect("it has a folder")).expect("the folder is made");

    std::fs::write(&project_file, vec![b'a'; GUIDE_MAX_BYTES + 1]).expect("an oversized guide is written");
    let answer = call(&mut app, json!({}));
    assert!(
        layers(&answer).iter().all(|l| l.0 != "project"),
        "a guide over the limit is left out: {answer}"
    );
    let problems = answer["problems"].as_array().unwrap_or_else(|| panic!("no problems: {answer}"));
    assert_eq!(problems.len(), 1, "one refusal: {answer}");
    let line = problems[0].to_string();
    assert!(line.contains(&project_file.display().to_string()), "the problem names the path: {line}");
    assert!(
        line.contains(&(GUIDE_MAX_BYTES + 1).to_string()),
        "and the size it is: {line}"
    );

    std::fs::write(&project_file, [0x23, 0x20, 0xff]).expect("invalid UTF-8 is written");
    let answer = call(&mut app, json!({}));
    let problems = answer["problems"].as_array().unwrap_or_else(|| panic!("no problems: {answer}"));
    assert_eq!(problems.len(), 1, "{answer}");
    let line = problems[0].to_string();
    assert!(line.contains(&project_file.display().to_string()), "the problem names the path: {line}");
    assert!(line.contains("UTF-8"), "and why it cannot be served: {line}");
    assert!(home.exists());
}

/// **No file either side: no layers, and nothing to report.**
#[test]
fn with_neither_file_the_guide_is_the_shipped_layer_alone_and_there_is_no_problem() {
    let (mut app, _home, _root) = editor(&format!("guide_none_{}", std::process::id()));
    let answer = call(&mut app, json!({}));
    assert_eq!(layers(&answer).len(), 0, "no guide is written, so the editor adds no layer: {answer}");
    assert_eq!(answer["problems"], json!([]), "and an absent file is not a refusal: {answer}");
}
