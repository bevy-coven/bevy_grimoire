//! **The animation tab takes an edit of `rigs.ron` at once: its rigs are measured again, and the rig the
//! author was on stays selected by name** (#202, the `rigs` kind leaves the restart list).
//!
//! The tab used to hold the manifest it read and say `needs_restart` about every later edit. A rig added
//! ahead of the selected one in the sorted list moves the selected rig's index, so a tab that took the
//! edit and kept the index would be on a different rig; and the reports it held described the manifest
//! before the edit, so a new rig had none until something asked. The assertions are on what the tab
//! holds and what is measured.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_cauldron::anim_tab::BenchState;
use bevy_cauldron::anim_watch::{BenchGeneration, BenchReports};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;
use bevy_grimoire_core::rigs::Rigs;

use crate::fixtures;

fn workspace_asset(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets").join(rel)
}

/// Frames until `done` holds, or panic with `what` after `secs`.
fn until(app: &mut App, secs: f64, what: &str, done: impl Fn(&App) -> bool) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < secs {
        if done(app) {
            return;
        }
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("never saw: {what}");
}

#[test]
fn a_rig_added_ahead_of_the_selected_one_is_measured_and_the_selection_stays_on_its_rig() {
    let root = fixtures::Fixture::new("rigs_edit_measured_again").descriptor("site/a", "alpha").build("m");
    let path = root.join("assets/grimoire/rigs.ron");
    std::fs::copy(workspace_asset("grimoire/rigs.ron"), &path).expect("rigs.ron");
    std::fs::create_dir_all(root.join("assets/characters")).expect("characters dir");
    std::fs::copy(workspace_asset("characters/valkyrie.glb"), root.join("assets/characters/valkyrie.glb"))
        .expect("valkyrie.glb");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    app.world_mut().insert_resource(Mode::Anim);
    for _ in 0..6 {
        app.update();
    }
    assert!(app.world().resource::<BenchState>().loaded, "the tab loaded the manifest");

    // The author is on the valkyrie, and it is measured.
    let at = app.world().resource::<BenchState>().names().iter().position(|n| *n == "valkyrie").expect("valkyrie is a rig");
    app.world_mut().resource_mut::<BenchState>().selected = at;
    until(&mut app, 30.0, "the valkyrie measured", |a| a.world().resource::<BenchReports>().by_rig.contains_key("valkyrie"));
    let generation = app.world().resource::<BenchGeneration>().0;

    // A rig that sorts before it is added on disk, by a hand or an agent.
    let had = app.world().resource::<BenchState>().names().len();
    let text = std::fs::read_to_string(&path).expect("rigs.ron");
    let mut rigs = Rigs::parse(&text).expect("the shipped manifest reads");
    let copy = rigs.rigs["valkyrie"].clone();
    rigs.rigs.insert("aardvark".to_owned(), copy);
    let edited = rigs.to_ron().expect("serialises");
    std::fs::write(&path, &edited).expect("write");

    until(&mut app, 30.0, "the tab holding the edit", |a| a.world().resource::<BenchState>().text.as_deref() == Some(edited.as_str()));
    let bench = app.world().resource::<BenchState>();
    assert_eq!(bench.names()[0], "aardvark");
    assert_eq!(bench.names()[bench.selected], "valkyrie", "the selection followed the valkyrie, not its old index {at}");
    assert_ne!(bench.selected, at, "the valkyrie's index moved, so this proves the selection followed by name");
    assert_eq!(bench.names().len(), had + 1, "the tab holds the new rig beside the {had} it had");

    // The reports were measured again: the new rig has one, which only a measurement after the edit can give.
    until(&mut app, 60.0, "the new rig and the valkyrie measured again", |a| {
        let reports = a.world().resource::<BenchReports>();
        reports.by_rig.contains_key("aardvark") && reports.by_rig.contains_key("valkyrie")
    });
    assert_ne!(app.world().resource::<BenchGeneration>().0, generation, "the reports changed under the edit");
    assert_eq!(app.world().resource::<Project>().root, root, "the project is the one the test built");
}
