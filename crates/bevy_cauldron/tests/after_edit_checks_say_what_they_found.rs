//! **The after-edit checks, run on an agent's operation, name a finding, say what they could not ask, and
//! stay silent about what is clear (#111).** No model: the transport answers by what the picture says.

use std::collections::BTreeMap;

use bevy_grimoire_decide::{Answer, Checks, Error, Request, Response, Transport};
use bevy_cauldron::after_edit::{AgentEdit, run_due};

struct Scene;
impl Transport for Scene {
    fn endpoint(&self) -> String {
        "scene".into()
    }
    fn send(&self, r: &Request) -> Result<Response, Error> {
        let p = if r.images.iter().any(|i| i.contains("floating")) { 0.93 } else { 0.04 };
        let answers = r.questions.keys().map(|id| (id.clone(), Answer::Noul { noul: p })).collect();
        Ok(Response { model: None, answers, usage: None })
    }
}

struct Down;
impl Transport for Down {
    fn endpoint(&self) -> String {
        "http://down".into()
    }
    fn send(&self, _: &Request) -> Result<Response, Error> {
        Err(Error::Unreachable { endpoint: self.endpoint(), detail: "refused".into() })
    }
}

fn edit() -> AgentEdit {
    AgentEdit {
        seq: 7,
        op: "placement_add".into(),
        operation_id: "op-7".into(),
        params: serde_json::json!({"piece": "barrel", "at": [1.0, 2.0]}),
        result: "placed barrel".into(),
    }
}

fn closeup(img: &'static str) -> impl Fn(&AgentEdit) -> BTreeMap<String, String> {
    move |_| [("closeup".to_string(), img.to_string())].into()
}

#[test]
fn a_floating_closeup_is_named_in_the_journal_and_the_checks_it_could_not_ask_are_said() {
    let reports = run_due(&[edit()], &Checks::builtin(), &Scene, &closeup("data:floating"));
    assert_eq!(reports.len(), 1);
    let r = &reports[0];
    assert_eq!(r.skipped, ["out_of_area", "toast_visible"], "need before/after and the view, which were not captured");
    assert!(r.errors.is_empty());
    let lines = r.journal_lines();
    assert!(lines.contains(&"floating 0.93 > 0.70 (after placement_add)".to_string()), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains("out_of_area") && l.contains("cannot capture")), "{lines:?}");
}

#[test]
fn a_grounded_closeup_says_nothing_about_the_checks_that_ran() {
    let r = &run_due(&[edit()], &Checks::builtin(), &Scene, &closeup("data:grounded"))[0];
    assert_eq!(r.findings.len(), 2, "floating and clipping answered");
    assert!(r.findings.iter().all(|f| !f.found));
    assert!(r.journal_lines().iter().all(|l| !l.contains("0.")), "no finding line: {:?}", r.journal_lines());
}

#[test]
fn an_endpoint_that_does_not_answer_is_an_error_line_never_a_finding() {
    let r = &run_due(&[edit()], &Checks::builtin(), &Down, &closeup("data:floating"))[0];
    assert!(r.findings.is_empty());
    assert_eq!(r.errors.len(), 2);
    assert!(r.errors[0].contains("http://down"), "{:?}", r.errors);
}
