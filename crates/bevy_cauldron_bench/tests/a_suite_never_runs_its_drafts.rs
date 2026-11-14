//! **A task draft is never a task of the suite until a person moves it there** (BM-07, #417).
//!
//! A hand-back made into a draft lands in `<suite>/drafts/<id>/task.yaml`, in the same folder the game's suite is
//! read from. If the loader read it, a draft nobody reviewed would be run, scored and counted, and one that does
//! not read would refuse the whole suite. So the loader skips `drafts/`, and the move is a file move: one folder up.

use bevy_cauldron_bench::Suite;

/// **A scratch path this test owns**, named with the test's name and the process id.
fn scratch(test: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bench-drafts-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn write(path: &std::path::Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{}: {e}", parent.display()));
    }
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

const MAP: &str = "version: 9\nname: house\nnote: null\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 4.0\n- 3.0\n- 4.0\nbash: null\nplacements: []\n";

fn task(id: &str) -> String {
    format!("id: {id}\ntitle: {id}\nsplit: dev\nmap: house\nprompt: ask\nchecks:\n  - project_unchanged: {{}}\n")
}

/// **A draft beside the tasks is not one of them**, whether or not it reads.
#[test]
fn a_draft_is_not_a_task_of_its_suite() {
    let root = scratch("skipped");
    write(&root.join("assets/grimoire/maps/house.map.yaml"), MAP);
    let suite = root.join(".dark_factory/bench");
    write(&suite.join("real-task/task.yaml"), &task("real-task"));
    write(&suite.join("drafts/present-1-a-draft/task.yaml"), "this: [is not, a task\n");
    let loaded = Suite::load(&root, &suite, &[]).unwrap_or_else(|p| panic!("the suite loads with a draft beside it: {}", p.line()));
    let ids: Vec<&str> = loaded.tasks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["real-task"], "only the task is in the suite");
    let _ = std::fs::remove_dir_all(&root);
}

/// **Moving a draft up one folder makes it a task**, which is the person's act and the only one.
#[test]
fn a_moved_draft_is_a_task() {
    let root = scratch("moved");
    write(&root.join("assets/grimoire/maps/house.map.yaml"), MAP);
    let suite = root.join(".dark_factory/bench");
    write(&suite.join("drafts/a-draft/task.yaml"), &task("a-draft"));
    assert!(
        Suite::load(&root, &suite, &[]).is_err(),
        "a suite with only a draft holds no task, and says so"
    );
    std::fs::rename(suite.join("drafts/a-draft"), suite.join("a-draft")).unwrap_or_else(|e| panic!("{e}"));
    let loaded = Suite::load(&root, &suite, &[]).unwrap_or_else(|p| panic!("the moved draft loads: {}", p.line()));
    assert_eq!(loaded.tasks.len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}
