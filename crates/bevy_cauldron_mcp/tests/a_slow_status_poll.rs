//! **#179: a status poll the busy editor does not answer within its budget is not the end of a
//! capture.**
//!
//! `grimoire_capture_sequence` polls `grimoire/capture_sequence_status` until the sequence ends. On a
//! loaded machine one poll can take longer than the five-second read budget while the editor is
//! still writing frames; measured on bmb (load 17) three clips in a row were abandoned that way.
//! These tests stand a stub editor on a local port: it reads each JSON-RPC request, and either
//! holds the connection past the budget without answering or answers it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use bevy_cauldron_mcp::brp::Brp;
use bevy_cauldron_mcp::sequence::wait_for_sequence;
use serde_json::{Value, json};

/// **Twice the read budget (five seconds)**, so a poll the stub holds runs out of time and the
/// deadline test has five seconds of headroom on a loaded machine.
const STALL: Duration = Duration::from_secs(10);

/// **Read one HTTP request to the end of its body**, so the stub never answers or closes a
/// connection the client is still writing to.
fn read_request(stream: &TcpStream) {
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("a request header line");
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().expect("a content length");
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).expect("the request body");
}

/// **A stub editor.** The `n`th connection (from 0) is answered with `answer(n)`'s JSON-RPC result,
/// or held open and unanswered for [`STALL`] when it gives `None`. Each connection has its own
/// thread, so a held one does not keep the next poll from being answered.
fn a_stub_editor(answer: fn(usize) -> Option<Value>) -> Brp {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("a bound address").port();
    std::thread::spawn(move || {
        for (n, stream) in listener.incoming().enumerate() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                read_request(&stream);
                let Some(result) = answer(n) else {
                    std::thread::sleep(STALL);
                    return;
                };
                let body = json!({"jsonrpc": "2.0", "id": 1, "result": result}).to_string();
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
                     Connection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    Brp::new(format!("http://127.0.0.1:{port}/"))
}

/// **The issue's acceptance: one poll stalls past the budget, the next answers done, and the wait
/// answers done with the frames** rather than refusing on the stalled poll.
#[tokio::test]
async fn a_poll_that_runs_out_of_its_budget_is_still_running() {
    let editor = a_stub_editor(|n| {
        (n > 0).then(|| {
            json!({
                "status": "done",
                "frames": [{"index": 0, "path": "/tmp/f000.png"}, {"index": 1, "path": "/tmp/f001.png"}],
            })
        })
    });
    let deadline = Instant::now() + Duration::from_secs(30);

    let status = wait_for_sequence(&editor, 7, deadline)
        .await
        .expect("a poll the editor was too busy to answer must not end the capture");

    assert_eq!(status["status"], "done", "the status read: {status}");
    assert_eq!(
        status["frames"].as_array().map(Vec::len),
        Some(2),
        "the frames of the answered poll come back: {status}"
    );
}

/// **The deadline still holds.** An editor that never answers a poll in time must not keep the
/// call waiting past the sequence's deadline: the wait refuses, and says how many polls went
/// unanswered.
#[tokio::test]
async fn polls_that_never_answer_end_at_the_deadline() {
    let editor = a_stub_editor(|_| None);
    let started = Instant::now();

    let refusal = wait_for_sequence(&editor, 7, started + Duration::from_secs(1))
        .await
        .expect_err("no poll was answered, so there is no status to give");

    let took = started.elapsed();
    assert!(
        took < STALL,
        "the first poll's budget (5 s) already passes the 1 s deadline, so the wait ends then, and it took {took:?}"
    );
    assert!(
        refusal.contains("1 status poll"),
        "the refusal says how many polls went unanswered, and read: {refusal}"
    );
}
