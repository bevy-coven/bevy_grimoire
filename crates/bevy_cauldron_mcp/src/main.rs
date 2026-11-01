//! **The MCP server** — the bevy_grimoire editor, over the Model Context Protocol.
//!
//! # What it is for
//!
//! The editor's verbs used to live in **key handlers**. An agent has no keyboard, so the only way
//! to reach the editor was to synthesise keystrokes — which is a simulation of a person, and the
//! brief's rule is explicit: *"Never drive the UI by clicking."*
//!
//! This server is the other door. **It exposes the editor's operations as MCP tools**, and each
//! tool dispatches through the editor's own registry — the same function a key handler calls — so
//! an agent and a person are doing **one** thing, not two.
//!
//! # The tools
//!
//! | tool | what it does |
//! |---|---|
//! | `grimoire_list_operations` | every operation, its parameters' schema, and how it is undone; `query` searches them |
//! | `grimoire_preview_operation` | what an operation would change; nothing changes |
//! | `grimoire_apply_operation` | run one, and record it in the operation log |
//! | `grimoire_undo_operation` | take one change back by its id, as a new log line |
//! | `grimoire_read_oplog` | the log, newest last |
//! | `grimoire_frame_view` | frame the Map camera on the map, a selection, placements or a rectangle |
//! | `grimoire_state` | what the editor is holding now, as JSON |
//! | `grimoire_capture_still` | one picture of the editor, plus the state |
//! | `grimoire_capture_sequence` | frames at 4 fps by default (2 to 15 a second), with an optional operation after frame 0 |
//! | `grimoire_snip` | one rectangle of the picture, cut out and written as a crop with its metadata |
//! | `grimoire_decide` | ask a named check or typed questions about pictures and facts; a probability for every answer |
//! | `grimoire_present` | put a result in front of the person and take their Keep or Undo all |
//! | `grimoire_list_pieces` | the library's pieces, a page at a time, with their label provenance |
//! | `grimoire_list_snips` | the snips the person took, newest first, with the newest picture |
//! | `grimoire_list_rules` | the block rules a layout is generated from, as the project states them |
//! | `grimoire_run_command` | run a registered editor command by its id, as its shortcut would |
//!
//! **Sixteen tools, and the set is small on purpose.** An agent's context is spent on the tools it
//! is offered, and a catalogue of ninety-six verbs is a catalogue of ninety-six things to choose
//! badly from. **The operation tools are a discovery surface**: the first names what exists, and
//! the others preview, run and take back what it names. Later iterations add one tool per
//! operation as the registry grows, and the discovery tool goes away when it is not needed.
//!
//! # The transport
//!
//! **Stdio, and only the MCP specification.** The server speaks JSON-RPC to its client over stdin
//! and stdout, and it reaches the editor over BRP — which is the editor's own transport, and the
//! editor is a separate process that owns the world.
//!
//! **Every path is confined to the project directory.** The server takes `--project` and refuses
//! anything outside it; the editor writes the files and the server never touches the disk.

use std::path::PathBuf;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ServerCapabilities, ServerInfo};
use rmcp::service::ServiceExt;
use rmcp::{ErrorData, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use bevy_cauldron_mcp::{brp, docs, sequence};

use brp::Brp;

/// **The parameters of `grimoire_list_operations`** (#141).
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListParams {
    /// **Words to search for**, such as `carve` or `note`. With them, only the operations whose
    /// name or parameters match, best first, and `total`, the number of operations there are.
    /// Without them, every operation.
    #[serde(default)]
    pub query: Option<String>,
}

/// **The parameters of `grimoire_preview_operation` and `grimoire_apply_operation`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationParams {
    /// **The operation's name**, as `grimoire_list_operations` gives it. For example
    /// `map_set_settings`.
    pub operation: String,
    /// **The operation's own parameters**, as that operation's schema describes them. For example
    /// `{"bounds": [64.0, 8.0, 64.0]}`.
    #[serde(default)]
    pub parameters: Value,
}

/// **The parameters of `grimoire_apply_operation`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyParams {
    /// **The operation's name**, as `grimoire_list_operations` gives it.
    pub operation: String,
    /// **The operation's own parameters**, as that operation's schema describes them.
    #[serde(default)]
    pub parameters: Value,
    /// **Whether to preview instead of applying.** `true` changes nothing: the map, the file and
    /// the log are all left alone, and the answer says what would have changed.
    #[serde(default)]
    pub dry_run: bool,
    /// **Your name as the holder of a note you claimed.** While you hold one, an operation that
    /// reaches outside the note's area plus one cell is refused, and so is one that needs a
    /// question answered first (removing a piece a person placed, changing more than 10 pieces,
    /// touching the margin): ask with note_ask, and go on once the person answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

/// **The parameters of `grimoire_undo_operation`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UndoParams {
    /// **The id of the change to take back**, as `grimoire_apply_operation` returned it or as
    /// `grimoire_read_oplog` lists it. Omitted: the newest change that has not been taken back.
    #[serde(default)]
    pub operation_id: Option<String>,
    /// **Whether to preview instead of undoing.** `true` changes nothing and says what the undo
    /// would set.
    #[serde(default)]
    pub dry_run: bool,
    /// **Take back the whole presented turn (E-084)**, newest change first, instead of one logged
    /// change.
    ///
    /// The person's Cmd+Z has always meant "the turn"; without this the agent's tool took one
    /// operation of it, so the same turn came back in different amounts depending on who asked.
    /// Refused when nothing has been presented, because a turn is a thing the agent declared.
    #[serde(default)]
    pub turn: bool,
}

/// **The parameters of `grimoire_run_command`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunCommandParams {
    /// **The command's id**, `<category>.<verb>`, such as `edit.undo` or `edit.redo`.
    pub id: String,
}

/// **The parameters of `grimoire_frame_view`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FrameParams {
    /// **What to frame.** `"map"`, `"selection"`, `{"placements": [ids]}` or `{"rect": [x0, z0, x1, z1]}`.
    pub target: FrameTargetArg,
}

/// **A frame target, as the caller writes it.** The editor judges every id and every number; this
/// type only shapes the call.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum FrameTargetArg {
    Named(NamedTarget),
    Placements(PlacementsArg),
    Rect(RectArg),
}

/// **A target with no arguments.** The whole map, or the locked placement.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum NamedTarget {
    Map,
    Selection,
}

/// `{"placements": [ids]}` — the placements to frame, as the map lists them.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlacementsArg {
    pub placements: Vec<String>,
}

/// `{"rect": [x0, z0, x1, z1]}` — a rectangle in map metres, with x1 > x0 and z1 > z0.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RectArg {
    pub rect: [f32; 4],
}

/// **The parameters of `grimoire_capture_still`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StillParams {
    /// **A name for the capture**, in snake_case. It names the file it is written under.
    pub label: String,
}

/// **The parameters of `grimoire_snip`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SnipParams {
    /// **A name for the snip**, in snake_case. It names the directory the crop is written under.
    pub label: String,
    /// **The rectangle, in window pixels: `[x0, y0, x1, y1]`**, origin at the top left, `x1` and
    /// `y1` exclusive.
    pub rect: [i64; 4],
}

/// **The parameters of `grimoire_list_snips`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListSnipsParams {
    /// **How many snips to list, newest first**, from 1 to 20. The default is 5, a screenful.
    #[serde(default = "five")]
    pub limit: usize,
}

/// **Five snips, a screenful of metadata.**
fn five() -> usize {
    5
}

/// **The parameters of `grimoire_capture_sequence`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SequenceParams {
    /// **A name for the sequence**, in snake_case. It names the directory the frames land in.
    pub label: String,
    /// **How long the sequence runs, in seconds.** Between a moment and ten.
    #[serde(default = "two_seconds")]
    pub seconds: f32,
    /// **How many frames a second.** Four by default; a whole rate from two to fifteen. A higher rate costs frame time while recording.
    #[serde(default = "four_fps")]
    pub fps: u32,
    /// **The operation to apply, once frame 0 is in.** Without it the sequence is the settle
    /// alone, which is what a reviewer wants when they are checking that nothing moved.
    #[serde(default)]
    pub operation: Option<SequenceOperation>,
}

/// **The operation a sequence applies.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SequenceOperation {
    /// **The operation's name**, as `grimoire_list_operations` gives it.
    pub name: String,
    /// **Its parameters.**
    #[serde(default)]
    pub parameters: Value,
}

/// **Two seconds, which is one contact sheet at four frames a second.**
fn two_seconds() -> f32 {
    2.0
}

/// **Four frames a second, the brief's default.**
fn four_fps() -> u32 {
    4
}

/// **The parameters of `grimoire_read_oplog`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LogParams {
    /// **Read from this sequence number on.** `0` is the beginning of the log.
    #[serde(default)]
    pub since_seq: u64,
    /// **How many entries at most**, 1 to 200. The default is 50, which is a screenful.
    #[serde(default = "fifty")]
    pub limit: usize,
}

/// **The parameters of `grimoire_state`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StateParams {
    /// **Where the pinned list starts**, counted in map order. `0` is the first pin. When there are
    /// more than 50 pins, the answer's `pinned_next` names the value to pass for the next page.
    #[serde(default)]
    pub pinned_from: usize,
    /// **A sent note's id, to read everything needed to work it** (the answer's `notes.bundle`): its
    /// text, its area and margin, the pieces near it, the kit's palette, the camera the person had,
    /// how to get a picture of the area, and the earlier notes and how they turned out. Claim the note
    /// first with `note_claim`; `notes.sent` lists what can be claimed.
    #[serde(default)]
    pub note: Option<String>,
}

/// **The parameters of `grimoire_present`.** A result, handed back for review.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PresentParams {
    /// What the result is called. Required: a presentation without a title is a mystery.
    pub title: String,
    /// One paragraph saying what was made.
    pub summary: String,
    /// **What the person asked for**, quoted back to them.
    #[serde(default)]
    pub specified: Vec<String>,
    /// **What you decided on your own.** The brief asks for this distinction by name: a person who
    /// cannot see it cannot correct the right half of the result.
    #[serde(default)]
    pub inferred: Vec<String>,
    /// **Up to three places worth checking.** More than three is not a short list; the editor
    /// refuses a fourth.
    #[serde(default)]
    pub check: Vec<CheckItemParam>,
    /// **The operations this covers**, as `grimoire_apply_operation` returned them. Undo all walks
    /// them newest first, so an id the log does not hold is refused.
    #[serde(default)]
    pub operation_ids: Vec<String>,
}

/// **One place worth a look**, as the agent states it.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckItemParam {
    /// What to look at, in your own words.
    pub what: String,
    /// Placement ids to frame in the viewport first, if any.
    #[serde(default)]
    pub placements: Vec<String>,
}

/// **The parameters of `grimoire_list_pieces`.** Every filter is optional and they combine.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PiecesParams {
    /// **Where the page starts**, counted in the filtered list. `0` is the first piece. When there
    /// are more than 50 pieces, the answer's `pieces_next` names the value to pass for the next page.
    #[serde(default)]
    pub pieces_from: usize,
    /// **`true` leaves out the pieces that name no mesh** (the flat footprint markers such as roads
    /// and fields), so the list is only the pieces that are drawn with a model.
    #[serde(default)]
    pub with_mesh: bool,
    /// **`true` keeps only the pieces no one has labelled**: no label record, or a record that
    /// labels none of its axes.
    #[serde(default)]
    pub unlabelled: bool,
    /// **`true` keeps only the pieces with an axis a model or an agent wrote and no person has confirmed.** Also spelled `unconfirmed`: it matches agent-written axes too, not only a model's.
    #[serde(default, alias = "unconfirmed")]
    pub by_model: bool,
    /// **Keeps only the pieces whose label record says this confidence**: `high`, `medium` or `low`.
    /// Any other word is refused.
    #[serde(default)]
    pub confidence: Option<String>,
    /// **Keeps only the pieces whose id contains this text.**
    #[serde(default)]
    pub id_contains: Option<String>,
}

/// **Fifty entries, which is a screenful.**
fn fifty() -> usize {
    50
}

/// **The server.**
#[derive(Clone)]
pub struct Grimoire {
    /// **The editor's BRP endpoint.**
    editor: Brp,
    /// **The project directory, and the only place the server may point at.**
    project: PathBuf,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Grimoire {
    /// **A server for the editor at `editor`, rooted at `project`.**
    pub fn new(editor: Brp, project: PathBuf) -> Self {
        let tool_router = Self::tool_router();
        Grimoire {
            editor,
            project,
            tool_router,
        }
    }

    /// **The project's root, for a tool that names a file.**
    pub fn project(&self) -> &std::path::Path {
        &self.project
    }

    /// **Grab a picture of the editor, as base64 PNG**, through `bevy_fairy/screenshot` and
    /// `bevy_fairy/capture` (`Brp::screenshot`), from the surface the editor draws into.
    async fn grab(&self) -> Result<String, String> {
        self.editor
            .screenshot(json!({"source": "offscreen"}))
            .await
            .map_err(|e| format!("the editor's {e}"))
    }

    /// **Where a capture lands.**
    ///
    /// **Under the project, in a directory the brief calls working files.** The brief says raw
    /// captures go under `target/loop/<date>-<sha>/`, and **the project directory is where the
    /// editor's own data lives** — so a capture is written beside the project, under `target`,
    /// and never into the project's own data.
    ///
    /// **The label is checked first, because it is joined into the path.** A label with `..`, a
    /// leading `/`, or an empty part would put the picture outside `captures`, or replace the
    /// base path with an absolute one. A refused label is answered before the editor is asked.
    fn capture_dir(&self, label: &str) -> Result<std::path::PathBuf, String> {
        if !is_capture_label(label) {
            return Err(format!(
                "`{label}` is not a capture label. Use letters, digits, `_` and `-`, with `/` \
                 between the parts; no empty part, `.` or `..`."
            ));
        }
        Ok(self
            .project
            .join("target")
            .join("loop")
            .join("captures")
            .join(label))
    }

    /// **A name for a capture, unique enough that two do not collide.**
    fn stamp(&self) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("{now}")
    }

    /// **Is the editor on the project this server was told about?**
    ///
    /// **The one thing the two processes have to agree on**, and the server cannot know it by
    /// itself: the editor is the one that knows which project it has open. **So the server asks
    /// the editor for its project root and compares**, and a mismatch is a refusal with a sentence
    /// rather than a change to somebody else's map.
    pub async fn check_project(&self) -> Result<(), String> {
        let told = self
            .project
            .canonicalize()
            .unwrap_or_else(|_| self.project.clone());
        let answer = self.editor.call("grimoire/project_root", json!({})).await?;
        let editor_root = answer
            .get("project_root")
            .and_then(|v| v.as_str())
            .ok_or("the editor did not say which project it has open.")?;
        let editor_root = std::path::PathBuf::from(editor_root);
        let editor_root = editor_root.canonicalize().unwrap_or(editor_root);
        if editor_root != told {
            return Err(format!(
                "the editor has `{}` open and this server was told `{}`. Point both at the same \
                 project, or the changes would land in the wrong one.",
                editor_root.display(),
                told.display()
            ));
        }
        Ok(())
    }

    /// **`grimoire_list_operations` — every operation, its schema, and how it is undone.**
    ///
    /// **A read.** It asks the running editor, so the list is the editor's own and cannot be out
    /// of date.
    #[tool(
        name = "grimoire_list_operations",
        description = "Every operation the running bevy_grimoire editor knows, with each one's \
                       parameters' JSON schema and how it is undone. Call this before \
                       grimoire_apply_operation so the parameters are the ones the operation \
                       actually takes. Returns the list; it does not change anything. The undo \
                       column names the strategy an operation declares; grimoire_undo_operation \
                       performs it. Pass query (words such as \"carve\" or \"note\") to get only \
                       the operations whose name or parameters match, best first, and total, the \
                       number of operations there are.",
        annotations(
            title = "List the editor's operations",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_operations(
        &self,
        Parameters(p): Parameters<ListParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.editor.call("grimoire/operation_list", json!({"query": p.query})).await {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`grimoire_preview_operation` — what an operation would change, and nothing changed.**
    ///
    /// **A read.** The editor's `plan` runs and the answer comes back; the map, the file and the
    /// log are untouched.
    #[tool(
        name = "grimoire_preview_operation",
        description = "What an operation would change, and nothing changed: the map, the data file \
                       and the operation log are all left alone. Use this to check a proposal \
                       before applying it. Returns what would change, or the refusal.",
        annotations(
            title = "Preview an editor operation",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn preview_operation(
        &self,
        Parameters(p): Parameters<OperationParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .editor
            .call(
                "grimoire/operation_preview",
                json!({"operation": p.operation, "parameters": p.parameters}),
            )
            .await
        {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`grimoire_apply_operation` — run one operation.**
    ///
    /// **Not a read.** It changes the map, writes the data file, and appends a line to the
    /// operation log. **`dry_run` previews instead**, and nothing is written.
    #[tool(
        name = "grimoire_apply_operation",
        description = "Run one of the editor's operations. Changes the editor (most operations \
                       change the open map and write its file; brush_arm only arms a piece) and \
                       appends a line to the operation log. Set dry_run to true to see \
                       what would change without changing it. Returns the operation's id, which \
                       can be cited in a report to the developer, and the log's own record of it. \
                       When the operation changes a document and a map is open, the answer's \
                       checks field names the edge faults and overlapping placements the change \
                       introduced (new, at most 20 of each) with new_total, and each kind's total \
                       on the whole map; it is null otherwise. A rules_set whose weights name a \
                       prefix no piece in the library starts answers tunes_nothing (in \
                       would_change on a preview, and in the summary on an apply), naming the \
                       prefix and the pieces it does not tune: the change is applied, because \
                       removing a weight list is how an undo restores one, so read that field \
                       rather than assuming a weight did something. \
                       The operation replaces the values it names rather than adding to them, \
                       so it is marked destructive. Take a change back with \
                       grimoire_undo_operation and the returned id.",
        annotations(
            title = "Apply an editor operation",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn apply_operation(
        &self,
        Parameters(p): Parameters<ApplyParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **The project is checked before anything is changed.** A change to the wrong project is
        // worse than no change, and the server cannot know which project the editor has open
        // except by asking.
        if let Err(why) = self.check_project().await {
            return Ok(refusal(&why));
        }
        match self
            .editor
            .call(
                "grimoire/operation_apply",
                json!({
                    "operation": p.operation,
                    "parameters": p.parameters,
                    "dry_run": p.dry_run,
                    "agent": p.agent,
                }),
            )
            .await
        {
            Ok(v) => {
                // **A refusal is an error, and the tool says so.** The editor answers a refusal
                // with `outcome: "refused"` and a reason, and the agent must be able to tell that
                // apart from a success without reading prose.
                let refused = v.get("outcome").and_then(|o| o.as_str()) == Some("refused");
                if refused {
                    Ok(CallToolResult::structured_error(v))
                } else {
                    Ok(CallToolResult::structured(v))
                }
            }
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`grimoire_state` — what the editor is holding right now.**
    ///
    /// **A read, and the answer to "what does the editor look like?"** — the question an agent
    /// asks before it changes anything, and the question it asks afterwards to check that the
    /// change landed.
    ///
    /// **It is a summary, not a dump.** The editor holds hundreds of descriptors and thousands of
    /// placements; **none of that is what an agent needs to know.** What it needs is the *shape* of
    /// the current state — which map, how big, how many pieces, what is armed, what is selected,
    /// which door and tab are open, and what the editor last said.
    #[tool(
        name = "grimoire_state",
        description = "What the editor is holding right now: the open map and its bounds, how many \
                       pieces it has, what is armed, what is selected, which door and tab are open, \
                       and what the editor last said and what has gone wrong. The editor section also \
                       names the next undo and redo (undo_next, redo_next) and how many are stacked \
                       (undo_depth, redo_depth). The map section names up to 50 pinned placements \
                       by id and reason (pinned). pinned_total is the full count; past 50, pass \
                       pinned_from (from pinned_next) to read the rest. With --live, game is the running \
                       game's verdict on the last reload: null (none), \"pending\" (it has not \
                       answered the latest write) or {ok, message, at_unix_ms, seq, kinds}; kinds has \
                       one entry for every file the game read, {kind, file, outcome (applied or \
                       refused), field, reason}, so a refusal names its file and field. reloads is the \
                       editor's own verdict on the files it draws from (terrain, water_field, volume, \
                       forest, trees, notes, family, rigs): {ok, seq, kinds} with the same entries, where \
                       an outcome is applied, refused (the file did not read: it names the field, and \
                       the editor keeps what it had) or needs_restart (the file reads but the editor \
                       holds that kind from when it started, as it does rigs.ron, so the edit is on \
                       disk and not in use); ok is false while any file is not applied, and seq counts \
                       decisions, one per edit. reload_files is what became of the latest edit of \
                       every file of the open project, as a running game says it: {now_unix_ms, game \
                       {connected, watching (this editor runs with --live, the only way it reads a \
                       game's verdict), at_unix_ms, seq}, files}, one entry per file {kind, file, state, \
                       field, reason, at_unix_ms (when the game decided), modified_unix_ms (when the \
                       file was written), editor (the editor's own verdict on it, or null)}; state is \
                       no_game_connected (no game verdict has reached the editor, so nothing answered), pending \
                       (written after the game's last decision, so its answer is about earlier text), \
                       game_applied, game_refused (with field and reason), needs_restart or not_read \
                       (the game's last decision did not read the file). frame is the editor's own \
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
        annotations(
            title = "Read the editor's state",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn state(
        &self,
        Parameters(p): Parameters<StateParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .editor
            .call("grimoire/state", json!({"pinned_from": p.pinned_from, "note": p.note}))
            .await
        {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`grimoire_present` — hand the result back for review (S1.7).**
    ///
    /// **The brief's fourth step, in one call.** A tool that only reports leaves the person to work
    /// out what changed; this puts the result in front of them as a Keep / Undo all question, with
    /// what they asked for beside what was assumed, and the operations it covers.
    ///
    /// **It returns at once.** An MCP call that blocked on a person would deadlock with the editor's
    /// own frame, so the answer is read later from `grimoire_state`'s `presentation.answer`.
    #[tool(
        name = "grimoire_present",
        description = "Put a result in front of the person: a title, a one-paragraph summary, what \
                       they specified, what you inferred, up to three places worth checking, and the \
                       operation ids it covers. The editor shows it as a question: Keep, or Undo all \
                       (which undoes every listed operation, newest first). This call returns at \
                       once; read the person's answer from grimoire_state's presentation.answer \
                       (\"kept\", \"undone\", or why the undo stopped). It does not change the map by \
                       itself.",
        annotations(
            title = "Present a result for review",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn present(
        &self,
        Parameters(p): Parameters<PresentParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **The first place to check is framed before the question goes up.** Reusing
        // `grimoire/frame_view` rather than adding a second framing mechanism: the beat is "look here",
        // and the editor already has one way to look somewhere.
        if let Some(first) = p.check.first()
            && !first.placements.is_empty()
        {
            let _ = self
                .editor
                .call(
                    "grimoire/frame_view",
                    json!({"target": {"placements": first.placements}}),
                )
                .await;
        }
        let body = json!({
            "title": p.title,
            "summary": p.summary,
            "specified": p.specified,
            "inferred": p.inferred,
            "check": p.check.iter().map(|c| json!({"what": c.what, "placements": c.placements})).collect::<Vec<_>>(),
            "operation_ids": p.operation_ids,
        });
        match self.editor.call("grimoire/present", body).await {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`grimoire_list_pieces` — the library's pieces, a page at a time.**
    ///
    /// **A read.** `grimoire_state` names the first page of ids; this is the list itself, with the
    /// mesh each piece is drawn with and its kinds, effects and looks, so an agent can choose a piece
    /// by what it is and what it looks like, not by its id alone.
    #[tool(
        name = "grimoire_list_pieces",
        description = "The library's pieces, a page at a time. Each row carries the piece's id, the \
                       mesh it is drawn with (null for a flat footprint marker), its kind, effects \
                       and looks, and its labels: who labelled each axis (model, agent or human), the \
                       record's model, date and confidence, which report what the record says, not whether anyone has checked it; by_model finds labels no human has confirmed. Filters combine: with_mesh, unlabelled, \
                       by_model, confidence (high, medium or low), id_contains. total is the true \
                       count after the filters. A page holds 50 rows; past 50, pass pieces_from \
                       (from pieces_next) to read the rest, and how_to_narrow names the filters \
                       when a filtered page is still capped. Refuses when no project is open, \
                       rather than answering an empty list. Returns that page as JSON, and changes \
                       nothing.",
        annotations(
            title = "List the library's pieces",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_pieces(
        &self,
        Parameters(p): Parameters<PiecesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .editor
            .call(
                "grimoire/pieces",
                json!({
                    "pieces_from": p.pieces_from,
                    "with_mesh": p.with_mesh,
                    "unlabelled": p.unlabelled,
                    "by_model": p.by_model,
                    "confidence": p.confidence,
                    "id_contains": p.id_contains,
                }),
            )
            .await
        {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`grimoire_capture_still` — a picture of the editor as it is now.**
    ///
    /// **A read of the screen, and a write of a file.** The picture comes from the editor's own
    /// surface — the world *and* the interface, because the editor draws both into one image —
    /// and the state beside it is the same summary `grimoire_state` returns, so a picture is never
    /// evidence on its own.
    ///
    /// **The file is written here and not by the editor.** The editor's own capture path writes
    /// one file in the working directory and overwrites it; **a tool that overwrote the last
    /// capture would make two runs indistinguishable.**
    #[tool(
        name = "grimoire_capture_still",
        description = "A picture of the editor as it is now -- the world and the interface, because \
                       the editor draws both into one image -- plus a state summary of the same \
                       moment. The picture is returned as an image and written to a file whose path \
                       is in the answer. The answer also carries a fingerprint of the picture's \
                       bytes: two stills with the same fingerprint are the same picture, so compare \
                       fingerprints rather than file paths to tell whether the screen changed. Use \
                       this after a change to see what it did, or before one to see what it was \
                       like.",
        annotations(
            title = "Capture a still of the editor",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn capture_still(
        &self,
        Parameters(p): Parameters<StillParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **The label first**: a refused label is answered before the editor is asked anything.
        let dir = match self.capture_dir(&p.label) {
            Ok(dir) => dir,
            Err(e) => return Ok(refusal(&e)),
        };
        // **The state first**, so the picture and the state are the same moment as near as the
        // editor can make them.
        let state = self
            .editor
            .call("grimoire/state", json!({}))
            .await
            .unwrap_or(Value::Null);
        let png = match self.grab().await {
            Ok(png) => png,
            Err(e) => return Ok(refusal(&e)),
        };
        // **The file, written here.** A directory per label, and a name that says when it was
        // taken, so two captures never collide.
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Ok(refusal(&format!("{}: {e}", dir.display())));
        }
        let path = dir.join(format!("still-{}.png", self.stamp()));
        let bytes = match base64_decode(&png) {
            Some(b) => b,
            None => return Ok(refusal("the editor's picture was not base64.")),
        };
        if let Err(e) = std::fs::write(&path, &bytes) {
            return Ok(refusal(&format!("{}: {e}", path.display())));
        }
        let answer = json!({
            "path": path.to_string_lossy(),
            "bytes": bytes.len(),
            // **What the picture IS, so two stills can be compared without opening them (E-062).**
            //
            // A caller that compares paths, or timestamps, or file sizes cannot tell "the screen did
            // not change" from "the file did not change" — and on 2026-10-24 that cost this loop a
            // whole iteration: two captures came back with identical MD5, which was read as a stale
            // screen and was in fact a repeated file. A fingerprint of the bytes is the fact the
            // question actually needs.
            "fingerprint": bevy_cauldron_mcp::fingerprint(&bytes),
            "state": state,
        });
        Ok(CallToolResult::success(vec![
            rmcp::model::ContentBlock::image(png, "image/png"),
            rmcp::model::ContentBlock::text(
                serde_json::to_string_pretty(&answer).unwrap_or_default(),
            ),
        ]))
    }

    /// **`grimoire_snip` -- one rectangle of the picture, and nothing else.**
    ///
    /// **The same grab as a still**, so a crop and a still taken at the same moment are the same
    /// pixels. **The rectangle is checked before anything is written**, so a refusal leaves no file
    /// behind.
    #[tool(
        name = "grimoire_snip",
        description = "A rectangle of the editor's picture, cut out and returned as an image. rect \
                       is [x0, y0, x1, y1] in window pixels, origin at the top left, x1 and y1 \
                       exclusive, so the same pixels a still is in. The crop is written to a file \
                       whose path is in the answer, with the state of the same moment. It writes \
                       one crop file under target/loop and changes no map data: it is not an \
                       operation, not logged and not undoable. Refuses, and writes nothing, on a \
                       rectangle that is empty, reversed, negative or outside the picture, and says \
                       the picture's size. Use it to look closer at one part of a still from \
                       grimoire_capture_still.",
        annotations(
            title = "Snip one rectangle of the editor",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn snip(
        &self,
        Parameters(p): Parameters<SnipParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **The label first**: a refused label is answered before the editor is asked anything.
        let dir = match self.capture_dir(&p.label) {
            Ok(dir) => dir,
            Err(e) => return Ok(refusal(&e)),
        };
        let state = self
            .editor
            .call("grimoire/state", json!({}))
            .await
            .unwrap_or(Value::Null);
        let png = match self.grab().await {
            Ok(png) => png,
            Err(e) => return Ok(refusal(&e)),
        };
        let bytes = match base64_decode(&png) {
            Some(b) => b,
            None => return Ok(refusal("the editor's picture was not base64.")),
        };
        let crop = match bevy_grimoire_snip::crop_png(&bytes, p.rect) {
            Ok(crop) => crop,
            Err(e) => return Ok(refusal(&e)),
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Ok(refusal(&format!("{}: {e}", dir.display())));
        }
        let path = dir.join(format!("snip-{}.png", self.stamp()));
        if let Err(e) = std::fs::write(&path, &crop.png) {
            return Ok(refusal(&format!("{}: {e}", path.display())));
        }
        let answer = json!({
            "path": path.to_string_lossy(),
            "bytes": crop.png.len(),
            "rect": p.rect,
            "size": crop.size,
            "picture": crop.picture,
            "state": state,
        });
        let encoded = base64_encode(&crop.png);
        Ok(CallToolResult::success(vec![
            rmcp::model::ContentBlock::image(encoded, "image/png"),
            rmcp::model::ContentBlock::text(
                serde_json::to_string_pretty(&answer).unwrap_or_default(),
            ),
        ]))
    }

    /// **`grimoire_decide` -- ask a decision model, and get a probability back.**
    ///
    /// **Advisory and authoring-time**: it changes no map data, and a finding never blocks or undoes
    /// anything. The endpoint is one per run, from `GRIMOIRE_DECIDE_ENDPOINT`, `_MODEL` and `_KEY`.
    #[tool(
        name = "grimoire_decide",
        description = "Ask a decision model a typed question about the editor's pictures and facts, and get a \
                       probability for every allowed answer, with nothing to parse. Two forms. check: a \
                       named check such as floating, clipping, outside_area, blank_frame or toast_visible \
                       (the editor's own, with the project's assets/grimoire/checks.ron over them, read on every \
                       call); it answers a finding with the check's P(yes), its threshold and whether it was \
                       found, and takes the editor's current picture itself when it needs view. questions: \
                       ad-hoc typed questions by id, each noul (yes or no, P(yes)), choice (named options, \
                       each one's probability) or score (levels, a weighted score), with state in words, \
                       images (project files, at most four) and facts (JSON). A check that needs a picture or \
                       facts it was not given is refused and names what is missing; it never answers from \
                       less. game: also look at the running game that play started: a frame from its \
                       bevy_fairy, named game, on the port in play.ron or game.port, and one BRP call's \
                       answer (game.state) as facts.game; a check that asks for the image game takes it \
                       itself, and a game's own checks live in the project's assets/grimoire/checks.ron. \
                       Advisory: it changes no map data and a finding never blocks or undoes an \
                       operation. The endpoint is one per run, local by default; a Cloudflare endpoint sends \
                       pictures off the LAN and is opt-in.",
        annotations(
            title = "Ask a decision model",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn decide(
        &self,
        Parameters(p): Parameters<bevy_cauldron_mcp::decide::DecideParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let checks = match bevy_cauldron_mcp::decide::load_checks(&self.project) {
            Ok(c) => c,
            Err(e) => return Ok(refusal(&e)),
        };
        let view = if bevy_cauldron_mcp::decide::needs_view(&p, &checks) {
            match self.grab().await {
                Ok(png) => Some(png),
                Err(e) => return Ok(refusal(&e)),
            }
        } else {
            None
        };
        let game = if bevy_cauldron_mcp::decide::needs_game(&p, &checks) {
            let asked = p.game.clone().unwrap_or_default();
            match bevy_cauldron_mcp::decide::fetch_game(&self.project, &asked).await {
                Ok(g) => Some(g),
                Err(e) => return Ok(refusal(&e)),
            }
        } else {
            None
        };
        let project = self.project.clone();
        let answer = tokio::task::spawn_blocking(move || {
            let http = bevy_grimoire_decide::Http::new(bevy_grimoire_decide::Config::from_env());
            bevy_cauldron_mcp::decide::decide_with(&p, &project, view.as_deref(), game.as_ref(), &http)
        })
        .await
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        Ok(match answer {
            Ok(v) => CallToolResult::structured(v),
            Err(e) => refusal(&e),
        })
    }

    /// **`grimoire_list_snips` -- the snips the person took, newest first.**
    ///
    /// **A read of files the editor wrote.** It needs no editor running and changes nothing: the
    /// records are the editor's own, read back in the shape the editor wrote them.
    #[tool(
        name = "grimoire_list_snips",
        description = "The snips the person took in the editor, newest first: each one's name, when \
                       it was taken, the map and the active tool, the note the person typed, the \
                       rectangle in window and picture pixels, the camera view, the cursor's world \
                       position and the placements inside the rectangle. Each entry names its \
                       picture file, and the newest picture comes back as an image. It reads files \
                       the editor wrote, so it needs no editor running and changes nothing. Returns \
                       how many snips there are, how many are shown, the snips still being saved \
                       (pending: confirmed in the editor, picture not yet written, so not yet in \
                       the snips list) and each .json file in the snips folder that is not a snip \
                       record or has no picture file, with the reason.",
        annotations(
            title = "List the snips the person took",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_snips(
        &self,
        Parameters(p): Parameters<ListSnipsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if !(1..=20).contains(&p.limit) {
            return Ok(refusal(&format!(
                "limit is {}, and it must be from 1 to 20.",
                p.limit
            )));
        }
        let dir = match self.capture_dir("snips") {
            Ok(dir) => dir,
            Err(e) => return Ok(refusal(&e)),
        };
        let (records, mut not_snips) = bevy_grimoire_snip::read_snips(&dir);
        let (open, pending_problems) = bevy_grimoire_snip::read_pending(&dir);
        not_snips.extend(pending_problems);
        // **A snip that already has its picture is finished, not pending.** Between the picture's
        // write and the pending record's removal, one stem is in both lists.
        let open: Vec<_> = open
            .into_iter()
            .filter(|r| !records.iter().any(|done| done.stem == r.stem))
            .collect();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let pending_total = open.len();
        let pending: Vec<Value> = open
            .iter()
            .take(p.limit)
            .map(|r| {
                let age_ms = now.saturating_sub(r.taken_at_ms);
                let status = if age_ms > bevy_grimoire_snip::PENDING_ABANDONED_AFTER_MS {
                    format!(
                        "abandoned: no picture after {} s, so it is not in snips. The editor quit or the capture was lost; take the snip again.",
                        age_ms / 1000
                    )
                } else {
                    "being saved: the picture has not landed yet, so it is not in snips".to_string()
                };
                let mut entry = serde_json::to_value(r).unwrap_or(Value::Null);
                if let Value::Object(map) = &mut entry {
                    map.insert("status".to_string(), json!(status));
                    map.insert("age_ms".to_string(), json!(age_ms));
                }
                entry
            })
            .collect();
        let mut content = Vec::new();
        if let Some(newest) = records.first() {
            let path = dir.join(format!("{}.png", newest.stem));
            match std::fs::read(&path) {
                Ok(bytes) => content.push(rmcp::model::ContentBlock::image(
                    base64_encode(&bytes),
                    "image/png",
                )),
                // **A picture that cannot be read is named, and the list still answers.**
                Err(e) => not_snips.push(format!(
                    "{}: the picture could not be read: {e}",
                    path.display()
                )),
            }
        }
        let shown: Vec<Value> = records
            .iter()
            .take(p.limit)
            .map(|r| {
                let mut entry = serde_json::to_value(r).unwrap_or(Value::Null);
                if let Value::Object(map) = &mut entry {
                    map.insert(
                        "picture_path".to_string(),
                        json!(dir.join(format!("{}.png", r.stem)).to_string_lossy()),
                    );
                }
                entry
            })
            .collect();
        let answer = json!({
            "dir": dir.to_string_lossy(),
            "total": records.len(),
            "shown": shown.len(),
            "snips": shown,
            "pending": pending,
            "pending_total": pending_total,
            "not_snips": not_snips,
        });
        content.push(rmcp::model::ContentBlock::text(
            serde_json::to_string_pretty(&answer).unwrap_or_default(),
        ));
        Ok(CallToolResult::success(content))
    }

    /// **`grimoire_list_rules` -- the rules a layout is generated from, as the project states them.**
    ///
    /// **A read of a project file.** `rules.ron` sits beside `kits.ron` under `assets/grimoire`, and
    /// the editor's own `Project::open` reads it there; this reads the same file, with the same
    /// parser and the same library-free validation ([`bevy_grimoire_core::rules::Rules::validate_numbers`]),
    /// so it needs no editor running and changes nothing.
    ///
    /// **It does not check descriptor ids against the library**, because the server has no library:
    /// `Project::open` loads the kits and then calls `validate_against`, and this tool cannot. So a
    /// block naming a piece the project does not have is answered here and refused by the editor, and
    /// the description says so rather than implying the two agree in every case. The loop's Inspect step asks an agent to say how
    /// varied a generated layout is and which constraint shaped it, and neither question can be
    /// answered by an agent that cannot read what the rules say.
    #[tool(
        name = "grimoire_list_rules",
        description = "The block rules the project states for generating layouts, read from \
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
        annotations(
            title = "List the block rules a layout is generated from",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_rules(&self) -> Result<CallToolResult, ErrorData> {
        // **The file the editor's own `Project::open` reads**, computed the same way, so the two
        // cannot disagree about where rules live.
        let path = self
            .project
            .join("assets")
            .join("grimoire")
            .join(bevy_grimoire_core::rules::FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            // **An absent file is no rules, not a failure.** `Project::open` treats it the same way:
            // a project without rules behaves as it always did, and an agent that asks must not get
            // a refusal that reads like a broken project. **A file that is present and blank is a
            // different case** and falls through to the parser below, which refuses it — the editor
            // refuses to open such a project, so answering "no rules" would call it clean.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let answer = json!({
                    "path": path.to_string_lossy(),
                    "version": bevy_grimoire_core::rules::RULES_VERSION,
                    "blocks": [],
                });
                return Ok(CallToolResult::success(vec![
                    rmcp::model::ContentBlock::text(
                        serde_json::to_string_pretty(&answer).unwrap_or_default(),
                    ),
                ]));
            }
            Err(e) => {
                return Ok(refusal(&format!(
                    "{} could not be read: {e}",
                    path.display()
                )));
            }
        };
        // **A blank file is refused, not read as "no rules".** `Project::open` parses whatever is
        // there, so a blank `rules.ron` fails the editor's open — and a reader that answered "no
        // blocks" would tell an agent the project is clean when the editor cannot start on it.
        let rules = match bevy_grimoire_core::rules::Rules::parse(&text) {
            Ok(rules) => rules,
            // **Refused by name, with the parser's reason.** An agent told "no rules" when the file
            // is broken would generate against nothing and report a result that had nothing to do
            // with what the person wrote.
            Err(e) => return Ok(refusal(&format!("{}: {e}", path.display()))),
        };
        // **And every number the editor would refuse it for.** `Rules::parse` is `ron::from_str`
        // and nothing else, so a file can parse and still be one `Project::open` rejects: a version
        // this program does not read, `attempts: 0`, a `cell` that is not a usable step, a footprint
        // size under 2x2, or a yaw that is not 0/180. Answering those as valid would hand an agent
        // numbers the editor will not generate from.
        if let Err(e) = rules.validate_numbers() {
            return Ok(refusal(&format!("{}: {e}", path.display())));
        }
        let answer = json!({
            "path": path.to_string_lossy(),
            "version": rules.version,
            "blocks": serde_json::to_value(&rules.blocks).unwrap_or(Value::Null),
        });
        Ok(CallToolResult::success(vec![
            rmcp::model::ContentBlock::text(
                serde_json::to_string_pretty(&answer).unwrap_or_default(),
            ),
        ]))
    }

    /// **`grimoire_capture_sequence` — a sequence of frames, with an action in the middle.**
    ///
    /// **The editor takes the frames, and that is the point.** The brief's rule is that frame 0 is
    /// the state *before* the action, and only the editor can promise that: the picture is of a
    /// rendered frame, so an action applied before frame 0 was read back would be *in* frame 0.
    /// **So the editor times the frames and applies the operation after frame 0**, and the delay is
    /// in the answer.
    #[tool(
        name = "grimoire_capture_sequence",
        description = "A sequence of frames of the editor, with an optional operation applied \
                       once the first frame is in -- so frame 0 is the state before the action and \
                       the later frames are what it did. The frames are written to a directory and \
                       their paths are in the answer. The editor takes the frames, so the timing \
                       does not depend on this server's round trips. The rate is two to fifteen frames a second. A frame is asked for only once the one before it is written, so a stalled \
                       editor delays the frames after it rather than losing them; a frame that does not land within five seconds ends the run with a reason, and the status is done only when every frame is written. The call waits for the run to end, for at most the sequence's length plus five seconds a frame and a margin.",
        annotations(
            title = "Capture a sequence of the editor",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn capture_sequence(
        &self,
        Parameters(p): Parameters<SequenceParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let dir = match self.capture_dir(&p.label) {
            Ok(dir) => dir,
            Err(e) => return Ok(refusal(&e)),
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Ok(refusal(&format!("{}: {e}", dir.display())));
        }
        let operation = p
            .operation
            .as_ref()
            .map(|o| json!({"name": o.name, "parameters": o.parameters}));
        let started = match self
            .editor
            .call(
                "grimoire/capture_sequence_start",
                json!({
                    "label": p.label,
                    "out_dir": dir.to_string_lossy(),
                    "seconds": p.seconds,
                    "fps": p.fps,
                    "operation": operation,
                }),
            )
            .await
        {
            Ok(v) => v,
            Err(e) => return Ok(refusal(&e)),
        };
        let id = started.get("id").and_then(|i| i.as_u64()).unwrap_or(0);
        // **Poll for the end.** The editor answers when the frames are in; the wait covers every
        // frame, not only the sequence's length.
        let expected_frames = (p.seconds * p.fps as f32) as u32;
        let deadline = std::time::Instant::now() + sequence_wait(p.seconds, expected_frames);
        let status = match sequence::wait_for_sequence(&self.editor, id, deadline).await {
            Ok(v) => v,
            Err(e) => return Ok(refusal(&e)),
        };
        // **The frames are read back and returned as images**, so the caller sees the sequence
        // without opening the files.
        let mut content = vec![];
        if let Some(frames) = status.get("frames").and_then(|f| f.as_array()) {
            for frame in frames {
                let Some(path) = frame.get("path").and_then(|p| p.as_str()) else {
                    continue;
                };
                let Ok(bytes) = std::fs::read(path) else {
                    continue;
                };
                content.push(rmcp::model::ContentBlock::image(
                    base64_encode(&bytes),
                    "image/png",
                ));
            }
        }
        content.push(rmcp::model::ContentBlock::text(
            serde_json::to_string_pretty(&status).unwrap_or_default(),
        ));
        Ok(CallToolResult::success(content))
    }

    /// **`grimoire_undo_operation` — take one change back, by its id.**
    ///
    /// **The undo is an operation like any other.** The editor looks the change up in the
    /// operation log, dispatches the inverse it recorded through the same registry the change went
    /// through, and appends a new line saying which line it undoes. **Nothing is deleted from the
    /// log.** A change that a newer change has built on is refused, with the newer change named,
    /// because putting the old values back would overwrite it.
    #[tool(
        name = "grimoire_undo_operation",
        description = "Take back one change made through grimoire_apply_operation. Pass \
                       operation_id, the id that call returned; omit it to take back the newest \
                       change that has not been taken back. The editor applies the inverse the change recorded, writes \
                       the data file, and appends a new log line naming the line it undoes; the \
                       log is never edited. Refuses, and changes nothing, when the change was \
                       already undone, when a newer change exists (the answer names it; undo that \
                       one first), when the map was changed outside the log since (a hand-edited \
                       file, the menu, or another map open; the answer names what differs), or \
                       when the change recorded no inverse. Undoing an undo puts the change back, \
                       and the change's own id takes it back again. Set dry_run to true to see what the undo would set. \
                       Set turn to true to take back the whole result you last presented with \
                       grimoire_present, newest change first, in one call — the same amount one Cmd+Z \
                       takes back — and the answer says how many changes it took with took_back. \
                       turn refuses when nothing has been presented, or when the presentation named \
                       no operations. \
                       Returns the undo's own operation id and what it set.",
        annotations(
            title = "Undo an editor operation",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn undo_operation(
        &self,
        Parameters(p): Parameters<UndoParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **The project is checked first**, for the reason `apply_operation` gives: an undo
        // against the wrong project would put the wrong file back.
        if let Err(why) = self.check_project().await {
            return Ok(refusal(&why));
        }
        match self
            .editor
            .call(
                "grimoire/operation_undo",
                // **`turn` travels too (E-084).** The body is built by hand, so a field the
                // editor's params grew is a field this line has to name — measured live: `turn:
                // true` reached the editor as "unknown field `turn`", was refused, and the answer
                // read like a one-change undo.
                json!({"operation_id": p.operation_id, "dry_run": p.dry_run, "turn": p.turn}),
            )
            .await
        {
            Ok(v) => {
                let refused = v.get("outcome").and_then(|o| o.as_str()) == Some("refused");
                if refused {
                    Ok(CallToolResult::structured_error(v))
                } else {
                    Ok(CallToolResult::structured(v))
                }
            }
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`grimoire_read_oplog` — the operation log.**
    ///
    /// **A read.** Every operation writes one line, from a person or an agent, and an undo is a
    /// new line rather than a deletion — so the log is the whole history.
    #[tool(
        name = "grimoire_read_oplog",
        description = "The editor's operation log, newest last. Every operation appends one line, \
                       whether a person or an agent did it. Read it to see what has been done to \
                       this project. A refusal is not logged, so a line here is a line about a \
                       change that happened. An undo is a new line naming the line it undoes.",
        annotations(
            title = "Read the operation log",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn read_oplog(
        &self,
        Parameters(p): Parameters<LogParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .editor
            .call(
                "grimoire/oplog_read",
                json!({"since_seq": p.since_seq, "limit": p.limit}),
            )
            .await
        {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`grimoire_frame_view` — frame the Map camera on a box.**
    ///
    /// **A camera move and nothing else.** The editor resolves the target, refuses it atomically
    /// if any name is wrong, and otherwise sets the view; the map, the operation log and the
    /// undo stack are untouched.
    #[tool(
        name = "grimoire_frame_view",
        description = "Frame the editor's Map camera on a box. target: \"map\" (the whole map), \
                       \"selection\" (the locked placement; refused when nothing is locked), \
                       {\"placements\": [ids]} (their union), or {\"rect\": [x0, z0, x1, z1]} (map \
                       metres). It only moves the camera: it never changes map data, is not logged \
                       or undoable, and is not an operation. Refuses, and moves nothing, on an \
                       unknown id, an empty selection or a flat rectangle. Returns what was framed, \
                       the focus and the view height in metres. \
                       Call grimoire_capture_still afterwards to see the result.",
        annotations(
            title = "Frame the editor's view",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn frame_view(
        &self,
        Parameters(p): Parameters<FrameParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .editor
            .call("grimoire/frame_view", json!({"target": p.target}))
            .await
        {
            Ok(v) => {
                let refused = v.get("outcome").and_then(|o| o.as_str()) == Some("refused");
                if refused {
                    Ok(CallToolResult::structured_error(v))
                } else {
                    Ok(CallToolResult::structured(v))
                }
            }
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`grimoire_run_command` — run one of the editor's registered commands by id** (UI-90, #256).
    ///
    /// **The same command a key, a menu or the palette runs.** The editor presses the command's
    /// chord and its one dispatcher answers, so `edit.undo` undoes as Cmd+Z does, and a command the
    /// key would refuse in this state is refused here with the editor's own reason.
    #[tool(
        name = "grimoire_run_command",
        description = "Run one of the editor's registered commands by id, exactly as its keyboard \
                       shortcut would: `edit.undo` takes back what Cmd+Z takes back, `edit.redo` \
                       puts it back. An id is `<category>.<verb>`. Refuses, and changes nothing, \
                       when the command is not enabled now (the answer says why: the tab, what is \
                       in hand, or a text field holding the keys), when the launcher is open, or \
                       when the id is unknown (the answer names the nearest ids as did_you_mean). \
                       `ran` means the shortcut was queued; the change lands within a frame or two, \
                       so read grimoire_state for the result. grimoire_state.ui says which screen, \
                       workspace and panels are open and which region has the keys. A command \
                       that is also an operation (a placement, a resize) is better done with \
                       grimoire_apply_operation, which names its effect and logs it.",
        annotations(
            title = "Run an editor command",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn run_command(
        &self,
        Parameters(p): Parameters<RunCommandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **The project is checked first**: a shortcut pressed in the wrong project's editor would
        // change the wrong map.
        if let Err(why) = self.check_project().await {
            return Ok(refusal(&why));
        }
        match self.editor.call("grimoire/run_command", json!({"id": p.id})).await {
            Ok(v) => {
                let refused = v.get("outcome").and_then(|o| o.as_str()) == Some("refused");
                if refused {
                    Ok(CallToolResult::structured_error(v))
                } else {
                    Ok(CallToolResult::structured(v))
                }
            }
            Err(e) => Ok(refusal(&e)),
        }
    }
}

/// **A refusal, as a tool result an agent can read.**
///
/// **`isError: true`, and the reason in both places** — the text content and the structured
/// content. An agent reads one and a human reads the other, and both need the sentence.
fn refusal(why: &str) -> CallToolResult {
    CallToolResult::structured_error(json!({"refused": why}))
}

/// **Is this a capture label that can be a path under `captures`?** Letters, digits, `_` and `-`
/// in parts, with `/` between them. No part is empty, so there is no leading or trailing `/` and
/// no `//`; no part is `.` or `..`, because `.` is not a character a label may use. A `\` or an
/// absolute path fails for the same reason.
fn is_capture_label(label: &str) -> bool {
    !label.is_empty()
        && label.split('/').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
}

#[tool_handler]
impl rmcp::ServerHandler for Grimoire {
    /// **A tool that does not exist is refused with the list of the ones that do.**
    ///
    /// **The generated handler answers "tool not found"**, which tells an agent that it guessed
    /// wrong and nothing else — and an agent that has to guess again is an agent that guesses
    /// again. The brief's rule is *"Invalid calls are rejected atomically, with a message saying
    /// what was wrong and what to change."* **What to change is a list of names.**
    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, rmcp::ErrorData> {
        let name = request.name.as_ref();
        if self.tool_router.get(name).is_none() {
            let all = self.tool_router.list_all();
            let names: Vec<&str> = all.iter().map(|t| t.name.as_ref()).collect();
            let result = CallToolResult::structured_error(json!({
                "refused": format!(
                    "`{name}` is not a tool this server has. The tools are: {}.",
                    names.join(", ")
                ),
            }));
            return Ok(result.into());
        }
        // **And the router, for a tool that does exist.** The `#[tool_handler]` macro generates
        // this call when the impl block does not carry its own — and this impl block does, so
        // the router is called here.
        let tcc = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
        self.tool_router.call(tcc).await
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "This server drives the bevy_grimoire world editor. Call grimoire_list_operations first to \
             learn the operations and their parameters, then grimoire_preview_operation to check a \
             change, and grimoire_apply_operation to make it. Every applied operation is recorded in \
             the editor's operation log.",
        )
    }
}

/// **Serve on stdio, until the client goes away.**
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut project: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--project" => project = args.next().map(PathBuf::from),
            "--emit-docs" => {
                // **The document is written to stdout, and the shell redirects it.** The server
                // never writes a file in the project — it does not own the project directory —
                // so the one write it can do is to a file the caller names.
                print!("{}", docs::render());
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "bevy_cauldron_mcp — the bevy_grimoire editor over the Model Context Protocol.\n\n\
                     USAGE:\n    bevy_cauldron_mcp [--project <dir>]\n\n\
                     --project <dir>   the project directory the editor is working on. The server \
                     never writes to it itself; the editor does. Defaults to the current \
                     directory.\n\n\
                     The editor's BRP endpoint comes from BEVY_BRP_PORT (default 15702), which is \
                     the same variable the editor reads."
                );
                return Ok(());
            }
            other => {
                eprintln!("bevy_cauldron_mcp: `{other}` is not an argument this server takes.");
                eprintln!("Try `--help`.");
                std::process::exit(2);
            }
        }
    }
    let project = project.unwrap_or_else(|| PathBuf::from("."));
    let server = Grimoire::new(Brp::from_env(), project);
    // **The project is checked when a tool is called, not when the server starts.**
    //
    // **A server that refuses to start is a server an agent cannot ask anything.** The editor may
    // be closed when the server starts and opened later — or the server may be started first on
    // purpose — and a server that exits because nothing is listening has told the agent
    // nothing except that the editor was not up. **So the check is per call**, and a call with
    // no editor answers with a sentence.
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

/// **Decode base64 into bytes.**
///
/// **Only the standard alphabet, and only the padded form** — which is what the editor's
/// capture path produces and what `base64::engine` writes.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(text).ok()
}

/// **Encode bytes into base64.**
fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// **The landing timeout, per frame.** A copy of `CAPTURE_TIMEOUT_S` in `bevy_cauldron`'s
/// `sequencer.rs`: the editor fails a run whose frame does not land within it.
const LANDING_TIMEOUT_S: f32 = 5.0;

/// **How long the caller waits for a sequence to finish.** The editor asks for at most one new frame
/// per update, up to four in flight, and fails the run on a frame that misses the landing timeout. A
/// run therefore takes at most one landing wait per frame, which still bounds it with frames overlapping.
/// The sequence's own length and a margin for the last readback are added on top.
fn sequence_wait(seconds: f32, expected_frames: u32) -> std::time::Duration {
    std::time::Duration::from_secs_f32(seconds + 10.0 + expected_frames as f32 * LANDING_TIMEOUT_S)
}

#[cfg(test)]
mod pieces_params_tests;

#[cfg(test)]
mod capture_label_tests {
    use super::is_capture_label;

    #[test]
    fn a_label_under_the_captures_directory_is_accepted() {
        for label in ["phase0-base", "town/house_cottage_plaster", "snip_check"] {
            assert!(
                is_capture_label(label),
                "`{label}` should be a capture label"
            );
        }
    }

    #[test]
    fn a_label_that_could_leave_the_directory_is_refused() {
        for label in [
            "",
            "../escape",
            "a/../b",
            "/tmp/escape",
            "a//b",
            "a/",
            "bad.label",
            "a\\b",
        ] {
            assert!(!is_capture_label(label), "`{label}` should be refused");
        }
    }
}

#[cfg(test)]
mod sequence_wait_tests {
    use super::sequence_wait;
    use std::time::Duration;

    /// **A slow run must not outlast its own wait.** Ten seconds at fifteen frames a second is
    /// 150 frames, and each may take up to a landing timeout of five seconds. The wait used to be the
    /// sequence's length plus ten seconds, 20 s here: a run still going at that point came back as
    /// "running" with no pictures, and the editor refused a new sequence until the run ended.
    #[test]
    fn the_wait_covers_every_frame_of_the_longest_run() {
        let wait = sequence_wait(10.0, 150);
        assert!(
            wait >= Duration::from_secs(150 * 5),
            "a 150-frame run can take a five-second landing per frame, but the caller waits only {wait:?}"
        );
        assert!(
            sequence_wait(10.0, 150) > sequence_wait(10.0, 40),
            "the wait must grow with the frames it has to cover"
        );
    }
}
