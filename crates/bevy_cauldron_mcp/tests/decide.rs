//! **`grimoire_decide`'s logic against a stand-in for the recorded model.** No live model is called.

use std::collections::BTreeMap;

use bevy_grimoire_decide::{Answer, Error, Request, Response, Transport};
use bevy_cauldron_mcp::decide::{DecideParams, NamedImage, decide, load_checks, needs_view};
use serde_json::json;

/// Answers P(yes) 0.93 when an image carries the bytes `floating`, else 0.04; a choice gets its first option.
struct Scene;
impl Transport for Scene {
    fn endpoint(&self) -> String {
        "scene".into()
    }
    fn send(&self, r: &Request) -> Result<Response, Error> {
        use bevy_grimoire_decide::Question;
        use base64::Engine as _;
        let floating = r.images.iter().any(|i| {
            let b64 = i.split_once("base64,").map_or("", |x| x.1);
            base64::engine::general_purpose::STANDARD.decode(b64).is_ok_and(|b| b == b"floating")
        });
        let p = if floating { 0.93 } else { 0.04 };
        let answers = r
            .questions
            .iter()
            .map(|(id, q)| {
                let a = match q {
                    Question::Noul { .. } => Answer::Noul { noul: p },
                    Question::Choice { criteria, .. } => {
                        let first = criteria.keys().next().unwrap().clone();
                        Answer::Choice {
                            choice: first.clone(),
                            probabilities: criteria.keys().map(|k| (k.clone(), if *k == first { 0.8 } else { 0.2 })).collect(),
                            confidence: 0.8,
                        }
                    }
                    Question::Score { .. } => unreachable!(),
                };
                (id.clone(), a)
            })
            .collect::<BTreeMap<_, _>>();
        Ok(Response { model: None, answers, usage: None })
    }
}

fn project(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-decide-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets/grimoire")).unwrap();
    dir
}

fn named(name: &str, path: &str) -> NamedImage {
    NamedImage { name: name.into(), path: path.into() }
}

#[test]
fn a_named_check_reports_a_finding_on_a_floating_capture_and_none_on_a_grounded_one() {
    let dir = project("named");
    std::fs::write(dir.join("floating.png"), b"floating").unwrap();
    std::fs::write(dir.join("grounded.png"), b"grounded").unwrap();
    let ask = |file: &str| {
        let p = DecideParams {
            check: Some("floating".into()),
            images: vec![named("closeup", file)],
            facts: Some(json!({"op": "placement_add"})),
            ..Default::default()
        };
        decide(&p, &dir, None, &Scene).unwrap()
    };
    let hit = ask("floating.png");
    assert_eq!(hit["finding"]["found"], true);
    assert_eq!(hit["message"], "floating 0.93 > 0.70");
    assert_eq!(hit["advisory"], true);
    let ok = ask("grounded.png");
    assert_eq!(ok["finding"]["found"], false);
    assert!(ok["message"].is_null());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn ad_hoc_questions_return_every_probability() {
    let dir = project("adhoc");
    let p = DecideParams {
        questions: Some(
            [(
                "mood".to_owned(),
                json!({"type": "choice", "instructions": "Which?", "criteria": {"calm": "still", "busy": "moving"}}),
            )]
            .into(),
        ),
        state: Some("a village".into()),
        ..Default::default()
    };
    let out = decide(&p, &dir, None, &Scene).unwrap();
    let probs = &out["answers"]["mood"]["probabilities"];
    assert_eq!(probs["busy"], 0.8);
    assert_eq!(probs["calm"], 0.2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_check_without_its_evidence_is_refused_naming_it() {
    let dir = project("missing");
    let p = DecideParams { check: Some("floating".into()), ..Default::default() };
    let err = decide(&p, &dir, None, &Scene).unwrap_err();
    assert!(err.contains("closeup, facts"), "{err}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_image_outside_the_project_is_refused() {
    let dir = project("escape");
    std::fs::write(dir.parent().unwrap().join("grimoire-decide-outside.png"), b"x").unwrap();
    let p = DecideParams {
        check: Some("floating".into()),
        images: vec![named("closeup", "../grimoire-decide-outside.png")],
        facts: Some(json!({})),
        ..Default::default()
    };
    let err = decide(&p, &dir, None, &Scene).unwrap_err();
    assert!(err.contains("outside the project"), "{err}");
    std::fs::remove_file(dir.parent().unwrap().join("grimoire-decide-outside.png")).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_projects_checks_are_read_on_every_call_and_replace_the_editors_of_the_same_name() {
    let dir = project("hot");
    assert!(load_checks(&dir).unwrap().get("floating").unwrap().threshold > 0.5);
    std::fs::write(
        dir.join("assets/grimoire/checks.ron"),
        r#"(checks: [(name: "floating", question: "Mine?", threshold: 0.2), (name: "mine", question: "Q?", threshold: 0.5)])"#,
    )
    .unwrap();
    let c = load_checks(&dir).unwrap();
    assert_eq!(c.get("floating").unwrap().threshold, 0.2);
    assert!(c.get("mine").is_some() && c.get("blank_frame").is_some());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_view_is_taken_only_when_asked_for_or_a_check_needs_it_and_was_not_given_it() {
    let checks = bevy_grimoire_decide::Checks::builtin();
    let view_check = checks.checks.iter().find(|c| c.images.iter().any(|i| i == "view")).expect("a built-in needs view").name.clone();
    let p = DecideParams { check: Some(view_check.clone()), ..Default::default() };
    assert!(needs_view(&p, &checks));
    let given = DecideParams { check: Some(view_check), images: vec![named("view", "v.png")], ..Default::default() };
    assert!(!needs_view(&given, &checks));
    assert!(!needs_view(&DecideParams { check: Some("floating".into()), ..Default::default() }, &checks));
    assert!(needs_view(&DecideParams { view: true, ..Default::default() }, &checks));
}
