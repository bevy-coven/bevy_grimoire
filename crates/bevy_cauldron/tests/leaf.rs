//! **The crate's boundary, as a test rather than a comment.**
//!
//! Root `CLAUDE.md` asks every mirrored crate for a dependency ratchet *"so widening the boundary
//! costs a deliberate edit rather than a passing build"*. This crate did not have one, and it is the
//! crate where it matters most: it is the only place in the workspace that speaks HTTP. `vlm.rs`
//! reaches a vision model over `ureq`, and the argument that this is safe rests entirely on the editor
//! never shipping. That argument is worth exactly as much as the list below is enforced.
//!
//! # Two things this deliberately does not do
//!
//! It does not forbid `bevy` — this crate *is* a Bevy application, which is the difference between it
//! and `bevy_grimoire_core`'s `engine_free.rs`. And it does not check the game's own manifest: the editor is
//! not a dependency of the game, and `cargo tree -i bevy_cauldron` is what would notice if that
//! changed.
//!
//! # Optional dependencies are dependencies
//!
//! It reads the `[dependencies]` section as written, so `bevy_debugger_bevy` counts even though it is
//! `optional = true` and absent from a default build. That is deliberate: the question this test asks
//! is *"what may this crate reach for"*, and a feature flag decides **when** rather than **whether**.
//! Adding the debugger tripped this on the first run, which is the ratchet working — the row below is
//! the deliberate edit it demanded.
//!
//! # Parsing
//!
//! By hand, over the `[dependencies]` section only. `docs/2026-08-08-handoff.md` records that the
//! existing ratchets split on `= . ' '` and have tripped on multi-line `features = [...]` arrays —
//! which this manifest has, on `bevy` — so this reads a dependency name as *the first token of a line
//! at zero indentation inside the section*, and ignores continuation lines outright.

use std::collections::BTreeSet;

/// Every crate `bevy_cauldron` may depend on directly, and why it is allowed.
///
/// Adding a row here is the deliberate edit. Removing the need for one is better.
const ALLOWED: &[(&str, &str)] = &[
    ("bevy_grimoire_core", "the engine-free schema, solvers and validation"),
    ("bevy_grimoire", "the one spawner, so a map cannot look one way here and another in the game"),
    ("bevy_grimoire_anim", "the game's real pose blender, so the bench previews what the game plays"),
    ("bevy_devshot", "the one capture rig"),
    ("bevy", "this is a Bevy application"),
    (
        "accesskit",
        "roles and setters for the widget layer's a11y tree; `bevy_a11y` does not re-export it, and \
         0.24 is what `bevy_feathers` itself depends on so the two cannot drift",
    ),
    ("ron", "map and library serialization"),
    ("serde", "derives on every schema type this crate reads and writes"),
    ("dirs", "the platform's own data directory, where the user's recent-projects list lives (UI-11, #217); no family crate finds platform directories"),
    ("serde_json", "the VLM chat request and reply bodies"),
    ("ureq", "the VLM transport — one blocking POST from a task-pool thread"),
    ("base64", "data-URI image parts for the booth renders"),
    ("image", "encodes the booth's RGBA readback to PNG"),
    (
        "bevy_grimoire_snip",
        "the one crop, shared by the editor's snip and the MCP's grimoire_snip, so a picture from either door is cut the same way",
    ),
    (
        "bevy_grimoire_decide",
        "the after-edit checks (#111): typed yes/no questions to a decision model about what an agent's operation changed — authoring time only, never in a game",
    ),
    ("arboard", "Cmd+C copies the live tab's text, problems first, for handing to an agent"),
    (
        "bevy_debugger_bevy",
        "the agent's BRP way in — OPTIONAL, reached only by `--features debugger`, so it is absent \
         from the resolved graph of an ordinary build (`cargo tree -i` finds no package). It is \
         what lets an agent press a key in this editor without taking the machine's keyboard",
    ),
    (
        "bevy_elements",
        "the owner's sky, climate, weather, terrain and water crate, hosted by the family layer (#96) — \
         OPTIONAL, reached only by `--features bevy_elements` and pinned by git rev, so an ordinary \
         build and a game that lists no family crate pay nothing",
    ),
    (
        "bevy_flora",
        "the owner's procedural trees and forests crate, hosted by the family layer (#96, #102) — \
         OPTIONAL, reached only by `--features bevy_flora` and pinned by git rev, so an ordinary \
         build and a game that lists no family crate pay nothing",
    ),
    (
        "bevy_gooey",
        "the owner's game-UI skin and widgets crate, hosted by the family layer (#147) — \
         REQUIRED since UI-01 (#209), pinned by git rev: the editor's whole UI is gooey, owned by `EditorUiPlugin`, \
         and never reached by a game, which links none of the editor crates",
    ),
    (
        "isomesh",
        "the owner's engine-free isosurface mesher (#101); the carved volume is sampled through it — \
         OPTIONAL, reached only by `--features isomesh` and pinned by git rev, so an ordinary build pays nothing",
    ),
    (
        "bevy_carnage",
        "the owner's fracture, wound and blood crate, hosted by the family layer (#108, #96) — OPTIONAL, reached only by `--features bevy_carnage` (without its render features) and pinned by git rev, so an ordinary build and a game that lists no family crate pay nothing",
    ),
    (
        "bevy_lights",
        "the owner's fires, torches and budgeted light pool crate, hosted by the family layer (#103) — \
         OPTIONAL, reached only by `--features bevy_lights` and pinned by git rev, so an ordinary \
         build and a game that lists no family crate pay nothing",
    ),
    (
        "bevy_light_grid",
        "the owner's CPU illuminance grid, hosted by the light map (#103) — OPTIONAL, reached only by `--features bevy_light_grid` and pinned by git rev, so an ordinary build and a game that lists no family crate pay nothing",
    ),
    (
        "bevy_orca",
        "the owner's reciprocal-avoidance crowd solver, hosted by the family layer (#107) — OPTIONAL, reached only by `--features bevy_orca` and pinned by git rev, so an ordinary build pays nothing",
    ),
    (
        "bevy_society",
        "the owner's living-society crate (people, needs, ties, factions), hosted by the family layer (#104, #96) — OPTIONAL, reached only by `--features bevy_society` and pinned by git rev, so an ordinary build pays nothing",
    ),
    (
        "bevy_speech_bubbles",
        "the owner's speech balloon, conversation graph and dialog box crate, hosted by the family layer (#106, #96) — OPTIONAL, reached only by `--features bevy_speech_bubbles` and pinned by git rev, so an ordinary build pays nothing",
    ),
    (
        "bevy_stigmergy",
        "the owner's decaying trail fields, hosted by the family layer (#107) — OPTIONAL, reached only by `--features bevy_stigmergy` and pinned by git rev, so an ordinary build pays nothing",
    ),
];

fn declared_dependencies(manifest: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut inside = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed == "[dependencies]";
            continue;
        }
        if !inside || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // A continuation line inside a multi-line table or array is indented; a dependency is not.
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some((name, _)) = trimmed.split_once('=') else {
            continue;
        };
        // `ron.workspace = true` — the dotted form names the crate before the dot.
        let name = name.trim().split('.').next().unwrap_or("").trim();
        if !name.is_empty() {
            out.insert(name.to_owned());
        }
    }
    out
}

#[test]
fn the_editor_depends_on_nothing_it_has_not_argued_for() {
    let manifest = include_str!("../Cargo.toml");
    let declared = declared_dependencies(manifest);
    assert!(
        !declared.is_empty(),
        "parsed no dependencies at all — the parser broke, not the manifest"
    );
    let allowed: BTreeSet<&str> = ALLOWED.iter().map(|(n, _)| *n).collect();
    let extra: Vec<&String> = declared.iter().filter(|d| !allowed.contains(d.as_str())).collect();
    assert!(
        extra.is_empty(),
        "bevy_cauldron declares {extra:?}, which nothing in this test argues for. Widening the \
         boundary is fine — add the crate to ALLOWED with the sentence that says why it earns its \
         place, and that sentence is what a reviewer reads."
    );
}

/// The list is a claim about the manifest, so a stale row is a lie in the other direction.
#[test]
fn the_allowed_list_has_no_rows_the_manifest_dropped() {
    let declared = declared_dependencies(include_str!("../Cargo.toml"));
    let stale: Vec<&str> = ALLOWED
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| !declared.contains(*n))
        .collect();
    assert!(
        stale.is_empty(),
        "ALLOWED still argues for {stale:?}, which the manifest no longer declares. A ratchet that \
         permits more than exists stops being a description of anything."
    );
}

/// Every row says why. A bare name is the comment this test replaced.
#[test]
fn every_allowed_dependency_carries_its_argument() {
    for (name, why) in ALLOWED {
        assert!(
            why.len() > 10,
            "`{name}` is allowed with no argument for it. The reason is the point — a list of names \
             is what the manifest already is."
        );
    }
}
