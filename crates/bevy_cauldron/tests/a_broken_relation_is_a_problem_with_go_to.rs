//! **PROBLEMS lists each broken relation as a row, and Go to selects the pieces it names** (UI-54, #240).
//!
//! The RELATIONS row says how many relations the map breaks and the first one in words. This is the rest: the inspector's PROBLEMS
//! section has a row for every unmet or missing relation, worded as the row is, with a Go to button that selects the pieces the
//! relation names (the one it is about first) and frames them. With nothing broken it says `no problems`, and draws no button.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_cauldron::harness;
use bevy_cauldron::problems_view::{ProblemGoTo, ProblemsList};
use bevy_cauldron::selection::Selection;

use crate::fixtures;

/// Every placement faces +z (yaw 0). The chair at (2, 2) has its table behind it at (2, 0); the crate at (8, 8.5) stands in the
/// door's swing at (8, 8).
fn editor(name: &str, relations: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/chair", "alpha")
        .descriptor("site/table", "alpha")
        .descriptor("site/door", "alpha")
        .descriptor("site/crate", "alpha")
        .place_as("chair", "site/chair", (2.0, 2.0))
        .place_as("table", "site/table", (2.0, 0.0))
        .place_as("door", "site/door", (8.0, 8.0))
        .place_as("crate", "site/crate", (8.0, 8.5))
        .build("m");
    let file = root.join("assets/grimoire/maps/m.relations.yaml");
    std::fs::write(&file, format!("version: 1\nrelations: [{relations}]\n")).unwrap_or_else(|e| panic!("{file:?}: {e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        app.update();
    }
    app
}

fn problem_texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let lists: Vec<Entity> = world.query_filtered::<Entity, With<ProblemsList>>().iter(world).collect();
    let all: Vec<(Entity, String)> = world.query::<(Entity, &Text)>().iter(world).map(|(e, t)| (e, t.0.clone())).collect();
    all.into_iter()
        .filter(|(entity, _)| {
            let mut at = Some(*entity);
            while let Some(current) = at {
                if lists.contains(&current) {
                    return true;
                }
                at = world.get::<ChildOf>(current).map(ChildOf::parent);
            }
            false
        })
        .map(|(_, text)| text)
        .collect()
}

fn go_to_buttons(app: &mut App) -> Vec<(Entity, Vec<String>)> {
    let world = app.world_mut();
    world.query::<(Entity, &ProblemGoTo)>().iter(world).map(|(e, g)| (e, g.pieces.clone())).collect()
}

/// **Four relations, in the file's own shape**: `kind` is a tagged value, and each variant's fields sit under
/// the tag. The list is written bare, so `format!` puts it inside `relations: [ … ]`.
const BROKEN: &str = r#"{ of: chair, kind: !Faces table },
  { of: chair, kind: !Near { to: table, within: 3.0 } },
  { of: door, kind: !ClearInFront { depth: 2.0, width: 1.0 } },
  { of: lamp, kind: !Near { to: door, within: 2.0 } }"#;

#[test]
fn each_broken_relation_is_a_row_in_the_words_of_the_relations_row() {
    let mut app = editor(&format!("problems_rows_{}", std::process::id()), BROKEN);
    let texts = problem_texts(&mut app);
    assert!(texts.contains(&"chair faces table".to_owned()), "an unmet relation is a row: {texts:?}");
    assert!(texts.contains(&"door clear in front (crate in the way)".to_owned()), "an unmet relation names what is in the way: {texts:?}");
    assert!(texts.contains(&"lamp is not on the map".to_owned()), "a relation naming a missing piece is a row: {texts:?}");
    assert!(!texts.iter().any(|t| t.contains("near table")), "a relation that holds is not a problem: {texts:?}");
}

#[test]
fn go_to_selects_the_pieces_the_relation_names_the_one_it_is_about_first() {
    let mut app = editor(&format!("problems_go_to_{}", std::process::id()), BROKEN);
    let buttons = go_to_buttons(&mut app);
    let (button, _) = buttons
        .iter()
        .find(|(_, pieces)| pieces == &["chair".to_owned(), "table".to_owned()])
        .unwrap_or_else(|| panic!("no Go to names chair and table: {buttons:?}"));
    app.world_mut().commands().trigger(Activate { entity: *button });
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().resource::<Selection>().ids(), ["chair", "table"]);
}

#[test]
fn a_map_whose_relations_all_hold_has_no_problems_and_no_go_to() {
    let mut app = editor(&format!("problems_none_{}", std::process::id()), "{ of: chair, kind: !Near { to: table, within: 3.0 } }");
    assert_eq!(problem_texts(&mut app), ["no problems"]);
    assert!(go_to_buttons(&mut app).is_empty());
}
