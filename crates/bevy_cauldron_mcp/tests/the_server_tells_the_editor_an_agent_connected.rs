//! **A server Cauldron's terminal started tells the editor its agent connected, and runs the self test
//! the agent is asked for** (`grimoire/agent_hello`, `grimoire_self_test`, `grimoire/self_test_result`).
//!
//! The server is started as an agent's client starts it, with the token a terminal tab gives it, against a
//! stub editor that records every BRP call it is sent.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// The token a terminal tab hands its agent's server.
const TOKEN: &str = "abc";
/// A 1x1 PNG, which is all a picture check reads the size of.
const ONE_PIXEL_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
/// How long the hello may take to reach the editor after the handshake.
const HELLO_WAIT: Duration = Duration::from_secs(5);

type Calls = Arc<Mutex<Vec<(String, Value)>>>;

/// One HTTP request's JSON-RPC body.
fn read_request(stream: &TcpStream) -> Value {
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
    serde_json::from_slice(&body).expect("the request is JSON")
}

/// What the stub editor answers each method with.
fn answer(method: &str) -> Value {
    match method {
        "grimoire/state" => json!({"screen": "launcher"}),
        "grimoire/operation_list" => json!({"operations": [{"name": "map_create"}, {"name": "map_set_settings"}]}),
        "bevy_fairy/screenshot" => json!({"id": 1}),
        "bevy_fairy/capture" => json!({"status": "done", "png_base64": ONE_PIXEL_PNG}),
        _ => json!({"known": true}),
    }
}

/// **A stub editor** that records every `(method, params)` it is sent; its port.
fn a_stub_editor(calls: Calls) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("a bound address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let calls = calls.clone();
            std::thread::spawn(move || {
                let request = read_request(&stream);
                let method = request["method"].as_str().unwrap_or_default().to_owned();
                let result = answer(&method);
                calls.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((method, request["params"].clone()));
                let body = json!({"jsonrpc": "2.0", "id": request["id"], "result": result}).to_string();
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    port
}

fn send(stdin: &mut std::process::ChildStdin, message: Value) {
    writeln!(stdin, "{message}").expect("the server's stdin is writable");
    stdin.flush().expect("the server's stdin flushes");
}

/// Read until the answer with the id asked for.
fn read(stdout: &mut BufReader<ChildStdout>, want: u64) -> Value {
    loop {
        let mut line = String::new();
        let n = stdout.read_line(&mut line).expect("the server's stdout is readable");
        assert!(n > 0, "the server's stdout ended before it answered");
        let value: Value = serde_json::from_str(line.trim_end()).expect("the server sent JSON");
        if value.get("id").and_then(Value::as_u64) == Some(want) {
            return value;
        }
    }
}

fn called(calls: &Calls, method: &str) -> Option<Value> {
    calls
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .find(|(m, _)| m == method)
        .map(|(_, params)| params.clone())
}

#[test]
fn the_handshake_is_told_to_the_editor_and_the_self_test_reports_its_three_checks() {
    let calls: Calls = Arc::default();
    let port = a_stub_editor(calls.clone());
    let mut child = Command::new(env!("CARGO_BIN_EXE_bevy_cauldron_mcp"))
        .env("BEVY_BRP_PORT", port.to_string())
        .env("CAULDRON_AGENT_TOKEN", TOKEN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the server binary starts");
    let mut stdin = child.stdin.take().expect("the server has a stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("the server has a stdout"));

    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "test-client", "version": "1.0"},
        }}),
    );
    let _ = read(&mut stdout, 1);
    send(&mut stdin, json!({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}));
    send(&mut stdin, json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}));
    let tools = read(&mut stdout, 2)["result"]["tools"].as_array().map(Vec::len).expect("the server lists its tools");

    let deadline = Instant::now() + HELLO_WAIT;
    let hello = loop {
        if let Some(hello) = called(&calls, "grimoire/agent_hello") {
            break hello;
        }
        assert!(Instant::now() < deadline, "the server never told the editor its agent connected");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(hello["token"], TOKEN);
    assert_eq!(hello["client"], json!({"name": "test-client", "version": "1.0"}));
    assert_eq!(hello["tools"], tools, "the hello counts the tools the client is offered");

    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "grimoire_self_test", "arguments": {}}}),
    );
    let answer = read(&mut stdout, 3);
    let _ = child.kill();
    let _ = child.wait();

    let content = answer["result"]["content"].as_array().expect("the self test answers with content");
    assert_eq!(content.first().map(|c| &c["type"]), Some(&json!("image")), "the picture comes first: {answer}");
    let text: Value = content
        .iter()
        .find(|c| c["type"] == "text")
        .and_then(|c| c["text"].as_str())
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or_else(|| panic!("the self test answers with its results as JSON text: {answer}"));
    let checks = text["checks"].as_array().expect("the results list their checks");
    assert_eq!(checks.len(), 3);
    assert!(checks.iter().all(|c| c["ok"] == true), "every check passes against a willing editor: {text}");
    assert!(checks.iter().any(|c| c["detail"] == "a 1x1 picture"), "the picture check reads the PNG's size: {text}");
    assert!(checks.iter().any(|c| c["detail"] == "the editor is on launcher"), "{text}");
    assert!(checks.iter().any(|c| c["detail"] == "2 operations"), "{text}");
    assert_eq!(text["told_the_editor"], true);

    let told = called(&calls, "grimoire/self_test_result").expect("the results were sent to the editor");
    assert_eq!(told["token"], TOKEN);
    assert_eq!(told["checks"], text["checks"], "the editor is told what the agent is told");
}
