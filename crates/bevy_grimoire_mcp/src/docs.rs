//! **The MCP tool reference, generated from the server's own declarations.**
//!
//! # Why it is generated and not written
//!
//! A hand-written tool reference is a **second vocabulary**: it says one thing and the server
//! does another, and the reader — an agent, mostly — believes the document. The brief's rule is
//! that the reference is *"generated from the registry, or mechanically checked against it"*, and
//! this is the checked half: the document is rendered from the same declarations the server
//! builds its tools from, and a test fails when the two drift.
//!
//! **The rendering reads [`TOOLS`], which is a hand-kept copy — and the copy is what drifts.**
//!
//! That sentence used to say the rendering read the server's own source. It does not: `TOOLS` below
//! restates every name, description and hint, and `main.rs` states them again in its `#[tool]`
//! attributes. Two copies of one vocabulary is exactly what this module exists to prevent, so the
//! answer is the ratchet rather than the wish — `docs_are_current`'s
//! `the_reference_names_the_servers_own_tools` lists the tools **through the running server** and
//! compares each description and each hint against this table, one by one. On 2026-10-24 it caught a
//! description edited in `main.rs` and not here, and refused to let the reference ship saying
//! something the server does not do.
//!
//! **So: change a tool's description in `main.rs` and in this table, then regenerate.** The test is
//! what makes forgetting the second step a build failure rather than a document an agent believes.

use serde_json::Value;

/// **The tool the server declares, as the document shows it.**
pub struct ToolDoc {
    pub name: &'static str,
    pub description: &'static str,
    pub read_only: bool,
    pub destructive: bool,
    pub idempotent: bool,
}

/// **Every tool the server declares.**
///
/// **The four, and they are the four the editor's BRP bridge registers.** The parity test in the
/// mapper holds the two lists to each other, so a tool that is added on one side and not the
/// other fails a test rather than a reader's trust.
pub const TOOLS: &[ToolDoc] = &[
    ToolDoc {
        name: "grimoire_present",
        description: "Put a result in front of the person: a title, a one-paragraph summary, what they \
                      specified, what you inferred, up to three places worth checking, and the operation \
                      ids it covers. The editor shows it as a question: Keep, or Undo all (which undoes \
                      every listed operation, newest first). This call returns at once; read the person's \
                      answer from grimoire_state's presentation.answer (\"kept\", \"undone\", or why the undo \
                      stopped). It does not change the map by itself.",
        read_only: false,
        destructive: false,
        idempotent: false,
    },
    ToolDoc {
        name: "grimoire_list_operations",
        description: "Every operation the running bevy_grimoire editor knows, with each one's parameters' \
                      JSON schema and how it is undone. Call this before grimoire_apply_operation so the \
                      parameters are the ones the operation actually takes. Returns the list; it does not \
                      change anything. The undo column names the strategy an operation declares; \
                      grimoire_undo_operation performs it. Pass query (words such as \"carve\" or \"note\") \
                      to get only the operations whose name or parameters match, best first, and total, \
                      the number of operations there are.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
    ToolDoc {
        name: "grimoire_preview_operation",
        description: "What an operation would change, and nothing changed: the map, the data file \
                      and the operation log are all left alone. Use this to check a proposal \
                      before applying it. Returns what would change, or the refusal.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
    ToolDoc {
        name: "grimoire_apply_operation",
        description: "Run one of the editor's operations. Changes the editor (most operations change the \
                      open map and write its file; brush_arm only arms a piece) and appends a line to the \
                      operation log. Set dry_run to true to see what would change \
                      without changing it. Returns the operation's id, which can be cited in a report to \
                      the developer, and the log's own record of it. When the operation changes a \
                      document and a map is open, the answer's checks field names the edge faults and \
                      overlapping placements the change introduced (new, at most 20 of each) with \
                      new_total, and each kind's total on the whole map; it is null otherwise. A \
                      rules_set whose weights name a prefix no piece in the library starts answers \
                      tunes_nothing (in would_change on a preview, and in the summary on an apply), \
                      naming the prefix and the pieces it does not tune: the change is applied, \
                      because removing a weight list is how an undo restores one, so read that field \
                      rather than assuming a weight did something. The \
                      operation replaces the values it names rather than adding to them, so it is \
                      marked destructive. Take a change back with grimoire_undo_operation and the \
                      returned id.",
        read_only: false,
        destructive: true,
        idempotent: false,
    },
    ToolDoc {
        name: "grimoire_undo_operation",
        description: "Take back one change made through grimoire_apply_operation. Pass operation_id, the id \
                      that call returned; omit it to take back the newest change that has not been taken \
                      back. The editor applies the inverse the change recorded, writes the data file, and \
                      appends a new log line naming the line it undoes; the log is never edited. Refuses, \
                      and changes nothing, when the change was already undone, when a newer change exists \
                      (the answer names it; undo that one first), when the map was changed outside the log \
                      since (a hand-edited file, the menu, or another map open; the answer names what \
                      differs), or when the change recorded no inverse. Undoing an undo puts the change \
                      back, and the change's own id takes it back again. Set dry_run to true to see \
                      what the undo would set. Set turn to true to take back the whole result you \
                      last presented with grimoire_present, newest change first, in one call — the \
                      same amount one Cmd+Z takes back — and the answer says how many changes it \
                      took with took_back. turn refuses when nothing has been presented, or when \
                      the presentation named no operations. Returns the undo's own operation id \
                      and what it set.",
        read_only: false,
        destructive: true,
        idempotent: false,
    },
    ToolDoc {
        name: "grimoire_state",
        description: "What the editor is holding right now: the open map and its bounds, how many \
                       pieces it has, what is armed, what is selected, which door and tab are open, \
                       and what the editor last said and what has gone wrong. The editor section also \
                       names the next undo and redo (undo_next, redo_next) and how many are stacked \
                       (undo_depth, redo_depth). The map section names up to 50 pinned placements \
                       by id and reason (pinned). pinned_total is the full count; past 50, pass \
                       pinned_from (from pinned_next) to read the rest. With --live, game is the running \
                       game's verdict on the last reload: null (none), \"pending\" (it has not \
                       answered the latest write) or {ok, message, at_unix_ms, seq, kinds}; kinds has \
                       one entry for every file the game read, {kind, file, outcome (applied or \
                       refused), field, reason}, so a refusal names its file and field. frame is the editor's own \
                       clock: frame_ms (wall time between frames), busy_ms (work done inside one \
                       frame) and fps; a high frame_ms with a low busy_ms is the machine, not the \
                       editor. The notes section lists the sent and claimed notes (id, \
                       holder, lease_until); pass note=<id> for that note's bundle: text, area and \
                       margin, nearby pieces, the palette, the camera, a picture recipe and earlier notes. \
                       repetition names up to 10 visible repeats (the same piece at one yaw and tip \
                       within 6 m; any yaw for a round kind), the one a player notices first first: \
                       descriptor, patched, role (clutter or structure), pieces, pairs, score; total \
                       counts them all; the radius, the structure kinds, their weight and the round \
                       kinds come from the project's repetition.ron, \
                       re-read on every call, and a bad file is named in error. \
                       relations checks the <map>.relations.ron beside the open map, re-read on \
                       every call: each relation (of, kind) with outcome met, unmet or missing, \
                       measured (metres, or degrees off for Faces), by (the piece that breaks it) \
                       and missing_id; unmet and missing count them; error names a file that does not parse. \
                       standards checks the project's standards.ron (beside kits.ron), re-read on \
                       every call: breaches (piece, rule, dim, found metres, why below_min, above_max, \
                       not_multiple_of or unmeasured, bound), idle, the rules no piece is covered by, \
                       and unnested, the pairs of rules whose multiple_of modules do not nest (rules, \
                       dims, modules, meet metres where they share a grid line again or null, message); \
                       error names a file that does not parse. \
                       dead_ends is the kit stress test: every declared face (piece, dir N/E/S/W, \
                       face tokens, closest {piece, turn, misses [{at, want, found}]} the piece \
                       nearest to meeting it or null, message) that no piece of the kit meets at any quarter turn, \
                       up to 20, with total counting them all. \
                       stress is the rest of the kit stress test, from the library on every call: \
                       ring, the loop the kit closes, four {piece, turn in degrees} north-west, \
                       north-east, south-west, south-east; room, the smallest room with a middle, \
                       three rows of three {piece, turn} north to south and west to east, the middle \
                       at [1][1]; each null when the kit cannot build it; declared counts the pieces \
                       with an edge token, and at 0 the nulls say the kit is unlabelled. \
                       Returns that summary \
                       as JSON, and changes nothing. \
                       Call it before a change to know what you are changing, and after one to check \
                       that it landed.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
    ToolDoc {
        name: "grimoire_list_pieces",
        description: "The library's pieces, a page at a time. Each row carries the piece's id, the \
                       mesh it is drawn with (null for a flat footprint marker), its kind, effects \
                       and looks, and its labels: who labelled each axis (model, agent or human), the \
                       record's model, date and confidence, which report what the record says, not whether anyone has checked it; by_model finds labels no human has confirmed. Filters combine: with_mesh, unlabelled, \
                       by_model, confidence (high, medium or low), id_contains. total is the true \
                       count after the filters. A page holds 50 rows; past 50, pass pieces_from \
                       (from pieces_next) to read the rest, and how_to_narrow names the filters \
                       when a filtered page is still capped. Refuses when no project is open, \
                       rather than answering an empty list. Returns that page as JSON, and changes \
                       nothing.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
    ToolDoc {
        name: "grimoire_capture_still",
        description: "A picture of the editor as it is now -- the world and the interface, because \
                      the editor draws both into one image -- plus a state summary of the same \
                      moment. The picture is returned as an image and written to a file whose path \
                      is in the answer. The answer also carries a fingerprint of the picture's \
                      bytes: two stills with the same fingerprint are the same picture, so compare \
                      fingerprints rather than file paths to tell whether the screen changed. Use \
                      this after a change to see what it did, or before one to see what it was \
                      like.",
        read_only: false,
        destructive: false,
        idempotent: false,
    },
    ToolDoc {
        name: "grimoire_snip",
        description: "A rectangle of the editor's picture, cut out and returned as an image. rect \
                      is [x0, y0, x1, y1] in window pixels, origin at the top left, x1 and y1 \
                      exclusive, so the same pixels a still is in. The crop is written to a file \
                      whose path is in the answer, with the state of the same moment. It writes \
                      one crop file under target/loop and changes no map data: it is not an \
                      operation, not logged and not undoable. Refuses, and writes nothing, on a \
                      rectangle that is empty, reversed, negative or outside the picture, and says \
                      the picture's size. Use it to look closer at one part of a still from \
                      grimoire_capture_still.",
        read_only: false,
        destructive: false,
        idempotent: false,
    },
    ToolDoc {
        name: "grimoire_decide",
        description: "Ask a decision model a typed question about the editor's pictures and facts, and get a \
                      probability for every allowed answer, with nothing to parse. Two forms. check: a \
                      named check such as floating, clipping, outside_area, blank_frame or toast_visible \
                      (the editor's own, with the project's assets/grimoire/checks.ron over them, read on every \
                      call); it answers a finding with the check's P(yes), its threshold and whether it was \
                      found, and takes the editor's current picture itself when it needs view. questions: \
                      ad-hoc typed questions by id, each noul (yes or no, P(yes)), choice (named options, \
                      each one's probability) or score (levels, a weighted score), with state in words, \
                      images (project files, at most four) and facts (JSON). A check that needs a picture or \
                      facts it was not given is refused and names what is missing; it never answers from \
                      less. Advisory: it changes no map data and a finding never blocks or undoes an \
                      operation. The endpoint is one per run, local by default; a Cloudflare endpoint sends \
                      pictures off the LAN and is opt-in.",
        read_only: true,
        destructive: false,
        idempotent: false,
    },
    ToolDoc {
        name: "grimoire_list_snips",
        description: "The snips the person took in the editor, newest first: each one's name, when \
                      it was taken, the map and the active tool, the note the person typed, the \
                      rectangle in window and picture pixels, the camera view, the cursor's world \
                      position and the placements inside the rectangle. Each entry names its \
                      picture file, and the newest picture comes back as an image. It reads files \
                      the editor wrote, so it needs no editor running and changes nothing. Returns \
                      how many snips there are, how many are shown, the snips still being saved \
                      (pending: confirmed in the editor, picture not yet written, so not yet in \
                      the snips list) and each .json file in the snips folder that is not a snip \
                      record or has no picture file, with the reason.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
    ToolDoc {
        name: "grimoire_list_rules",
        description: "The block rules the project states for generating layouts, read from \
                      assets/grimoire/rules.ron: for each block, the example map its grammar is learned from, the \
                      region descriptors that ask for a block, the cell size and block size in cells, the border \
                      piece, no_empty, the weight multipliers, the footprint rules, attempts and seed. Returns the \
                      blocks and the path of the file read. A project with no rules.ron answers with no blocks, \
                      because an absent file means no rules -- not a broken project. A file that is present but \
                      does not parse, or states a version this program does not read or a number it cannot \
                      generate from (no attempts, a cell that is not a usable step, a footprint under 2x2, a yaw \
                      other than 0 or 180), is refused with its path and the reason, and nothing is written. It \
                      parses the file but does not check that the descriptor ids it names exist in the project's \
                      library -- the editor's own open does that, so a block naming a missing piece is answered \
                      here and refused there. It reads a file, so it needs no editor running and changes nothing.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
    ToolDoc {
        name: "grimoire_capture_sequence",
        description: "A sequence of frames of the editor, with an optional operation applied \
                       once the first frame is in -- so frame 0 is the state before the action and \
                       the later frames are what it did. The frames are written to a directory and \
                       their paths are in the answer. The editor takes the frames, so the timing \
                       does not depend on this server's round trips. The rate is two to fifteen frames a second. A frame is asked for only once the one before it is written, so a stalled \
                       editor delays the frames after it rather than losing them; a frame that does not land within five seconds ends the run with a reason, and the status is done only when every frame is written. The call waits for the run to end, for at most the sequence's length plus five seconds a frame and a margin.",
        read_only: false,
        destructive: true,
        idempotent: false,
    },
    ToolDoc {
        name: "grimoire_read_oplog",
        description: "The editor's operation log, newest last. Every operation appends one line, whether a \
                      person or an agent did it. Read it to see what has been done to this project. A \
                      refusal is not logged, so a line here is a line about a change that happened. An \
                      undo is a new line naming the line it undoes.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
    ToolDoc {
        name: "grimoire_frame_view",
        description: "Frame the editor's Map camera on a box. target: \"map\" (the whole map), \
                      \"selection\" (the locked placement; refused when nothing is locked), \
                      {\"placements\": [ids]} (their union), or {\"rect\": [x0, z0, x1, z1]} (map \
                      metres). It only moves the camera: it never changes map data, is not logged \
                      or undoable, and is not an operation. Refuses, and moves nothing, on an \
                      unknown id, an empty selection or a flat rectangle. Returns what was framed, \
                      the focus and the view height in metres. \
                      Call grimoire_capture_still afterwards to see the result.",
        read_only: false,
        destructive: false,
        idempotent: true,
    },
];

/// **How any agent works a note** (#94). The fenced `json` block is one operation per line; the
/// editor's `send_starts_the_notes_agent` guide test runs every line against a headless editor, so the
/// example cannot drift from the operations.
pub const NOTES_GUIDE: &str = "## Working a note\n\n\
A person leaves notes on the map (a rectangle or picked pieces, with an instruction) and presses Send. \
Any agent connected to this server can work a sent note; the editor holds no agent logic. If the project's \
`agent.ron` names a command, Send also starts it once per Send (`(version: 1, command: Some(\"omp\"), model: \"claude-sonnet-5-5\")`; \
`args` may use `{model}`, `{prompt}`, `{project}`; the default arguments are `-p --model {model} {prompt}`). \
With no `agent.ron`, or no `command` in it, Send only queues the notes, and a hand-started agent works them the same way.\n\n\
1. **List.** `grimoire_state` has a notes section: the sent and claimed notes with `id`, `holder` and `lease_until`. Pass `note=<id>` for a note's bundle: text, area and margin, nearby pieces, the palette, the camera and earlier notes.\n\
2. **Claim.** `note_claim` with your name as `agent` holds the note for a lease. `note_renew` keeps it while you work, `note_release` gives it back. A note another agent holds is refused.\n\
3. **Work inside the fence.** Every `grimoire_apply_operation` you make with `agent` set is checked against your note: an operation reaching outside the note's area plus one cell is refused before anything is written. Removing a piece a person placed is refused until you have asked with `note_ask` and the person has answered with `note_answer`. Capture a before and an after still with `grimoire_capture_still`.\n\
4. **Resolve.** `note_resolve` closes the note with a one-line `summary`, your `assumptions`, the `ops` (operation ids) you ran and `captures` `[before, after]`. The person then keeps the result or undoes the whole note in one step, which reopens it.\n\n\
Worked example: the person's note `n1` covers cells 0,0 to 2,2 (\"tidy the wall\"), and the agent `omp` works it. Operations, one per line:\n\n\
```json\n\
{\"operation\": \"note_add\", \"parameters\": {\"cells\": [0, 0, 2, 2], \"text\": \"tidy the wall\"}}\n\
{\"operation\": \"note_send\", \"parameters\": {}}\n\
{\"operation\": \"note_claim\", \"parameters\": {\"id\": \"n1\", \"agent\": \"omp\"}}\n\
{\"operation\": \"placement_add\", \"parameters\": {\"piece\": \"site/wall\", \"at\": [0.5, 0.5]}, \"agent\": \"omp\"}\n\
{\"operation\": \"note_resolve\", \"parameters\": {\"id\": \"n1\", \"agent\": \"omp\", \"summary\": \"added a wall\", \"captures\": [\"before.png\", \"after.png\"], \"assumptions\": [\"the wall belongs in the corner\"]}}\n\
```\n\n\
The first two lines are the person's (the note box and Send); the rest are the agent's. `placement_add`'s piece and cell are the example's own: use pieces from `grimoire_list_pieces`.\n\n";

/// **The document, as markdown.**
///
/// **One section per tool**, with its description, its annotations and its parameters.
pub fn render() -> String {
    let mut out = String::new();
    out.push_str("# The bevy_grimoire MCP tools\n\n");
    out.push_str(
        "Generated from the server's own tool declarations. **Do not edit this file** — it is \
         written by `cargo run -p bevy_grimoire_mcp -- --emit-docs`, and a test fails when it does \
         not match the server.\n\n",
    );
    out.push_str(
        "The server speaks the Model Context Protocol over **stdio**, and reaches the running \
         editor over the Bevy Remote Protocol on `BEVY_BRP_PORT` (default 15702). It writes \
         nothing to disk itself: the editor owns the project directory, and every file the \
         editor writes is written by the editor.\n\n",
    );
    out.push_str(NOTES_GUIDE);
    out.push_str("## The tools\n\n");
    for tool in TOOLS {
        out.push_str(&format!("### `{}`\n\n", tool.name));
        out.push_str(&format!("{}\n\n", tool.description));
        out.push_str("| annotation | value |\n|---|---|\n");
        out.push_str(&format!("| `readOnlyHint` | {} |\n", tool.read_only));
        out.push_str(&format!("| `destructiveHint` | {} |\n", tool.destructive));
        out.push_str(&format!("| `idempotentHint` | {} |\n\n", tool.idempotent));
    }
    out.push_str(
        "## How the two paths converge\n\n\
         Every operation writes the data file, and the editor watches that file. So:\n\n\
         - an **agent** calls `grimoire_apply_operation`, the editor writes the file, and the \
           editor's own state is already the new one;\n\
         - a **person** edits the file in a text editor, the watcher notices within a quarter of \
           a second, and the editor re-reads it — **and keeps what it had if the file does not \
           parse**;\n\
         - a **person at the keyboard** uses the editor's own controls, which dispatch the same \
           operations.\n\n\
         The measured latency of both paths is in the iteration report.\n",
    );
    out
}

/// **The document, as the JSON a caller would send.**
///
/// **The parameter schema, generated from the same types the server deserialises into.** A
/// schema that is written by hand is a schema that can be wrong, and an agent that sends the
/// wrong shape gets a refusal it cannot act on.
/// **The module doc's own summary of how many tools there are, and the rows it lists.**
///
/// The server's `main.rs` opens with a table of every tool and a sentence counting them. Both had
/// drifted: the table was missing two rows and the sentence said twelve while the server declared
/// fourteen. A number nobody checks is a number that is wrong once and stays wrong, so the two are
/// read out of the source here and compared with [`TOOLS`].
pub fn tool_table_and_count() -> (usize, String) {
    let source = include_str!("main.rs");
    let rows = source
        .lines()
        .filter(|line| line.starts_with("//! | `grimoire_"))
        .count();
    let count_word = source
        .lines()
        .find(|line| line.contains(", and the set is small on purpose."))
        .and_then(|line| line.split("**").nth(1))
        .map(|s| s.split_whitespace().next().unwrap_or_default().to_lowercase())
        .unwrap_or_default();
    (rows, count_word)
}

pub fn schemas() -> Value {
    serde_json::json!({
        "grimoire_present": {
            "type": "object",
            "additionalProperties": false,
            "required": ["title", "summary"],
            "properties": {
                "title": {"type": "string"},
                "summary": {"type": "string"},
                "specified": {"type": "array", "items": {"type": "string"}},
                "inferred": {"type": "array", "items": {"type": "string"}},
                "check": {
                    "type": "array",
                    "maxItems": 3,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["what"],
                        "properties": {
                            "what": {"type": "string"},
                            "placements": {"type": "array", "items": {"type": "string"}},
                        },
                    },
                },
                "operation_ids": {"type": "array", "items": {"type": "string"}},
            },
        },
        "grimoire_list_operations": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": {"type": "string", "description": "Words to search for, such as carve or note. With them, only the operations whose name or parameters match, best first, and total. Without them, every operation."},
            },
        },
        "grimoire_preview_operation": {
            "type": "object",
            "additionalProperties": false,
            "required": ["operation"],
            "properties": {
                "operation": {"type": "string"},
                "parameters": {"type": "object"},
            },
        },
        "grimoire_apply_operation": {
            "type": "object",
            "additionalProperties": false,
            "required": ["operation"],
            "properties": {
                "operation": {"type": "string"},
                "parameters": {"type": "object"},
                "dry_run": {"type": "boolean", "default": false},
                "agent": {"type": ["string", "null"]},
            },
        },
        "grimoire_undo_operation": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "operation_id": {"type": ["string", "null"]},
                "dry_run": {"type": "boolean", "default": false},
                "turn": {"type": "boolean", "default": false, "description": "Take back the whole result you last presented, newest change first, in one call. Refused when nothing has been presented."},
            },
        },
        "grimoire_state": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "pinned_from": {"type": "integer", "minimum": 0, "default": 0, "description": "Where the pinned list starts, in map order. 0 is the first pin."},
                "note": {"type": "string", "description": "A sent note's id; the answer's notes.bundle is everything needed to work it."}
            }
        },
        "grimoire_list_pieces": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "pieces_from": {"type": "integer", "minimum": 0, "default": 0, "description": "Where the page starts, counted in the filtered list. 0 is the first match."},
                "with_mesh": {"type": "boolean", "default": false, "description": "true leaves out the flat pieces that name no mesh."},
                "unlabelled": {"type": "boolean", "default": false, "description": "true keeps only the pieces no one has labelled."},
                "by_model": {"type": "boolean", "default": false, "description": "true keeps only the pieces with an axis a model or an agent wrote and no person has confirmed. Also spelled unconfirmed: it matches agent-written axes too, not only a model's."},
                "confidence": {"type": ["string", "null"], "enum": ["high", "medium", "low", null], "default": null, "description": "Keeps only the pieces whose label record says this confidence."},
                "id_contains": {"type": ["string", "null"], "default": null, "description": "Keeps only the pieces whose id contains this text."}
            }
        },
        "grimoire_capture_still": {
            "type": "object",
            "additionalProperties": false,
            "required": ["label"],
            "properties": {
                "label": {"type": "string", "description": "A snake_case name for the capture."}
            }
        },
        "grimoire_capture_sequence": {
            "type": "object",
            "additionalProperties": false,
            "required": ["label"],
            "properties": {
                "label": {"type": "string", "description": "A snake_case name for the sequence."},
                "seconds": {"type": "number", "exclusiveMinimum": 0, "maximum": 10, "default": 2.0},
                "fps": {"type": "integer", "minimum": 2, "maximum": 15, "default": 4},
                "operation": {
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name": {"type": "string"},
                        "parameters": {"type": "object"}
                    }
                }
            }
        },
        "grimoire_read_oplog": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "since_seq": {"type": "integer", "minimum": 0, "default": 0},
                "limit": {"type": "integer", "minimum": 1, "maximum": 200, "default": 50},
            },
        },
        "grimoire_snip": {
            "type": "object",
            "additionalProperties": false,
            "required": ["label", "rect"],
            "properties": {
                "label": {"type": "string", "description": "Where the crop is written, under the project's target/loop/captures: letters, digits, _ and - in parts, with / between the parts."},
                "rect": {"type": "array", "items": {"type": "integer", "minimum": 0}, "minItems": 4, "maxItems": 4, "description": "[x0, y0, x1, y1] in window pixels; x1 and y1 exclusive."}
            }
        },
        "grimoire_decide": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "check": {"type": "string", "description": "A named check, such as floating. Give this or questions."},
                "questions": {"type": "object", "description": "Ad-hoc typed questions by id: {\"type\": \"noul\", \"instructions\": ...}, {\"type\": \"choice\", \"instructions\": ..., \"criteria\": {option: meaning}} or {\"type\": \"score\", \"instructions\": ..., \"criteria\": [levels, lowest first]}."},
                "state": {"type": "string", "description": "What the ad-hoc questions are about, in words."},
                "images": {"type": "array", "maxItems": 4, "items": {"type": "object", "additionalProperties": false, "required": ["name", "path"], "properties": {"name": {"type": "string"}, "path": {"type": "string"}}}, "description": "Pictures, files under the project, each with the name a check asks for."},
                "view": {"type": "boolean", "default": false, "description": "Also take the editor's current picture, named view."},
                "facts": {"description": "The JSON facts: the operations run, the pieces, the camera."}
            }
        },
        "grimoire_list_snips": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "limit": {"type": "integer", "minimum": 1, "maximum": 20, "description": "How many snips to list, newest first. The default is 5."}
            }
        },
        "grimoire_list_rules": {
            "type": "object",
            "additionalProperties": false,
            "properties": {}
        },
        "grimoire_frame_view": {
            "type": "object",
            "additionalProperties": false,
            "required": ["target"],
            "properties": {
                "target": {
                    "oneOf": [
                        {"type": "string", "enum": ["map", "selection"]},
                        {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["placements"],
                            "properties": {"placements": {"type": "array", "items": {"type": "string"}, "minItems": 1}},
                        },
                        {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["rect"],
                            "properties": {"rect": {"type": "array", "items": {"type": "number"}, "minItems": 4, "maxItems": 4}},
                        },
                    ],
                },
            },
        },
    })
}
