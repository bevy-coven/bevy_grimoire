//! **`grimoire_style` returns the chain the editor composed, with the pictures it cites as image content** (SG-02, #401).
//!
//! The editor holds the open project and the person's trust, so it composes the chain (`grimoire/style`); this server
//! forwards the request, returns the text, and attaches each picture the editor named, read from the file the editor
//! resolved, each preceded by its caption so an agent that cannot see still has the words. The server is started as an
//! agent's client starts it, against a stub editor that records the request it is sent.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

type Calls = Arc<Mutex<Vec<(String, Value)>>>;

/// The first eight bytes of a PNG: all the server forwards is the file's bytes, in base64.
const PNG_HEAD: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

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

/// **A stub editor** that answers `grimoire/style` with `answer` (or refuses a map named `nowhere`) and records every call.
fn a_stub_editor(calls: Calls, answer: Value) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("a bound address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let (calls, answer) = (calls.clone(), answer.clone());
            std::thread::spawn(move || {
                let request = read_request(&stream);
                let method = request["method"].as_str().unwrap_or_default().to_owned();
                let params = request["params"].clone();
                calls.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((method.clone(), params.clone()));
                let body = match method.as_str() {
                    "grimoire/style" if params["map"] == "nowhere" => json!({
                        "jsonrpc": "2.0", "id": request["id"],
                        "error": {"code": -32000, "message": "the project has no map `nowhere`; its maps are `m`."},
                    }),
                    "grimoire/style" => json!({"jsonrpc": "2.0", "id": request["id"], "result": answer}),
                    _ => json!({"jsonrpc": "2.0", "id": request["id"], "result": {"known": true}}),
                }
                .to_string();
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

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Server {
    fn start(port: u16) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_bevy_cauldron_mcp"))
            .env("BEVY_BRP_PORT", port.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the server binary starts");
        let stdin = child.stdin.take().expect("the server has a stdin");
        let stdout = BufReader::new(child.stdout.take().expect("the server has a stdout"));
        let mut server = Server { child, stdin, stdout, next_id: 0 };
        let hello = server.request(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "style-test", "version": "1.0"}}),
        );
        assert!(hello.get("error").is_none(), "the handshake was refused: {hello}");
        server.notify("notifications/initialized", json!({}));
        server
    }

    fn notify(&mut self, method: &str, params: Value) {
        writeln!(self.stdin, "{}", json!({"jsonrpc": "2.0", "method": method, "params": params})).expect("stdin");
        self.stdin.flush().expect("stdin flushes");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        writeln!(self.stdin, "{}", json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).expect("stdin");
        self.stdin.flush().expect("stdin flushes");
        loop {
            let mut line = String::new();
            assert!(self.stdout.read_line(&mut line).expect("stdout is readable") > 0, "the server ended before it answered");
            let value: Value = serde_json::from_str(line.trim_end()).unwrap_or_else(|e| panic!("not JSON: {line} ({e})"));
            if value.get("id").and_then(Value::as_u64) == Some(id) {
                return value;
            }
        }
    }

    fn style(&mut self, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": "grimoire_style", "arguments": arguments}))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-style-mcp-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    dir
}

/// The content blocks of an answer, by type.
fn blocks<'a>(answer: &'a Value, kind: &str) -> Vec<&'a Value> {
    answer["result"]["content"].as_array().expect("content").iter().filter(|b| b["type"] == kind).collect()
}

/// **The text comes first, then each picture preceded by its caption and path, as an image.**
#[test]
fn the_text_comes_back_and_each_cited_picture_follows_as_an_image_after_its_caption() {
    let dir = scratch("pictures");
    let fog = dir.join("fog.png");
    std::fs::write(&fog, PNG_HEAD).expect("a picture");
    let wall = dir.join("wall.png");
    std::fs::write(&wall, PNG_HEAD).expect("another");
    let answer = json!({
        "text": "## design/style.md\n\nThe game is grey.\n",
        "files": [{"place": "game", "path": "design/style.md", "bytes": 18, "state": "served"}],
        "pictures": [
            {"file": "design/board/fog.png", "path": fog.to_string_lossy(), "caption": "the fog at noon", "cited_by": "design/style.md", "bytes": 8, "mime": "image/png"},
            {"file": "design/board/wall.png", "path": wall.to_string_lossy(), "caption": "the wall at dusk", "cited_by": "design/style.md", "bytes": 8, "mime": "image/png"},
        ],
        "left_out": [{"file": "design/board/huge.png", "caption": "too big", "cited_by": "design/style.md", "reason": "it is 2000000 bytes"}],
        "withheld": [],
        "trouble": null,
    });
    let calls: Calls = Arc::default();
    let mut server = Server::start(a_stub_editor(calls.clone(), answer));
    let got = server.style(json!({"map": "m", "at": [1.0, 2.0], "topic": "dialogue"}));
    assert!(got["result"]["isError"].as_bool() != Some(true), "{got}");

    let sent = calls.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter().find(|(m, _)| m == "grimoire/style").map(|(_, p)| p.clone());
    assert_eq!(sent, Some(json!({"map": "m", "at": [1.0, 2.0], "topic": "dialogue"})), "the request reaches the editor whole");

    let content = got["result"]["content"].as_array().expect("content");
    assert_eq!(content[0]["type"], "text");
    assert!(content[0]["text"].as_str().expect("text").contains("The game is grey."), "the chain's text comes first: {got}");
    let images = blocks(&got, "image");
    assert_eq!(images.len(), 2, "both pictures are attached: {got}");
    assert!(images.iter().all(|i| i["mimeType"] == "image/png" && i["data"].as_str().is_some_and(|d| !d.is_empty())));
    // Each picture is preceded by a text block holding its caption and path.
    let kinds: Vec<&str> = content.iter().map(|b| b["type"].as_str().unwrap_or("")).collect();
    assert_eq!(kinds, vec!["text", "text", "image", "text", "image"], "{kinds:?}");
    assert!(content[1]["text"].as_str().expect("label").contains("the fog at noon"), "{got}");
    assert!(content[1]["text"].as_str().expect("label").contains("design/board/fog.png"), "{got}");
    assert!(content[3]["text"].as_str().expect("label").contains("the wall at dusk"), "{got}");
}

/// **A picture the server cannot read is named in the text, and the answer still arrives.**
#[test]
fn a_picture_that_cannot_be_read_is_named_and_the_answer_still_arrives() {
    let dir = scratch("unreadable");
    let answer = json!({
        "text": "## design/style.md\n\nGrey.\n",
        "files": [], "left_out": [], "withheld": [], "trouble": null,
        "pictures": [{"file": "design/board/gone.png", "path": dir.join("gone.png").to_string_lossy(), "caption": "gone", "cited_by": "design/style.md", "bytes": 8, "mime": "image/png"}],
    });
    let mut server = Server::start(a_stub_editor(Arc::default(), answer));
    let got = server.style(json!({}));
    assert!(got["result"]["isError"].as_bool() != Some(true), "{got}");
    assert!(blocks(&got, "image").is_empty());
    let texts: String = blocks(&got, "text").iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n");
    assert!(texts.contains("Grey.") && texts.contains("gone") && texts.contains("could not be read"), "{texts}");
}

/// **A refusal from the editor reaches the agent with its reason.**
#[test]
fn a_refusal_from_the_editor_reaches_the_agent_with_its_reason() {
    let mut server = Server::start(a_stub_editor(Arc::default(), json!({"text": ""})));
    let got = server.style(json!({"map": "nowhere"}));
    assert_eq!(got["result"]["isError"], true, "{got}");
    assert!(got.to_string().contains("no map `nowhere`"), "{got}");
}

/// **With no editor the tool says why it cannot answer, and does not read the files itself.** The person's trust is the
/// editor's to hold, so a server that served style files without it would be a route around it.
#[test]
fn with_no_editor_the_tool_refuses_and_says_the_person_s_trust_is_the_editors() {
    let closed = TcpListener::bind("127.0.0.1:0").expect("a port").local_addr().expect("addr").port();
    let mut server = Server::start(closed);
    let got = server.style(json!({}));
    assert_eq!(got["result"]["isError"], true, "{got}");
    let said = got.to_string();
    assert!(said.contains("trust") && said.contains("editor"), "{said}");
}

/// **The tool is declared read-only, and its description names the trust.**
#[test]
fn the_tool_is_declared_read_only_and_says_a_style_file_needs_the_persons_trust() {
    let mut server = Server::start(a_stub_editor(Arc::default(), json!({"text": ""})));
    let listed = server.request("tools/list", json!({}));
    let tool = listed["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .find(|t| t["name"] == "grimoire_style")
        .unwrap_or_else(|| panic!("the server does not offer `grimoire_style`: {listed}"));
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["destructiveHint"], false);
    let description = tool["description"].as_str().expect("description");
    assert!(description.contains("allowed") && description.contains("withheld"), "{description}");
    assert!(description.contains("most specific"), "{description}");
}

/// **The guide names the tool and asks for a wish about what the style did not say; the generated reference lists it.**
#[test]
fn the_guide_and_the_generated_reference_name_the_tool() {
    let guide = bevy_cauldron_mcp::docs::GUIDE;
    assert!(guide.contains("`grimoire_style`"), "the guide names the tool");
    assert!(guide.contains("before you shape it"), "and asks for it before a place is shaped");
    assert!(guide.contains("Say in your wish what the style did not tell you"), "and for a wish about what it left unsaid");
    let reference = include_str!("../../../docs/mcp-tools.md");
    assert!(reference.contains("### `grimoire_style`"), "the generated reference lists the tool");
}
