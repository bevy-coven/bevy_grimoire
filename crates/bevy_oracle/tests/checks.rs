use bevy_oracle::*;
use std::collections::BTreeMap;

/// A model stand-in for the recorded exchanges of a labelled capture set: it answers by what the image
/// string says, and records nothing else. The real model is called only in calibration runs.
struct Scene;
impl Transport for Scene {
    fn endpoint(&self) -> String {
        "scene".into()
    }
    fn send(&self, r: &Request) -> Result<Response, Error> {
        let floating = r.images.iter().any(|i| i.contains("floating"));
        let p = if floating { 0.93 } else { 0.04 };
        let answers = r
            .questions
            .keys()
            .map(|id| (id.clone(), Answer::Noul { noul: p }))
            .collect::<BTreeMap<_, _>>();
        Ok(Response { model: None, answers, usage: None })
    }
}

fn evidence(img: &str) -> Evidence {
    Evidence {
        images: [("closeup".to_string(), img.to_string())].into(),
        facts: Some(serde_json::json!({"op": "placement_add", "piece": "barrel", "y": 3.0})),
    }
}

#[test]
fn builtin_checks_parse_and_name_the_five() {
    let c = Checks::builtin();
    let names: Vec<_> = c.checks.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["floating", "clipping", "out_of_area", "blank_frame", "toast_visible"]);
    assert!(c.checks.iter().all(|c| c.calibration.is_none()), "none has earned a calibration yet");
}

#[test]
fn a_floating_capture_is_a_finding_and_a_grounded_one_is_not() {
    let floating = Checks::builtin().get("floating").unwrap().clone();
    let hit = floating.run(&Scene, &evidence("data:floating")).unwrap();
    assert!(hit.found);
    assert_eq!(hit.message().as_deref(), Some("floating 0.93 > 0.70"));
    let ok = floating.run(&Scene, &evidence("data:grounded")).unwrap();
    assert!(!ok.found);
    assert_eq!(ok.message(), None);
}

#[test]
fn a_check_missing_an_image_or_the_facts_says_so_instead_of_answering() {
    let floating = Checks::builtin().get("floating").unwrap().clone();
    let none = Evidence::default();
    match floating.run(&Scene, &none) {
        Err(Error::MissingEvidence { missing, .. }) => assert_eq!(missing, "closeup, facts"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_request_sends_images_in_the_checks_order_and_the_facts() {
    let c = Checks::builtin().get("out_of_area").unwrap().clone();
    let ev = Evidence {
        images: [("after".into(), "A".into()), ("before".into(), "B".into())].into(),
        facts: Some(serde_json::json!({"area": [0, 0, 4, 4]})),
    };
    let r = c.request(&ev).unwrap();
    assert_eq!(r.images, ["B", "A"]);
    assert!(r.state.contains("before, after") && r.state.contains("\"area\""));
}

#[test]
fn a_games_check_replaces_the_editors_of_the_same_name_and_adds_its_own() {
    let game = Checks::from_ron(
        r#"(checks: [(name: "floating", question: "Floating?", threshold: 0.5),
                     (name: "lava", question: "Is there lava?", threshold: 0.9)])"#,
    )
    .unwrap();
    let m = Checks::builtin().merged(game);
    assert_eq!(m.get("floating").unwrap().threshold, 0.5);
    assert!(m.get("lava").is_some() && m.get("clipping").is_some());
    assert_eq!(m.checks.len(), 6);
}

#[test]
fn checks_ron_rejects_duplicates_and_thresholds_outside_zero_to_one() {
    let dup = r#"(checks: [(name: "a", question: "q", threshold: 0.5), (name: "a", question: "q", threshold: 0.5)])"#;
    assert!(matches!(Checks::from_ron(dup), Err(Error::Checks(m)) if m.contains("two checks")));
    let hi = r#"(checks: [(name: "a", question: "q", threshold: 1.5)])"#;
    assert!(matches!(Checks::from_ron(hi), Err(Error::Checks(m)) if m.contains("threshold")));
}

#[test]
fn only_a_calibrated_check_at_the_bar_may_block() {
    let mut c = Checks::builtin().get("floating").unwrap().clone();
    assert!(!c.may_block(0.9));
    c.calibration = Some(Calibration {
        check: "floating".into(),
        model: "clef-flash".into(),
        set_size: 20,
        agreement: 0.95,
        threshold: 0.7,
        date: "2026-10-29".into(),
    });
    assert!(c.may_block(0.9) && !c.may_block(0.96));
    let round = Checks { checks: vec![c.clone()], ..Checks::default() };
    assert_eq!(Checks::from_ron(&round.to_ron().unwrap()).unwrap(), round);
}
