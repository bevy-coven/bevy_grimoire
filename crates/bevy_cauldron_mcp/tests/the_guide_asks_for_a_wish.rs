//! **The guide asks every agent for its wish, after the hand-back** (#355, AW-03).
//!
//! The series (tracking #352) gives an agent a way to say what the editor was missing: `grimoire_wish`
//! records the one tool that would most have helped with the task it just handed back. A tool an agent
//! never hears about is a tool nobody calls, so the guide — the one document an agent reads before it
//! calls anything — carries a **After a task: reflect** section that says when to wish, what a good
//! wish is, and what a wish is not.
//!
//! **The field names come from the tool's own input schema, not from the guide.** A guide that names
//! `tool` while the schema calls it `what` is a guide that teaches an agent to write a call the server
//! refuses — so the names are read out of `tools/list`'s schema for `grimoire_wish` and every one of
//! them must appear, as a field the agent types, inside the section.
//!
//! **The server is called as an outside agent calls it**: a subprocess, MCP over stdio, on a port
//! nothing listens on, so a passing test proves the section is shipped rather than fetched.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

/// **The heading the section must be under**, exactly as the issue names it.
const REFLECT_HEADING: &str = "## After a task: reflect";

/// **The heading the reflect section follows**, which is where the hand-back happens.
const HAND_BACK_HEADING: &str = "## Hand back";

/// **The heading after it**, so the section sits between the hand-back and the note workflow rather
/// than at the end of the guide where an agent reads it after it has already stopped working.
const NEXT_HEADING: &str = "## Working a note";

/// **The document as the repository holds it.**
const ON_DISK: &str = include_str!("../../../docs/mcp-tools.md");

/// **A local port with nothing listening on it**: bind an ephemeral port, then let it go. The kernel
/// does not hand the same ephemeral port straight back out, so it stays closed for the test.
fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("an ephemeral port is available for the closed-port probe");
    let port = listener
        .local_addr()
        .expect("the probe's listener has an address")
        .port();
    drop(listener);
    port
}

/// **A running MCP server, as a subprocess, with no editor behind it.**
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
                "clientInfo": {"name": "bevy_grimoire's reflect test", "version": "0.1.0"},
            }),
        );
        assert!(
            answer.get("error").is_none(),
            "the server refused the handshake: {answer}"
        );
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
            let value: Value = serde_json::from_str(line.trim_end()).unwrap_or_else(|e| {
                panic!("the server sent a line that is not JSON: {} ({e})", line.trim_end())
            });
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

    /// **The tool list the server returns**, as the raw declarations.
    fn tools(&mut self) -> Vec<Value> {
        let listed = self.request("tools/list", json!({}));
        listed["result"]["tools"]
            .as_array()
            .unwrap_or_else(|| panic!("the server's tool list is not an array: {listed}"))
            .clone()
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

/// **A `## ` section of the guide, from its heading to the next one**, or a panic naming the heading
/// that is missing.
fn section<'a>(guide: &'a str, heading: &str) -> &'a str {
    let start = guide.find(heading).unwrap_or_else(|| {
        panic!("the guide has no `{heading}` section. Its headings are: {:?}", headings(guide))
    });
    let rest = &guide[start + heading.len()..];
    let end = rest.find("\n## ").map(|at| start + heading.len() + at).unwrap_or(guide.len());
    &guide[start..end]
}

/// **Every `## ` heading in the guide**, so a failure says which sections there are.
fn headings(guide: &str) -> Vec<String> {
    guide
        .lines()
        .filter(|line| line.starts_with("## "))
        .map(str::to_owned)
        .collect()
}

/// **The names a schema declares, at this level and one level down**, as the fields an agent would
/// type.
///
/// **One level down because `after` is an object**: `{presentation: "present-1"}` or `{note: "n3"}`,
/// and the agent types those two names too. Nothing deeper, because the wish has no third level.
///
/// **`$ref` is followed into `$defs`**, because schemars puts the `after` object there and a walker
/// that stopped at the reference would check one field of the five and pass.
fn declared_fields(schema: &Value) -> Vec<String> {
    fn names(value: &Value, root: &Value, out: &mut Vec<String>) {
        if let Some(reference) = value["$ref"].as_str() {
            let name = reference.rsplit('/').next().unwrap_or_default();
            if let Some(resolved) = root["$defs"].get(name) {
                names(resolved, root, out);
            }
            return;
        }
        let Some(properties) = value["properties"].as_object() else { return };
        for (name, field) in properties {
            out.push(name.clone());
            names(field, root, out);
        }
    }
    let mut out = Vec::new();
    names(schema, schema, &mut out);
    out
}

/// **The guide the server returns names `grimoire_wish`, every field it takes, and says where the
/// section sits.**
///
/// **The fields are the schema's**, read from the running server's `tools/list`: the guide and the
/// tool cannot drift, and a rename in `WishParams` fails here rather than teaching an agent a field
/// the server refuses.
#[test]
fn the_guide_asks_for_a_wish_and_names_every_field() {
    let mut server = Server::start();
    let declared = server.tools();
    let wish = declared
        .iter()
        .find(|tool| tool["name"] == "grimoire_wish")
        .unwrap_or_else(|| {
            panic!(
                "the server does not offer `grimoire_wish`. It offers: {:?}",
                declared
                    .iter()
                    .map(|tool| tool["name"].clone())
                    .collect::<Vec<_>>()
            )
        });
    let fields = declared_fields(&wish["inputSchema"]);
    assert!(
        fields.len() >= 5,
        "`grimoire_wish`'s schema declares {} fields, which is too few for this test to mean \
         anything: {fields:?}",
        fields.len()
    );

    let answer = server.call("grimoire_guide", json!({}));
    let guide = server.text_of(&answer);
    let reflect = section(&guide, REFLECT_HEADING);

    // **Right after the hand-back.** A reflect section at the end of the guide is read after the
    // agent has finished and stopped; one after `## Hand back` is read where the hand-back is.
    assert!(
        guide.find(HAND_BACK_HEADING) < guide.find(REFLECT_HEADING),
        "`{REFLECT_HEADING}` does not follow `{HAND_BACK_HEADING}`. The headings are: {:?}",
        headings(&guide)
    );
    assert!(
        guide.find(REFLECT_HEADING) < guide.find(NEXT_HEADING),
        "`{REFLECT_HEADING}` is not before `{NEXT_HEADING}`, so it is out of the task order. The \
         headings are: {:?}",
        headings(&guide)
    );

    // **The tool, and each field, as a name the agent types.** A field named in prose without the
    // backticks an example uses is a field an agent guesses at.
    assert!(
        reflect.contains("`grimoire_wish`"),
        "the reflect section never names the tool: {reflect}"
    );
    let unnamed: Vec<&String> = fields
        .iter()
        .filter(|field| !reflect.contains(&format!("`{field}`")))
        .collect();
    assert!(
        unnamed.is_empty(),
        "the reflect section never names {unnamed:?}, so an agent that reads the guide cannot write \
         the call `grimoire_wish` takes. Its fields, from the tool's own schema, are {fields:?}.\n\
         --- the section ---\n{reflect}"
    );
}

/// **The section says what a wish is not**, in the three shapes the issue names.
///
/// **Not decoration.** A wish that is a bug report, a request for more permissions, or the same tool
/// asked for twice is a queue entry the person cannot act on, so the guide says so where the wishing
/// happens. Each sentence is looked for by the words it is about, because the wording is the guide's
/// and only the subject is the contract.
#[test]
fn the_reflect_section_says_what_a_wish_is_not() {
    let mut server = Server::start();
    let answer = server.call("grimoire_guide", json!({}));
    let guide = server.text_of(&answer);
    let reflect = section(&guide, REFLECT_HEADING).to_lowercase();
    for (subject, what) in [
        ("a wish is not", "the section never says what a wish is not"),
        ("bug", "the section never says a bug belongs in the hand-back, not in a wish"),
        ("permission", "the section never rules out a wish for more permissions"),
        ("already wished", "the section never rules out repeating a wish from this session"),
    ] {
        assert!(reflect.contains(subject), "{what}: {reflect}");
    }
    assert!(
        reflect.contains("no wish"),
        "the section never says that a task nothing made hard gets no wish, so an agent writes an \
         empty one: {reflect}"
    );
}

/// **The wish example in the section is a wish the editor accepts.**
///
/// **A guide whose example is refused is worse than no example**: an agent copies it, gets a refusal
/// naming a field, and spends the turn it just saved finding out why. So the example is parsed as
/// JSON, its field names are compared with the tool's own `required` list, and the wish it describes
/// is handed to [`bevy_grimoire_core::wish::Wish::validate`] — **the validator the editor refuses
/// with**, not a copy of its limits written here. A schema with no `maxLength` would let a 250-character
/// `tool` through a length check that only read the schema; the validator does not.
#[test]
fn the_reflect_sections_example_is_a_wish_the_editor_takes() {
    let mut server = Server::start();
    let declared = server.tools();
    let wish = declared
        .iter()
        .find(|tool| tool["name"] == "grimoire_wish")
        .unwrap_or_else(|| panic!("the server does not offer `grimoire_wish`"));
    let schema = &wish["inputSchema"];
    let required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap_or_else(|| panic!("`grimoire_wish` declares no required list: {schema}"))
        .iter()
        .filter_map(|name| name.as_str())
        .collect();

    let answer = server.call("grimoire_guide", json!({}));
    let guide = server.text_of(&answer);
    let reflect = section(&guide, REFLECT_HEADING);
    let block = reflect
        .split("```text\n")
        .nth(1)
        .and_then(|rest| rest.split("```").next())
        .unwrap_or_else(|| {
            panic!("the reflect section carries no ```text example of a wish: {reflect}")
        });
    let example: Value = serde_json::from_str(block.trim()).unwrap_or_else(|e| {
        panic!("the reflect section's example is not JSON, so it cannot be copied: {block} ({e})")
    });

    let missing: Vec<&&str> = required
        .iter()
        .filter(|field| example.get(**field).is_none())
        .collect();
    assert!(
        missing.is_empty(),
        "the example leaves out {missing:?}, which `grimoire_wish` requires: {block}"
    );
    let extra: Vec<&String> = example
        .as_object()
        .map(|object| {
            object
                .keys()
                .filter(|key| schema["properties"].get(*key).is_none())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        extra.is_empty(),
        "the example sends {extra:?}, which `grimoire_wish` has no field for: {block}"
    );
    // **The hand-back the example cites is one of the two shapes the tool takes**, and exactly one:
    // a wish citing both is refused before the editor ever sees it.
    let after = example["after"].as_object().unwrap_or_else(|| {
        panic!("the example's `after` is not an object, so it names no hand-back: {block}")
    });
    let cited: Vec<&String> = after
        .iter()
        .filter(|(_, value)| value.as_str().is_some_and(|id| !id.trim().is_empty()))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        cited.len(),
        1,
        "the example's `after` names {cited:?}; a wish follows exactly one hand-back: {block}"
    );

    // **The editor's own validator**, the one `grimoire/wish` refuses with.
    let field = |name: &str| example[name].as_str().unwrap_or_default().to_owned();
    let as_a_wish = bevy_grimoire_core::wish::Wish {
        after: after[cited[0]].as_str().unwrap_or_default().to_owned(),
        task: field("task"),
        hardship: field("hardship"),
        tool: field("tool"),
        example_call: field("example_call"),
        ..bevy_grimoire_core::wish::Wish::default()
    };
    assert!(
        as_a_wish.validate().is_ok(),
        "the guide's example is a wish the editor refuses — {} — so an agent that copies it spends \
         a turn being told no: {block}",
        as_a_wish.validate().unwrap_err()
    );
}

/// **The shipped reference carries the section too**, so an agent that reads `docs/mcp-tools.md`
/// instead of calling the tool learns the same thing.
///
/// **The whole document is compared with the server's rendering by `docs_are_current`**; this is the
/// section's own row, because the acceptance for #355 names it.
#[test]
fn the_shipped_reference_carries_the_reflect_section() {
    assert!(
        ON_DISK.contains(REFLECT_HEADING),
        "docs/mcp-tools.md has no `{REFLECT_HEADING}` section. Regenerate it with `cargo run -q -p \
         bevy_cauldron_mcp -- --emit-docs > docs/mcp-tools.md`."
    );
}
