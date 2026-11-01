//! **An operation waiting for the editor's picture is asked about once: with the picture when it lands,
//! without it when it never does (#111).** A late picture for an operation already released is dropped.

use bevy_oracle::Checks;
use bevy_cauldron::after_edit::{AgentEdit, SHOT_FRAMES, ShotWait, wants_view};

fn edit(seq: u64) -> AgentEdit {
    AgentEdit {
        seq,
        op: "placement_add".into(),
        operation_id: format!("op-{seq}"),
        params: serde_json::json!({}),
        result: "ok".into(),
    }
}

#[test]
fn the_builtin_checks_want_the_view_and_a_set_without_one_does_not() {
    assert!(wants_view(&Checks::builtin()), "toast_visible needs `view`");
    assert!(!wants_view(&Checks::default()));
}

#[test]
fn a_picture_that_lands_releases_its_operations_once() {
    let mut w = ShotWait::default();
    let t = w.hold(vec![edit(1)]);
    assert!(w.tick().is_empty());
    assert_eq!(w.deliver(t).map(|e| e.len()), Some(1));
    assert!(w.deliver(t).is_none(), "a second picture for the ticket asks nothing again");
}

#[test]
fn a_picture_that_never_lands_releases_after_the_bound_and_a_late_one_is_dropped() {
    let mut w = ShotWait::default();
    let t = w.hold(vec![edit(2)]);
    for _ in 0..SHOT_FRAMES - 1 {
        assert!(w.tick().is_empty());
    }
    let released = w.tick();
    assert_eq!(released.len(), 1);
    assert_eq!(released[0][0].seq, 2);
    assert!(w.deliver(t).is_none());
}
