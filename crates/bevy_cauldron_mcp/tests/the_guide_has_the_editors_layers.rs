//! **`grimoire_guide` serves three layers, and says so when it can only serve one** (#347, AG-02).
//!
//! The shipped guide is compiled into the server; the person's and the project's are read by the
//! **editor**, which is the only program that knows where they are (`grimoire/agent_guide`). So this
//! server puts its own layer first and appends what the editor read — which means the one case that can
//! go wrong without an editor is the one this test pins: an agent must be told the two other layers were
//! **not read**, not left to infer that they are empty.
//!
//! **The server is called as an outside agent calls it**: a subprocess, MCP over stdio, on a port nothing
//! listens on.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("an ephemeral port");
    listener.local_addr().expect("the bound port").port()
}

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Server {
    fn start() -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_bevy_cauldron_mcp"))
            .env("BEVY_BRP_PORT", closed_port().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the server binary starts");
        let stdin = child.stdin.take().expect("the server has a stdin");
        let stdout = BufReader::new(child.stdout.take().expect("the server has a stdout"));
        let mut server = Server { child, stdin, stdout, next_id: 0 };
        let answer = server.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "bevy_grimoire's guide-layer test", "version": "0.1.0"},
            }),
        );
        assert!(answer.get("error").is_none(), "the handshake was refused: {answer}");
        server.notify("notifications/initialized", json!({}));
        server
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let body = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.stdin, "{body}").expect("the server's stdin is writable");
        self.stdin.flush().expect("the server's stdin flushes");
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).expect("the server's stdout is readable");
            assert!(read > 0, "the server's stdout ended before it answered");
            let value: Value = serde_json::from_str(line.trim_end())
                .unwrap_or_else(|e| panic!("a line that is not JSON: {line} ({e})"));
            if value.get("id").and_then(|i| i.as_u64()) == Some(id) {
                return value;
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        let body = json!({"jsonrpc": "2.0", "method": method, "params": params});
        writeln!(self.stdin, "{body}").expect("the server's stdin is writable");
        self.stdin.flush().expect("the server's stdin flushes");
    }

    fn text_of(&self, answer: &Value) -> String {
        assert!(
            answer["result"]["isError"].as_bool() != Some(true),
            "the tool refused: {answer}"
        );
        answer["result"]["content"]
            .as_array()
            .and_then(|blocks| blocks.first())
            .and_then(|block| block["text"].as_str())
            .unwrap_or_else(|| panic!("the answer carries no text: {answer}"))
            .to_owned()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// **With no editor the shipped layer is whole, and the two files are named as not read.**
///
/// An agent whose guide silently omitted a person's rules would follow the shipped ones and wonder why
/// the person kept correcting it. The sentence is what makes the absence readable, and it names both
/// paths so the agent can read them itself.
#[test]
fn a_guide_read_with_no_editor_says_the_persons_and_the_projects_files_were_not_read() {
    let mut server = Server::start();
    let answer = server.request(
        "tools/call",
        json!({"name": "grimoire_guide", "arguments": {}}),
    );
    let guide = server.text_of(&answer);
    assert!(
        guide.contains("Working a note"),
        "the shipped guide is what the server returns first: {guide}"
    );
    assert!(
        guide.contains("AGENTS.md"),
        "the answer names the files it could not read: {guide}"
    );
    assert!(
        guide.contains(".dark_factory/AGENTS.md"),
        "and the project's path, which an agent cannot guess: {guide}"
    );
    // **The sentence is about the layers and not a failure of the tool**: an agent must still be able
    // to work from what it was given.
    assert!(
        answer["result"]["isError"].as_bool() != Some(true),
        "a guide read with no editor is still a guide: {answer}"
    );
}

/// **The tool's description says what it now returns**, read from the server's own declarations.
///
/// The reference and the server are two copies of one vocabulary (`docs.rs`), and the two are compared
/// by `docs_are_current`; this test reads the *server's* copy, so an agent reading `tools/list` learns
/// that the answer is three layers and not one string.
#[test]
fn the_tools_declaration_says_the_answer_covers_the_persons_and_the_projects_guides() {
    let mut server = Server::start();
    let listed = server.request("tools/list", json!({}));
    let tool = listed["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("no tool list: {listed}"))
        .iter()
        .find(|tool| tool["name"] == "grimoire_guide")
        .unwrap_or_else(|| panic!("the server does not offer `grimoire_guide`"));
    let description = tool["description"].as_str().unwrap_or_default();
    assert!(
        description.contains("person's own guide") && description.contains("project's"),
        "the description does not say the answer carries the other two layers: {description}"
    );
}
