//! **Two agents writing one project's operation log never claim the same `seq` (E-004).**
//!
//! The sequence number was counted from the file before the line was appended, so writers that
//! counted at the same moment wrote the same number — and the same `operation_id`, which is what an
//! agent cites to undo a change.

use bevy_cauldron::ops::{self, Ctx, Source};
use serde_json::json;

use crate::fixtures;

fn writer(root: std::path::PathBuf, session: String) -> Ctx {
    Ctx {
        root: Some(root),
        project: None,
        open: None,
        source: Source::Human,
        session,
        proposal: None,
    }
}

#[test]
fn concurrent_writers_get_distinct_seq_and_operation_ids() {
    let root = fixtures::Fixture::new("two_writers_seq").build("m");
    const WRITERS: usize = 8;
    const EACH: usize = 6;
    let handles: Vec<_> = (0..WRITERS)
        .map(|w| {
            let root = root.clone();
            std::thread::spawn(move || {
                let mut ctx = writer(root, format!("agent{w}"));
                (0..EACH)
                    .map(|_| {
                        match ops::dispatch(&mut ctx, "door_open", &json!({"door": "kit"}))
                            .unwrap_or_else(|e| panic!("door_open refused: {e}"))
                        {
                            ops::Applied::Done { entry, .. } => (entry.seq, entry.operation_id),
                            other => panic!("not applied: {other:?}"),
                        }
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let mut returned: Vec<(u64, String)> =
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect();
    let total = WRITERS * EACH;
    let on_disk = ops::read_entries(&ops::oplog_path_at(&root));
    assert_eq!(on_disk.len(), total, "every change is in the log");
    let mut seqs: Vec<u64> = on_disk.iter().map(|e| e.seq).collect();
    seqs.sort();
    assert_eq!(seqs, (0..total as u64).collect::<Vec<_>>(), "seq is 0..n with no repeat");
    let mut ids: Vec<&str> = on_disk.iter().map(|e| e.operation_id.as_str()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), total, "operation ids are unique");
    returned.sort();
    for (seq, id) in &returned {
        assert!(
            on_disk.iter().any(|e| e.seq == *seq && &e.operation_id == id),
            "the id a caller was handed ({id}) is the one on disk"
        );
    }
}
