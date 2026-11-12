//! **An agent can ask the model about a piece and apply the answer, as two operations** (E-043).
//!
//! There is one labelling path since #113: the decision model, which answers a typed question with a
//! probability per vocabulary token and generates nothing to parse. The endpoint is a loopback stub
//! that answers one decision request, so nothing leaves the machine and no live model is called.
//! `label_suggest` must write nothing; `label_apply` must write what was proposed, stamped with the
//! model that answered, and one undo must put the whole descriptor back.

use std::io::{Read, Write};

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::{Value, json};

use crate::fixtures;

/// The model the stub's response names — never the one the request asked for.
const ANSWERED: &str = "stub-decider";

/// **One decision endpoint on a loopback port**, answering a request of *any* shape: every `noul`
/// is answered with P(yes), every `choice` with its first option at 0.9. It returns the endpoint URL
/// and a handle yielding the request body it was sent, so a test can read what was asked.
fn stub(kind_tokens: &'static [&'static str]) -> (String, std::thread::JoinHandle<Value>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let handle = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().expect("a request arrives");
        let mut buf = Vec::new();
        let mut tmp = [0u8; 8192];
        let body_at;
        loop {
            let n = sock.read(&mut tmp).expect("the request reads");
            if n == 0 {
                panic!("the connection closed before a whole request arrived");
            }
            buf.extend_from_slice(&tmp[..n]);
            let text = String::from_utf8_lossy(&buf).to_string();
            if let Some(end) = text.find("\r\n\r\n") {
                let len = text
                    .to_ascii_lowercase()
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>()))
                    .transpose()
                    .expect("a length")
                    .unwrap_or(0);
                if buf.len() >= end + 4 + len {
                    body_at = end + 4;
                    break;
                }
            }
        }
        let request: Value = serde_json::from_slice(&buf[body_at..]).expect("the request body is JSON");
        // **Answer each question in its own kind.** A `noul` wants P(yes); a `choice` wants one
        // option and a probability per option; answering everything as a `noul` is refused.
        let mut answers = serde_json::Map::new();
        for (id, q) in request["questions"].as_object().expect("questions").iter() {
            let answer = match q["type"].as_str() {
                Some("noul") => {
                    let named = id.split_once(':').is_some_and(|(_, t)| kind_tokens.contains(&t));
                    json!({"type": "noul", "noul": if named { 0.97 } else { 0.02 }})
                }
                Some("choice") => {
                    let criteria = q["criteria"].as_object().expect("criteria");
                    let pick = criteria.keys().next().expect("an option").clone();
                    let probabilities: serde_json::Map<String, Value> = criteria
                        .keys()
                        .map(|o| (o.clone(), json!(if *o == pick { 0.9 } else { 0.01 })))
                        .collect();
                    json!({"type": "choice", "choice": pick, "probabilities": probabilities, "confidence": 0.9})
                }
                other => panic!("the path asked a question kind this stub has no answer for: {other:?}"),
            };
            answers.insert(id.clone(), answer);
        }
        let body = json!({"model": ANSWERED, "answers": answers}).to_string();
        write!(
            sock,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .expect("the answer writes");
        request
    });
    (format!("http://127.0.0.1:{port}/v1/systemone"), handle)
}

fn context(tag: &str, url: &str) -> Ctx {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag)
        .kind_tokens(&["lamp"])
        .descriptor("bench", "alpha")
        .build("m");
    // **`GRIMOIRE_LABEL_PATH=chat` is in the file on purpose**: there is one path now, and a
    // setting left behind in a project's `.env` must select nothing rather than quietly choose
    // a second one.
    std::fs::write(
        root.join(".env"),
        format!("GRIMOIRE_LABEL_PATH=chat\nGRIMOIRE_DECIDE_ENDPOINT={url}\n"),
    )
    .expect(".env");
    for shot in ["front.png", "side.png"] {
        std::fs::write(root.join(shot), b"\x89PNG\r\n\x1a\nnot a real picture").expect("shot");
    }
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() }
}

fn kind_of(ctx: &Ctx) -> Vec<String> {
    ctx.project.as_ref().unwrap().measured.descriptors.iter().find(|d| d.id == "bench").unwrap().kind.clone()
}

#[test]
fn a_suggestion_writes_nothing_and_its_apply_is_stamped_and_undoable() {
    let (url, server) = stub(&["lamp"]);
    let mut ctx = context("label_suggest_apply", &url);
    assert_eq!(kind_of(&ctx), vec!["prop".to_owned()]);

    let Ok(Applied::Done { outcome, .. }) = ops::dispatch(
        &mut ctx,
        "label_suggest",
        &json!({"id": "bench", "front": "front.png", "side": "side.png"}),
    ) else {
        panic!("label_suggest did not apply");
    };
    // The request really was a decision request over the project's own vocabulary.
    let asked = server.join().expect("stub");
    let questions = asked["questions"].as_object().expect("questions");
    assert!(
        questions.keys().any(|id| id == "kind:lamp"),
        "the one path asks a noul per vocabulary token: {:?}",
        questions.keys().collect::<Vec<_>>()
    );

    let after = outcome.after.expect("the proposal rides in `after`");
    assert_eq!(after["suggestion"]["kind"], json!(["lamp"]));
    assert_eq!(after["provenance"]["model"], ANSWERED, "the model that answered, not the one asked");
    assert_eq!(kind_of(&ctx), vec!["prop".to_owned()], "a suggestion must not write the piece");

    let Ok(Applied::Done { operation_id, .. }) = ops::dispatch(
        &mut ctx,
        "label_apply",
        &json!({"id": "bench", "suggestion": after["suggestion"], "provenance": after["provenance"]}),
    ) else {
        panic!("label_apply did not apply");
    };
    assert_eq!(kind_of(&ctx), vec!["lamp".to_owned()]);
    let labels = ctx.project.as_ref().unwrap().measured.descriptors.iter().find(|d| d.id == "bench").unwrap().labels.clone();
    assert_eq!(labels.expect("a label record").model.as_deref(), Some(ANSWERED));

    ops::undo(&mut ctx, Some(&operation_id)).unwrap_or_else(|e| panic!("undo refused: {}", e.0));
    assert_eq!(kind_of(&ctx), vec!["prop".to_owned()], "the undo did not put the descriptor back");
}

#[test]
fn a_shot_outside_the_project_is_not_sent() {
    let mut ctx = context("label_suggest_path", "http://127.0.0.1:9/v1/systemone");
    for bad in ["/etc/passwd", "../front.png"] {
        let r = ops::dispatch(&mut ctx, "label_suggest", &json!({"id": "bench", "front": bad, "side": "side.png"}));
        let Err(e) = r else { panic!("`{bad}` was accepted") };
        assert!(e.0.contains("project-relative"), "{}", e.0);
    }
}

#[test]
fn a_proposal_that_needs_turning_is_refused() {
    let mut ctx = context("label_suggest_turn", "http://127.0.0.1:9/v1/systemone");
    let suggestion = json!({
        "what": "a lamp", "kind": ["lamp"], "effects": [], "look": [], "offers_surfaces": [],
        "mount": null, "front": null,
        "needs_turn": {"axis": "x", "turns": 1, "why": "on its side"},
        "note": null, "rooms": [], "group": null, "confidence": "High", "token_proposals": []
    });
    let r = ops::dispatch(
        &mut ctx,
        "label_apply",
        &json!({"id": "bench", "suggestion": suggestion, "provenance": {"model": "m", "date": "2026-10-30", "attempts": 1}}),
    );
    let Err(e) = r else { panic!("a proposal needing a turn was applied") };
    assert!(e.0.contains("needs turning"), "{}", e.0);
    assert_eq!(kind_of(&ctx), vec!["prop".to_owned()]);
}
