//! **The MCP tool reference matches the server.**
//!
//! The brief's rule: *"MCP tools and `docs/mcp-tools.md` are generated from the registry, or
//! mechanically checked against it."* **This is the checked half** — the document is rendered
//! from the server's own declarations and compared with the file in the repository.
//!
//! **A stale copy fails the test**, and the failure says how to fix it. A hand-written reference
//! is a second vocabulary: it says one thing and the server does another, and the reader — an
//! agent, mostly — believes the document.

/// **The document, as the repository holds it.**
const ON_DISK: &str = include_str!("../../../docs/mcp-tools.md");

#[test]
fn the_tool_reference_is_current() {
    let rendered = bevy_cauldron_mcp::docs::render();
    assert_eq!(
        rendered, ON_DISK,
        "docs/mcp-tools.md does not match the server's own tool declarations. Regenerate it with \
         `cargo run -q -p bevy_cauldron_mcp -- --emit-docs > docs/mcp-tools.md`."
    );
}

/// **The reference names every tool the server has, and no others.**
///
/// **The tool list is the server's**, so a tool that was added and not documented — or
/// documented and removed — is caught here rather than by a reader.
#[test]
fn the_reference_names_every_tool() {
    let rendered = bevy_cauldron_mcp::docs::render();
    for tool in bevy_cauldron_mcp::docs::TOOLS {
        assert!(
            rendered.contains(&format!("### `{}`", tool.name)),
            "the reference does not document the tool `{}`.",
            tool.name
        );
    }
}

/// **The reference's schemas cover every tool.**
#[test]
fn every_tool_has_a_schema() {
    let schemas = bevy_cauldron_mcp::docs::schemas();
    let map = schemas.as_object().expect("the schemas are an object");
    for tool in bevy_cauldron_mcp::docs::TOOLS {
        assert!(
            map.contains_key(tool.name),
            "the tool `{}` has no parameter schema, so an agent cannot tell what to send it.",
            tool.name
        );
    }
    assert_eq!(
        map.len(),
        bevy_cauldron_mcp::docs::TOOLS.len(),
        "there are more schemas than tools, or more tools than schemas, so one of the lists is \
         stale."
    );
}

/// **The reference's tool list is the SERVER's tool list, not a copy of it.**
///
/// **The first version compared the reference with `docs::TOOLS`** — and `docs::TOOLS` is a
/// hand-written copy of the `#[tool]` attributes. **So changing an annotation in the server
/// passed every test**, and the reference could say the opposite of what the server does.
///
/// **So the server's own tool list is the one compared.** The server is asked, over MCP, what
/// tools it has; the reference is asked the same way; and the two must agree.
#[tokio::test]
async fn the_reference_names_the_servers_own_tools() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};

    let mut child = Command::new(env!("CARGO_BIN_EXE_bevy_cauldron_mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the server binary starts");
    let mut stdin = child.stdin.take().expect("the server has a stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("the server has a stdout"));

    let send =
        |stdin: &mut std::process::ChildStdin, id: u64, method: &str, params: serde_json::Value| {
            let body = serde_json::json!({
                "jsonrpc": "2.0", "id": id, "method": method, "params": params,
            });
            writeln!(stdin, "{body}").expect("the server's stdin is writable");
            stdin.flush().expect("the server's stdin flushes");
        };
    // **A notification has no id and no answer.** Sending one with an id makes the server treat
    // it as a request, and it answers it — so the reader waiting for the next id takes the
    // notification's answer instead and the real answer is never read.
    let notify = |stdin: &mut std::process::ChildStdin, method: &str| {
        let body = serde_json::json!({"jsonrpc": "2.0", "method": method, "params": {}});
        writeln!(stdin, "{body}").expect("the server's stdin is writable");
        stdin.flush().expect("the server's stdin flushes");
    };
    // **Read until the answer with the id asked for.** The server may send a notification in
    // between — the handshake's own — and a reader that takes the first line would take that
    // instead of the answer.
    let read =
        |stdout: &mut BufReader<std::process::ChildStdout>, want: u64| -> serde_json::Value {
            loop {
                let mut line = String::new();
                let n = stdout
                    .read_line(&mut line)
                    .expect("the server's stdout is readable");
                assert!(n > 0, "the server's stdout ended before it answered");
                let value: serde_json::Value =
                    serde_json::from_str(line.trim_end()).expect("the server sent JSON");
                if value.get("id").and_then(|i| i.as_u64()) == Some(want) {
                    return value;
                }
            }
        };

    // **The handshake carries a protocol version and a client name**, because the server reads
    // them — an initialize with empty params is not a handshake, and the server answers it with
    // an error the reader then waits past.
    send(
        &mut stdin,
        1,
        "initialize",
        serde_json::json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "bevy_grimoire's own test", "version": "0.1.0"},
        }),
    );
    let _ = read(&mut stdout, 1);
    // **The notification is a notification, and it has no answer.** It is sent and not read, and
    // the reader skips it because it carries no id.
    notify(&mut stdin, "notifications/initialized");
    send(&mut stdin, 3, "tools/list", serde_json::json!({}));
    let listed = read(&mut stdout, 3);
    // **The process is waited for**, not just killed: a killed child is a zombie until it is
    // reaped, and a test file that leaves one behind is a test file that leaks.
    let _ = child.kill();
    let _ = child.wait();

    let server: Vec<String> = listed["result"]["tools"]
        .as_array()
        .expect("the tool list is an array")
        .iter()
        .map(|t| t["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    let documented: Vec<String> = bevy_cauldron_mcp::docs::TOOLS
        .iter()
        .map(|t| t.name.to_owned())
        .collect();
    // **The same SET, and not the same order.** The server lists its tools in the order the
    // router holds them, which is alphabetical; the reference is in the order they are declared,
    // which is the order a reader wants. **The order is a presentation choice and the set is
    // the contract** — so the set is compared, and a tool that is in one and not the other is
    // the failure.
    let mut server_sorted = server.clone();
    server_sorted.sort();
    let mut documented_sorted = documented.clone();
    documented_sorted.sort();
    assert_eq!(
        server_sorted, documented_sorted,
        "the reference documents {documented:?} and the server has {server:?}. The reference is \
         generated from the server's declarations, and a hand-written copy of them is a second \
         vocabulary."
    );

    // **And each tool says the same thing in both places.** Names alone were the first version of
    // this check, and the reference then said `grimoire_apply_operation` was not destructive while
    // the server said it was — an agent reading the reference would treat a tool that replaces
    // values as harmless. **The description and the three hints are the contract**, so they are
    // compared one by one, against the server's own answer.
    let mut drift = Vec::new();
    for doc in bevy_cauldron_mcp::docs::TOOLS {
        let Some(tool) = listed["result"]["tools"]
            .as_array()
            .and_then(|tools| tools.iter().find(|t| t["name"] == doc.name))
        else {
            continue;
        };
        if tool["description"].as_str() != Some(doc.description) {
            drift.push(format!(
                "{}: description\n  server:    {}\n  reference: {}",
                doc.name, tool["description"], doc.description
            ));
        }
        let hints = &tool["annotations"];
        for (hint, documented) in [
            ("readOnlyHint", doc.read_only),
            ("destructiveHint", doc.destructive),
            ("idempotentHint", doc.idempotent),
        ] {
            if hints[hint].as_bool() != Some(documented) {
                drift.push(format!(
                    "{}: {hint} is {} on the server and {documented} in the reference",
                    doc.name, hints[hint]
                ));
            }
        }
    }
    assert!(
        drift.is_empty(),
        "the reference and the server disagree, so an agent reading docs/mcp-tools.md is told \
         something the server does not do:\n{}",
        drift.join("\n")
    );
}

/// **Every tool is prefixed, and the prefix is what keeps the namespaces apart.**
#[test]
fn every_tool_is_prefixed() {
    for tool in bevy_cauldron_mcp::docs::TOOLS {
        assert!(
            tool.name.starts_with("grimoire_"),
            "the tool `{}` is not prefixed with `grimoire_`.",
            tool.name
        );
    }
}

/// **The module doc's tool table and its count are the server's, not a copy that drifted.**
///
/// `main.rs` opens with a table of every tool and a sentence saying how many there are. Both are
/// documentation a reader trusts, and both had gone stale: two tools (`grimoire_present`,
/// `grimoire_list_pieces`) were missing rows, and the sentence said twelve while the server declared
/// fourteen. A count nobody checks is a count that is wrong once and stays wrong.
#[test]
fn the_tool_table_and_its_count_are_the_servers() {
    let (rows, count_word) = bevy_cauldron_mcp::docs::tool_table_and_count();
    assert_eq!(
        rows,
        bevy_cauldron_mcp::docs::TOOLS.len(),
        "the module doc lists {rows} tools and the reference lists {}",
        bevy_cauldron_mcp::docs::TOOLS.len()
    );
    let words = [
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven",
        "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen",
        "twenty",
    ];
    let said = words
        .iter()
        .position(|w| *w == count_word)
        .map(|i| i + 1)
        .unwrap_or_else(|| panic!("the sentence counts the tools as `{count_word}`, which is not a number"));
    assert_eq!(
        said,
        bevy_cauldron_mcp::docs::TOOLS.len(),
        "the module doc says there are {said} tools and the server declares {}",
        bevy_cauldron_mcp::docs::TOOLS.len()
    );
}
