//! **An agent's change off the map is followed too** (AF-03, #359; series agent follow, #356).
//!
//! AF-02 (#358) followed a change **on the map**: the camera eased to it, and a change to a library
//! piece, a composition, a rig, a note or a project file raised nothing at all. The person reading the
//! receipt saw `the Familiar described \`lamp_tall\`` and had no way to see the piece it described.
//!
//! So `Shown` — the one answer per operation to *where its change is seen* (AF-01, #357) — is now the
//! one answer to *where the editor goes*. [`bevy_cauldron::reveal`] opens the door and the panel the
//! change is on, selects the row, and lets the list scroll it into view; the camera is framed only for
//! a change that is on the map.
//!
//! # What is proved here
//!
//! 1. **One operation per off-map `Shown` variant, applied over BRP from the Map door, ends on the
//!    right door and panel with the changed row selected and inside its list's visible area.** The
//!    position is measured from the row's own `ComputedNode` and `UiGlobalTransform` against the
//!    list's, never from the arithmetic the reveal itself uses — a test that re-ran the reveal's
//!    computation would pass on a camera that never moved.
//! 2. **A `Shown` the editor has no page for raises a problem naming the operation and its shown**,
//!    rather than doing nothing: the missing view is then a work item instead of a silent no-op.
//!
//! # Why the rows are measured, not the resources
//!
//! Setting `ImportState::selected_library_id` and reading it back proves the resource was written. The
//! person's question is whether the row they are told to look at is on screen, which is the same
//! standard `every_list_follows_its_selection` holds every follower to — and the same one that caught
//! the two scrolls reported broken in 2026-08.

use std::time::Duration;

use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{Door, Mode};

use crate::fixtures;

/// **A rules file with real numbers**, so `rules_set` has a block to write and the no-page test is
/// about the page rather than about the refusal that precedes it. `alpha` is the fixture's piece.
const RULES: &str = r"# the block rules
version: 1
blocks:
- example: m
  regions: []
  cell: 2.0
  size:
  - 4
  - 4
  border: null
  no_empty: true
  weights:
  - prefix: alpha
    times: 2.0
  footprints: []
  attempts: 8
  seed: 7
";

/// **One frame of the test clock.** The reveal is a queue carried out a frame at a time, and a
/// crossing is `PreUpdate` + a state transition, so a test that stepped real frames would be reading
/// the editor mid-crossing. 60 Hz, as the follow's own tests step it.
const FRAME: f32 = 1.0 / 60.0;

/// **How many frames a reveal may take**: a crossing, the panel's first build, and the follower's
/// one-frame-late scroll. Generous on purpose — a fixed count is a count somebody guessed, and this
/// one only has to be longer than the work.
const SETTLE: usize = 40;

/// A project with a piece, a tile, a composition, a rig, a registered data file, a pack and a note:
/// one subject per off-map `Shown` variant, so each test can name the operation that changes it.
///
/// The rigs manifest and the character it measures come from the workspace's own `assets`, the way
/// `rig_adopt_measured_is_an_operation` gets them: a fixture rigs.yaml would have to invent a GLB.
fn project(tag: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("lamp_tall", "alpha")
        .descriptor("bed", "alpha")
        .place("bed", (2.0, 2.0))
        .composition("nook", &[("seat", "bed", (2.0, 2.0))])
        .side(64.0)
        .rules_text(RULES)
        .build("m");
    let grimoire = root.join("assets/grimoire");
    // The rig manifest and the mesh it measures, from the workspace.
    std::fs::copy(workspace_asset("grimoire/rigs.yaml"), grimoire.join("rigs.yaml")).expect("rigs.yaml");
    std::fs::create_dir_all(root.join("assets/characters")).expect("characters dir");
    std::fs::copy(workspace_asset("characters/valkyrie.glb"), root.join("assets/characters/valkyrie.glb"))
        .expect("valkyrie.glb");
    // A registered data file with two leaves, so the DATA table has rows to select.
    let kit = root.join("assets/grimoire/furniture");
    std::fs::write(kit.join("mob.yaml"), "# tuning\nmob:\n  speed: 1.50\n  name: orc\n").expect("mob.yaml");
    std::fs::write(
        kit.join("data.yaml"),
        "{version: 1, files: [{id: \"tuning/mob\", path: \"mob.yaml\", bounds: {\"mob.speed\": {min: 0.0, max: 9.0}}}]}",
    )
    .expect("data.yaml");
    // A note on the open map, so `Note` has one to select.
    let notes = bevy_grimoire_core::notes::path_for_map(&grimoire.join("maps/m.map.yaml"));
    let note = bevy_grimoire_core::notes::Notes {
        version: bevy_grimoire_core::notes::NOTES_VERSION,
        notes: vec![bevy_grimoire_core::notes::Note {
            id: "note-1".to_owned(),
            targets: vec![bevy_grimoire_core::notes::Target::Placement("bed@0".to_owned())],
            text: "the bed is in the corner".to_owned(),
            state: bevy_grimoire_core::notes::State::Draft,
            site: None,
            history: Vec::new(),
            auto: false,
        }],
        lease_secs: bevy_grimoire_core::notes::DEFAULT_LEASE_SECS,
    };
    std::fs::write(&notes, ron_string(&note)).expect("notes file");
    root
}

/// **A value as RON**, through the crate that owns the type — a hand-written note row drifted from the
/// schema the first time, which is what `status: None` was.
fn ron_string<T: serde::Serialize>(value: &T) -> String {
    bevy_grimoire_core::yaml::to_string(value).unwrap_or_else(|e| panic!("the note does not serialize: {e}"))
}

fn workspace_asset(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets").join(rel)
}

/// **The editor on the Map door, with an agent's door open and a clock the test steps.**
fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(*app.world().resource::<Door>(), Door::Map, "the test starts on the Map door");
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

/// **Where a laid-out node stands**, in physical pixels: its centre and its half-size.
fn box_of(app: &App, e: Entity) -> Option<(Vec2, Vec2)> {
    let node = app.world().get::<ComputedNode>(e)?;
    let at = app.world().get::<UiGlobalTransform>(e)?;
    Some((at.translation, node.size() * 0.5))
}

/// **`row` lies inside `list`'s visible area**, measured from the two nodes' own geometry.
///
/// `Err` is the sentence a failing assertion prints, naming where the row landed relative to the list
/// — the same shape `the_editor_follows_an_agents_change` uses for a camera, one surface over.
fn inside(app: &App, row: Entity, list: Entity) -> Result<(), String> {
    let (row_mid, row_half) = box_of(app, row).ok_or_else(|| "the row is not laid out".to_owned())?;
    let (list_mid, list_half) = box_of(app, list).ok_or_else(|| "the list is not laid out".to_owned())?;
    let (top, bottom) = (list_mid.y - list_half.y, list_mid.y + list_half.y);
    let (row_top, row_bottom) = (row_mid.y - row_half.y, row_mid.y + row_half.y);
    if row_half.y <= 0.0 || list_half.y <= 0.0 {
        return Err(format!("a zero-height node: row {row_half:?}, list {list_half:?}"));
    }
    if row_top >= top && row_bottom <= bottom {
        Ok(())
    } else {
        Err(format!(
            "the row spans {row_top:.1}..{row_bottom:.1} and the list {top:.1}..{bottom:.1}"
        ))
    }
}

/// **The one entity carrying `T`, when the test names a marker the editor has exactly one of.**
fn one<T: Component>(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<T>>();
    let found: Vec<Entity> = q.iter(world).collect();
    assert_eq!(found.len(), 1, "expected one entity of this marker, found {}", found.len());
    found[0]
}

/// **The scrollable list a row sits in**, found by walking up from the row: the Compose panel's body is
/// private, and "is this row inside the thing that scrolls it" is the honest question anyway.
fn scrollable_ancestor(app: &mut App, row: Entity) -> Entity {
    use bevy::ui::ScrollPosition;
    let world = app.world_mut();
    let mut owner = world.query::<&ChildOf>();
    let mut scrolls = world.query_filtered::<Entity, With<ScrollPosition>>();
    let mut at = row;
    loop {
        if scrolls.get(world, at).is_ok() {
            return at;
        }
        let Ok(parent) = owner.get(world, at) else {
            panic!("no scrollable ancestor holds the row");
        };
        at = parent.0;
    }
}

/// **The door and panel the editor ended on**, as words: `Mode` carries no `Debug`, and the words are
/// what a person reads on the strip anyway.
fn door_and_tab(app: &App) -> (Door, &'static str) {
    (*app.world().resource::<Door>(), app.world().resource::<Mode>().label())
}

/// **The row for a library piece**, by the id the Meshes shelf names it.
fn library_row(app: &mut App, id: &str) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &bevy_cauldron::tiles::LibraryRow)>();
    q.iter(world)
        .find(|(_, r)| r.0 == id)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("no row for `{id}` on the Meshes shelf"))
}

// ── One operation per off-map `Shown` variant ──────────────────────────────────────────────────

/// **`Piece`** — `piece_set_note` describes a library piece, and the editor opens the Meshes shelf
/// with that piece selected.
#[test]
fn a_pieces_page_opens_on_the_meshes_shelf_with_the_piece_selected() {
    let root = project("reveal_piece");
    let mut app = editor(&root);
    let answer = apply(
        &mut app,
        "piece_set_note",
        serde_json::json!({"id": "lamp_tall", "note": "a tall floor lamp"}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    settle(&mut app);

    assert_eq!(
        door_and_tab(&app),
        (Door::Kit, "MESHES"),
        "the change is a library piece, whose page is the kit door's Meshes shelf"
    );
    assert_eq!(
        app.world().resource::<bevy_cauldron::tiles::ImportState>().selected_library_id.as_deref(),
        Some("lamp_tall"),
        "the shelf did not select the piece the change was to"
    );
    let row = library_row(&mut app, "lamp_tall");
    let list = one::<bevy_cauldron::tiles::CandidateList>(&mut app);
    if let Err(why) = inside(&app, row, list) {
        panic!("the piece's row is not inside the shelf's visible area: {why}");
    }
}

/// **`Tile`** — a piece's lattice. **The page is the Meshes panel, not the Tiles one**, and the
/// measurement is why: `tiles::rebuild_detail` returns into `build_detail` for `Mode::Tiles` (the open
/// *composition*), and the `SUBGRID` block, `draw_subgrid`, `pick_lattice` and `click_lattice` all run
/// on the Meshes panel. Opening the Tiles tab would put the person in front of a different subject.
#[test]
fn a_tiles_lattice_opens_on_the_meshes_panel_with_that_piece_selected() {
    let root = project("reveal_tile");
    let mut app = editor(&root);
    // `bed` is a fixture descriptor, so its lattice is derivable; 0,0,0 is its own cell.
    let answer = apply(
        &mut app,
        "piece_set_cells",
        serde_json::json!({"id": "bed", "cells": [[0, 0, 0]], "solid": true}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    settle(&mut app);

    assert_eq!(
        door_and_tab(&app),
        (Door::Kit, "MESHES"),
        "a piece's lattice is drawn on the Meshes panel, and the reveal went elsewhere"
    );
    assert_eq!(
        app.world().resource::<bevy_cauldron::tiles::ImportState>().selected_library_id.as_deref(),
        Some("bed"),
        "the shelf did not select the piece whose lattice changed"
    );
    let row = library_row(&mut app, "bed");
    let list = one::<bevy_cauldron::tiles::CandidateList>(&mut app);
    if let Err(why) = inside(&app, row, list) {
        panic!("the tile's row is not inside the list's visible area: {why}");
    }
}

/// **`Composition`** — a group is the Compose panel's list. `composition_define` names the group in
/// its parameters, so the page can be opened before the write is read back.
#[test]
fn a_compositions_page_opens_on_the_compose_panel_with_that_group_selected() {
    let root = project("reveal_composition");
    let mut app = editor(&root);
    let answer = apply(
        &mut app,
        "composition_define",
        serde_json::json!({"id": "corner", "from": ["bed@0"]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    settle(&mut app);

    assert_eq!(
        door_and_tab(&app),
        (Door::Kit, "COMPOSE"),
        "a group is defined on the Compose panel, and the reveal went elsewhere"
    );
    let state = app.world().resource::<bevy_cauldron::compose::ComposeState>();
    let names: Vec<&str> = app
        .world()
        .resource::<Project>()
        .compositions
        .compositions
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(
        names.get(state.selected).copied(),
        Some("corner"),
        "the Compose panel's selection is {names:?}[{}], not the group the change defined",
        state.selected
    );
    // **The group's own row**: several rows carry `RowSelected` — the armed group, the focused
    // member — so the id drawn on the row is what names the one the change was to. Found by walking
    // up from the text itself, which is the row's own child.
    let row = {
        let world = app.world_mut();
        let mut texts = world.query::<(Entity, &Text)>();
        // **The row's own words**, not the "MEMBERS OF" heading above it: the row reads
        // `corner  —  1 member(s), anchored`, and a heading is a section rather than a row.
        let at = texts
            .iter(world)
            .find(|(_, t)| t.0.starts_with("corner") && t.0.contains("member(s)"))
            .map(|(e, _)| e)
            .unwrap_or_else(|| {
                let drawn: Vec<String> = texts.iter(world).map(|(_, t)| t.0.clone()).collect();
                panic!("no text draws the group `corner`; the panel draws {drawn:?}")
            });
        let mut owner = world.query::<&ChildOf>();
        let mut selected = world.query::<&bevy_cauldron::chrome::RowSelected>();
        let mut at = at;
        loop {
            if selected.get(world, at).is_ok() {
                break at;
            }
            let Ok(parent) = owner.get(world, at) else {
                panic!("the group's text is not inside a selected row");
            };
            at = parent.0;
        }
    };
    let list = scrollable_ancestor(&mut app, row);
    if let Err(why) = inside(&app, row, list) {
        panic!("the group's row is not inside the Compose panel's visible area: {why}");
    }
}

/// **`Rig`** — a rig is the rig bench's list. `rig_adopt_measured` names the rig in its parameters.
#[test]
fn a_rigs_page_opens_on_the_rig_bench_with_that_rig_selected() {
    let root = project("reveal_rig");
    let mut app = editor(&root);
    let rig = app
        .world()
        .resource::<Project>()
        .root
        .join("assets/grimoire/rigs.yaml");
    let text = std::fs::read_to_string(&rig).expect("the fixture copied rigs.yaml");
    let name = text
        .lines()
        .find_map(|l| {
            let line = l.trim();
            let (key, rest) = line.split_once(":")?;
            (rest.trim_start().starts_with('(') && key.starts_with('"')).then(|| key.trim_matches('"').to_owned())
        })
        .expect("rigs.yaml names one rig a line");
    let rig = name;
    let answer = apply(
        &mut app,
        "rig_adopt_measured",
        serde_json::json!({"rig": rig}),
    );
    // The write may be refused (the mesh is measured by the bench, and a refusal is not this test's
    // subject); what the reveal reads is the `shown`, so a refusal is a different subject entirely.
    assert_eq!(answer["outcome"], "applied", "the rig adopts its measured values: {answer}");
    settle(&mut app);
    assert_eq!(
        door_and_tab(&app),
        (Door::Rigs, "ANIM"),
        "a rig's page is the Rigs door's bench"
    );
    let bench = app.world().resource::<bevy_cauldron::anim_tab::BenchState>();
    assert_eq!(
        bench.names().get(bench.selected).copied(),
        Some(rig.as_str()),
        "the bench's selection is not the rig the change was to"
    );
}

/// **`File`** — a registered data file is the DATA panel, the only file page the editor has until the
/// Files workspace lands (GD-11, #373). The table's cursor goes to the file's first leaf.
#[test]
fn a_data_files_page_opens_on_the_data_panel_at_that_file() {
    let root = project("reveal_file");
    let mut app = editor(&root);
    let answer = apply(
        &mut app,
        "data_set",
        serde_json::json!({"file": "tuning/mob", "path": "mob.speed", "value": 2.0}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    settle(&mut app);

    assert_eq!(
        door_and_tab(&app),
        (Door::Map, "MAP"),
        "a data file's page is the Map door's DATA panel"
    );
    let world = app.world_mut();
    let mut q = world.query::<(&bevy_gooey::Table, &bevy_gooey::TableCursor)>();
    let (_, cursor) = q.single(world).expect("the DATA panel has one table");
    assert!(
        cursor.0.is_some(),
        "the table's cursor was not put on the file's first leaf, so nothing says which row to read"
    );
}

/// **`Settings`** — a section of the editor's settings. A pack's exclusion is the project
/// configuration screen's `Packs the kit leaves out` list.
#[test]
fn a_settings_page_opens_on_the_project_configuration_at_that_section() {
    let root = project("reveal_settings");
    let mut app = editor(&root);
    let answer = apply(
        &mut app,
        "pack_set_excluded",
        serde_json::json!({"pack": "alpha", "excluded": true}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    settle(&mut app);
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<bevy_cauldron::project_config::ProjectConfigDialog>>();
    assert_eq!(
        q.iter(world).count(),
        1,
        "the pack's page is the project configuration screen, and it did not open"
    );
}

/// **`Note`** — a note is the open map's Notes list. `note_edit` changes one note's text.
#[test]
fn a_notes_page_opens_on_the_notes_panel_with_that_note_selected() {
    let root = project("reveal_note");
    let mut app = editor(&root);
    let answer = apply(
        &mut app,
        "note_edit",
        serde_json::json!({"id": "note-1", "text": "the bed is by the window"}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    settle(&mut app);

    assert_eq!(
        door_and_tab(&app),
        (Door::Map, "MAP"),
        "a note is on the open map, so its page is the Map door"
    );
    assert_eq!(
        app.world()
            .get_resource::<bevy_cauldron::editor::SelectedNote>()
            .and_then(|s| s.0.clone())
            .as_deref(),
        Some("note-1"),
        "the Notes list did not select the note the change was to"
    );
}

/// **A `Shown` with no page raises a problem naming the operation and its shown.**
///
/// `rules_set` rewrites `assets/grimoire/rules.yaml`, and the editor has no page that shows a project
/// file yet — the Files workspace is GD-11 (#373). Doing nothing is the bug this closes: the person
/// would read `the Familiar changed a rule` and have nowhere to look.
#[test]
fn a_change_the_editor_has_no_page_for_raises_a_problem_naming_it() {
    let root = project("reveal_no_page");
    let mut app = editor(&root);
    let before = app
        .world()
        .resource::<EditorState>()
        .status
        .problems()
        .len();
    let answer = apply(
        &mut app,
        "rules_set",
        serde_json::json!({"example": "m", "seed": 19}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    settle(&mut app);

    let status = &app.world().resource::<EditorState>().status;
    let raised = &status.problems()[before..];
    assert!(
        raised.iter().any(|p| p.text.contains("rules_set") && p.text.contains("rules.yaml")),
        "a change with no page said nothing about it. The problems raised were {raised:?}"
    );
}
