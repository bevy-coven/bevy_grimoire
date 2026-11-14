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

use bevy_grimoire_core::lesson::{LESSON_MAX_CHARS, WHY_MAX_CHARS};
use bevy_grimoire_core::wish::{WISH_FIELD_MAX_CHARS, WISH_TOOL_MAX_CHARS};
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
        name: "grimoire_guide",
        description: "How to work in Cauldron, in the order a task goes: orient, change, what to do \
                       when a person pauses the agent's writes, how to show your work, and how to \
                       hand a result back. Call this before any other tool on your first task in a \
                       project: the other tools say what one tool does, and this says which of them \
                       in what order, and what the person on the other end expects. It returns the \
                       shipped guide, then the person's own guide and the open project's, each under \
                       a heading that names its source and path. A project's guide is served only once \
                       the person has allowed that exact content (or trusted the project), so an \
                       unread one arrives as one line saying they have not allowed it, never as its \
                       text. It takes no parameters and changes nothing.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
    ToolDoc {
        name: "grimoire_present",
        description: "Put a result in front of the person: a title, a one-paragraph summary, what they \
                      specified, what you inferred, up to three places worth checking, and the operation \
                      ids it covers. The editor shows it as a question: Keep, or Undo all (which undoes \
                      every listed operation, newest first). This call returns at once; read the person's \
                      answer from grimoire_state's presentation.answer (\"kept\", \"undone\", or why the undo \
                      stopped). It does not change the map by itself. If you called grimoire_style, also \
                      cite the style lines you worked to: \
                      followed, a list of {path, line} taken from a chain grimoire_style returned in \
                      this session. The person's card lists each line's text with a link that opens it; \
                      a line from a page you were never served, past a page's end, blank, or on a page \
                      that has changed since is refused with the entry and the reason. A hand-back with \
                      no style involved cites nothing.",
        read_only: false,
        destructive: false,
        idempotent: false,
    },
    ToolDoc {
        name: "grimoire_wish",
        description: "After a grimoire_present hand-back (or a note you resolved), name the one tool that would most \
                      have helped with that task: after, the hand-back it follows ({presentation: \"present-1\"} or \
                      {note: \"n3\"}); task, what the task was; hardship, what cost steps; tool, the single tool, named \
                      and described in one sentence, inside the limit its schema states; example_call, a call to it \
                      as you would like to write it, inside its own. The editor queues the wish and the editor \
                      adds the context it already has — the hand-back's title, the operations the task applied, its version and your \
                      client's name and version — so do not restate those. It returns at once with the wish's id and \
                      \"waiting\"; read grimoire_state's wishes for what became of it. One wish per hand-back: a second \
                      one citing the same hand-back, or one citing an id this editor never handed back, is refused \
                      with the reason. An empty field, or one past its limit, is refused naming the field — a wish is \
                      never trimmed. It writes nothing outside the editor's own queue.",
        read_only: false,
        destructive: false,
        idempotent: false,
    },
    ToolDoc {
        name: "grimoire_lesson",
        description: "After a grimoire_present hand-back (or a note you resolved), propose one line \
                      for this project's agent guide: what the task taught that the next agent \
                      should know. Fields: after, the hand-back it follows ({presentation: \
                      \"present-1\"} or {note: \"n3\"}); lesson, the one line the guide would \
                      gain, inside the limit its schema states and with no newline; why, why it is \
                      worth the line, also one line, inside its own. The editor queues the proposal \
                      in the AGENT block and the person keeps or discards it; only their Keep \
                      writes the line to .dark_factory/AGENTS.md, so this call writes nothing. It \
                      returns at once with the lesson's id, the line it would add, and \"waiting\"; \
                      read grimoire_state's lessons for what became of it. One lesson per \
                      hand-back: a second citing the same hand-back, or one citing an id this \
                      editor never handed back, is refused with the reason, as is a newline, an \
                      empty field or one past its limit — a lesson is never trimmed. A project \
                      with no agent guide is refused, and the person creates one on the project \
                      page. A lesson is optional: one worth keeping beats one per task.",
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
        name: "grimoire_schema",
        description: "The JSON Schema of one kind of project file, generated from the Rust type the \
                      editor reads, so it describes exactly what the reader accepts. Its properties \
                      carry the field's doc comment as description, and a field that names another \
                      file's value carries x-dark-factory-ref (the kind it names). kind is a file \
                      kind's id: map, kits, library, vocab, compositions, rigs, rules, standards, \
                      notes, relations, terrain, volume, agent, play, destructible_marks and the \
                      rest. Returns the schema as JSON; it changes nothing. The same schema is on \
                      disk at .dark_factory/schema/<kind>.schema.json.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
    ToolDoc {
        name: "grimoire_preview_operation",
        description: "What an operation would change, and nothing changed: the map, the data file \
                      and the operation log are all left alone. Use this to check a proposal \
                      before applying it. Returns what would change and shown, where the change \
                      would be seen (the place the apply will name), or the refusal.",
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
                      marked destructive. The answer's shown field says where the change is seen — a \
                      box on the map, a library piece, a composition, a rig, a note, a project file, \
                      or nowhere — which is what the person's view follows. Take a change back with \
                      grimoire_undo_operation and the returned id.",
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
                      the presentation named no operations. Returns the undo's own operation id, \
                      what it set, and shown: where the inverse's change is seen, which for a \
                      put-back is where the piece stands again.",
        read_only: false,
        destructive: true,
        idempotent: false,
    },
    ToolDoc {
        name: "grimoire_jobs_stop",
        description: "Ask a background job to stop, the same request the status band's popover makes: \
                      the producer stops at its own next seam, so the answer is `asked` and \
                      grimoire_state.jobs shows it go. `key` is the job's own name — `thumbs` is the \
                      palette's thumbnail bake — and grimoire_state.jobs.items lists every key that \
                      is running. Refuses, and stops nothing, on a key that is not running, naming \
                      what is. Nothing is lost when a bake is stopped: importing a piece, or editing \
                      the library at all, asks for the new pieces' portraits again.",
        read_only: false,
        destructive: false,
        idempotent: true,
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
                       refused), field, reason}, so a refusal names its file and field. reloads is the \
                       editor's own verdict on the files it draws from (terrain, water_field, volume, \
                       forest, trees, notes, family, rigs): {ok, seq, kinds} with the same entries, where \
                       an outcome is applied, refused (the file did not read: it names the field, and \
                       the editor keeps what it had) or needs_restart (the file reads but the editor \
                       holds that kind from when it started, as it does rigs.yaml, so the edit is on \
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
                       kinds come from the project's repetition.yaml, \
                       re-read on every call, and a bad file is named in error. \
                       relations checks the <map>.relations.yaml beside the open map, re-read on \
                       every call: each relation (of, kind) with outcome met, unmet or missing, \
                       measured (metres, or degrees off for Faces), by (the piece that breaks it) \
                       and missing_id; unmet and missing count them; error names a file that does not parse. \
                       standards checks the project's standards.yaml (beside kits.yaml), re-read on \
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
                      like. Pass source \"window\" to capture what the window itself shows (its \
                      swapchain) instead of what the editor drew; on macOS an unfocused window \
                      reads as one flat colour.",
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
                      (the editor's own, with the project's assets/grimoire/checks.yaml over them, read on every \
                      call); it answers a finding with the check's P(yes), its threshold and whether it was \
                      found, and takes the editor's current picture itself when it needs view. questions: \
                      ad-hoc typed questions by id, each noul (yes or no, P(yes)), choice (named options, \
                      each one's probability) or score (levels, a weighted score), with state in words, \
                      images (project files, at most four) and facts (JSON). A check that needs a picture or \
                      facts it was not given is refused and names what is missing; it never answers from \
                      less. game: also look at the running game that play started: a frame from its \
                      bevy_fairy, named game, on the port in play.yaml or game.port, and one BRP call's \
                      answer (game.state) as facts.game; a check that asks for the image game takes it \
                      itself, and a game's own checks live in the project's assets/grimoire/checks.yaml. \
                      Advisory: it changes no map data and a finding never blocks or undoes an \
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
        name: "grimoire_feedback_list",
        description: "The reports the person filed about Cauldron itself, newest first: each \
                      one's name, when it was taken, which screen and which project were up, \
                      the window's shape, each thing that was pointed at (a node of the \
                      interface with its chain of roles and text, a dragged area, the whole \
                      screen, or a point in the map with its cell and the piece there), the \
                      relation between every two of them worked out from their rectangles \
                      (contains, overlaps, touches or apart, the overlap in square pixels, and \
                      which is drawn in front), the note the person typed, and the editor's log \
                      tail. Each entry names its whole picture and the same picture with every \
                      pick outlined and every overlap washed, and the newest picture comes back as \
                      an image. It reads files the editor wrote, so it needs no editor running and \
                      changes nothing. Returns how many reports there are, how many are shown, and \
                      each .json file in the folder that is not a report or has no picture, with \
                      the reason.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
    ToolDoc {
        name: "grimoire_project_feedback_list",
        description: "The reports the person filed about this project, newest first: each one's \
                      name, when it was taken, which screen and which project were up, the \
                      window's shape, each thing that was pointed at (a node of the interface \
                      with its chain of roles and text, a dragged area, the whole screen, or a \
                      point in the map with its cell and the piece there), the relation between \
                      every two of them worked out from their rectangles (contains, overlaps, \
                      touches or apart, the overlap in square pixels, and which is drawn in \
                      front), the note the person typed, and the editor's log tail. The files are \
                      in the project's target/feedback. Each entry names its whole picture and \
                      the same picture with every pick outlined and every overlap washed, and the \
                      newest picture comes back as an image. It reads files the editor wrote, so \
                      it needs no editor running and changes nothing. Returns how many reports \
                      there are, how many are shown, and each .json file in the folder that is not \
                      a report or has no picture, with the reason.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
    ToolDoc {
        name: "grimoire_style",
        description: "The style of one place, as an agent should read it before shaping that place: the \
                      game's style files first, then the region's, the level's and the area's that holds \
                      `at`, so the most specific place comes last. Each place gives its style.md and, \
                      when `topic` is asked for, its file for that topic; each file arrives under a \
                      heading naming its path. With no `map` you get the game's files alone. A style \
                      file is served only once the person has allowed that exact content (or trusted \
                      the project): one that has not been allowed is withheld and arrives as one line \
                      saying it is waiting for them, never as its text. The pictures the served text \
                      cites come back as images, each after a line with its path and its caption, at \
                      most 8 of at most 1 MiB each; one left out is named with its path, its caption \
                      and the reason. `map` must be a map of the project, `at` ([x, z] in map metres) \
                      must be inside it, and `topic` must be one the game has: anything else is refused \
                      with the reason. It changes nothing.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
    ToolDoc {
        name: "grimoire_style_propose",
        description: "Propose a change to a style page you may not write yourself: the game's \
                      (design/style.md and the game's topic files) or a region's. Fields: path, the \
                      page relative to the project root; edits, small exact edits in order, each \
                      {find, replace} with find exactly once in the page as the edit before it left \
                      it; why, one sentence on why the page should say it. The editor queues the \
                      proposal in the AGENT block, shown as the change to the page, and the person \
                      keeps or discards it: only their Keep writes, so this call writes nothing. It \
                      returns at once with the proposal's id and \"waiting\"; read grimoire_state's \
                      style_proposals for what became of it (\"kept\" or \"discarded\"). A find that \
                      is not exactly once, a page the person has not allowed (you cannot change what \
                      you have not been shown), a result over the style limits, a second proposal \
                      for a page that has one waiting, and a why past its limit are refused with the \
                      reason. A level's or an area's page is not proposed: change it with the \
                      operation style_edit.",
        read_only: false,
        destructive: false,
        idempotent: false,
    },
    ToolDoc {
        name: "grimoire_list_rules",
        description: "The block rules the project states for generating layouts, read from \
                      assets/grimoire/rules.yaml: for each block, the example map its grammar is learned from, the \
                      region descriptors that ask for a block, the cell size and block size in cells, the border \
                      piece, no_empty, the weight multipliers, the footprint rules, attempts and seed. Returns the \
                      blocks and the path of the file read. A project with no rules.yaml answers with no blocks, \
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
    ToolDoc {
        name: "grimoire_run_command",
        description: "Run one of the editor's registered commands by id, exactly as its keyboard \
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
        read_only: false,
        destructive: true,
        idempotent: false,
    },
    ToolDoc {
        name: "grimoire_read_log",
        description: "The running editor's own log, read from the file it writes on this machine: \
                      startup stages, the window self-check, warnings, errors and panics. Works even \
                      when the editor is not answering. Pass lines (default 200, at most 2000) for \
                      the newest lines.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
    ToolDoc {
        name: "grimoire_open_project",
        description: "Open a project folder in the running editor, from any screen including the \
                      Projects home, optionally on a named map. The open runs on the editor's next \
                      frame: call grimoire_state afterwards to see it.",
        read_only: false,
        destructive: false,
        idempotent: false,
    },
    ToolDoc {
        name: "grimoire_self_test",
        description: "Cauldron's power-on self test, for an agent started in Cauldron's terminal: \
                      reads the editor's state, counts its operations and takes one picture of the \
                      editor, tells the editor how each went (it shows them to the person), and \
                      returns the picture and the results. Call it once when asked to run the self \
                      test; it changes nothing.",
        read_only: true,
        destructive: false,
        idempotent: true,
    },
];

/// **The guide an agent reads before its first call** (#346, AG-01).
///
/// **One source for the prose.** It is `guide.md`, beside this file, compiled in with
/// `include_str!`: `grimoire_guide` returns this string, and [`render`] writes the same string into
/// `docs/mcp-tools.md`, so the shipped guide and the document cannot disagree — there is one text
/// and two readers, not two copies.
///
/// **Its examples stay executable.** The fenced `json` block is one operation per line; the editor's
/// `send_starts_the_notes_agent` guide test runs every line against a headless editor, so an example
/// cannot drift from the operations that exist.
pub const GUIDE: &str = include_str!("guide.md");

/// **The document, as markdown.**
///
/// **One section per tool**, with its description, its annotations and its parameters.
pub fn render() -> String {
    let mut out = String::new();
    out.push_str("# The bevy_grimoire MCP tools\n\n");
    out.push_str(
        "Generated from the server's own tool declarations. **Do not edit this file** — it is \
         written by `cargo run -p bevy_cauldron_mcp -- --emit-docs`, and a test fails when it does \
         not match the server.\n\n",
    );
    out.push_str(
        "The server speaks the Model Context Protocol over **stdio**, and reaches the running \
         editor over the Bevy Remote Protocol on `BEVY_BRP_PORT` (default 15702). It writes \
         nothing to disk itself: the editor owns the project directory, and every file the \
         editor writes is written by the editor.\n\n",
    );
    out.push_str(GUIDE);
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
    let mut document = serde_json::json!({
        "grimoire_guide": {
            "type": "object",
            "additionalProperties": false,
            "properties": {},
        },
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
                "followed": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["path", "line"],
                        "properties": {
                            "path": {"type": "string", "description": "The page, as grimoire_style headed it."},
                            "line": {"type": "integer", "minimum": 1, "description": "The line, counted from 1."},
                        },
                    },
                    "description": "The style lines the work followed, from a chain grimoire_style returned in this session. Each must be a non-empty line a page still says.",
                },
            },
        },
        "grimoire_wish": {
            "type": "object",
            "additionalProperties": false,
            "required": ["after", "task", "hardship", "tool", "example_call"],
            "properties": {
                "after": {
                    "type": "object",
                    "additionalProperties": false,
                    "description": "The hand-back the wish follows: exactly one of presentation or note.",
                    "properties": {
                        "presentation": {"type": ["string", "null"], "description": "A presentation's id, as grimoire_present returned it."},
                        "note": {"type": ["string", "null"], "description": "A note's id, as the agent resolved it."},
                    },
                },
                "task": {"type": "string", "maxLength": WISH_FIELD_MAX_CHARS, "description": "What the task was."},
                "hardship": {"type": "string", "maxLength": WISH_FIELD_MAX_CHARS, "description": "What cost steps."},
                "tool": {"type": "string", "maxLength": WISH_TOOL_MAX_CHARS, "description": "The single tool that would most have helped, named and described in one sentence."},
                "example_call": {"type": "string", "maxLength": WISH_FIELD_MAX_CHARS, "description": "A call to that tool, as you would like to write it."},
            },
        },
        "grimoire_schema": {
            "type": "object",
            "additionalProperties": false,
            "required": ["kind"],
            "properties": {
                "kind": {"type": "string", "description": "The kind of file's schema, a file kind's id: map, kits, library, vocab, compositions, rigs, rules, standards, notes, relations, terrain, volume, agent, play, destructible_marks and the rest."},
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
        "grimoire_jobs_stop": {
            "type": "object",
            "additionalProperties": false,
            "required": ["key"],
            "properties": {
                "key": {"type": "string", "description": "Which job to stop: its own name. thumbs is the palette's thumbnail bake."}
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
                "label": {"type": "string", "description": "A snake_case name for the capture."},
                "source": {"type": "string", "enum": ["surface", "window"], "default": "surface", "description": "surface: the image the editor draws into. window: what the window itself shows (its swapchain)."}
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
                "facts": {"description": "The JSON facts: the operations run, the pieces, the camera."},
                "game": {"type": "object", "additionalProperties": false, "description": "Also look at the running game: a frame from its bevy_fairy, named game, and one BRP call's answer as facts.game.", "properties": {
                    "port": {"type": "integer", "minimum": 0, "maximum": 65535, "description": "The port the game's bevy_fairy answers on. Default: play.yaml's port."},
                    "state": {"type": "object", "additionalProperties": false, "required": ["method"], "properties": {"method": {"type": "string", "description": "The BRP method, such as world.query."}, "params": {"description": "Its parameters, if it takes any."}}, "description": "One BRP call to the game, whose answer becomes facts.game."}
                }}
            }
        },
        "grimoire_list_snips": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "limit": {"type": "integer", "minimum": 1, "maximum": 20, "description": "How many snips to list, newest first. The default is 5."}
            }
        },
        "grimoire_feedback_list": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "limit": {"type": "integer", "minimum": 1, "maximum": 20, "description": "How many reports to list, newest first. The default is 5."}
            }
        },
        "grimoire_project_feedback_list": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "limit": {"type": "integer", "minimum": 1, "maximum": 20, "description": "How many reports to list, newest first. The default is 5."}
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
        "grimoire_run_command": {
            "type": "object",
            "additionalProperties": false,
            "required": ["id"],
            "properties": {
                "id": {"type": "string", "description": "The command's id, `<category>.<verb>`, such as `edit.undo`."},
            },
        },
        "grimoire_read_log": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "lines": {"type": "integer", "minimum": 1, "maximum": 2000, "default": 200, "description": "How many of the newest lines."},
            },
        },
        "grimoire_open_project": {
            "type": "object",
            "additionalProperties": false,
            "required": ["root"],
            "properties": {
                "root": {"type": "string", "description": "The project folder, a path on the editor's machine."},
                "map": {"type": ["string", "null"], "default": null, "description": "A map to open it on."},
            },
        },
        "grimoire_self_test": {
            "type": "object",
            "additionalProperties": false,
            "properties": {},
        },
    });
    // **The lesson's schema, merged here rather than inside the literal above**:
    // `serde_json::json!` expands one nesting level per value, and this table as a whole reaches the
    // crate's `recursion_limit` before any single entry does. The document is the same either way —
    // `every_tool_has_a_schema` reads the map, not the expression it came from.
    if let Some(map) = document.as_object_mut() {
        map.insert("grimoire_lesson".to_owned(), lesson_schema());
        map.insert("grimoire_style".to_owned(), style_schema());
        map.insert("grimoire_style_propose".to_owned(), style_propose_schema());
    }
    document
}

/// **The parameters of `grimoire_style_propose`** (SG-07, #406).
fn style_propose_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["path", "edits", "why"],
        "properties": {
            "path": {"type": "string", "description": "The page, relative to the project root: design/style.md, or a region's page."},
            "edits": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["find", "replace"],
                    "properties": {
                        "find": {"type": "string", "description": "The exact text to replace. It must be in the page exactly once; a span that matches twice is refused with the count, and you make it longer until it matches once."},
                        "replace": {"type": "string", "description": "What takes its place; empty takes the span out."},
                    },
                },
                "description": "Small exact edits of the page, applied in order. Not whole files: an exact span is what a person can review as a diff.",
            },
            "why": {"type": "string", "description": "One sentence on why the page should say it, with no newline."},
        },
    })
}

/// **The parameters of `grimoire_style`** (SG-02, #401), which the literal above cannot hold.
fn style_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "map": {"type": ["string", "null"], "default": null, "description": "A map of the project, by name. With none, the answer is the game's files alone."},
            "at": {"type": ["array", "null"], "items": {"type": "number"}, "minItems": 2, "maxItems": 2, "default": null, "description": "A point on that map, [x, z] in map metres; the area that holds it closes the chain. It must be inside the map, and it needs map."},
            "topic": {"type": ["string", "null"], "default": null, "description": "A topic the game states, such as dialogue: each place that has a file for it adds that file."},
        },
    })
}

/// **The parameters of `grimoire_lesson`**, which the literal above cannot hold.
fn lesson_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["after", "lesson", "why"],
        "properties": {
            "after": {
                "type": "object",
                "additionalProperties": false,
                "description": "The hand-back the lesson follows: exactly one of presentation or note.",
                "properties": {
                    "presentation": {"type": ["string", "null"], "description": "A presentation's id, as grimoire_present returned it."},
                    "note": {"type": ["string", "null"], "description": "A note's id, as the agent resolved it."},
                },
            },
            "lesson": {"type": "string", "maxLength": LESSON_MAX_CHARS, "description": "The one line the guide would gain. No newline: a lesson is one line."},
            "why": {"type": "string", "maxLength": WHY_MAX_CHARS, "description": "Why it is worth the line."},
        },
    })
}
