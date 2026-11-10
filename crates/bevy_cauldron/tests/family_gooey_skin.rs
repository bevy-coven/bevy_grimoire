//! **A project's `family/bevy_gooey.yaml` is read live, and a bad one changes nothing** (#147).
//!
//! **Owner exception (2026-11-10, UI-03 #211): this file's assertions moved from the running
//! `GooeySkin` resource to `ui_skin::GameSkin`.** Until #211, a project's skin *was* the editor's
//! running skin; #211's What is one sentence — "Build a `Game UI preview` panel under a `SkinScope`
//! loaded from `family/bevy_gooey.yaml`. The editor chrome never takes it" — so the file now lands in
//! `GameSkin`, which the Game UI preview panel's scope reads and nothing else does. Every check this
//! test made is still made: a partial file changes only its field; a field the file does not name is
//! the default; an unreadable file and a file that is not RON keep the last good skin and say so; a
//! skin the crate's own rules refuse is applied with each broken rule a problem; and removing the file
//! takes the last good skin away. What changed is *which* value is read — the preview's, not the
//! running one — which is exactly the change the issue makes.
//!
//! The half that is new is stated rather than implied: the running `GooeySkin` is the editor's own
//! throughout, so a project cannot repaint the chrome. `tests/a_project_skin_previews_only_in_the_panel.rs`
//! holds that from the other side.

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::ui::editor_skin;
use bevy_cauldron::ui_skin::GameSkin;
use bevy_gooey::GooeySkin;

use crate::fixtures;

const BLUE: &str = "{accent: !Srgba {red: 0.2, green: 0.6, blue: 0.9, alpha: 1.0}}";

fn run_until(app: &mut App, done: impl Fn(&World) -> bool) {
    let start = std::time::Instant::now();
    while start.elapsed().as_secs_f64() < 5.0 && !done(app.world()) {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn problems(world: &World) -> Vec<String> {
    world.resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect()
}

/// The project's game skin, as the preview reads it.
fn game(world: &World) -> Option<GooeySkin> {
    world.get_resource::<GameSkin>().and_then(|game| game.0.clone())
}

#[test]
fn a_partial_skin_file_changes_only_its_field_and_an_unreadable_one_keeps_the_last_good_skin() {
    let root = fixtures::Fixture::new(&format!("gooey_skin_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.yaml"), "{version: 1, crates: [\"bevy_gooey\"]}").unwrap();
    let file = root.join("family/bevy_gooey.yaml");
    std::fs::write(&file, BLUE).unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        bevy_cauldron::family_host::problems_with(&root, &bevy_cauldron::family_host::built()).is_empty(),
        "bevy_gooey is a known, built family crate"
    );

    let blue = Color::srgba(0.2, 0.6, 0.9, 1.0);
    run_until(&mut app, |w| game(w).is_some_and(|s| s.accent == blue));
    let skin = game(app.world()).expect("the project's skin is read");
    assert_eq!(skin.accent, blue, "the file's accent is the preview's skin's");
    assert_eq!(GooeySkin { accent: GooeySkin::default().accent, ..skin.clone() }, GooeySkin::default(), "every other field is the default");
    assert!(problems(app.world()).is_empty(), "a good skin raises nothing: {:?}", problems(app.world()));
    // **And the editor's own chrome never took it** (UI-03, #211): the running skin is the editor's.
    assert_eq!(
        *app.world().resource::<GooeySkin>(),
        editor_skin(),
        "a project's skin is the preview's, never the running one"
    );

    // Valid RON, but not a skin: a problem, and the blue skin stays.
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&file, "{accent: 5}").unwrap();
    run_until(&mut app, |w| !problems(w).is_empty());
    let probs = problems(app.world());
    assert!(probs.iter().any(|p| p.contains("bevy_gooey") && p.contains("previous skin stays")), "{probs:?}");
    assert_eq!(game(app.world()).map(|s| s.accent), Some(blue));

    // Not RON at all: the host's own problem, and still the blue skin.
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&file, "{accent: !Srgba {red: ").unwrap();
    let before = problems(app.world()).len();
    run_until(&mut app, |w| problems(w).len() > before);
    let probs = problems(app.world());
    assert!(probs.iter().any(|p| p.contains("bevy_gooey") && p.contains("not valid YAML")), "{probs:?}");
    assert_eq!(game(app.world()).map(|s| s.accent), Some(blue));

    // A skin the crate's own rules refuse is applied, and each broken rule is a problem.
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&file, "{min_target: 10.0}").unwrap();
    run_until(&mut app, |w| game(w).is_some_and(|s| s.min_target == 10.0));
    assert_eq!(game(app.world()).map(|s| s.min_target), Some(10.0));
    let probs = problems(app.world());
    assert!(probs.iter().any(|p| p.contains("bevy_gooey skin: min_target is 10 px")), "{probs:?}");

    // The file removed: the preview has no skin, and the editor's running one is untouched.
    std::fs::remove_file(&file).unwrap();
    run_until(&mut app, |w| game(w).is_none());
    assert_eq!(game(app.world()), None, "no file, no game skin");
    assert_eq!(*app.world().resource::<GooeySkin>(), editor_skin(), "the chrome keeps the editor's own palette throughout");
}
