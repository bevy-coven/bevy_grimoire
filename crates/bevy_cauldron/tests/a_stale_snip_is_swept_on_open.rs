//! **A pending snip left by an editor that ended is cleared when the next project opens (C6 gap 6).**
//!
//! `snip::sweep_stale_pending` runs on the first frame with a project. A `pending-*.json` already in
//! the snips folder then has no picture still coming for it, so it must go. A finished picture in the
//! same folder is not pending and must stay.

use bevy_cauldron::harness;

use crate::fixtures;

const SNIPS: &str = "target/loop/captures/snips";

#[test]
fn a_pending_record_left_by_an_earlier_editor_is_cleared_on_open() {
    let root = fixtures::Fixture::new("snip_sweep")
        .descriptor("site/wall", "alpha")
        .build("m");
    let dir = root.join(SNIPS);
    std::fs::create_dir_all(&dir).expect("the snips folder is made");
    std::fs::write(dir.join("pending-snip-1.json"), "{}").expect("the stale record is written");
    std::fs::write(dir.join("snip-0.png"), b"png").expect("the finished picture is written");

    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }

    assert!(
        !dir.join("pending-snip-1.json").exists(),
        "a pending record with no picture still coming must be cleared on open"
    );
    assert!(
        dir.join("snip-0.png").exists(),
        "a finished picture is not pending and must stay"
    );
}
