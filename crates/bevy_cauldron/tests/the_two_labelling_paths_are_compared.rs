//! **On the same labelled kit, the decision path's agreement is measured against the old path's, and
//! the record is a file this test reads** (D5, #113's first acceptance item).
//!
//! Both paths answer a labelled set of real pieces: `decide` with closed `choice` questions over the
//! project's vocabulary, `vlm` with the free-form JSON that `vlm.rs` asks for. A run writes
//! `fixtures/label_comparison.ron`; this test keeps the record honest, and the old path may only be
//! removed in a commit whose record shows the new one equal or better.
//!
//! The record is written by a run that needs a served model, which a test may not make (the repo's
//! rule: tests never call a live model). So the test reads, and refuses a record whose numbers do not
//! add up or whose set is not the one committed.
//!
//! **What the `vlm` number is, and is not.** A decision endpoint cannot be asked for free-form text, so
//! the run cannot reproduce `vlm.rs`'s prompt; it asks both paths the same closed question over the same
//! options and compares the decisions. That makes `vlm`'s figure a **ceiling**: the real path also pays a
//! reprompt for every token it generates that the vocabulary rejects. It is enough for D5's rule ("the old
//! path goes when the new one is equal or better") and not enough to claim the old path scored that well;
//! `examples/label_compare.rs` says the same where the number is made.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy_cauldron::label_decide::Comparison;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

/// A piece in the committed kit. Only `labels` is read here — the count and the axes both come from
/// it — but the whole entry must parse, so a kit that has drifted from `LabelledPiece`'s shape fails
/// here rather than silently in a run.
#[derive(Debug, serde::Deserialize)]
struct Labelled {
    #[allow(dead_code)]
    pub state: String,
    pub labels: BTreeMap<String, String>,
}

#[test]
fn the_decision_path_s_agreement_is_recorded_against_the_old_path_s() {
    let text = std::fs::read_to_string(fixture("label_comparison.ron")).unwrap_or_else(|e| {
        panic!(
            "fixtures/label_comparison.ron: {e}. The comparison is written by a run: `decide_check`'s \
             sibling for labelling, or a recorded exchange; a number typed nowhere is not a measurement"
        )
    });
    let record: Comparison = ron::from_str(&text).unwrap_or_else(|e| panic!("{e}"));

    // **The same set for both paths**, which is the acceptance item's own words. A comparison over two
    // different sets compares nothing.
    let set: Vec<Labelled> = ron::from_str(
        &std::fs::read_to_string(fixture("label_kit.ron")).expect("fixtures/label_kit.ron"),
    )
    .expect("fixtures/label_kit.ron parses");
    assert_eq!(
        record.pieces,
        set.len(),
        "the record compares {} pieces and the committed set holds {}",
        record.pieces,
        set.len()
    );
    // **Every piece labels at least one axis**, and no label names an axis the labeller does not ask
    // about — otherwise a piece contributes nothing and the agreement is measured over less than the
    // record claims.
    for piece in &set {
        assert!(
            !piece.labels.is_empty(),
            "`{}` has no labels, so it measures neither path",
            piece.state
        );
        for axis in piece.labels.keys() {
            assert!(
                bevy_cauldron::label_decide::axes().iter().any(|(f, _)| f == axis),
                "`{}` labels `{axis}`, which is not an axis the labeller asks about",
                piece.state
            );
        }
    }

    // Both paths must have answered, and the counts must be consistent with their own totals.
    for (name, right, asked) in [
        ("decide", record.decide_right, record.decide_asked),
        ("vlm", record.vlm_right, record.vlm_asked),
    ] {
        assert!(asked > 0, "the {name} path answered nothing, so its agreement means nothing");
        assert!(
            right <= asked,
            "the {name} path claims {right} right of {asked} asked; more right than asked is a typo"
        );
        let model = if name == "decide" { &record.decide_model } else { &record.vlm_model };
        assert!(!model.is_empty(), "the record names the {name} path's model");
    }

    // **The item's own rule, stated as a check:** the old path may not be removed while the new one is
    // worse. A record that shows `decide` behind is allowed to exist — it is the measurement — but this
    // test then says so loudly, because the next step is to improve the questions rather than to delete
    // the only path that works.
    assert!(
        record.decide() >= record.vlm() - 1e-9,
        "the decision path agrees on {:.0}% and the old path on {:.0}%: the old path may only be \
         removed when the new one is equal or better, and this record says it is not — so it stays, \
         and the questions are what needs the work (D5's second acceptance item)",
        record.decide() * 100.0,
        record.vlm() * 100.0
    );
}
