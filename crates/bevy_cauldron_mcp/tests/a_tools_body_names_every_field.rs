//! **A tool's request body names every field the editor's params have (B-052).**
//!
//! The MCP server is a bridge: each tool takes typed parameters and forwards them to the editor over
//! BRP by **building the body by hand** — `json!({"operation_id": p.operation_id, …})`. That body is a
//! second copy of a struct that lives in `bevy_cauldron::ops_brp`, and the server crate cannot
//! depend on the mapper (it is the bridge's client), so nothing could see the two drift.
//!
//! **They drifted, and it was invisible until a live run.** E-084 added `turn` to both `UndoParams`
//! structs; the field was accepted by serde at the server, never put on the wire, and the editor
//! answered *"unknown field `turn`"*. The mapper's own tests drive `grimoire/operation_undo` directly,
//! so every deterministic check passed while the shipped tool could not do what its description said.
//!
//! # What this test is, and what it is not
//!
//! It is a **source-text ratchet**, in the shape `docs_are_current` already uses for the tool
//! reference and `docs::tool_table_and_count` uses for the tool table. It reads both files and
//! compares the field names — it does not parse Rust, so it cannot see a field renamed in one place
//! and re-`json!`-ed under its old name in the other beyond what the text says. That is the right
//! level: the defect is *a name missing from a list*, and a list is what text holds.
//!
//! **The list below is the contract.** Adding a tool that forwards parameters means adding its two
//! paths here; the test fails on a table entry whose body names a different set of fields than the
//! editor's struct, in either direction.

/// **The editor's param structs**, read at compile time.
///
/// `include_str!` rather than `read_to_string`: a path resolved at run time is relative to the
/// *process*, so it works from the crate root and not from wherever a test harness happens to run.
/// `docs_are_current` reads `docs/mcp-tools.md` the same way, for the same reason.
const EDITOR: &str = include_str!("../../bevy_cauldron/src/ops_brp.rs");
/// **The tool bodies**, read at compile time.
const SERVER: &str = include_str!("../src/main.rs");

/// **Every tool that builds a BRP body by hand**, as `(the BRP method, the editor's struct name)`.
///
/// A tool that forwards nothing (`grimoire_state`, `grimoire_capture_still`, the capture tools) is absent
/// on purpose: there is no second copy of anything to check.
const FORWARDING: &[(&str, &str)] = &[
    ("grimoire/operation_preview", "PreviewParams"),
    ("grimoire/operation_apply", "ApplyParams"),
    ("grimoire/operation_undo", "UndoParams"),
    ("grimoire/pieces", "PiecesParams"),
    ("grimoire/oplog_read", "LogParams"),
    ("grimoire/frame_view", "FrameParams"),
    ("grimoire/run_command", "RunCommandParams"),
];

/// **The field names of a struct's body**, as the source text spells them.
///
/// Reads from `struct <name>` to the first `}` at column zero — every one of these structs is
/// module-level and its fields are one per line, so the brace closes it and nothing else is captured.
fn fields_of(source: &str, name: &str) -> Vec<String> {
    let head = format!("pub struct {name} {{");
    let at = source
        .find(&head)
        .unwrap_or_else(|| panic!("no `{head}` in the source this test read"));
    let body = &source[at + head.len()..];
    let end = body.find("\n}").unwrap_or_else(|| panic!("`{name}` is never closed"));
    body[..end]
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            // `pub name: Type,` — the field's name is the word after `pub`.
            let rest = line.strip_prefix("pub ")?;
            let field = rest.split(':').next()?.trim();
            // A line inside a doc comment never starts with `pub`, and a `pub fn` is not a field.
            (!field.is_empty() && !field.contains('(')).then(|| field.to_owned())
        })
        .collect()
}

/// **The field names a `json!({…})` body writes**, as its source text spells them.
///
/// The body begins at `<method>` and ends at the `json!` call's own parenthesised group, tracked so a
/// nested object (a `target`, a placements list) does not end the scan early.
fn body_fields(source: &str, method: &str) -> Vec<String> {
    let needle = format!("\"{method}\"");
    let at = source
        .find(&needle)
        .unwrap_or_else(|| panic!("no BRP body for `{method}`"));
    let rest = &source[at + needle.len()..];
    let open = rest
        .find("json!(")
        .unwrap_or_else(|| panic!("`{method}` has no json! body"));
    let text = &rest[open + "json!(".len()..];
    // **Track nesting from the `json!`'s own paren**, so a nested object is inside the scan.
    let mut depth = 1i32;
    let mut end = text.len();
    for (i, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    // **Only the keys at the body's own depth 1.** A nested object's keys belong to another
    // contract — the placements list is the editor's, not this tool's.
    //
    // A `json!` body is `{"key": value, …}`: a key is a quoted string followed by `:` at depth 1.
    // Scanning for that shape is simpler and less wrong than tracking "am I at a key start", which
    // mis-read `"operation": p.operation` as a key named ` p.operation` — measured on this test's
    // own first run.
    let mut fields = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<(usize, String)> = None;
    let bytes: Vec<char> = text[..end].chars().collect();
    let mut i = 0usize;
    while i < bytes.len() {
        let ch = bytes[i];
        match ch {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            '"' => {
                // Read the string, then decide by what follows.
                let from = i + 1;
                let mut j = from;
                while j < bytes.len() && bytes[j] != '"' {
                    j += 1;
                }
                let word: String = bytes[from..j.min(bytes.len())].iter().collect();
                // What comes after the closing quote, ignoring spaces.
                let mut k = j + 1;
                while k < bytes.len() && bytes[k].is_whitespace() {
                    k += 1;
                }
                let followed_by_colon = bytes.get(k) == Some(&':');
                if depth == 1 && followed_by_colon {
                    fields.push(word);
                } else {
                    quote = Some((i, word));
                }
                i = j + 1;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    let _ = quote;
    fields
}

/// **Every hand-built body names exactly its editor struct's fields.**
///
/// Both directions, because the two failures are different bugs: a field the body omits is a
/// parameter the tool accepts and drops (B-052, measured), and a field the body names and the struct
/// does not have is a `deny_unknown_fields` refusal at the editor — the tool answering "unknown
/// field" to a call its own schema said was valid.
#[test]
fn every_forwarded_body_names_its_editors_fields() {
    let editor = EDITOR;
    let server = SERVER;

    for (method, name) in FORWARDING {
        let mut expected = fields_of(editor, name);
        let mut wrote = body_fields(server, method);
        expected.sort();
        wrote.sort();
        assert_eq!(
            wrote, expected,
            "`{method}`'s hand-built body and `{name}` disagree. The body must name every field the \
             editor's params have: a field it drops is a parameter this tool accepts and silently \
             discards, which is why `turn` reached the editor as \"unknown field\" (B-052)."
        );
    }
}

/// **The table is not stale**: every BRP method the server forwards to is either checked above or
/// named as forwarding nothing.
///
/// **This is the half that catches the next tool**, rather than the next drift of a known one. A new
/// `self.editor.call("grimoire/…", json!({…}))` that nobody adds to the table would otherwise be
/// unchecked — which is precisely how the first drift happened.
#[test]
fn every_forwarded_brp_method_is_checked() {
    let server = SERVER;
    // **Methods that take no parameters from a tool**, or whose params are built from something
    // other than a `*Params` struct. Each one is named here so the list is a decision.
    const NOT_FORWARDED: &[&str] = &[
        "grimoire/project_root",
        "grimoire/operation_list",
        "grimoire/present",
        "grimoire/state",
        "grimoire/capture_sequence_start",
        "grimoire/capture_sequence_status",
    ];
    let mut missing = Vec::new();
    for (at, _) in server.match_indices("self.editor.call(\"grimoire/") {
        let rest = &server[at + "self.editor.call(\"".len()..];
        let Some(end) = rest.find('"') else { continue };
        let method = &rest[..end];
        // **Only the ones that build a body from the tool's own parameters.**
        let after = &rest[end..];
        let builds = after.starts_with(",\n") || after.starts_with(',');
        if !builds {
            continue;
        }
        if FORWARDING.iter().any(|(m, _)| *m == method) || NOT_FORWARDED.contains(&method) {
            continue;
        }
        missing.push(method.to_owned());
    }
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "these BRP methods are called with a request body and are in neither the checked table nor \
         the named-not-forwarded list: {missing:?}. Add each to `FORWARDING` with the editor's param \
         struct name, or to `NOT_FORWARDED` with a reason."
    );
}
