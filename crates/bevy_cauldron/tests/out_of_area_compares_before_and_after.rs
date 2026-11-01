//! **`out_of_area` asks about the before and after pictures together, with the edit's facts (#111).**
//! No model: the transport reports what it was sent.

use std::collections::BTreeMap;
use std::sync::Mutex;

use bevy_grimoire_decide::{Answer, Checks, Error, Request, Response, Transport};
use bevy_cauldron::after_edit::{AgentEdit, run_due};

#[derive(Default)]
struct Spy(Mutex<Vec<Request>>);
impl Transport for Spy {
    fn endpoint(&self) -> String {
        "spy".into()
    }
    fn send(&self, r: &Request) -> Result<Response, Error> {
        self.0.lock().unwrap().push(r.clone());
        let p = if r.images.len() == 2 && r.images[0] != r.images[1] { 0.9 } else { 0.05 };
        let answers = r.questions.keys().map(|id| (id.clone(), Answer::Noul { noul: p })).collect();
        Ok(Response { model: None, answers, usage: None })
    }
}

fn edit() -> AgentEdit {
    AgentEdit {
        seq: 3,
        op: "placement_add".into(),
        operation_id: "op-3".into(),
        params: serde_json::json!({"piece": "barrel", "at": [1.0, 2.0]}),
        result: "placed barrel".into(),
    }
}

fn pics(before: &'static str, after: &'static str) -> impl Fn(&AgentEdit) -> BTreeMap<String, String> {
    move |_| [("before".to_string(), before.to_string()), ("after".to_string(), after.to_string())].into()
}

#[test]
fn a_changed_picture_is_a_finding_and_the_request_carries_both_pictures_and_the_edit() {
    let spy = Spy::default();
    let r = &run_due(&[edit()], &Checks::builtin(), &spy, &pics("data:a", "data:b"))[0];
    assert!(r.skipped.iter().all(|s| s != "out_of_area"), "{:?}", r.skipped);
    let f = r.findings.iter().find(|f| f.check == "out_of_area").expect("out_of_area answered");
    assert!(f.found);
    let sent = spy.0.lock().unwrap();
    let req = sent.iter().find(|q| q.images.len() == 2).expect("a two-picture request");
    assert_eq!(req.images, ["data:a", "data:b"], "before first, then after");
    assert!(serde_json::to_string(req).unwrap().contains("op-3"), "the facts ride along");
}

#[test]
fn identical_pictures_are_clear() {
    let r = &run_due(&[edit()], &Checks::builtin(), &Spy::default(), &pics("data:a", "data:a"))[0];
    let f = r.findings.iter().find(|f| f.check == "out_of_area").unwrap();
    assert!(!f.found);
}
