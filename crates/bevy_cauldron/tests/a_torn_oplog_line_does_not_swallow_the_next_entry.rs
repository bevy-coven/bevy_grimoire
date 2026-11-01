//! **A write cut short loses one log line, not the next operation's too** (#206).
//!
//! `ops::append` wrote its line straight after whatever the file ended with. After a crash or a full
//! disk left the last line without its `\n`, the next entry was glued onto the torn text, and every
//! reader failed that one physical line: two operations lost for one torn write, and the `seq` the
//! second was given claimed by a line nobody could read.

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::Project;
use serde_json::json;

use crate::fixtures;

fn make(ctx: &mut Ctx, name: &str) -> String {
    match ops::dispatch(ctx, "map_create", &json!({"name": name, "bounds": [8.0, 4.0, 8.0]})) {
        Ok(Applied::Done { operation_id, .. }) => operation_id,
        other => panic!("the create did not apply: {other:?}"),
    }
}

#[test]
fn a_log_cut_short_mid_line_keeps_the_next_operation_on_its_own_line() {
    let root = fixtures::Fixture::new(&format!("torn_oplog_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let project = app.world().resource::<Project>().clone();
    let mut ctx = Ctx { project: Some(project.clone()), session: "test".to_owned(), ..Ctx::default() };
    let path = ops::oplog_path(&project);

    make(&mut ctx, "first");
    make(&mut ctx, "second");
    let whole = std::fs::read_to_string(&path).unwrap();
    assert_eq!(whole.lines().count(), 2, "two operations, two lines");

    // A third write that died half way: no closing brace and no newline.
    let torn = &whole.lines().last().unwrap()[..40];
    std::fs::write(&path, format!("{whole}{torn}")).unwrap();

    let after = make(&mut ctx, "third");

    let entries = ops::read_entries(&path);
    assert_eq!(entries.len(), 3, "the two good lines and the new one read; only the torn text is lost");
    assert_eq!(entries.last().unwrap().operation_id, after, "the new entry is the one that read");
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 4, "the torn text is one damaged line of its own");
    assert!(text.ends_with('\n'));
}
