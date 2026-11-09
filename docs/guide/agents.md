# Working with an agent

An AI agent can look at your map, change it, and undo its own changes, through the running editor. You describe the problem, the agent looks and acts, and you check the result on screen. Every change the agent makes goes into the same operation log as your own changes, and you can undo it with Cmd+Z in the editor.

This page uses a few words you may not know yet. A **door** is one of the editor's three workspaces: Map, kit, and Rigs. A **kit** is a folder of meshes plus its `library.ron`. A **piece** is a library entry. A **placement** is a piece placed on a map. A **stamp** is a composition placed on a map. A **composition** is a saved group of pieces. The **bash** is the named combination of kits a map offers, or none for every kit it binds.

## Set up the agent

Goal: connect an MCP client to the running editor, so the agent can read and change the open project.

1. Start the editor on your project folder. For the repo's own project, run `cargo run -p bevy_emerge_mapper -- .` from the repo root. The editor serves the Bevy Remote Protocol (BRP) on port 15702 by default. Set `BEVY_BRP_PORT` before launching if that port is taken.
2. Build the MCP server with `cargo build -p bevy_emerge_mcp`. The binary is `target/debug/bevy_emerge_mcp`.
3. Add the server to your client's config. Use absolute paths. Point `--project` at the project folder, the same folder you passed to the editor:

```json
{
  "mcpServers": {
    "bevy_emerge": {
      "command": "/absolute/path/to/bevy_emerge/target/debug/bevy_emerge_mcp",
      "args": ["--project", "/absolute/path/to/your/project"],
      "env": { "BEVY_BRP_PORT": "15702" }
    }
  }
}
```

Leave out the `env` block if you did not change the port. Set the same value on the editor if you did.

What you see: ask the agent to call `emerge_state`. It answers with the open map, the door you are on, and the undo depth. If the editor is not running, the server answers with a sentence saying so, and changes nothing.

If it goes wrong: a refusal that says no editor answered means the editor is not running, or it is on another port. Check that both sides use the same `BEVY_BRP_PORT`.

## A worked session

Goal: remove a piece that blocks a gate, and undo the removal if you change your mind.

Say to the agent what you see. For example: "The abbey blocks the gate. Take it out." The agent then does the following.

1. It calls `emerge_state` to read the open map and the placements. The answer gives the abbey's id.
2. It calls `emerge_capture_still` to see the gate before the change.
3. It calls `emerge_preview_operation` with `operation: "placement_remove"` and `parameters: {"id": "<the abbey's id>"}`. The answer says what would change. Nothing changes yet.
4. It calls `emerge_apply_operation` with the same operation and parameters. The answer carries an `operation_id`.
5. It calls `emerge_capture_still` again. The gate is clear.
6. You press Cmd+Z in the editor. The abbey comes back. `emerge_read_oplog` shows the undo as a new line.

To turn a placement instead of removing it, use `placement_set_yaw` with `id` and `yaw`. Yaw wraps to 0 to 360, so `-15` is `345`.

![The whole editor window: the agent turns the abbey with placement_set_yaw and removes its table with placement_remove; the inspector's LAST line and history list name each change as the agent's; two Cmd+Z presses take both back](../media/agent-drives.gif)

The inspector on the left shows the same thing you would read in the tool answers: LAST names the newest change, UNDO names what Cmd+Z takes back next, and the history list marks the agent's changes with `agent`.

![An agent removes the abbey with placement_remove, and the person's Cmd+Z puts it back.](../media/agent-removes-then-undo.gif)

What you see: after step 4 the abbey is gone from the map and the gate is open. After your Cmd+Z the abbey stands on the gate again, and the operation log has one new undo line.

If it goes wrong: `placement_remove` takes away any piece standing on the placement too, so check the still in step 2 before you apply.

## How an edit travels

An agent never writes a file itself. The server passes each call to the editor, and the editor does the work.

1. The agent lists the operations with `emerge_list_operations`. The names are the same as in the table below.
2. The agent previews one with `emerge_preview_operation`. A preview refuses bad input and says what would change, and it writes nothing.
3. The agent applies it with `emerge_apply_operation`. The editor changes the open map or the library in memory.
4. The editor writes the file it changed under `assets/emerge/`, and appends one line to `assets/emerge/oplog.jsonl`. The editor watches the project files, so the open editor shows the new state.
5. To take a change back, the agent calls `emerge_undo_operation`. The undo is a new log line.

Each apply answers with an `operation_id`, which names its line in the log.

## Operations

Goal: know what an agent can change, and what undoes each change.

| Operation | What it does | Undo |
|---|---|---|
| `map_set_settings` | Sets a map's origin, bounds, axis, bash, or note. `null` clears the bash or the note. | Puts the old settings back |
| `placement_set_pin` | Pins or unpins a placement. | Puts the old pin back |
| `placement_set_yaw` | Turns a placement to a new yaw in degrees, wrapped to 0 to 360. | Puts the old yaw back |
| `placement_remove` | Removes a placement, and any piece standing on it. | Puts the pieces back |
| `stamp_remove` | Removes a stamp from the map. | Puts the stamp back |
| `map_rename` | Renames the open map. The name is forced to snake_case, so `Town Two` becomes `town_two`. | Puts the old name back |
| `brush_arm` | Arms a library piece as the brush. `null` puts the brush down. | None: it changes the view, not the map |
| `door_open` | Opens the kit, map, or rigs door, and optionally one of its tabs. | None: it changes the view, not the map |
| `piece_select` | Selects a library piece on the Meshes shelf. | None: it changes the view, not the map |
| `label_set` | Sets one label axis (kind, effects, look, or surfaces) on a piece. | Puts the old labels back |

`map_set_settings` refuses `bounds` and `axis` in the same call.

`door_open` can wait. If you leave a map that has unsaved edits, the answer comes back with `waiting` set to `"the map has unsaved changes"`. The editor then asks you about the unsaved changes, and the agent cannot answer that question for you.

## Seeing what the agent sees

Goal: give the agent a picture of the editor, and read back what the person saw.

`emerge_capture_still` returns one picture of the whole window, world and interface together, plus a summary of the state at that moment. The picture is also written to a file, and the answer gives its path and a **fingerprint** of the picture's bytes. Use it before and after a change. **Compare fingerprints, not paths**: two stills with the same fingerprint are the same picture, so a fingerprint that did not move says the screen did not change. Reading the raw `bevy_debugger/screenshot` method instead is a trap: it answers with an id in about twenty milliseconds and writes the file a frame or more later — measured, about 200 ms — so a read at that moment finds no file at all, or the previous capture if the path was reused. This tool polls `bevy_debugger/capture` until the frame is done and writes the file itself, which is why it is the one to use.

`emerge_capture_sequence` takes a series of frames. Its `label` names the run, `seconds` sets the length (default 2, up to 10), and `fps` sets the rate (default 4, from 2 to 15). With an `operation`, the editor applies it after frame 0. Frame 0 is the state before the change, and the later frames show what it did. The run is done only when every frame is written. A frame that does not land within five seconds ends the run with a reason.

`emerge_frame_view` moves the Map camera without changing any map data. Its `target` is `"map"`, `"selection"`, `{"placements": [ids]}`, or `{"rect": [x0, z0, x1, z1]}` in map metres. It is not logged and not undoable. Call `emerge_capture_still` after it to see the result.

A snip is a region you mark for the agent. Press Cmd+\ in the editor, drag a rectangle, type a one-line note, and press Enter to save. Press Escape to cancel. The editor writes `snip-<ms>.png` and `snip-<ms>.json` under `target/loop/captures/snips/`. Those files stay out of `assets/emerge/`, so a game never loads them.

A snip that is confirmed but not yet written is pending. The editor clears pending snips once, when a project is first opened in the session. `emerge_list_snips` reads the snips you took. It takes a `limit` from 1 to 20, default 5. It changes nothing. A snip is not logged and not undoable.

`emerge_snip` is a different tool. The agent gives it a `label` and a `rect` in window pixels, and it writes one crop of the editor's picture. The crop is not logged and not undoable.

![The person snips a box over a wall and writes a note; the agent reads the snip and turns the wall; the person's Cmd+Z takes the turn back.](../media/hero.gif)

The state read has its own paging. `emerge_state` takes `pinned_from` to page the list of pins, and returns `pinned_next` when more follow. It also reports `undo_next`, `undo_depth`, `redo_next`, `redo_depth`, `leaving`, and `door_waiting`.

## Who did what

Goal: find out who changed a map, and take back only the change you want.

Every log line records whether a person or an agent made the change. The log is `assets/emerge/oplog.jsonl`. Read it with `emerge_read_oplog`, which lists lines newest last. A refusal is not logged, so every line is a change that happened.

`emerge_list_pieces` lists the library a page at a time, 50 pieces per page. Pass `pieces_from` to read the next page. Filters combine. `with_mesh` keeps pieces that have a mesh. `unlabelled` keeps pieces with no label. `by_model` keeps pieces with a label that a model or an agent set and no person has changed since. `confidence` takes `high`, `medium`, or `low`, and refuses anything else. `id_contains` matches part of an id. Without an open project, the call refuses. It does not answer with an empty list.

`emerge_undo_operation` takes an optional `operation_id`. Without one, it takes back the newest change still in effect. With one, it takes back that change. Pass `dry_run` to check the undo without making it.

An undo refuses, and says why, in these cases:

- A newer change to the same map is still in effect. The refusal names that change, so you can undo it first.
- The state no longer matches what the change left. Someone changed the map after it, and putting the old values back would overwrite that work.
- The change was logged before undo existed, so it has nothing to put back.
- The operation id is not in the log. `emerge_read_oplog` lists the ids.

A removal with `put_back` is the undo of another removal. An agent cannot apply it directly. Take the removal back with `emerge_undo_operation`, or with Cmd+Z in the editor.

Cmd+Z in the editor undoes the newest change on the door you are on. The keys page lists which doors keep a history.

## Limits

- The editor must be running. The server answers with a sentence when it cannot reach the editor, and changes nothing.
- An editor built without the `debugger` feature has no BRP endpoint, so the server cannot reach it. The feature is on by default.
- The agent can turn, pin, remove, rename, label, and set settings. It cannot place a new piece, stamp a composition, or fill an area. The registry has no operation for those. Place them yourself in the editor, as the building maps page describes.
- The server makes no change itself. Every change to the map goes through the editor's operations above.
- `emerge_snip` and `emerge_frame_view` change no map data. They are not logged, and they cannot be undone.
- `emerge_capture_sequence` waits for the run to end: at most the sequence's length, plus ten seconds, plus five seconds for each frame.
- A still shows one moment on screen. An agent reads only what is in the picture, so frame the area with `emerge_frame_view` before you ask for a still.

For the full schema of every tool, parameter, and annotation, see [docs/mcp-tools.md](../mcp-tools.md). For the keys the editor uses, see [keys](keys.md). For labels and the kit door, see [kits and labels](kits-and-labels.md). For map work, see [building maps](building-maps.md). For the files in a project, see [data files](data-files.md).
