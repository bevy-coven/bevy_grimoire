//! `grimoire_list_pieces` takes `unconfirmed` as another spelling of `by_model`: the filter matches
//! agent-written axes too, so the model-only name misleads (E-052).

use super::PiecesParams;

#[test]
fn unconfirmed_is_the_by_model_filter() {
    let p: PiecesParams = serde_json::from_value(serde_json::json!({"unconfirmed": true})).unwrap();
    assert!(p.by_model);
    let p: PiecesParams = serde_json::from_value(serde_json::json!({"by_model": true})).unwrap();
    assert!(p.by_model);
}
