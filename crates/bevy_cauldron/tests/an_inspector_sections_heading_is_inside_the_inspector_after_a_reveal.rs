//! **A followed inspector section's heading is inside the inspector's visible area** (AF-03, #359).
//!
//! `Shown::Settings` names three kinds of section; the ones the Map door's inspector draws (`notes`, `problems`, `data`, and a
//! family crate's settings) are headings of one long scroll, and a reveal brings the heading on screen. `the_reveal_selects_a_row_a_person_can_see`
//! measures the pack's switch, the note's row and the data leaf; **no test measured the heading itself**, which is what "the
//! section is on screen" means for these. Each is asked for the way a person's Go to anything asks, and measured from the
//! heading's own `ComputedNode` against the inspector's scrolled box.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use bevy_cauldron::editor::MapControlsPane;
use bevy_cauldron::harness;
use bevy_cauldron::ops::Shown;
use bevy_cauldron::reveal::{InspectorSection, Revealing, Section};

use crate::fixtures;

const FRAME: f32 = 1.0 / 60.0;
const SETTLE: usize = 60;

fn project(tag: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("lamp_tall", "alpha").descriptor("bed", "alpha").place("bed", (2.0, 2.0)).side(64.0).build("m");
    let kit = root.join("assets/grimoire/furniture");
    std::fs::write(kit.join("mob.yaml"), "# tuning\nmob:\n  speed: 1.50\n  hp: 7\n").expect("mob.yaml");
    std::fs::write(kit.join("data.yaml"), "{version: 1, files: [{id: \"tuning/mob\", path: \"mob.yaml\", bounds: {\"mob.speed\": {min: 0.0, max: 9.0}}}]}").expect("data.yaml");
    root
}

fn centre_and_half(app: &App, e: Entity) -> Option<(Vec2, Vec2)> {
    let node = app.world().get::<ComputedNode>(e)?;
    let at = app.world().get::<UiGlobalTransform>(e)?;
    Some((at.translation, node.size() * 0.5))
}

fn heading_of(app: &mut App, want: InspectorSection) -> Option<Entity> {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &Section)>();
    q.iter(world).find(|(_, s)| s.0 == want).map(|(e, _)| e)
}

#[test]
fn each_inspector_section_a_reveal_opens_has_its_heading_inside_the_inspector() {
    let root = project("section_headings");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    for _ in 0..8 {
        app.update();
    }
    for (name, section) in [("problems", InspectorSection::Problems), ("data", InspectorSection::Data), ("notes", InspectorSection::Notes), ("problems", InspectorSection::Problems)] {
        app.world_mut().resource_mut::<Revealing>().request_by_person("show_section", &Shown::Settings { section: name.to_owned() });
        for _ in 0..SETTLE {
            app.update();
        }
        let heading = heading_of(&mut app, section).unwrap_or_else(|| panic!("the inspector draws no `{name}` heading"));
        let pane = {
            let world = app.world_mut();
            world.query_filtered::<Entity, With<MapControlsPane>>().single(world).expect("the inspector pane")
        };
        let (mid, half) = centre_and_half(&app, heading).unwrap_or_else(|| panic!("the `{name}` heading is not laid out"));
        let (pane_mid, pane_half) = centre_and_half(&app, pane).expect("the pane is laid out");
        assert!(half.y > 0.0 && pane_half.y > 0.0, "{name}: a zero-height node");
        let (top, bottom) = (pane_mid.y - pane_half.y, pane_mid.y + pane_half.y);
        let (h_top, h_bottom) = (mid.y - half.y, mid.y + half.y);
        assert!(h_top >= top && h_bottom <= bottom, "the `{name}` heading spans {h_top:.1}..{h_bottom:.1} and the inspector {top:.1}..{bottom:.1}");
    }
}
