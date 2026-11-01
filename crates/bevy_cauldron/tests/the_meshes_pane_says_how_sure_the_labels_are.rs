//! **The Meshes pane says how sure the labels are, and who wrote them (cycle 8, items 3 and 17).**
//!
//! # What the bugs were
//!
//! 1. The labels row of a mesh's detail gave the model and the date (`gemma · 2026-10-20`) and
//!    nothing about how sure the model said it was. The Tiles proposal line printed the Rust
//!    variant (`High`).
//! 2. The row was painted unchecked whenever a model had written the record, so a piece a person
//!    had confirmed in full was still flagged *"a machine decided this and you have not"*, while
//!    its per-axis chips were correctly painted checked. The pane contradicted itself.
//!
//! # What is being proved
//!
//! - **The labels row reads the confidence as a word**, first, so the word a person scans for is the
//!   part a long model name cannot push out of the panel.
//! - **The row is unchecked iff some axis is still a model's**, by the same predicate that
//!   `grimoire_list_pieces` `by_model` asks.
//! - **The date is the model's labelling date** while a model axis is open. A date is given only for
//!   a writer the record can name: a model's date, never an agent's or a person's, because `at` holds
//!   the most recent write by anyone and the row cannot say which writer that was. Once every axis is
//!   confirmed, the row says `checked <date>` instead.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use serde_json::json;

use crate::fixtures;

/// **The editor, with the bridge open**, on a kit whose one piece was labelled by `gemma-x`.
fn editor(name: &str) -> App {
    open(fixtures::Fixture::new(name).labelled_descriptor("fence", "alpha", "gemma-x", "High"))
}

/// **The editor, with the bridge open**, on a kit whose one piece carries the labels `fixture` wrote.
fn open(fixture: fixtures::Fixture) -> App {
    let root = fixture.build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    app
}

/// **Call one of the editor's BRP commands through the registered method**, as the MCP server does.
fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::RemoteMethodSystemId;
    use bevy::remote::RemoteMethods;
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("no method `{method}` is registered"),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

/// **Every drawn text and its box**, in physical pixels: (text, top y, height).
fn drawn_texts(app: &mut App) -> Vec<(String, f32, f32)> {
    for _ in 0..3 {
        app.update();
    }
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &ComputedNode, &UiGlobalTransform, &bevy::ui::Node)>();
    q.iter(world)
        .filter(|(_, node, tf, style)| {
            style.display == bevy::ui::Display::Flex
                && node.size().x > 0.0
                && node.size().y > 0.0
                && tf.translation.y.is_finite()
        })
        .map(|(t, node, tf, _)| {
            (
                t.0.clone(),
                tf.translation.y - node.size().y / 2.0,
                node.size().y,
            )
        })
        .collect()
}

/// **Ask for the kit door, select `fence` as a click on its row does, and read the labels row.**
///
/// Returns the row's text and its ink, or `None` when no labels row is drawn.
fn labels_row(app: &mut App) -> Option<(String, Color)> {
    // Ask for the kit door, for real. It opens on the Meshes tab.
    call(
        app,
        "grimoire/operation_apply",
        json!({"operation": "door_open", "parameters": {"door": "kit"}, "dry_run": false}),
    );
    for _ in 0..4 {
        app.update();
    }
    app.world_mut()
        .resource_mut::<bevy_cauldron::tiles::ImportState>()
        .selected_library_id = Some("fence".to_owned());
    drawn_texts(app);
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &TextColor)>();
    q.iter(world)
        .find(|(t, _)| t.0.contains("confidence ") || t.0.starts_with("by hand") || t.0.starts_with("agent set"))
        .map(|(t, c)| (t.0.clone(), c.0))
}

#[test]
fn the_labels_row_reads_the_confidence_as_a_word() {
    let mut app = editor("labels_say_how_sure");
    let texts_before = drawn_texts(&mut app);
    assert!(texts_before.iter().all(|(t, _, _)| !t.contains("confidence")));
    let (text, ink) = labels_row(&mut app).expect("the labels row should be drawn");

    // The confidence as a word first, then the model's file stem and the date.
    let want = "confidence high · gemma-x · 2026-10-20";
    assert_eq!(text, want, "the labels row should read {want:?}");

    // **A machine wrote it, and nobody has confirmed it.** The labels row wears the same colour
    // as a model's word elsewhere, not the author's own text colour.
    assert_eq!(
        ink,
        bevy_cauldron::chrome::UNCHECKED,
        "the model-written labels row should be drawn in the unchecked colour"
    );
}

/// **A record a person has confirmed in full is not flagged.** Every axis is `Human`, so no model
/// axis is open, and the row must not paint it unchecked — the chips beside it already say *you did*.
#[test]
fn a_record_a_person_confirmed_in_full_is_not_drawn_unchecked() {
    let mut app = open(
        fixtures::Fixture::new("labels_all_confirmed").labelled_by(
            "fence",
            "alpha",
            "(kind: Some(Human), effects: Some(Human), look: Some(Human))",
            r#"model: Some("gemma-x"), at: "2026-10-21", confidence: Some(High)"#,
        ),
    );
    let (text, ink) = labels_row(&mut app).expect("the labels row should be drawn");
    assert_eq!(
        text, "confidence high · gemma-x · checked 2026-10-21",
        "every axis is confirmed, so the row says when a person checked it, not the model's date"
    );
    assert_eq!(
        ink,
        bevy_cauldron::chrome::TEXT,
        "a fully confirmed record must not wear the model's unchecked colour"
    );
}

/// **While a model axis is open, the row gives the model's date and names who wrote each other
/// axis.** A person's or an agent's write carries no date of its own on the row.
#[test]
fn an_open_model_axis_keeps_the_models_date_and_names_who_wrote_the_rest() {
    let mut app = open(fixtures::Fixture::new("labels_mixed").labelled_by(
        "fence",
        "alpha",
        "(kind: Some(Model), look: Some(Human))",
        r#"model: Some("gemma-x"), model_at: Some("2026-10-20"), at: "2026-10-21", confidence: Some(High)"#,
    ));
    let (text, ink) = labels_row(&mut app).expect("the labels row should be drawn");
    assert_eq!(text, "confidence high · gemma-x · 2026-10-20 · by hand set look");
    assert_eq!(ink, bevy_cauldron::chrome::UNCHECKED);
}

/// **The row names the model as a person reads it.** A model stored by its full path shows its file
/// stem; the path itself stays in the record, where `grimoire/pieces` still reports it whole.
#[test]
fn the_labels_row_shows_the_models_file_stem_not_its_path() {
    let mut app = open(fixtures::Fixture::new("labels_model_path").labelled_by(
        "fence",
        "alpha",
        "(kind: Some(Model), effects: Some(Model), look: Some(Model))",
        r#"model: Some("/Users/me/models/Qwen3-VL-8B-Instruct-Q4_K_M.gguf"), at: "2026-10-20", confidence: Some(High)"#,
    ));
    let (text, _) = labels_row(&mut app).expect("the labels row should be drawn");
    assert_eq!(text, "confidence high · Qwen3-VL-8B-Instruct-Q4_K_M · 2026-10-20");
}

/// **An entity's ancestors, nearest first.**
fn ancestors(world: &World, mut e: Entity) -> Vec<Entity> {
    let mut out = Vec::new();
    while let Some(parent) = world.get::<ChildOf>(e).map(|c| c.parent()) {
        out.push(parent);
        e = parent;
    }
    out
}

/// **The labels row's laid-out box and the INSPECTOR panel's box, in physical pixels**, as
/// `(left, right)` pairs, with the panel's padding already taken off its right edge.
///
/// The panel is derived **from the row**, not from a title: the Kit door spawns **two** panels titled
/// exactly `"INSPECTOR"` — the Tiles panel (`tiles.rs`:4339) and the Anim panel (`anim_tab.rs`:343),
/// which is spawned `Display::None` and stays in the world. Looking the title up by string took
/// whichever came first in ECS order, and after a door crossing's teardown that was sometimes the
/// hidden Anim panel, so the assertion compared the row against a panel it never sat in.
fn labels_row_and_inspector_x(app: &mut App) -> ((f32, f32), (f32, f32)) {
    let world = app.world_mut();
    let find_text = |world: &mut World, want: &dyn Fn(&str) -> bool| {
        let mut q = world.query::<(Entity, &Text)>();
        q.iter(world).find(|(_, t)| want(&t.0)).map(|(e, _)| e)
    };
    let labels = find_text(world, &|t| t.contains("confidence ")).expect("the labels row is drawn");
    let panel = ancestors(world, labels)
        .into_iter()
        .find(|&a| world.get::<bevy_cauldron::chrome::Ground>(a).is_some())
        .expect("the labels row sits in a panel");
    // The panel the row is in must be the one whose title a person can read, so the two
    // `INSPECTOR` titles are told apart by having a laid-out size: zero size is this crate's
    // visibility test (`tests/headless.rs`).
    let titles: Vec<Entity> = {
        let mut q = world.query::<(Entity, &Text, &ComputedNode)>();
        q.iter(world)
            .filter(|(_, t, node)| t.0 == "INSPECTOR" && node.size() != Vec2::ZERO)
            .map(|(e, _, _)| e)
            .collect()
    };
    assert!(
        titles
            .iter()
            .any(|&e| ancestors(world, e).contains(&panel)),
        "the labels row's panel is not the visible INSPECTOR"
    );
    let x_span = |e: Entity| {
        let node = world.get::<ComputedNode>(e).expect("a laid-out node");
        let tf = world.get::<UiGlobalTransform>(e).expect("a placed node");
        let half = node.size().x / 2.0;
        (tf.translation.x - half, tf.translation.x + half)
    };
    let (panel_left, panel_right) = x_span(panel);
    let scale = 1.0 / world.get::<ComputedNode>(panel).expect("a laid-out panel").inverse_scale_factor();
    let inset = bevy_cauldron::chrome::PAD * scale;
    let (left, right) = x_span(labels);
    ((left, right), (panel_left + inset, panel_right - inset))
}

/// **A long model stem keeps the whole labels row inside the INSPECTOR, with the confidence first.**
///
/// The row's value was one no-wrap line clipped at the row's edge, so a model stored by its full
/// path with a long file stem ran its end — the confidence word — past the panel's edge, where no
/// one could read it. The row now wraps like the mesh row above it.
///
/// Failed before the fix: the labels row spanned x 97.0..557.0 px while the INSPECTOR's inner edge
/// was at 455.6 px, so the row ran about 100 px past the panel and its end was clipped.
#[test]
fn a_long_model_stem_wraps_inside_the_inspector_and_keeps_the_confidence_first() {
    let mut app = open(fixtures::Fixture::new("labels_long_stem").labelled_descriptor(
        "fence",
        "alpha",
        "/home/ladvien/models/Ternary-Bonsai-2-27B-gguf/Ternary-Bonsai-2-27B-PTQ1_0.gguf",
        "High",
    ));
    let (text, _) = labels_row(&mut app).expect("the labels row should be drawn");
    drawn_texts(&mut app);
    let ((left, right), (panel_left, panel_right)) = labels_row_and_inspector_x(&mut app);
    assert!(
        left >= panel_left && right <= panel_right,
        "the labels row spans x {left:.1}..{right:.1} px, outside the INSPECTOR's x {panel_left:.1}..{panel_right:.1} px"
    );
    assert_eq!(
        text, "confidence high · Ternary-Bonsai-2-27B-PTQ1_0 · 2026-10-20",
        "the confidence leads and the whole file stem is kept"
    );
}

/// **An agent's write on a legacy model record keeps the model's date and names the agent.**
///
/// A record written before `model_at` existed carries the model's date in `at` alone. The agent's
/// write moved `at` to today, so the row read today as the model's date and never said an agent had
/// written the axis. Failed before the fix: the row read `confidence high · gemma-x · <today>`.
#[test]
fn an_agent_write_on_a_legacy_model_record_keeps_the_models_date() {
    let mut app = open(
        fixtures::Fixture::new("labels_agent_legacy").look_tokens(&["worn"]).labelled_by(
            "fence",
            "alpha",
            "(kind: Some(Model), look: Some(Model))",
            r#"model: Some("gemma-x"), at: "2026-10-11", confidence: Some(High)"#,
        ),
    );
    let answer = call(
        &mut app,
        "grimoire/operation_apply",
        json!({"operation": "label_set", "parameters": {"id": "fence", "axis": "look", "tokens": ["worn"]}, "dry_run": false}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let (text, ink) = labels_row(&mut app).expect("the labels row should be drawn");
    assert_eq!(
        text, "confidence high · gemma-x · 2026-10-11 · agent set look",
        "the model still owns kind, so its date and name stay; the agent's look is named, not dated"
    );
    assert_eq!(ink, bevy_cauldron::chrome::UNCHECKED);
}

/// **When the model wrote no axis still standing, the row names no model.** Failed before the fix:
/// the row read `confidence high · gemma-x · <today>`, crediting the model for the agent's look.
#[test]
fn an_agent_write_that_leaves_no_model_axis_names_no_model() {
    let mut app = open(
        fixtures::Fixture::new("labels_agent_no_model_left").look_tokens(&["worn"]).labelled_by(
            "fence",
            "alpha",
            "(look: Some(Model))",
            r#"model: Some("gemma-x"), at: "2026-10-11", confidence: Some(High)"#,
        ),
    );
    let answer = call(
        &mut app,
        "grimoire/operation_apply",
        json!({"operation": "label_set", "parameters": {"id": "fence", "axis": "look", "tokens": ["worn"]}, "dry_run": false}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let (text, ink) = labels_row(&mut app).expect("the labels row should be drawn");
    assert_eq!(text, "agent set look", "no model axis is left, so no model is named");
    assert_eq!(ink, bevy_cauldron::chrome::UNCHECKED);
}

/// **An agent's write on a hand-labelled record is not called "by hand".** Failed before the fix: the
/// row read `by hand · <today>`, crediting a person with the agent's write.
#[test]
fn an_agent_write_on_a_hand_labelled_record_is_not_called_by_hand() {
    let mut app = open(
        fixtures::Fixture::new("labels_agent_by_hand").look_tokens(&["worn"]).labelled_by(
            "fence",
            "alpha",
            "(look: Some(Human))",
            r#"at: "2026-10-11""#,
        ),
    );
    let answer = call(
        &mut app,
        "grimoire/operation_apply",
        json!({"operation": "label_set", "parameters": {"id": "fence", "axis": "look", "tokens": ["worn"]}, "dry_run": false}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let (text, ink) = labels_row(&mut app).expect("the labels row should be drawn");
    assert_eq!(text, "agent set look");
    assert_eq!(ink, bevy_cauldron::chrome::UNCHECKED);
}
