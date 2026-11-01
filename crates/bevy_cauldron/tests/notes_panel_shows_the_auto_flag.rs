//! **A note that lets the agent decide minor choices says so in the Notes panel** (E-070, #135).
//! `Note.auto` was settable over MCP and from the note box, but the panel's row looked the same as
//! any other note's, so a person could not see which notes the agent would not ask about.

use bevy::prelude::*;
use bevy_grimoire_core::notes::{self, Notes, Target};
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

#[test]
fn an_auto_note_carries_an_auto_chip_and_a_plain_note_does_not() {
    let root = fixtures::Fixture::new("notes_auto_chip").descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let path = notes::path_for_map(&app.world().resource::<OpenMap>().map_path);
    let mut n = Notes::default();
    n.add(vec![Target::Cell(0, 0)], "courtyard").unwrap();
    n.add(vec![Target::Cell(1, 1)], "a wall").unwrap();
    n.set_auto("n1", true).unwrap();
    n.save(&path).unwrap();
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        app.update();
    }
    let world = app.world_mut();
    let texts: Vec<(Entity, String, Option<Entity>, Vec2)> = world
        .query::<(Entity, &Text, Option<&ChildOf>, &ComputedNode)>()
        .iter(world)
        .map(|(e, t, p, c)| (e, t.0.clone(), p.map(|p| p.parent()), c.size()))
        .collect();
    let row_of = |line: &str| texts.iter().find(|t| t.1 == line).unwrap_or_else(|| panic!("no row {line:?} in {:?}", texts.iter().map(|t| &t.1).collect::<Vec<_>>())).2;
    let chips: Vec<_> = texts.iter().filter(|t| t.1 == "auto").collect();
    assert_eq!(chips.len(), 1, "exactly one note is auto, so exactly one chip is drawn");
    let chip = chips[0];
    // The chip's text sits in its own node; its row is that node's parent, the same row as n1's text.
    let chip_row = world.get::<ChildOf>(chip.2.expect("the chip text has a parent")).map(|p| p.parent());
    assert_eq!(chip_row, row_of("n1 [sent] courtyard"), "the chip is on the auto note's row");
    assert_ne!(chip_row, row_of("n2 [sent] a wall"), "and not on the plain note's row");
    assert!(chip.3.x > 0.0 && chip.3.y > 0.0, "the chip is laid out with a size, so it is drawn: {:?}", chip.3);
}
