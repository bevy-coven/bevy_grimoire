//! **A sequence takes the same checks an agent call takes (cycle 9 gap 4).**
//!
//! `grimoire/operation_apply` refuses a `piece_select` off the Meshes shelf, applies the selection to
//! the shelf, and puts a `label_set`'s library change back into the live project. The capture
//! sequencer ran the same operation through the registry and did none of those three things: it
//! answered `applied` for a piece the shelf would refuse, left the shelf where it was, and dropped the
//! library change with its clone. These tests drive a real sequence and read what the editor shows.

use bevy::app::App;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;
use bevy_cauldron::sequencer::{Operation, SequenceParams, Sequencer};
use bevy_cauldron::tiles::{Door, ImportState, Mode};
use serde_json::{Value, json};

use crate::fixtures;

fn editor_at(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .look_tokens(&["worn"])
        .build("m");
    let mut app =
        harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn look_of(app: &App, id: &str) -> Vec<String> {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("`{id}` is not in the live measured library"))
        .look
        .clone()
}

/// Run one sequence that carries `operation` until it finishes. Returns its `operation_result`, and
/// the live look of `watch` on the first frame the result appears. That frame is read before the
/// watcher, which re-opens the project about 250 ms after a write, can bring the library back in line
/// with the disk on its own.
fn run_sequence(app: &mut App, name: &str, operation: Operation, watch: &str) -> (Value, Vec<String>) {
    let out = std::env::temp_dir().join(format!("grimoire-seq-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("the out directory is made");
    let params = SequenceParams {
        label: name.to_owned(),
        out_dir: out.to_string_lossy().into_owned(),
        seconds: 1.0,
        fps: 4,
        operation: Some(operation),
    };
    let id = app
        .world_mut()
        .resource_mut::<Sequencer>()
        .start(&params, 0.0)
        .expect("the sequence starts");

    // A headless run ends `failed` with the render-world reason; that is expected and is not under test.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut result = Value::Null;
    let mut look_at_result = Vec::new();
    while std::time::Instant::now() < deadline {
        app.update();
        let status = app.world().resource::<Sequencer>().status(id);
        if result.is_null() && !status["operation_result"].is_null() {
            result = status["operation_result"].clone();
            look_at_result = look_of(app, watch);
        }
        if matches!(
            status.get("status").and_then(|s| s.as_str()),
            Some("done") | Some("failed")
        ) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    (result, look_at_result)
}

#[test]
fn a_sequence_refuses_a_piece_the_shelf_is_not_showing() {
    let mut app = editor_at("seq_piece_refused");
    *app.world_mut().resource_mut::<Door>() = Door::Map;
    for _ in 0..4 {
        app.update();
    }
    assert_ne!(
        *app.world().resource::<Door>(),
        Door::Kit,
        "the test starts off the kit door, so the shelf is not on screen"
    );
    let (result, _) = run_sequence(
        &mut app,
        "piece_refused",
        Operation {
            name: "piece_select".to_owned(),
            parameters: json!({"id": "site/wall"}),
        },
        "site/wall",
    );
    assert_eq!(
        result["outcome"], "refused",
        "a sequence answered {result} for a piece the shelf is not showing"
    );
    assert!(
        result["refusal"]
            .as_str()
            .is_some_and(|r| r.contains("Meshes shelf")),
        "the refusal names the shelf, and says what to open: {result}"
    );
}

#[test]
fn a_sequence_selects_a_piece_on_the_shelf_as_an_agent_call_does() {
    let mut app = editor_at("seq_piece_applied");
    *app.world_mut().resource_mut::<Door>() = Door::Kit;
    for _ in 0..4 {
        app.update();
    }
    let (result, _) = run_sequence(
        &mut app,
        "piece_applied",
        Operation {
            name: "piece_select".to_owned(),
            parameters: json!({"id": "site/wall"}),
        },
        "site/wall",
    );
    assert_eq!(result["outcome"], "applied", "{result}");
    assert_eq!(
        app.world()
            .resource::<ImportState>()
            .selected_library_id
            .as_deref(),
        Some("site/wall"),
        "the shelf shows the piece the sequence selected"
    );
}

#[test]
fn a_sequence_label_is_on_the_live_library_the_map_draws_from() {
    let mut app = editor_at("seq_label_put_back");
    let (result, look_at_result) = run_sequence(
        &mut app,
        "label_put_back",
        Operation {
            name: "label_set".to_owned(),
            parameters: json!({"id": "site/wall", "axis": "look", "tokens": ["worn"]}),
        },
        "site/wall",
    );
    assert_eq!(result["outcome"], "applied", "{result}");
    assert_eq!(
        look_at_result,
        vec!["worn".to_string()],
        "the live project holds the sequence's label on the frame the sequence answered, not the one it started with"
    );
}
