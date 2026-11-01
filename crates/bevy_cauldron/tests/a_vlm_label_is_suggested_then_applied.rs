//! **An agent can ask the model about a piece and apply the answer, as two operations** (E-043).
//!
//! The model is a loopback stub that answers one canned chat completion, so nothing leaves the machine.
//! `label_suggest` must write nothing; `label_apply` must write what was proposed, stamped with the model
//! that answered, and one undo must put the whole descriptor back.

use std::io::{Read, Write};

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

const ANSWERED: &str = "stub-vision-model";

/// One HTTP exchange on a loopback port, answering a fixed completion.
fn stub(content: &str) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let body = json!({"model": ANSWERED, "choices": [{"message": {"content": content}}]}).to_string();
    let handle = std::thread::spawn(move || {
        let Ok((mut sock, _)) = listener.accept() else { return };
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        loop {
            let Ok(n) = sock.read(&mut tmp) else { return };
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            let text = String::from_utf8_lossy(&buf);
            if let Some(end) = text.find("\r\n\r\n") {
                let len = text
                    .lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
                    .and_then(|l| l.split(':').nth(1))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if buf.len() >= end + 4 + len {
                    break;
                }
            }
        }
        let _ = write!(
            sock,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
    });
    (format!("http://127.0.0.1:{port}/v1/chat/completions"), handle)
}

fn context(tag: &str, url: &str) -> Ctx {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag)
        .kind_tokens(&["lamp"])
        .descriptor("bench", "alpha")
        .build("m");
    std::fs::write(
        root.join(".env"),
        format!("GRIMOIRE_VLM_KEY=k\nGRIMOIRE_VLM_URL={url}\nGRIMOIRE_VLM_MODEL=asked\n"),
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
    let (url, server) = stub(r#"{"what": "a wall lamp", "kind": ["lamp"]}"#);
    let mut ctx = context("vlm_label", &url);
    assert_eq!(kind_of(&ctx), vec!["prop".to_owned()]);

    let Ok(Applied::Done { outcome, .. }) = ops::dispatch(
        &mut ctx,
        "label_suggest",
        &json!({"id": "bench", "front": "front.png", "side": "side.png"}),
    ) else {
        panic!("label_suggest did not apply");
    };
    server.join().expect("stub");
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
    let mut ctx = context("vlm_label_path", "http://127.0.0.1:9/v1/chat/completions");
    for bad in ["/etc/passwd", "../front.png"] {
        let r = ops::dispatch(&mut ctx, "label_suggest", &json!({"id": "bench", "front": bad, "side": "side.png"}));
        let Err(e) = r else { panic!("`{bad}` was accepted") };
        assert!(e.0.contains("project-relative"), "{}", e.0);
    }
}

#[test]
fn a_proposal_that_needs_turning_is_refused() {
    let mut ctx = context("vlm_label_turn", "http://127.0.0.1:9/v1/chat/completions");
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
