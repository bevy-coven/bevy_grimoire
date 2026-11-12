//! **The status band says when a reload refused a file or needs a restart** (UI-34, #228).
//!
//! The band's left end answers what is in hand (`the_status_band_counts_the_set_in_hand`); this is
//! the other half of the same acceptance item — *"a refused reload turns the problems item to the
//! warning kind"*. A person editing beside a running game asks two questions, and only the first
//! had an answer on screen: how many am I carrying, and did my edit take.
//!
//! Three claims, and each can fail:
//!
//! 1. **Every file applied says nothing.** A band that said "0 problems" on every frame is a row a
//!    reader stops seeing, and it is the state the editor is in almost all the time.
//! 2. **A refused file turns the item to the warning kind**, in the running skin's `accent`, which
//!    is the issue's own wording and gooey's rule for [`bevy_gooey::StatusKind::Warning`]. The editor
//!    keeps running and the previous state stays; what did not take is what the mark is for.
//! 3. **A file that reads but needs a restart is the same kind**, and a person fixing one still has
//!    to see the other, so both counts are named.
//!
//! The readout is `chrome::reload_readout`, and both the system and this file call it, so an
//! assertion here is an assertion about what is painted rather than about a resource that happens
//! to hold the same value.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_grimoire_core::live::KindVerdict;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    (root, app)
}

/// What the band currently says, and the kind it says it as, or `None` when the readout is absent.
fn said(app: &mut App) -> Option<(String, bevy_gooey::StatusKind, Color)> {
    let mut q = app.world_mut().query_filtered::<
        (&Text, &bevy_gooey::StatusKind, &TextColor),
        With<bevy_cauldron::chrome::ProblemsCount>,
    >();
    q.iter(app.world())
        .next()
        .map(|(t, k, c)| (t.0.clone(), *k, c.0))
}

fn skin(app: &App) -> bevy_gooey::GooeySkin {
    app.world()
        .get_resource::<bevy_gooey::GooeySkin>()
        .expect("the editor cannot run without a GooeySkin")
        .clone()
}

fn record(app: &mut App, verdict: KindVerdict) {
    app.world_mut()
        .resource_mut::<bevy_cauldron::live::EditorReloads>()
        .decide(vec![verdict]);
    app.update();
}

/// **Every file applied says nothing**, so the band does not carry a permanent "0 problems".
#[test]
fn a_band_with_nothing_wrong_says_nothing() {
    let (_root, mut app) = editor("problems-clean");
    record(&mut app, KindVerdict::applied("kit", "kit.yaml"));
    let (text, kind, ink) = said(&mut app).expect("the band must carry the readout even when it is empty");
    assert_eq!(
        text, "",
        "an applied file must leave the band silent; `{text}` on every edit is a receipt"
    );
    assert_eq!(
        kind,
        bevy_gooey::StatusKind::Info,
        "nothing wrong is `Info`, not a mark beside a word"
    );
    assert_eq!(ink, skin(&app).text_muted, "an empty readout is drawn in the muted ink");
}

/// **A refused file turns the item to the warning kind, in the skin's `accent`.**
#[test]
fn a_refused_file_turns_the_item_to_the_warning_kind() {
    let (_root, mut app) = editor("problems-refused");
    record(
        &mut app,
        KindVerdict::refused("kit", "kit.yaml", Some("pieces[3].yaw".to_owned()), "not a number"),
    );
    let (text, kind, ink) = said(&mut app).expect("the band must carry the readout");
    assert_eq!(
        text, "1 refused",
        "the band must say what became of the file; the journal has the detail, the band the count"
    );
    assert_eq!(
        kind,
        bevy_gooey::StatusKind::Warning,
        "a refused reload turns the problems item to the warning kind: the editor keeps running on \
         the previous state, and a person has to see that their edit did not take"
    );
    assert_eq!(
        ink,
        skin(&app).accent,
        "the ink must come from the running skin, so a theme edit reaches the band"
    );
}

/// **A file that reads but needs a restart is a warning**, and a refusal beside it outranks it.
#[test]
fn a_file_needing_a_restart_is_a_warning_under_a_refusal() {
    let (_root, mut app) = editor("problems-restart");
    record(
        &mut app,
        KindVerdict::needs_restart("rigs", "rigs.yaml", "restart to load the rigs"),
    );
    let (text, kind, ink) = said(&mut app).expect("the band must carry the readout");
    assert_eq!(text, "1 needs a restart");
    assert_eq!(
        kind,
        bevy_gooey::StatusKind::Warning,
        "a valid file the program cannot take live is a warning: nothing was lost, something waits"
    );
    assert_eq!(ink, skin(&app).accent, "a warning's ink is the skin's `accent`");

    // A refusal beside it is the graver fact, so the item says the error and names both counts.
    record(
        &mut app,
        KindVerdict::refused("kit", "kit.yaml", None, "does not parse"),
    );
    let (text, kind, ink) = said(&mut app).expect("the band must carry the readout");
    assert_eq!(
        text, "1 refused · 1 needs a restart",
        "both counts, because a person who fixes the refusal still has a restart owed"
    );
    assert_eq!(kind, bevy_gooey::StatusKind::Warning);
    assert_eq!(ink, skin(&app).accent);
}
