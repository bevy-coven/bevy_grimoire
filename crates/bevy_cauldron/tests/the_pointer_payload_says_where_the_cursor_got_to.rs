//! **`grimoire/pointer` reports every step of the input chain, and each step can fail on its own**
//! (UI-30, #224).
//!
//! Three passes lost time to a live drag that does nothing while a headless one works. Every attempt
//! ended at the same wall: the editor's input state is **not observable over BRP**. Measured on a
//! running editor: `PointerLocation` came back `{}`, and no `bevy_fairy` resource was reflectable — so
//! a pass could not tell whether the recorder's cursor arrived, whether `view::Pointer` held a
//! position, or whether the picker had a location. Those are different failures and they look
//! identical from outside.
//!
//! This pins the payload's honesty: it never invents a step, and a missing one is `null` rather than a
//! zero that would read like a real position.

#[test]
fn a_missing_cursor_is_null_and_not_a_position_at_the_origin() {
    let payload = bevy_cauldron::ops_brp::pointer_payload(None, None, false);
    assert!(
        payload["cursor"].is_null(),
        "no cursor must read `null`: a `[0, 0]` would be a claim that the pointer is at the top-left \
         corner, which is a different and much more confusing failure: {payload}"
    );
    assert!(payload["target"].is_null(), "no location is no target");
    assert!(payload["position"].is_null(), "no location is no position");
    assert_eq!(payload["over_ui"], serde_json::json!(false));
}

#[test]
fn each_step_is_reported_separately_so_a_failure_can_be_located() {
    // **The failure pass 281 measured**: the cursor is known and the picker has nothing, which is what
    // a live editor showed and a headless app never does.
    let payload = bevy_cauldron::ops_brp::pointer_payload(
        Some(bevy::math::Vec2::new(218.5, 398.0)),
        None,
        false,
    );
    assert_eq!(payload["cursor"], serde_json::json!([218.5, 398.0]), "the cursor is `view::Pointer`'s");
    assert!(
        payload["target"].is_null(),
        "the picker has no location here — that is the fault, and it must be visible as one rather \
         than silently matching the cursor: {payload}"
    );

    // And the healthy case: every step filled, the picker on the surface image as the interface
    // camera is.
    let payload = bevy_cauldron::ops_brp::pointer_payload(
        Some(bevy::math::Vec2::new(218.5, 398.0)),
        Some(("Image(..)".to_owned(), bevy::math::Vec2::new(218.5, 398.0))),
        false,
    );
    assert_eq!(payload["target"], serde_json::json!("Image(..)"));
    assert_eq!(payload["position"], serde_json::json!([218.5, 398.0]));
}

/// **The picker's own state is the next question**, and pass 283 could not ask it: after the schedule
/// fix `inject_clicks` *sees* the press, yet the drag still does nothing. `pressing` and `dragging` are
/// `bevy_picking`'s own counts (`PointerButtonState`), and a press that arrives without a drag is
/// exactly `pressing = 1, dragging = 0` — the shape measured headlessly before the `Move` was written.
///
/// Reporting them turns "the drag does nothing" into one of two answers: the press never became picking
/// input (0 and 0), or it became a press and no drag (1 and 0), or the drag ran and the divider was not
/// its target (1 and 1 with no fraction change).
#[test]
fn the_picker_s_pressing_and_dragging_counts_are_reported() {
    let payload = bevy_cauldron::ops_brp::pointer_payload_with_buttons(None, None, false, None);
    assert!(
        payload["pressing"].is_null() && payload["dragging"].is_null(),
        "with no pointer state to read, both are `null` rather than a fabricated `0`: a real zero \
         means the picker looked and found nothing, which is a different answer: {payload}"
    );

    let payload = bevy_cauldron::ops_brp::pointer_payload_with_buttons(None, None, false, Some((1, 0)));
    assert_eq!(payload["pressing"], serde_json::json!(1));
    assert_eq!(
        payload["dragging"],
        serde_json::json!(0),
        "a press with no drag is the shape that made four passes think the press was lost"
    );
}
