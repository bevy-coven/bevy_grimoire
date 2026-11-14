//! **The runner's "is the map open yet" probe is pinned to the editor's own wording** (#433 review).
//!
//! `map_is_open` dry-runs `note_remove` for a note that does not exist and reads the refusal: "no map is open ..."
//! before the editor has opened its map. It depends on the operation's name and on that sentence, which live in
//! `bevy_cauldron`. If the editor rewords the refusal the probe would stop waiting (every refusal would read as "open")
//! and `work-the-open-note` would flake again, so this test reads the editor's source and fails first.

use bevy_cauldron_bench::run::{MAP_PROBE_OP, NO_MAP_OPEN};

fn editor_source(file: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../bevy_cauldron/src").join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// **The editor still refuses with the sentence the probe looks for**, from the note operations the probe calls.
#[test]
fn the_editor_still_refuses_with_the_probes_sentence() {
    let notes = editor_source("ops_notes.rs");
    assert!(
        notes.contains(&format!("\"{NO_MAP_OPEN}, ")),
        "ops_notes.rs refuses with a sentence that starts `{NO_MAP_OPEN}, `: the runner's map probe reads exactly that"
    );
}

/// **The operation the probe calls is still one the editor has**, under that name.
#[test]
fn the_probe_operation_still_exists() {
    let notes = editor_source("ops_notes.rs");
    assert!(
        notes.contains(&format!("\"{MAP_PROBE_OP}\"")),
        "ops_notes.rs names an operation `{MAP_PROBE_OP}`, which the runner's map probe dry-runs"
    );
    assert!(
        notes.contains("no note {} on this map") || notes.contains("no note {id} on this map"),
        "and it still refuses an unknown note by saying so, which is what tells the probe a map is open"
    );
}
