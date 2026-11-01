//! **A project's `family/bevy_gooey.ron` is the running UI skin, and edits reach it live** (#147).

use bevy::prelude::*;
use bevy_grimoire_mapper::editor::EditorState;
use bevy_gooey::GooeySkin;

use crate::fixtures;

const BLUE: &str = "(accent: Srgba((red: 0.2, green: 0.6, blue: 0.9, alpha: 1.0)))";

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

#[test]
fn a_partial_skin_file_changes_only_its_field_and_an_unreadable_one_keeps_the_last_good_skin() {
    let root = fixtures::Fixture::new(&format!("gooey_skin_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_gooey\"])").unwrap();
    let file = root.join("family/bevy_gooey.ron");
    std::fs::write(&file, BLUE).unwrap();
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        bevy_grimoire_mapper::family_host::problems_with(&root, &bevy_grimoire_mapper::family_host::built()).is_empty(),
        "bevy_gooey is a known, built family crate"
    );

    let blue = Color::srgba(0.2, 0.6, 0.9, 1.0);
    run_until(&mut app, |w| w.get_resource::<GooeySkin>().is_some_and(|s| s.accent == blue));
    let skin = app.world().resource::<GooeySkin>().clone();
    assert_eq!(skin.accent, blue, "the file's accent is the running skin's");
    assert_eq!(GooeySkin { accent: GooeySkin::default().accent, ..skin.clone() }, GooeySkin::default(), "every other field is the default");
    assert!(problems(app.world()).is_empty(), "a good skin raises nothing: {:?}", problems(app.world()));

    // Valid RON, but not a skin: a problem, and the blue skin stays.
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&file, "(accent: 5)").unwrap();
    run_until(&mut app, |w| !problems(w).is_empty());
    let probs = problems(app.world());
    assert!(probs.iter().any(|p| p.contains("bevy_gooey") && p.contains("previous skin stays")), "{probs:?}");
    assert_eq!(app.world().resource::<GooeySkin>().accent, blue);

    // Not RON at all: the host's own problem, and still the blue skin.
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&file, "(accent: Srgba((red: ").unwrap();
    let before = problems(app.world()).len();
    run_until(&mut app, |w| problems(w).len() > before);
    let probs = problems(app.world());
    assert!(probs.iter().any(|p| p.contains("bevy_gooey") && p.contains("not valid RON")), "{probs:?}");
    assert_eq!(app.world().resource::<GooeySkin>().accent, blue);

    // A skin the crate's own rules refuse is applied, and each broken rule is a problem.
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&file, "(min_target: 10.0)").unwrap();
    run_until(&mut app, |w| w.resource::<GooeySkin>().min_target == 10.0);
    assert_eq!(app.world().resource::<GooeySkin>().min_target, 10.0);
    let probs = problems(app.world());
    assert!(probs.iter().any(|p| p.contains("bevy_gooey skin: min_target is 10 px")), "{probs:?}");

    // The file removed: the crate's default skin is back.
    std::fs::remove_file(&file).unwrap();
    run_until(&mut app, |w| *w.resource::<GooeySkin>() == GooeySkin::default());
    assert_eq!(*app.world().resource::<GooeySkin>(), GooeySkin::default());
}
