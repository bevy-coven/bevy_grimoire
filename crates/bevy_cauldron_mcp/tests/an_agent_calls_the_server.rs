//! **The MCP server, called as an outside agent would call it.**
//!
//! The brief's rule: *"When you add or change a tool, test it as an outside agent would: list the
//! tools, read only the description, call the tool, and check the result is usable with no
//! project knowledge. Add an integration test that calls it through an MCP client."*
//!
//! # What this does
//!
//! **It starts the server as a subprocess and speaks MCP to it over stdio** — the same way the
//! agent's client does. **It does not call the tool functions**, because a test that calls the
//! function is a test of the function and not of the server: it would not notice that the
//! description was wrong, that the tool was not registered, or that the answer was not shaped
//! like an MCP answer.
//!
//! **It reads only the description** — which is the point. The test asks the server what tools it
//! has, reads what the server says about them, and then calls one. **If the description were
//! not enough to make the call, the test cannot make it**, and that is the failure the brief is
//! asking about.

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

/// **A running MCP server, as a subprocess.**
struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Server {
    /// **Start the server and shake hands.**
    fn start() -> Server {
        Server::start_with(&[])
    }

    /// **Start the server with arguments, as an agent's client does, such as `--project`.**
    ///
    /// **On a port nothing listens on.** These tests drive the server with no editor running, and
    /// several assert the refusal that says so. The default BRP port (15702) is whatever the
    /// developer's machine has on it: on 2026-10-23 an ssh tunnel to another Mac held it, the
    /// snip tool reached a remote editor, and the test failed on a PNG where it wanted a refusal.
    fn start_with(args: &[&str]) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_bevy_cauldron_mcp"))
            .args(args)
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
        };
        let answer = server.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "bevy_grimoire's own test", "version": "0.1.0"},
            }),
        );
        assert!(
            answer.get("error").is_none(),
            "the server refused the handshake: {answer}"
        );
        // **The notification that completes the handshake.** A notification has no id and no
        // answer, so it is written and not read back.
        server.notify("notifications/initialized", json!({}));
        server
    }

    /// **Send a request and read its answer.**
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let body = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.stdin, "{body}").expect("the server's stdin is writable");
        self.stdin.flush().expect("the server's stdin flushes");
        // **Read until the answer with this id.** The server may send notifications in between,
        // and an answer to an earlier request may still be in flight.
        loop {
            let mut line = String::new();
            let read = self
                .stdout
                .read_line(&mut line)
                .expect("the server's stdout is readable");
            assert!(read > 0, "the server's stdout ended before it answered");
            let line = line.trim_end();
            let value: Value = serde_json::from_str(line).unwrap_or_else(|e| {
                panic!("the server sent a line that is not JSON: {line} ({e})")
            });
            if value.get("id").and_then(|i| i.as_u64()) == Some(id) {
                return value;
            }
        }
    }

    /// **Send a notification.**
    fn notify(&mut self, method: &str, params: Value) {
        let body = json!({"jsonrpc": "2.0", "method": method, "params": params});
        writeln!(self.stdin, "{body}").expect("the server's stdin is writable");
        self.stdin.flush().expect("the server's stdin flushes");
    }

    /// **Call a tool.**
    fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// **The server lists its tools, and every one of them says enough to be called.**
///
/// **This is the brief's criterion, and it is checked by reading only the description.** The
/// test does not know what the tools do; it reads what the server says and asks whether that is
/// enough to know what to send and what comes back.
#[test]
fn the_server_lists_tools_that_say_what_they_do() {
    let mut server = Server::start();
    let listed = server.request("tools/list", json!({}));
    let tools = listed["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("the server's tool list is not an array: {listed}"));
    assert!(
        !tools.is_empty(),
        "the server lists no tools, so an agent has nothing to call: {listed}"
    );
    for tool in tools {
        let name = tool["name"].as_str().unwrap_or_default();
        let description = tool["description"].as_str().unwrap_or_default();
        assert!(!name.is_empty(), "a tool has no name: {tool}");
        assert!(
            name.starts_with("grimoire_"),
            "the tool `{name}` is not prefixed with `grimoire_`."
        );
        assert!(
            description.len() > 40,
            "the tool `{name}` has a description of {} characters, which is too short to say what \
             it does, what it does not do, and what comes back. An agent reads this and nothing \
             else.",
            description.len()
        );
        // **The annotations are there and they are booleans.** A missing annotation is not a
        // false one, and a client that reads a missing one cannot tell a read from a write.
        let annotations = &tool["annotations"];
        for hint in ["readOnlyHint", "destructiveHint", "idempotentHint"] {
            assert!(
                annotations.get(hint).and_then(|v| v.as_bool()).is_some(),
                "the tool `{name}` has no `{hint}` annotation, so a client cannot tell what it \
                 does to the editor: {annotations}"
            );
        }
        // **And a schema, so the agent knows what to send.**
        assert!(
            tool.get("inputSchema").is_some(),
            "the tool `{name}` has no input schema, so an agent cannot tell what to send it."
        );
    }
}

/// **A call with no editor running is a refusal with a sentence, and not a crash.**
///
/// **The agent's server outlives the editor sometimes** — the editor is closed and reopened, and
/// the server is the long-lived one. So a call with nothing on the other end must come back as a
/// readable answer.
#[test]
fn a_call_with_no_editor_is_a_refusal_and_not_a_crash() {
    let mut server = Server::start();
    let answer = server.call("grimoire_list_operations", json!({}));
    // **The call is answered**, and the answer says the editor is not there.
    assert!(
        answer.get("error").is_none(),
        "the call was refused at the protocol level, so the agent cannot read why: {answer}"
    );
    let text = answer["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("editor") || text.contains("BEVY_BRP_PORT"),
        "the refusal does not say the editor was not there, so an agent does not know what to do \
         about it: {text}"
    );
}

/// **The new tools are listed, and their descriptions are enough to call them.**
///
/// **The brief's test: "list the tools, read only the description, call the tool, and check the
/// result is usable with no project knowledge."** This reads the descriptions and checks that
/// each of the three new tools says what it does, what it does not do, and what comes back — and
/// that the annotations are the ones the tool's behaviour earns.
#[test]
fn the_capture_and_state_tools_say_what_they_do() {
    let mut server = Server::start();
    let listed = server.request("tools/list", json!({}));
    let tools = listed["result"]["tools"]
        .as_array()
        .expect("the tool list is an array");

    // **The three new tools, and what each of them must say.**
    let expected: &[(&str, bool, bool, bool)] = &[
        // name, read_only, destructive, idempotent
        // **A read is idempotent** — `grimoire_state` changes nothing, so calling it twice is the
        // same as calling it once. The two captures are not: each writes a file.
        ("grimoire_state", true, false, true),
        ("grimoire_capture_still", false, false, false),
        ("grimoire_capture_sequence", false, true, false),
        ("grimoire_list_snips", true, false, true),
        ("grimoire_list_pieces", true, false, true),
    ];
    let mut missing = Vec::new();
    for (name, read_only, destructive, idempotent) in expected {
        let Some(tool) = tools.iter().find(|t| t["name"] == *name) else {
            missing.push(format!("the tool `{name}` is not listed"));
            continue;
        };
        let description = tool["description"].as_str().unwrap_or_default();
        // **The description has to say what comes back**, or an agent cannot tell a result from
        // a failure.
        assert!(
            description.contains("Returns")
                || description.contains("returns")
                || description.contains("answer")
                || description.contains("picture"),
            "the tool `{name}` does not say what comes back: {description}"
        );
        let annotations = &tool["annotations"];
        assert_eq!(
            annotations["readOnlyHint"].as_bool(),
            Some(*read_only),
            "the tool `{name}` has the wrong readOnlyHint: {annotations}"
        );
        assert_eq!(
            annotations["destructiveHint"].as_bool(),
            Some(*destructive),
            "the tool `{name}` has the wrong destructiveHint: {annotations}"
        );
        assert_eq!(
            annotations["idempotentHint"].as_bool(),
            Some(*idempotent),
            "the tool `{name}` has the wrong idempotentHint: {annotations}"
        );
    }
    assert!(missing.is_empty(), "these tools are missing: {missing:?}");
}

/// **The undo tool is listed, its hints match what it does, and a call with no editor is a
/// refusal an agent can read.**
///
/// **Destructive, because an undo replaces values** -- it puts the old ones back over the new
/// ones, the same kind of write `grimoire_apply_operation` makes -- and **not idempotent**, because a
/// second identical call takes back a different change (or redoes the first). An agent that read
/// the hints as "safe to retry" would undo twice.
#[test]
fn the_undo_tool_says_what_it_does_and_refuses_without_an_editor() {
    let mut server = Server::start();
    let listed = server.request("tools/list", json!({}));
    let tool = listed["result"]["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|t| t["name"] == "grimoire_undo_operation"))
        .cloned()
        .unwrap_or_else(|| panic!("grimoire_undo_operation is not listed: {listed}"));
    let description = tool["description"].as_str().unwrap_or_default();
    for promise in ["operation_id", "Refuses", "newer change", "Returns"] {
        assert!(
            description.contains(promise) || description.contains(&promise.to_lowercase()),
            "the description does not mention `{promise}`, so an agent reading only it cannot \
             tell how to call the tool or what a refusal means: {description}"
        );
    }
    let hints = &tool["annotations"];
    assert_eq!(hints["readOnlyHint"].as_bool(), Some(false), "{hints}");
    assert_eq!(hints["destructiveHint"].as_bool(), Some(true), "{hints}");
    assert_eq!(hints["idempotentHint"].as_bool(), Some(false), "{hints}");
    // **The schema names the id and the preview**, and nothing else.
    let props = &tool["inputSchema"]["properties"];
    assert!(
        props.get("operation_id").is_some() && props.get("dry_run").is_some(),
        "the tool's schema does not take operation_id and dry_run: {}",
        tool["inputSchema"]
    );

    // **No editor is running, so the answer is a refusal that says so** -- not a protocol error.
    let answer = server.call(
        "grimoire_undo_operation",
        json!({"operation_id": "0001-test"}),
    );
    assert!(
        answer.get("error").is_none(),
        "refused at the protocol level: {answer}"
    );
    assert_eq!(
        answer["result"]["isError"].as_bool(),
        Some(true),
        "{answer}"
    );
    let text = answer["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("editor") || text.contains("BEVY_BRP_PORT"),
        "the refusal does not say the editor was not there: {text}"
    );
}

/// **The state tool says its cap and how to page past it**, so an agent that reads only the listing
/// knows the pin list is capped at 50, how to tell a cut list, and which argument reads the rest.
#[test]
fn the_state_tool_says_its_cap_and_how_to_page_it() {
    let mut server = Server::start();
    let listed = server.request("tools/list", json!({}));
    let tool = listed["result"]["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|t| t["name"] == "grimoire_state"))
        .cloned()
        .unwrap_or_else(|| panic!("grimoire_state is not listed: {listed}"));
    let description = tool["description"].as_str().unwrap_or_default();
    assert!(
        description.contains("up to 50")
            && description.contains("pinned_total")
            && description.contains("pinned_from")
            && description.contains("pinned_next"),
        "grimoire_state must say the list is capped at 50, how to tell a cut list, and how to page it: {description}"
    );
    assert!(
        tool["inputSchema"]["properties"]["pinned_from"].is_object(),
        "grimoire_state must take pinned_from, so a reader can page the list: {tool}"
    );
}

/// **An unknown tool is refused, and the answer says which tools there are.**
#[test]
fn an_unknown_tool_is_refused_with_the_list() {
    let mut server = Server::start();
    let answer = server.call("grimoire_no_such_tool", json!({}));
    let text = answer["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("grimoire_"),
        "the refusal does not name the tools that do exist, so an agent has to guess again: {text}"
    );
}

/// **The frame tool is listed, it only moves the camera, and a call with no editor is a refusal an
/// agent can read.**
///
/// **Not destructive, because no map data changes** -- the view is the rig's focus and height, not a
/// placement. **Not read-only**, because the view does change. **Idempotent**, because framing the same
/// target twice lands on the same view. The description has to say it is not logged or undoable, so an
/// agent does not look for an undo it cannot have, and it has to send the agent to the still.
#[test]
fn the_frame_tool_says_it_only_moves_the_camera() {
    let mut server = Server::start();
    let listed = server.request("tools/list", json!({}));
    let tool = listed["result"]["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|t| t["name"] == "grimoire_frame_view"))
        .cloned()
        .unwrap_or_else(|| panic!("grimoire_frame_view is not listed: {listed}"));
    let description = tool["description"].as_str().unwrap_or_default();
    for promise in [
        "only moves the camera",
        "not logged or undoable",
        "grimoire_capture_still",
        "Returns",
    ] {
        assert!(
            description.contains(promise),
            "the description does not say `{promise}`, so an agent cannot tell what the tool does \
             or what to call next: {description}"
        );
    }
    let hints = &tool["annotations"];
    assert_eq!(hints["readOnlyHint"].as_bool(), Some(false), "{hints}");
    assert_eq!(hints["destructiveHint"].as_bool(), Some(false), "{hints}");
    assert_eq!(hints["idempotentHint"].as_bool(), Some(true), "{hints}");
    assert!(
        tool["inputSchema"]["properties"].get("target").is_some(),
        "the tool's schema does not take a target: {}",
        tool["inputSchema"]
    );

    // **No editor is running, so the answer is a refusal that says so** -- not a protocol error.
    let answer = server.call("grimoire_frame_view", json!({"target": "map"}));
    assert!(
        answer.get("error").is_none(),
        "refused at the protocol level: {answer}"
    );
    assert_eq!(
        answer["result"]["isError"].as_bool(),
        Some(true),
        "{answer}"
    );
    let text = answer["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("editor") || text.contains("BEVY_BRP_PORT"),
        "the refusal does not say the editor was not there: {text}"
    );
}

/// **A capture label is a path, so one that leaves the project is refused before the editor is
/// asked.** `captures/../escape` and `/tmp/escape` must not reach the disk: with no editor
/// running, the only answer that can be right is the label's refusal, not the editor's.
#[test]
fn a_capture_label_that_leaves_the_project_is_refused_before_the_editor_is_asked() {
    let mut server = Server::start();
    for label in ["../escape", "/tmp/escape"] {
        let answer = server.call("grimoire_capture_still", json!({"label": label}));
        assert!(
            answer.get("error").is_none(),
            "refused at the protocol level: {answer}"
        );
        assert_eq!(
            answer["result"]["isError"].as_bool(),
            Some(true),
            "{answer}"
        );
        let text = answer["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        assert!(
            text.contains("not a capture label"),
            "a label of `{label}` was not refused as a label, so the tool may have written \
             outside the project: {text}"
        );
    }
}

#[test]
fn the_snip_tool_says_what_it_crops_and_refuses_without_an_editor() {
    let mut server = Server::start();
    let listed = server.request("tools/list", json!({}));
    let tool = listed["result"]["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|t| t["name"] == "grimoire_snip"))
        .cloned()
        .unwrap_or_else(|| panic!("grimoire_snip is not listed: {listed}"));
    let description = tool["description"].as_str().unwrap_or_default();
    for promise in [
        "window pixels",
        "x0, y0, x1, y1",
        "not an operation",
        "not logged",
        "grimoire_capture_still",
        "Refuses",
    ] {
        assert!(
            description.contains(promise),
            "the description does not say `{promise}`, so an agent cannot tell what the tool does \
             or how to call it: {description}"
        );
    }
    let hints = &tool["annotations"];
    assert_eq!(hints["readOnlyHint"].as_bool(), Some(false), "{hints}");
    assert_eq!(hints["destructiveHint"].as_bool(), Some(false), "{hints}");
    assert_eq!(hints["idempotentHint"].as_bool(), Some(false), "{hints}");
    assert!(
        tool["inputSchema"]["properties"].get("rect").is_some(),
        "the tool's schema does not take a rect: {}",
        tool["inputSchema"]
    );

    // **No editor is running, so the answer is a refusal that says so** -- not a protocol error.
    let answer = server.call(
        "grimoire_snip",
        json!({"label": "snip_check", "rect": [0, 0, 100, 100]}),
    );
    assert!(
        answer.get("error").is_none(),
        "refused at the protocol level: {answer}"
    );
    assert_eq!(
        answer["result"]["isError"].as_bool(),
        Some(true),
        "{answer}"
    );
    let text = answer["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("editor") || text.contains("BEVY_BRP_PORT"),
        "the refusal does not say the editor was not there: {text}"
    );

    // **A label that leaves the project is refused by the tool before the editor is asked**, the
    // same as the still tool, so the check holds whether or not an editor is running.
    let escaped = server.call(
        "grimoire_snip",
        json!({"label": "../escape", "rect": [0, 0, 100, 100]}),
    );
    let text = escaped["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("not a capture label"),
        "a label of `../escape` was not refused as a label: {escaped}"
    );
}

/// **An outside agent reaches the pin operation through the server, with no editor running.**
///
/// E-024: `placement_set_pin` is the operation an agent calls to pin or unpin a placement. The
/// test lists the tools, reads the apply tool's description to learn it takes an `operation` name,
/// and calls it with that name. With no editor the answer must be a refusal that says the editor
/// is not there — not "unknown operation", which would mean the server's own surface does not
/// carry the operation and an agent could never learn its name.
#[test]
fn an_agent_can_name_the_pin_operation_and_is_told_the_editor_is_not_there() {
    let mut server = Server::start();
    let listed = server.request("tools/list", json!({}));
    let tools = listed["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("tools/list gives no tools: {listed}"));
    let apply = tools
        .iter()
        .find(|t| t["name"] == "grimoire_apply_operation")
        .unwrap_or_else(|| panic!("grimoire_apply_operation is not listed: {listed}"));
    let description = apply["description"].as_str().unwrap_or_default();
    assert!(
        description.contains("operation"),
        "the apply tool's description does not say it takes an operation name: {description}"
    );

    let answer = server.call(
        "grimoire_apply_operation",
        json!({
            "operation": "placement_set_pin",
            "parameters": {"id": "m1", "pinned": true, "because": "held for the roof"},
        }),
    );
    let text = answer["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("editor") || text.contains("BEVY_BRP_PORT"),
        "the pin operation was not taken as a name the server knows: {answer}"
    );
    assert!(
        !text.contains("unknown operation"),
        "the server does not carry placement_set_pin, so an agent cannot learn it: {text}"
    );
}

/// **The snips the person took, listed through the server as an agent would list them.**
///
/// **The brief's test, with no editor running:** the test lists the tools and reads the
/// description of `grimoire_list_snips`, writes two snips the way the editor writes them (through
/// the shared crate), and calls the tool. The folder is the only thing the tool reads, so the
/// answer must come back without an editor, newest first, with the newest picture, and a file
/// that is not a snip must be named rather than hidden.
#[test]
fn an_agent_lists_the_snips_newest_first_with_the_newest_picture() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_nanos();
    let project =
        std::env::temp_dir().join(format!("grimoire_list_snips_{}_{nanos}", std::process::id()));
    let dir = project.join("target/loop/captures/snips");
    std::fs::create_dir_all(&dir).expect("the snips folder can be made");

    // **The older snip is written second**, so the order comes from the time and not from the
    // order the files were made.
    bevy_grimoire_snip::write_snip(
        &dir,
        "snip-2000",
        picture(),
        [0, 0, 8, 8],
        record("snip-2000", 2000, "the gate is too narrow"),
    )
    .expect("the newer snip is written");
    bevy_grimoire_snip::write_snip(
        &dir,
        "snip-1000",
        picture(),
        [0, 0, 8, 8],
        record("snip-1000", 1000, "the wall is in the wrong place"),
    )
    .expect("the older snip is written");
    std::fs::write(dir.join("stray.json"), "{\"not\": \"a snip\"}")
        .expect("the stray file is written");

    let mut server = Server::start_with(&["--project", project.to_str().expect("a UTF-8 path")]);
    let listed = server.request("tools/list", json!({}));
    let tool = listed["result"]["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|t| t["name"] == "grimoire_list_snips"))
        .cloned()
        .unwrap_or_else(|| panic!("grimoire_list_snips is not listed: {listed}"));
    let description = tool["description"].as_str().unwrap_or_default();
    for promise in ["newest first", "no editor running", "Returns"] {
        assert!(
            description.contains(promise),
            "the description does not say `{promise}`: {description}"
        );
    }

    let answer = server.call("grimoire_list_snips", json!({}));
    assert!(
        answer.get("error").is_none(),
        "refused at the protocol level: {answer}"
    );
    assert_ne!(answer["result"]["isError"], json!(true), "{answer}");
    let content = answer["result"]["content"]
        .as_array()
        .expect("the answer has content");
    // **The newest picture comes back as an image**, a PNG.
    let image = content
        .iter()
        .find(|b| b["type"] == "image")
        .expect("the newest picture comes back as an image");
    assert_eq!(image["mimeType"], "image/png", "{image}");
    assert!(
        image["data"]
            .as_str()
            .unwrap_or_default()
            .starts_with("iVBORw0KGgo"),
        "the picture is not a PNG"
    );
    let body = answer_text(content);
    assert_eq!(body["total"], 2, "{body}");
    assert_eq!(body["shown"], 2, "{body}");
    let snips = body["snips"].as_array().expect("the snips are a list");
    assert_eq!(snips[0]["stem"], "snip-2000", "newest first: {body}");
    assert_eq!(snips[0]["note"], "the gate is too narrow", "{body}");
    assert_eq!(snips[1]["stem"], "snip-1000", "{body}");
    let picture_path = snips[0]["picture_path"].as_str().unwrap_or_default();
    assert!(
        picture_path.ends_with("snip-2000.png") && std::path::Path::new(picture_path).exists(),
        "the newest snip's picture path does not name a file: {picture_path}"
    );
    let not_snips = body["not_snips"].as_array().expect("not_snips is a list");
    assert!(
        not_snips
            .iter()
            .any(|p| p.as_str().unwrap_or_default().contains("stray.json")),
        "the file that is not a snip is hidden: {body}"
    );

    // **A limit of one shows one and still says there are two.**
    let one = server.call("grimoire_list_snips", json!({"limit": 1}));
    let body = answer_text(one["result"]["content"].as_array().expect("content"));
    assert_eq!(
        (body["total"].as_u64(), body["shown"].as_u64()),
        (Some(2), Some(1)),
        "{body}"
    );

    // **A limit outside 1 to 20 is a refusal with the range**, not a silent clamp.
    let zero = server.call("grimoire_list_snips", json!({"limit": 0}));
    assert_eq!(zero["result"]["isError"].as_bool(), Some(true), "{zero}");
    let text = zero["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("1 to 20"),
        "the refusal does not give the range: {text}"
    );

    let _ = std::fs::remove_dir_all(&project);
}

/// **A 16 by 16 picture of one colour**, as the editor would hand over its window's pixels.
fn picture() -> (u32, u32, Vec<u8>) {
    (16, 16, vec![200; 16 * 16 * 4])
}

/// **A snip record with the fields the editor fills in**, for the folder the server reads.
fn record(stem: &str, taken_at_ms: u64, note: &str) -> bevy_grimoire_snip::SnipRecord {
    bevy_grimoire_snip::SnipRecord {
        stem: stem.into(),
        taken_at_ms,
        map: "town".into(),
        active_tool: "Place".into(),
        note: note.into(),
        rect_window_px: [0.0, 0.0, 8.0, 8.0],
        scale_factor: 1.0,
        rect_picture_px: [0, 0, 8, 8],
        size: [8, 8],
        picture: [16, 16],
        picture_file: String::new(),
        camera: None,
        cursor_world: None,
        placements: Vec::new(),
    }
}

/// **The text block of an answer, parsed as JSON.** The tool answers with the picture first and
/// the JSON second, so an agent reads the text block, and this does the same.
fn answer_text(content: &[Value]) -> Value {
    let text = content
        .iter()
        .find(|b| b["type"] == "text")
        .and_then(|b| b["text"].as_str())
        .expect("the answer has a text block");
    serde_json::from_str(text).expect("the text block is JSON")
}

/// **A folder with its own snips in it, and a fresh project path for each test.**
fn snips_folder(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_nanos();
    let project = std::env::temp_dir().join(format!(
        "grimoire_list_snips_{name}_{}_{nanos}",
        std::process::id()
    ));
    let dir = project.join("target/loop/captures/snips");
    std::fs::create_dir_all(&dir).expect("the snips folder can be made");
    (project, dir)
}

/// **A snip whose picture is missing is named, and the rest of the list still answers.** Before the
/// fix the whole list was refused with "No such file or directory", so the snip that was kept came back
/// as nothing at all.
#[test]
fn a_snip_whose_picture_is_missing_is_named_and_the_list_still_answers() {
    let (project, dir) = snips_folder("missing_picture");
    bevy_grimoire_snip::write_snip(
        &dir,
        "snip-2000",
        picture(),
        [0, 0, 8, 8],
        record("snip-2000", 2000, "kept"),
    )
    .expect("the kept snip is written");
    bevy_grimoire_snip::write_snip(
        &dir,
        "snip-1000",
        picture(),
        [0, 0, 8, 8],
        record("snip-1000", 1000, "picture gone"),
    )
    .expect("the snip whose picture goes is written");
    std::fs::remove_file(dir.join("snip-1000.png")).expect("the picture is removed");

    let mut server = Server::start_with(&["--project", project.to_str().expect("a UTF-8 path")]);
    let answer = server.call("grimoire_list_snips", json!({}));
    assert!(
        answer["result"]["isError"].as_bool() != Some(true),
        "a missing picture refused the whole list: {answer}"
    );
    let body = answer_text(answer["result"]["content"].as_array().expect("content"));
    assert_eq!(body["shown"], 1, "{body}");
    assert_eq!(body["snips"][0]["stem"], "snip-2000", "{body}");
    let not_snips = body["not_snips"].as_array().expect("not_snips is a list");
    assert!(
        not_snips
            .iter()
            .any(|p| p.as_str().unwrap_or_default().contains("snip-1000")),
        "the snip whose picture is missing is not named: {body}"
    );
}

/// **A snip confirmed in the editor and not yet written is listed as pending, not as nothing.**
/// The picture lands a frame or more after the confirm, so an agent that lists straight after the
/// Enter must see the snip, with a status that says it is still being saved.
#[test]
fn a_snip_still_being_saved_is_listed_as_pending() {
    let (project, dir) = snips_folder("pending");
    bevy_grimoire_snip::write_pending(&dir, &record("snip-3000", now_ms(), "the gate is too narrow"))
        .expect("the pending record is written");

    let mut server = Server::start_with(&["--project", project.to_str().expect("a UTF-8 path")]);
    let answer = server.call("grimoire_list_snips", json!({}));
    assert!(
        answer["result"]["isError"].as_bool() != Some(true),
        "{answer}"
    );
    let body = answer_text(answer["result"]["content"].as_array().expect("content"));
    assert_eq!(body["total"], 0, "a pending snip is not a finished one: {body}");
    assert_eq!(body["pending"][0]["stem"], "snip-3000", "{body}");
    assert_eq!(
        body["pending"][0]["note"], "the gate is too narrow",
        "{body}"
    );
    assert!(
        body["pending"][0]["status"]
            .as_str()
            .unwrap_or_default()
            .contains("being saved"),
        "a pending snip does not say it is still being saved: {body}"
    );
    let _ = std::fs::remove_dir_all(&project);
}

/// **The picture is named by the record file's own name.** A record that names a stem with path
/// separators must not send the picture outside the folder. Before the fix the stem inside the record
/// was joined onto the folder, so the picture path pointed at `../../escape.png`.
#[test]
fn a_record_cannot_send_the_picture_out_of_the_snips_folder() {
    let (project, dir) = snips_folder("escaping_stem");
    bevy_grimoire_snip::write_snip(
        &dir,
        "snip-4000",
        picture(),
        [0, 0, 8, 8],
        record("snip-4000", 4000, "the real one"),
    )
    .expect("the snip is written");
    std::fs::write(
        dir.join("snip-4000.json"),
        serde_json::to_string(&record("../../escape", 4000, "the stem lies"))
            .expect("the record serialises"),
    )
    .expect("the record is rewritten with an escaping stem");

    let mut server = Server::start_with(&["--project", project.to_str().expect("a UTF-8 path")]);
    let answer = server.call("grimoire_list_snips", json!({}));
    assert!(
        answer["result"]["isError"].as_bool() != Some(true),
        "refused at the protocol level: {answer}"
    );
    let body = answer_text(answer["result"]["content"].as_array().expect("content"));
    let picture_path = body["snips"][0]["picture_path"]
        .as_str()
        .unwrap_or_default();
    assert!(
        picture_path.ends_with("/snip-4000.png") && !picture_path.contains(".."),
        "the picture left the snips folder or was renamed by the record: {picture_path}"
    );
}

/// **The clock now, in Unix milliseconds**, for a record that should read as fresh.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_millis() as u64
}

/// **A pending record whose picture never landed is marked abandoned, not "being saved" forever.**
/// The editor quit, or the screenshot was lost, before the readback; after ten seconds the record
/// says so and carries its age.
#[test]
fn a_pending_snip_with_no_picture_after_ten_seconds_is_marked_abandoned() {
    let (project, dir) = snips_folder("abandoned");
    bevy_grimoire_snip::write_pending(&dir, &record("snip-6000", now_ms() - 60_000, "stuck"))
        .expect("the pending record is written");

    let mut server = Server::start_with(&["--project", project.to_str().expect("a UTF-8 path")]);
    let answer = server.call("grimoire_list_snips", json!({}));
    let body = answer_text(answer["result"]["content"].as_array().expect("content"));
    let status = body["pending"][0]["status"].as_str().unwrap_or_default();
    assert!(
        status.contains("abandoned"),
        "a record a minute old still reads as being saved: {body}"
    );
    let age = body["pending"][0]["age_ms"].as_u64().unwrap_or_default();
    assert!(age >= 60_000, "the age is missing or short: {body}");
    let _ = std::fs::remove_dir_all(&project);
}

/// **A snip that already has its picture is not also listed as pending.** Between the snip's own
/// write and the pending record's removal, a list showed it in both places.
#[test]
fn a_snip_that_has_its_picture_is_not_also_listed_as_pending() {
    let (project, dir) = snips_folder("written_twice");
    bevy_grimoire_snip::write_snip(
        &dir,
        "snip-7000",
        picture(),
        [0, 0, 8, 8],
        record("snip-7000", now_ms(), "done"),
    )
    .expect("the snip is written");
    bevy_grimoire_snip::write_pending(&dir, &record("snip-7000", now_ms(), "done"))
        .expect("the pending record is written");

    let mut server = Server::start_with(&["--project", project.to_str().expect("a UTF-8 path")]);
    let answer = server.call("grimoire_list_snips", json!({}));
    let body = answer_text(answer["result"]["content"].as_array().expect("content"));
    assert_eq!(body["total"], 1, "{body}");
    assert_eq!(
        body["pending"].as_array().map(Vec::len),
        Some(0),
        "a finished snip is also listed as being saved: {body}"
    );
    let _ = std::fs::remove_dir_all(&project);
}

/// **The pending list is capped by `limit`, and the true count is named.** A burst of confirms
/// must not be answered with every record in full.
#[test]
fn the_pending_list_is_capped_by_limit_and_names_its_total() {
    let (project, dir) = snips_folder("pending_cap");
    for n in 1..=3u64 {
        bevy_grimoire_snip::write_pending(
            &dir,
            &record(&format!("snip-{}", 8000 + n), now_ms(), "burst"),
        )
        .expect("the pending record is written");
    }

    let mut server = Server::start_with(&["--project", project.to_str().expect("a UTF-8 path")]);
    let answer = server.call("grimoire_list_snips", json!({"limit": 1}));
    let body = answer_text(answer["result"]["content"].as_array().expect("content"));
    assert_eq!(body["pending"].as_array().map(Vec::len), Some(1), "{body}");
    assert_eq!(body["pending_total"], 3, "{body}");
    let _ = std::fs::remove_dir_all(&project);
}

/// **A pending record that will not parse is named in `not_snips`.** A crash mid-write leaves one
/// of these, and before the fix it was in neither list.
#[test]
fn a_pending_record_that_does_not_parse_is_named() {
    let (project, dir) = snips_folder("broken_pending");
    std::fs::write(dir.join("pending-snip-9000.json"), "{ not a record")
        .expect("the broken record is written");

    let mut server = Server::start_with(&["--project", project.to_str().expect("a UTF-8 path")]);
    let answer = server.call("grimoire_list_snips", json!({}));
    let body = answer_text(answer["result"]["content"].as_array().expect("content"));
    let named = body["not_snips"]
        .as_array()
        .expect("not_snips is a list")
        .iter()
        .any(|p| p.as_str().unwrap_or_default().contains("pending-snip-9000.json"));
    assert!(named, "the broken pending record is reported nowhere: {body}");
    let _ = std::fs::remove_dir_all(&project);
}

/// **A still says what the picture IS, so two of them can be compared** — E-062.
///
/// On 2026-10-24 this loop compared two captures by their **files** and got identical bytes; it read
/// that as "the screen did not change" and built a whole (wrong) diagnosis on it. The real cause was
/// a capture read back before its file landed — a raw `bevy_debugger/screenshot` returns an id in
/// ~20 ms and writes the PNG a frame or two later.
///
/// The tool that does await the write already exists (`grimoire_capture_still` polls
/// `bevy_debugger/capture` until the frame is done), so the missing piece was **a fact in the answer
/// that says whether two stills are the same picture**. The fingerprint is that fact: an agent
/// comparing two captures from different frames gets two different numbers without opening either
/// file, and two stills of an unchanged screen get the same one.
///
/// This test pins the contract on the shape of the answer, which is what an agent reads. The live
/// half — that the editor's own frames differ after a camera move — is the loop's own measurement,
/// recorded in the iteration report.
#[test]
fn a_still_says_what_the_picture_is_so_two_can_be_compared() {
    let listed = {
        let mut server = Server::start();
        server.request("tools/list", json!({}))
    };
    let tool = listed["result"]["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|t| t["name"] == "grimoire_capture_still"))
        .cloned()
        .unwrap_or_else(|| panic!("grimoire_capture_still is not listed: {listed}"));
    let description = tool["description"].as_str().unwrap_or_default();
    assert!(
        description.contains("fingerprint"),
        "the description an outside agent reads must say the answer carries a fingerprint, or the \
         number is a field nobody knows to compare: {description}"
    );
    assert!(
        description.to_lowercase().contains("same picture")
            || description.to_lowercase().contains("identical"),
        "and it must say what the number is FOR: {description}"
    );
}

/// **The fingerprint is the published FNV-1a, and it distinguishes pictures by their bytes.**
///
/// **This calls the server's own function**, not a restatement of it: `bevy_cauldron_mcp::fingerprint`
/// is the one `grimoire_capture_still` uses. A test of a copy would pass while the server's version
/// had lost its zero-padding or reversed the xor/multiply order — the failure a duplicate invites.
///
/// The vectors are the published ones (Fowler, Noll & Vo, *FNV*, 1991), so a "hash" that merely
/// distinguishes its inputs cannot pass by accident.
#[test]
fn the_fingerprint_matches_the_published_vectors() {
    for (input, want) in [
        ("", "cbf29ce484222325"),
        ("a", "af63dc4c8601ec8c"),
        ("foobar", "85944171f73967e8"),
        ("hello", "a430d84680aabd0b"),
    ] {
        assert_eq!(
            bevy_cauldron_mcp::fingerprint(input.as_bytes()),
            want,
            "FNV-1a 64 of {input:?} is not the published value"
        );
    }
}

/// **Two pictures differing by one byte do not share a fingerprint.**
#[test]
fn the_fingerprint_distinguishes_pictures_by_their_bytes() {
    let a = bevy_cauldron_mcp::fingerprint(&[200u8; 64]);
    assert_eq!(a.len(), 16, "sixteen hex digits, one token: {a:?}");
    assert!(
        a.chars().all(|c| c.is_ascii_hexdigit()),
        "not hexadecimal: {a:?}"
    );
    assert_eq!(
        a,
        bevy_cauldron_mcp::fingerprint(&[200u8; 64]),
        "the same bytes must fingerprint the same"
    );
    // At the ends and the middle, because a hash that only looked at a prefix would pass a spot
    // check.
    for at in [0usize, 31, 63] {
        let mut other = vec![200u8; 64];
        other[at] = 201;
        assert_ne!(
            bevy_cauldron_mcp::fingerprint(&other),
            a,
            "a picture differing at byte {at} must not share a fingerprint"
        );
    }
}

/// **`grimoire_present` is listed, and refuses readably with no editor** (S1.7).
///
/// The brief's test for a new tool: list it, read only its description, call it, and check the answer
/// is usable with no project knowledge. The description has to say what it returns *and* that the
/// answer is not immediate — an agent that expects the person's decision in the same reply would call
/// it again and put up a second question.
#[test]
fn grimoire_present_is_listed_and_refuses_readably_with_no_editor() {
    let mut server = Server::start();
    let listed = server.request("tools/list", json!({}));
    let present = listed["result"]["tools"]
        .as_array()
        .expect("tools/list answers a list")
        .iter()
        .find(|t| t["name"] == "grimoire_present")
        .unwrap_or_else(|| panic!("grimoire_present is not published, so an agent cannot present: {listed}"));
    let description = present["description"].as_str().unwrap_or_default();
    assert!(
        description.contains("Keep") && description.contains("presentation.answer"),
        "the description does not say how the person answers or where the agent reads it, so an \
         agent will expect the decision in the same reply: {description:?}"
    );
    assert_eq!(
        present["annotations"]["readOnlyHint"], false,
        "a presentation puts a question on screen, so it is not read-only"
    );

    let answer = server.call(
        "grimoire_present",
        json!({"title": "a courtyard", "summary": "nothing was made"}),
    );
    assert!(
        answer.get("error").is_none(),
        "the call was refused at the protocol level, so the agent cannot read why: {answer}"
    );
    let text = answer["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("editor") || text.contains("BEVY_BRP_PORT"),
        "the refusal does not say the editor was not there: {text}"
    );
}

/// **A project with a `rules.ron` in it, and the path that holds it.**
///
/// The rules a layout is generated from are project data at `<root>/assets/grimoire/rules.ron`, beside
/// `kits.ron`. The editor is not needed to read them: they are a file, so the server answers with the
/// editor closed, as `grimoire_list_snips` does.
fn project_with_rules(name: &str, rules: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "grimoire_rules_{name}_{}_{nanos}",
        std::process::id()
    ));
    let dir = root.join("assets/grimoire");
    std::fs::create_dir_all(&dir).expect("the grimoire folder can be made");
    std::fs::write(dir.join("rules.ron"), rules).expect("the rules file is written");
    (root, dir)
}

/// **The rules a layout is generated from, as the project states them.**
///
/// The loop's Inspect step says *"for generated content, the agent runs several seeds and reports how
/// varied the results are"*, and the loop log records that the brief's own Generate lens asks whether
/// one constraint collapses the variety. None of that is possible while the rules are a file only a
/// person can read: an agent that generates a layout cannot say what shaped it, and cannot try one
/// number without opening a text editor.
///
/// **Why this test exists at all.** `grimoire_list_pieces` and `grimoire_state` do not carry rules, and
/// the agent's own tool list had no word for them; the rules were reachable only by reading a file off
/// disk, which is the parity gap this tool closes.
#[test]
fn an_agent_can_read_the_rules_a_layout_is_generated_from() {
    let rules = r#"(
    version: 1,
    blocks: [
        (
            example: "town_block",
            regions: ["town/houses_brick"],
            cell: 2.0,
            size: (16, 16),
            border: Some("town/wall_segment"),
            no_empty: true,
            weights: [ ( prefix: "town/road", times: 2.5 ) ],
            attempts: 40,
            seed: 1234,
        ),
    ],
)"#;
    let (root, _dir) = project_with_rules("read", rules);
    let mut server = Server::start_with(&["--project", root.to_str().expect("a UTF-8 path")]);

    let answer = server.call("grimoire_list_rules", json!({}));
    assert!(
        answer.get("error").is_none(),
        "the call was refused at the protocol level, so an agent cannot read why: {answer}"
    );
    assert!(
        answer["result"]["isError"].as_bool() != Some(true),
        "reading the rules was refused: {answer}"
    );
    let body = answer_text(answer["result"]["content"].as_array().expect("content"));
    let blocks = body["blocks"].as_array().expect("blocks is a list");
    assert_eq!(blocks.len(), 1, "the one stated block is not listed: {body}");
    let block = &blocks[0];
    assert_eq!(block["example"], "town_block", "{body}");
    assert_eq!(block["weights"][0]["prefix"], "town/road", "{body}");
    assert_eq!(block["weights"][0]["times"], 2.5, "{body}");
    assert_eq!(block["attempts"], 40, "{body}");
    // **The path is in the answer**, so an agent can cite the file it read and the person can open it.
    assert!(
        body["path"].as_str().unwrap_or_default().ends_with("rules.ron"),
        "the answer does not name the file it read: {body}"
    );

    // **Every field the description promises is in the answer**, not a sample of them: an agent
    // reads this to decide which number to change, and a field silently missing from the JSON is
    // one it cannot cite.
    for field in ["example", "regions", "cell", "size", "border", "no_empty", "weights", "footprints", "attempts", "seed"] {
        assert!(
            block.get(field).is_some(),
            "`{field}` is promised by the description but absent from the block: {block}"
        );
    }

    // **A project that states no rules says so, rather than failing.** An absent file is no rules —
    // the same rule `Project::open` applies — and an agent that asks must not get a refusal that
    // reads like a broken project.
    let (bare, _) = project_with_rules("absent", "");
    std::fs::remove_file(bare.join("assets/grimoire/rules.ron")).expect("the file is removed");
    let mut server = Server::start_with(&["--project", bare.to_str().expect("a UTF-8 path")]);
    let answer = server.call("grimoire_list_rules", json!({}));
    let body = answer_text(answer["result"]["content"].as_array().expect("content"));
    assert_eq!(
        body["blocks"].as_array().map(Vec::len),
        Some(0),
        "a project with no rules file must answer with no blocks, not a refusal: {body}"
    );
}

/// **A file the editor would refuse to open is refused here too, whatever the reason.**
///
/// `Rules::parse` is `ron::from_str` and nothing else, so "it parsed" is not "the editor will open
/// it": a blank file does not parse at all, and a file that parses can still state a version this
/// program does not read, no attempts, a cell that is not a usable step, a footprint smaller than its
/// own front and back rows, or a yaw that is not implemented. An agent handed any of those as valid
/// would report numbers the editor cannot generate from.
#[test]
fn rules_the_editor_would_refuse_are_refused_here() {
    // **Each case is one the editor refuses**, and the four are the four shapes `Rules::validate`
    // separates: a parse error, a version, a number, and a shape.
    let cases: &[(&str, &str, &str)] = &[
        // (name, the file, a word the refusal must carry)
        ("blank", "", "rules.ron"),
        (
            "version",
            r#"( version: 99, blocks: [] )"#,
            "version 99",
        ),
        (
            "attempts",
            r#"( version: 1, blocks: [ ( example: "m", cell: 2.0, size: (4, 4), attempts: 0, seed: 1 ) ] )"#,
            "at least one attempt",
        ),
        (
            "cell",
            r#"( version: 1, blocks: [ ( example: "m", cell: 0.0, size: (4, 4), attempts: 1, seed: 1 ) ] )"#,
            "not a usable step",
        ),
        (
            "footprint",
            r#"( version: 1, blocks: [ ( example: "m", cell: 2.0, size: (4, 4), attempts: 1, seed: 1,
                footprints: [ ( prefix: "p", sizes: [(1, 2)], yaws: [0.0], gap: 0, min_count: 1 ) ] ) ] )"#,
            "at least 2x2",
        ),
        (
            "yaw",
            r#"( version: 1, blocks: [ ( example: "m", cell: 2.0, size: (4, 4), attempts: 1, seed: 1,
                footprints: [ ( prefix: "p", sizes: [(2, 2)], yaws: [90.0], gap: 0, min_count: 1 ) ] ) ] )"#,
            "0 or 180",
        ),
    ];
    for (name, rules, word) in cases {
        let (root, _dir) = project_with_rules(name, rules);
        let mut server = Server::start_with(&["--project", root.to_str().expect("a UTF-8 path")]);
        let answer = server.call("grimoire_list_rules", json!({}));
        let body = answer_text(answer["result"]["content"].as_array().expect("content"));
        let text = serde_json::to_string(&body).unwrap_or_default();
        assert!(
            body.get("refused").is_some(),
            "the `{name}` rules were answered as valid, and the editor will not open them: {body}"
        );
        assert!(
            text.contains(word),
            "the `{name}` refusal does not say `{word}`, so an agent cannot tell what to fix: {body}"
        );
    }
}

/// **A rules file that does not parse is refused by name, with the reason.**
///
/// The loop's rule is that bad data never takes the editor down and never reads as an empty answer: an
/// agent told "no rules" when the file is broken would generate against nothing and report a result
/// that had nothing to do with what the person wrote.
#[test]
fn a_broken_rules_file_is_refused_by_name() {
    let (root, dir) = project_with_rules("broken", "( version: 1, blocks: [ oops");
    let mut server = Server::start_with(&["--project", root.to_str().expect("a UTF-8 path")]);
    let answer = server.call("grimoire_list_rules", json!({}));
    let body = answer_text(answer["result"]["content"].as_array().expect("content"));
    assert!(
        body.get("refused").is_some() || answer["result"]["isError"].as_bool() == Some(true),
        "a rules file that does not parse answered as if it were fine: {body}"
    );
    let text = serde_json::to_string(&body).unwrap_or_default();
    assert!(
        text.contains("rules.ron"),
        "the refusal does not name the file, so an agent cannot tell which data is wrong: {body}"
    );
    assert!(
        dir.join("rules.ron").is_file(),
        "the refused file must be left exactly as it was"
    );
}
