//! **A shows answer names the file the operation writes, and an area is a rectangle**
//! (#357's review, M1 and M2).
//!
//! `Operation::shows` is what sends a person's view to the change an agent made. So the answer has
//! to be the *write's own* place: `trigger_set` keeps a map's triggers in
//! `<grimoire_dir>/scripts/<map>.yaml` and `tree_edit` keeps a tree in
//! `<grimoire_dir>/trees/<name>.tree.yaml` — the authoring kit's folder, which is
//! `assets/grimoire/site` in a project whose kit is `site`, never `assets/grimoire` itself.
//!
//! A hard-coded `assets/grimoire/scripts/<map>.yaml` sends the person — and the follow (#358) — to a
//! file nothing writes, and in the shipped project it is also the wrong folder: the kits sit in
//! `assets/grimoire/<kit>/`.
//!
//! `layout_propose`'s schema takes its `area` corners in either order, and [`Shown::Map`] promises
//! `x0 <= x1` and `z0 <= z1`. A rectangle handed over sideways is the same area, and the frame code
//! refuses an inverted box — so a proposal asked for bottom-right-first named a box that could
//! never be framed.
//!
//! # What is proved here
//!
//! 1. **`trigger_set` and `tree_edit` name the file each of them writes**, read off the project the
//!    operation itself would write in — the kit's directory, with the kit's own name in the path,
//!    which is what a hard-coded `assets/grimoire/...` cannot produce.
//! 2. **Both answers are files, not folders**: a `map` or a `name` the operation would accept always
//!    names one file, and only a parameter it would refuse names nothing.
//! 3. **`layout_propose` names a rectangle in `x0 <= x1`, `z0 <= z1` order whatever order its
//!    corners arrive in**, for every one of the four ways round.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Shown};
use bevy_cauldron::project::Project;

use crate::fixtures;

/// **A project whose authoring kit is `site`**, so the kit's own directory is in the path and a
/// hard-coded `assets/grimoire` is a different string.
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .kit("site", "site", &["site/wall"])
        .descriptor("furniture/chair", "alpha")
        .place("furniture/chair", (0.0, 0.0))
        .build("m")
}

fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **The context `shows` is asked in**, built the way `grimoire/operation_preview` builds it: the
/// editor's own project and open map, and no view.
fn context(app: &App) -> ops::Ctx {
    ops::Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(app.world().resource::<bevy_cauldron::project::OpenMap>().clone()),
        session: "test".to_owned(),
        ..ops::Ctx::default()
    }
}

fn shown(app: &App, operation: &str, params: serde_json::Value) -> Shown {
    let op = ops::find(operation).unwrap_or_else(|| panic!("`{operation}` is a registered operation"));
    op.shows(&context(app), &params)
}

fn path_of(shown: &Shown, what: &str) -> String {
    match shown {
        Shown::File { path, .. } => path.clone(),
        other => panic!("{what} names a file, and answered {other:?}"),
    }
}

/// **1. `trigger_set` names the file it writes**, under the authoring kit's own folder.
#[test]
fn a_trigger_set_names_the_file_it_writes() {
    let root = project(&format!("shows_triggers_{}", std::process::id()));
    let app = editor(&root);
    let kit_dir = app.world().resource::<Project>().grimoire_dir.clone();
    let relative = kit_dir
        .strip_prefix(&root)
        .expect("the kit is inside the project")
        .to_string_lossy()
        .replace('\\', "/");

    let named = path_of(&shown(&app, "trigger_set", serde_json::json!({"map": "m"})), "a trigger");
    assert_eq!(
        named,
        format!("{relative}/scripts/m.yaml"),
        "`trigger_set` names where its change is seen and must name the file it writes: the \
         authoring kit's own `scripts/m.yaml`. It answered `{named}`."
    );
    // **A file, never the folder** — and with no `map`, nothing a person can look at at all.
    assert!(
        named.ends_with(".yaml"),
        "a trigger names one file, and `{named}` is a folder"
    );
    assert_eq!(
        shown(&app, "trigger_set", serde_json::json!({})),
        Shown::MapWhole,
        "`trigger_set` with no `map` has no file to name, so it names no place"
    );
    assert_eq!(
        shown(&app, "trigger_set", serde_json::json!({"map": "a/b"})),
        Shown::MapWhole,
        "a `map` the operation itself refuses names nothing, rather than a path nothing writes"
    );
}

/// **1b. `tree_edit` names the file it writes**, under the same folder.
#[test]
fn a_tree_edit_names_the_file_it_writes() {
    let root = project(&format!("shows_trees_{}", std::process::id()));
    let app = editor(&root);
    let kit_dir = app.world().resource::<Project>().grimoire_dir.clone();
    let relative = kit_dir
        .strip_prefix(&root)
        .expect("the kit is inside the project")
        .to_string_lossy()
        .replace('\\', "/");

    let named = path_of(&shown(&app, "tree_edit", serde_json::json!({"name": "oak"})), "a tree");
    assert_eq!(
        named,
        format!("{relative}/trees/oak.tree.yaml"),
        "`tree_edit` names where its change is seen and must name the file it writes: the \
         authoring kit's own `trees/oak.tree.yaml`. It answered `{named}`."
    );
    assert_eq!(
        shown(&app, "tree_edit", serde_json::json!({"name": "a/b"})),
        Shown::MapWhole,
        "a `name` the operation itself refuses names nothing, rather than a path nothing writes"
    );
}

/// **2. `layout_propose` names a rectangle whatever order its corners arrive in.**
#[test]
fn a_proposed_area_names_a_rectangle_whichever_corner_comes_first() {
    let root = project(&format!("shows_area_{}", std::process::id()));
    let app = editor(&root);

    // Four ways round: bottom-left first, bottom-right first, top-right first, top-left first.
    let corners = [(2.0, 3.0), (18.0, 3.0), (18.0, 17.0), (2.0, 17.0)];
    let want = [2.0, 3.0, 18.0, 17.0];
    for start in 0..4 {
        let area = [
            corners[start].0,
            corners[start].1,
            corners[(start + 2) % 4].0,
            corners[(start + 2) % 4].1,
        ];
        let answer = shown(
            &app,
            "layout_propose",
            serde_json::json!({"source": "learned", "area": area}),
        );
        let Shown::Map { rect, .. } = answer else {
            panic!("a proposal with an area names a place on the map, and answered {answer:?}");
        };
        assert_eq!(
            rect, want,
            "an area given as {area:?} is the same rectangle as {want:?}, and a `Shown::Map` \
             promises `x0 <= x1` and `z0 <= z1`: the frame code refuses an inverted box"
        );
        assert!(
            rect[0] <= rect[2] && rect[1] <= rect[3],
            "the answer {rect:?} is an inverted rectangle"
        );
    }
}
