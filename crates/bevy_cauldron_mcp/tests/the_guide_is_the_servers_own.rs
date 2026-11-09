//! **`grimoire_guide`: the shipped guide, served by the server, and named first.**
//!
//! The issue (#346, AG-01): a hand-written guide to the editor's tools, shipped with the MCP server
//! and returned by one read-only tool. Two things can drift, and each has a test here:
//!
//! - **The tool list.** A tool added to the server and not to the guide is a tool an agent reading
//!   the guide never learns about — **so the guide is checked against the tool list the server
//!   itself returns over MCP**, not against a copy of it. The one exception is `grimoire_guide`
//!   itself: the guide is what it returns, so it does not name itself.
//! - **The instructions.** `get_info` is the one thing an agent reads before it calls anything, so a
//!   guide the instructions never mention is a document nobody opens.
//!
//! **The server is called as an outside agent calls it**: a subprocess, MCP over stdio, on a port
//! nothing listens on, so a passing test proves the guide is shipped rather than fetched.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

/// **A local port with nothing listening on it**: bind an ephemeral port, then let it go. The
/// kernel does not hand the same ephemeral port straight back out, so it stays closed for the
/// length of a test.
fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    listener.local_addr().expect("the bound port").port()
}

/// **A running MCP server, as a subprocess, with no editor behind it.**
struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    /// What the server said in its handshake, kept for the instructions test.
    initialize: Value,
}

impl Server {
    /// **Start the server and shake hands**, the way an agent's client does.
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
        let mut server = Server {
            child,
            stdin,
            stdout,
            next_id: 0,
            initialize: Value::Null,
        };
        let answer = server.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "bevy_grimoire's guide test", "version": "0.1.0"},
            }),
        );
        assert!(
            answer.get("error").is_none(),
            "the server refused the handshake: {answer}"
        );
        server.initialize = answer;
        // **A notification has no id and no answer**, so it is written and not read back.
        server.notify("notifications/initialized", json!({}));
        server
    }

    /// **Send a request and read its answer**, skipping any notification in between.
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let body = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.stdin, "{body}").expect("the server's stdin is writable");
        self.stdin.flush().expect("the server's stdin flushes");
        loop {
            let mut line = String::new();
            let read = self
                .stdout
                .read_line(&mut line)
                .expect("the server's stdout is readable");
            assert!(read > 0, "the server's stdout ended before it answered");
            let line = line.trim_end();
            let value: Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("the server sent a line that is not JSON: {line} ({e})"));
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

    /// **Call a tool.**
    fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }

    /// **The tool list the server returns**, as `(name, annotations)` pairs.
    fn tools(&mut self) -> Vec<(String, Value)> {
        let listed = self.request("tools/list", json!({}));
        listed["result"]["tools"]
            .as_array()
            .unwrap_or_else(|| panic!("the server's tool list is not an array: {listed}"))
            .iter()
            .map(|tool| {
                (
                    tool["name"].as_str().unwrap_or_default().to_owned(),
                    tool["annotations"].clone(),
                )
            })
            .collect()
    }

    /// **The text of a tool's answer**, or the refusal it came back with.
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

/// **`grimoire_guide` is declared, read-only, idempotent, and answers with its text.**
///
/// **A guide that is a read is annotated as one.** The tool is offered to every client, and a client
/// that cannot tell a read from a write asks the person before it calls one — so the hints are part
/// of the contract, not decoration.
#[test]
fn the_guide_is_a_read_that_needs_no_editor() {
    let mut server = Server::start();
    let tools = server.tools();
    let (_, annotations) = tools
        .iter()
        .find(|(name, _)| name == "grimoire_guide")
        .unwrap_or_else(|| {
            panic!(
                "the server does not offer `grimoire_guide`. It offers: {:?}",
                tools.iter().map(|(name, _)| name).collect::<Vec<_>>()
            )
        });
    assert_eq!(
        annotations["readOnlyHint"].as_bool(),
        Some(true),
        "`grimoire_guide` changes nothing, so it is read-only: {annotations}"
    );
    assert_eq!(
        annotations["idempotentHint"].as_bool(),
        Some(true),
        "the same call returns the same guide, so it is idempotent: {annotations}"
    );

    // **No editor is running** — the port was closed before the server started — and the guide still
    // answers, because it is compiled into the server.
    let answer = server.call("grimoire_guide", json!({}));
    let guide = server.text_of(&answer);
    assert!(
        guide.contains("Working a note"),
        "the guide's own note section is missing from what the tool returned, so it is not the \
         shipped guide: {guide}"
    );
}

/// **The instructions send an agent to the guide before any other tool.**
///
/// `get_info` is what an agent reads first. A guide the instructions never name is a document
/// nobody opens, and one named after the operation tools is a document read too late to change how
/// the task is worked.
#[test]
fn the_instructions_name_the_guide_first() {
    let server = Server::start();
    let instructions = server.initialize["result"]["instructions"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        instructions.contains("grimoire_guide"),
        "the server's instructions never name `grimoire_guide`, so an agent never learns the guide \
         exists: {instructions}"
    );
    let guide_at = instructions.find("grimoire_guide").unwrap_or(usize::MAX);
    let others_at = instructions
        .find("grimoire_")
        .and_then(|first| {
            instructions[first + "grimoire_".len()..]
                .find("grimoire_")
                .map(|next| first + "grimoire_".len() + next)
        })
        .unwrap_or(usize::MAX);
    assert!(
        guide_at < others_at,
        "the instructions name another tool before `grimoire_guide`, so an agent reads about a tool \
         before it reads the guide that says how to use it: {instructions}"
    );
    assert!(
        instructions.contains("first"),
        "the instructions name the guide without saying to call it first: {instructions}"
    );
}

/// **The guide names every tool the server declares, read from the server's own list.**
///
/// **The list is the server's, not a copy**: `tools/list` is asked, and the guide is asked, and the
/// two are compared. A tool added to the server and not to the guide is a tool an agent reading the
/// guide never learns about — the failure this test exists to catch.
#[test]
fn the_guide_names_every_tool_the_server_declares() {
    let mut server = Server::start();
    let declared: Vec<String> = server
        .tools()
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| name != "grimoire_guide")
        .collect();
    assert!(
        declared.len() > 20,
        "the server declares {} tools, which is too few for this test to mean anything",
        declared.len()
    );
    let answer = server.call("grimoire_guide", json!({}));
    let guide = server.text_of(&answer);
    let missing: Vec<&String> = declared
        .iter()
        .filter(|name| !guide.contains(name.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "the guide never names {missing:?}, so an agent that reads only the guide does not know \
         those tools exist."
    );
}
