//! **A rule can be changed, and one Cmd+Z puts it back (E-081).**
//!
//! `grimoire_list_rules` shipped last iteration and answers what a project's rules say: Sanctuary's
//! `town_block` is a 17×17 block over 2 m cells with `weights: [(prefix: "town/house_", times: 100.0)]`,
//! `attempts: 64` and `seed: 600`. **An agent that can read that and cannot change it has the
//! diagnosis and not the cure** — every other tunable in the project is a registered operation
//! (`map_set_settings`, `label_set`, `map_rename`) and this one was a text editor.
//!
//! # What is being proved
//!
//! 1. **A weight changes on disk and in memory**, and the author's comments survive the write. That
//!    second half is the reason this is text surgery rather than a re-serialisation: a rules file is
//!    prose about a layout as much as it is numbers, and `to_string_pretty` would delete every
//!    comment in it — the failure `ron_surgery` exists to prevent.
//! 2. **Undo puts the number back**, in both places, so one Cmd+Z takes back what an agent did.
//! 3. **A refusal changes nothing**, and says which rule and which field.
//! 4. **Both front ends reach it**, which `operations_have_two_front_ends` asserts for every
//!    operation — so this test only needs the agent's.
//!
//! # Why the writes are checked by parsing, not by string match
//!
//! A test that asserts `times: 50.0` appears in the file proves the write happened and not that the
//! file still means what it did plus that one number. Every assertion here re-parses `rules.ron`
//! through `bevy_grimoire_core::rules::Rules` and compares the parsed field, so a rewrite that broke
//! the file fails even when it left the expected characters behind.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;

use crate::fixtures;

/// **A rules file with comments in it**, which is the case the text surgery exists for. The
/// descriptor ids a rule names must be in the fixture's library, so `alpha` is the piece.
const RULES: &str = "// the block rules: learned from `m`\n\
(\n\
    version: 1,\n\
    blocks: [\n\
        (\n\
            example: \"m\",\n\
            regions: [],\n\
            cell: 2.0,\n\
            size: (4, 4),\n\
            border: None,\n\
            no_empty: true,\n\
            // houses should outnumber the road\n\
            weights: [(prefix: \"alpha\", times: 2.0)],\n\
            footprints: [],\n\
            attempts: 8,\n\
            seed: 7,\n\
        ),\n\
    ],\n\
)\n";

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .side(8.0)
        .descriptor("alpha", "alpha")
        .rules(RULES)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn apply(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "rules_set", "parameters": parameters, "dry_run": false}),
    )
}

fn undo(app: &mut App) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": null, "dry_run": false}),
    )
}

fn rules_text(app: &App) -> String {
    let path = app.world().resource::<Project>().project_dir.join("rules.ron");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// **The file, parsed**, so a broken rewrite cannot pass by containing the right characters.
fn on_disk(app: &App) -> bevy_grimoire_core::rules::Rules {
    bevy_grimoire_core::rules::Rules::parse(&rules_text(app))
        .unwrap_or_else(|e| panic!("rules.ron no longer parses: {e}\n{}", rules_text(app)))
}

fn in_memory(app: &App) -> bevy_grimoire_core::rules::Rules {
    app.world().resource::<Project>().rules.clone()
}

/// **A weight multiplier changes, on disk and in the project, and the comments survive.**
#[test]
fn a_rule_weight_changes_on_disk_and_in_memory() {
    let mut app = editor("rules_weight");
    assert_eq!(on_disk(&app).blocks[0].weights[0].times, 2.0, "the fixture states 2.0");

    let reply = apply(
        &mut app,
        serde_json::json!({"example": "m", "weights": [{"prefix": "alpha", "times": 50.0}]}),
    );
    assert!(
        !reply.to_string().contains("refused"),
        "the weight change was refused: {reply}"
    );

    assert_eq!(
        on_disk(&app).blocks[0].weights[0].times,
        50.0,
        "the file was not written: {reply}"
    );
    assert_eq!(
        in_memory(&app).blocks[0].weights[0].times,
        50.0,
        "the project still holds the old weight, so a generate would use the number the file no longer has"
    );
    let text = rules_text(&app);
    assert!(
        text.contains("// houses should outnumber the road") && text.starts_with("// the block rules"),
        "the write deleted the author's comments, which is the whole reason this is text surgery: {text}"
    );
}

/// **One Cmd+Z puts the number back**, in the file and in the project — an agent's change is the
/// person's to take back, as every other operation's is.
#[test]
fn undoing_a_rule_change_puts_the_number_back() {
    let mut app = editor("rules_weight_undo");
    apply(
        &mut app,
        serde_json::json!({"example": "m", "weights": [{"prefix": "alpha", "times": 50.0}]}),
    );
    assert_eq!(on_disk(&app).blocks[0].weights[0].times, 50.0);

    reply_is_applied(undo(&mut app));
    assert_eq!(
        on_disk(&app).blocks[0].weights[0].times,
        2.0,
        "undo left rules.ron with the agent's weight"
    );
    assert_eq!(
        in_memory(&app).blocks[0].weights[0].times,
        2.0,
        "undo left the project with the agent's weight"
    );
}

/// **The scalar fields change too**, because a seed and an attempt count are the other two numbers
/// an author tunes when a layout comes out wrong.
#[test]
fn a_seed_and_an_attempt_count_change() {
    let mut app = editor("rules_scalars");
    let reply = apply(
        &mut app,
        serde_json::json!({"example": "m", "seed": 99, "attempts": 40}),
    );
    assert!(!reply.to_string().contains("refused"), "refused: {reply}");
    let rules = on_disk(&app);
    assert_eq!(rules.blocks[0].seed, 99, "{reply}");
    assert_eq!(rules.blocks[0].attempts, 40, "{reply}");
    // **And the numbers it was not asked about are untouched**, which is what makes this a scoped
    // change rather than a rewrite.
    assert_eq!(rules.blocks[0].cell, 2.0);
    assert_eq!(rules.blocks[0].size, (4, 4));
    assert!(rules.blocks[0].no_empty, "no_empty is untouched");
    assert_eq!(rules.blocks[0].weights.len(), 1);
}

/// **A rule that is not there is refused by name**, and nothing is written — an agent told "applied"
/// for a block that does not exist would report a change nobody can find.
#[test]
fn a_rule_that_does_not_exist_is_refused_and_nothing_is_written() {
    let mut app = editor("rules_absent");
    let before = rules_text(&app);
    let reply = apply(
        &mut app,
        serde_json::json!({"example": "no_such_block", "seed": 3}),
    );
    assert!(
        reply.to_string().contains("refused"),
        "a rule that does not exist was applied: {reply}"
    );
    assert_eq!(
        rules_text(&app),
        before,
        "a refused change wrote to the file: {reply}"
    );
}

/// **A field this operation does not own is refused rather than ignored**, so a caller cannot believe
/// it changed `size` when the operation only writes numbers that are one value each.
#[test]
fn a_field_this_operation_does_not_own_is_refused() {
    let mut app = editor("rules_unknown_field");
    let before = rules_text(&app);
    let reply = apply(
        &mut app,
        serde_json::json!({"example": "m", "size": [9, 9]}),
    );
    assert!(
        reply.to_string().contains("refused"),
        "a field the operation does not write was accepted and ignored: {reply}"
    );
    assert_eq!(rules_text(&app), before, "a refusal wrote to the file");
}

/// **A whole-number weight keeps its `.0`**, so a file the author wrote is not reformatted by a change
/// to one other number.
///
/// Found live on Sanctuary's own `times: 100.0`, which came back as `times: 50` after a write: the
/// value was right and the spelling wrong, and the file's diff then carried a change nobody made. The
/// unit is a float in the schema and `50.0f64.to_string()` is `50`, so the renderer has to say the
/// decimal itself.
#[test]
fn a_whole_number_weight_keeps_its_decimal_point() {
    let mut app = editor("rules_decimal");
    apply(
        &mut app,
        serde_json::json!({"example": "m", "weights": [{"prefix": "alpha", "times": 50.0}]}),
    );
    let text = rules_text(&app);
    assert!(
        text.contains("times: 50.0"),
        "a whole-number weight lost its decimal point, so the file reads as an edit the author did not make: {text}"
    );
    assert_eq!(on_disk(&app).blocks[0].weights[0].times, 50.0);
}

/// **A change and its undo leave the file byte for byte as it was**, which is what lets an author
/// trust that taking an agent's change back did not quietly reformat their rules.
#[test]
fn a_change_and_its_undo_leave_the_file_unchanged() {
    let mut app = editor("rules_round_trip");
    let before = rules_text(&app);
    apply(
        &mut app,
        serde_json::json!({
            "example": "m",
            "seed": 1234,
            "weights": [{"prefix": "alpha", "times": 50.0}]
        }),
    );
    assert_ne!(rules_text(&app), before, "the write did nothing");
    reply_is_applied(undo(&mut app));
    assert_eq!(
        rules_text(&app),
        before,
        "undo left the file different from what it was, so taking a change back is itself an edit"
    );
}


/// **A field name inside a quoted string is not the field.**
///
/// `set_field` finds a field by its `name:`. A weight prefix is arbitrary text, so a file holding
/// `weights: [(prefix: "cell:x", times: 2.0)]` contains the characters `cell:` inside a string —
/// and a `set_field("cell", …)` that matched there would replace part of a prefix and leave the
/// real `cell` alone, producing a file the editor cannot open. This is the corruption case the
/// surgery has to survive, and it is a test rather than a comment because the failure is silent.
#[test]
fn a_field_name_inside_a_prefix_does_not_hijack_the_write() {
    const TRICKY: &str = "(version: 1, blocks: [(example: \"m\", regions: [], cell: 2.0, \
size: (4, 4), border: None, no_empty: true, \
weights: [(prefix: \"cell:x\", times: 2.0)], footprints: [], attempts: 8, seed: 7)])\n";
    // **The order matters and that is the point.** `cell:` appears before `weights:` in a file
    // written by the editor, so `find` takes the real one. A hand-edited file can put `weights`
    // first, and then the same characters are the ones `find` reaches. Both orders are run, because
    // only the second one exercises the hazard.
    const TRICKY_WEIGHTS_FIRST: &str = "(version: 1, blocks: [(example: \"m\", regions: [], \
weights: [(prefix: \"cell:x\", times: 2.0)], cell: 2.0, size: (4, 4), border: None, \
no_empty: true, footprints: [], attempts: 8, seed: 7)])\n";
    let root = fixtures::Fixture::new("rules_tricky")
        .side(8.0)
        .descriptor("alpha", "alpha")
        .rules(TRICKY)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    let reply = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "rules_set", "parameters": {"example": "m", "cell": 3.0}, "dry_run": false}),
    );
    assert!(!reply.to_string().contains("refused"), "refused: {reply}");
    let rules = bevy_grimoire_core::rules::Rules::parse(
        &std::fs::read_to_string(
            app.world().resource::<Project>().project_dir.join("rules.ron"),
        )
        .expect("on disk"),
    )
    .expect("the rewrite must still parse");
    assert_eq!(rules.blocks[0].cell, 3.0, "the cell was not changed: {reply}");
    assert_eq!(
        rules.blocks[0].weights[0].prefix, "cell:x",
        "the prefix was corrupted by a field-name match inside it: {reply}"
    );

    // **And with the weights first**, where a naive `find` reaches the prefix before the real field.
    let root = fixtures::Fixture::new("rules_tricky_order")
        .side(8.0)
        .descriptor("alpha", "alpha")
        .rules(TRICKY_WEIGHTS_FIRST)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    let reply = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "rules_set", "parameters": {"example": "m", "cell": 3.0}, "dry_run": false}),
    );
    let text = std::fs::read_to_string(
        app.world().resource::<Project>().project_dir.join("rules.ron"),
    )
    .expect("on disk");
    let rules = bevy_grimoire_core::rules::Rules::parse(&text)
        .unwrap_or_else(|e| panic!("the rewrite must still parse: {e}\n{text}"));
    assert_eq!(rules.blocks[0].cell, 3.0, "the cell was not changed: {reply}");
    assert_eq!(
        rules.blocks[0].weights[0].prefix, "cell:x",
        "the prefix was corrupted by a field-name match inside it: {text}"
    );
}

fn reply_is_applied(reply: serde_json::Value) {
    assert!(
        !reply.to_string().contains("refused"),
        "the operation was refused: {reply}"
    );
}

/// **The seed field takes the keyboard while it is open (E-081).**
///
/// `editor::Fields::typing` is the ONE place a text field is registered, and its own doc says
/// *"Adding a field is adding a line, here and nowhere else."* A field missing from it does not merely
/// fail to swallow its keys: `Phase::Text` runs before `Phase::Act`, so the field edits and the Map's
/// verb fires on the same keystroke — `N` renames the map, `R` turns the piece under the cursor, `K`
/// reopens this very field on top of itself.
///
/// **This test exists because the first draft missed the line.** It was found by this iteration's own
/// review, not by a failure, which is the worse way to find it: nothing on screen looks wrong while a
/// seed is typed, and the damage is a second change the author never asked for.
#[test]
fn the_seed_field_holds_the_keyboard_while_it_is_open() {
    use bevy::input::ButtonState;
    use bevy::input::keyboard::Key;
    use bevy_cauldron::editor::EditorState;

    let mut app = editor("rules_typing_guard");
    let before = rules_text(&app);
    // The field is open, as `K` leaves it.
    app.world_mut().resource_mut::<EditorState>().rule_seed = Some(String::new());
    app.update();

    // **`N` is `rename_map` on the Map** and a letter a seed cannot hold, so it isolates the guard:
    // if the field holds the keyboard the map's name is untouched and the field stays empty.
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(bevy::input::keyboard::KeyboardInput {
            key_code: KeyCode::KeyN,
            logical_key: Key::Character("n".into()),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
    app.update();

    let state = app.world().resource::<EditorState>();
    assert!(
        state.rule_seed.is_some(),
        "the seed field lost the keyboard: typing closed it"
    );
    assert_eq!(
        state.rule_seed.as_deref(),
        Some(""),
        "a letter is not a digit, so the field takes it and holds nothing — but it must not close"
    );
    // **The rename verb's own prompt, word for word.** The first version of this test looked for
    // `"renam"` and passed without the guard in place, because `rename_keys` says *"type a new name
    // for …"* — the substring was never there, so the assertion could not fail. A test that cannot
    // fail is worse than no test, and this one was caught by reverting the fix and watching it stay
    // green.
    assert!(
        !state.status.note_text().contains("type a new name"),
        "the rename verb answered while the seed field held the keyboard: `{}`",
        state.status.note_text()
    );
    assert_eq!(
        app.world().resource::<Project>().open_map_name.as_deref().map(str::to_owned),
        Some("m".to_owned()),
        "a letter typed into the seed field renamed the map"
    );
    assert_eq!(
        rules_text(&app),
        before,
        "a letter typed into the seed field wrote to the rules file"
    );
}
