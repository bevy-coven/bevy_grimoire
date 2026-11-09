**Read this before you call anything else.** The tool descriptions say what one tool does; this says what order they come in and what the person on the other end expects. Cauldron is a world editor a person is looking at while you work — every change you make is on their screen before they read your answer.

## Orient

**Start with the state, not with a change.** `grimoire_state` is the whole picture: the open map and its bounds, how many pieces it has, what is armed, what is selected, which door and tab are open, the undo and redo stacks, and what has gone wrong. Read it first and often; it is cheap and it never changes anything.

**Then read what the person already told you about this project.** `grimoire_project_feedback_list` returns the reports they filed about this project from the editor itself (Cmd+Shift+D): each one's note, the interface nodes and map points they pointed at, and a picture of the screen at that moment. If they filed a report saying the wall tool draws over the road, that is the ground truth of what bothers them — read `grimoire_feedback_list` for the reports about Cauldron itself, and `grimoire_list_snips` for the pictures they cut out to show you something.

**Before you change a map, learn what it is made of.** `grimoire_list_pieces` is the library, a page at a time with each piece's labels; `grimoire_list_rules` is the grammar a generated layout comes from. `grimoire_read_oplog` says what has already been done to this project, by anyone, and whether it was taken back.

## Change

**Four tools, in this order.** `grimoire_list_operations` says what operations exist and what parameters each takes — call it first, because a parameter name you guessed is a refused operation. `grimoire_preview_operation` runs the operation's own check and returns what would change while changing nothing. `grimoire_apply_operation` makes the change and records it in the operation log, returning the id you cite later. `grimoire_undo_operation` takes one change back by that id, and `dry_run` on either of the middle two previews without writing.

**Your own default is `dry_run: true` for a change to the map's shape.** A `map_set_settings`, a `map_create`, a `fill` or a `layout_accept` moves ground the person is standing on, so preview it before you apply it and say in your summary that you did. The tool's own `dry_run` defaults to false; it is your habit, not the tool's, that has to be safe.

**An operation that replaces values is destructive, not additive.** `grimoire_apply_operation` overwrites the fields it names, so an undo is the only way back. Take one when a proposal is uncertain: the log line is cheap and the person's map is not.

**An operation refused is an operation that changed nothing**, and the refusal names the field and the reason. Read it, fix the parameter, call again — do not reach for `grimoire_run_command` to do the same thing by a different door. That tool runs a registered editor command by its id, exactly as its keyboard shortcut would, and it is for what has no operation.

## When the person says wait

**View ▸ Pause agent writes refuses every write with one sentence.** The answer reads: the agent's writes are paused, so `<operation>` changed nothing; a person turned them off with View > Pause agent writes; turn that off to let the agent write again; previews still work. That is not an error to work around and not a reason to retry: a person asked for it. **Read it and wait**, keep planning with `grimoire_preview_operation` and `grimoire_state`, and tell them in one line what you would do when they unpause.

## Show your work

**Let the editor show the change; do not frame it yourself.** Apply the operation and the view goes to it: the camera eases onto where the change can be seen, the pieces it touched are ringed for a moment, and the status line names it (`the Familiar placed bed@2`). A change made on another door opens the Map first. **So do not call `grimoire_frame_view` after a change** — the editor has already done it, and a call of your own is a second camera move that fights the follow. It is still the tool for pointing at something you did not change: a piece the person asked about, a corner they cannot find.

**A still before and a still after.** `grimoire_capture_still` returns a picture of the editor as it is now — the world and the interface, because the editor draws both into one image — plus the state of the same moment and a fingerprint of the picture's bytes. Two stills with the same fingerprint are the same picture, so compare fingerprints rather than paths to tell whether the screen changed. Use `grimoire_snip` to cut one rectangle out of a still and look closer at it, and `grimoire_capture_sequence` when the point is the motion rather than the result: its first frame is taken before the operation you name and the later frames are what it did.

**When a judgement is uncertain, ask a model rather than guessing.** `grimoire_decide` answers a named check such as `floating` or `clipping`, or typed questions of your own, with a probability for every allowed answer. It is advisory: a finding never blocks or undoes an operation, and it changes no map data.

## Hand back

**Every task ends with `grimoire_present`, or with `note_resolve` for a note.** A presentation is a title, one paragraph of summary, what the person specified, what you inferred, up to three places worth checking and the operation ids it covers. The editor shows it as a question — Keep, or Undo all — and this call returns at once: read their answer from `grimoire_state`'s presentation, where `kept` and `undone` say which they chose. A task that ends without one is a task the person cannot accept in one click.

**Say what you did not do.** An operation you previewed and did not apply, a note you left for them, a file that needs their hand: put it in the summary. The editor's status line and its toast are theirs; `grimoire_read_log` is the editor's own log file, read from disk, and it answers when the editor is not running at all — so it is where you look when something failed and `grimoire_state` did not say why.

**Three tools that are not part of a task.** `grimoire_open_project` opens a project folder in the running editor, from any screen including the Projects home, and is the first call when they name a project rather than a map. `grimoire_jobs_stop` asks a background job such as the palette's thumbnail bake to stop, and `grimoire_self_test` is Cauldron's power-on self test, run once when its own terminal starts you: it reads the state, counts the operations, takes one picture and returns them.

## After a task: reflect

**Once you have handed back, look at what it cost.** After `grimoire_present`, or after `note_resolve` for a note, ask yourself one question: which single tool would most have made that easier? Then record the answer with `grimoire_wish` — exactly one wish, citing that hand-back. This is the one thing the person reading a finished result cannot see, and it is how the editor learns what it is missing.

**A wish is the hand-back's id plus four fields.** `after` names the hand-back the wish follows: `presentation` set to an id `grimoire_present` returned, such as `present-1`, or `note` set to an id you resolved, such as `n3` — exactly one of the two, because a wish citing both is a wish about two tasks. `task` is what the task was. `hardship` is what cost steps: where the work stalled, repeated itself, or needed a second call to learn what one call should have said. `tool` is the single tool, named and described in one sentence, 200 characters at most. `example_call` is a call to it, written out, 600 characters at most.

```text
{"after": {"presentation": "present-1"}, "task": "laid a wall of site/wall along the north edge", "hardship": "finding which of the 96 operations places a run of wall took three grimoire_list_operations calls and one refused grimoire_apply_operation", "tool": "grimoire_list_operations returning the operation a plain-English phrase maps to, so a placement is found by what it does rather than by its verb.", "example_call": "grimoire_list_operations {\"query\": \"place a run of wall\"}"}
```

**The hardship is the steps it cost, not a complaint.** *"Three calls to find the operation"* is something the person can act on. *"The tool list is unhelpful"* is not. Say what you did and what it would have been; the person files the wish as an issue only when it names a change someone can make.

**A wish is not a bug, and it is not a request for more permissions.** A tool that answered wrongly, or refused something it should have allowed, belongs in the hand-back's summary where the person can see it beside the result — not in a wish, which is about what is missing rather than what is broken. Asking for more permissions, a wider fence, or an editor write you were refused is also not a wish: say it in the hand-back and let the person decide.

**A wish is not a tool you already wished for in this session.** If you asked for it once, it is already queued and a second row is noise the person has to read and reject. Wish for a different tool, or make no wish.

**Wish and move on.** `grimoire_wish` returns as soon as the wish is queued — it does not wait for a person to read it, because they may be away and your turn may be the last one. Do not poll `grimoire_state` for its answer, and do not repeat the wish because nothing happened yet. **If nothing was hard, make no wish at all**: an empty wish costs the person a row to read and reject, and it teaches them that the queue is not worth reading.

## Working a note

A person leaves notes on the map (a rectangle or picked pieces, with an instruction) and presses Send. Any agent connected to this server can work a sent note; the editor holds no agent logic. If the project's `agent.ron` names a command, Send also starts it once per Send (`(version: 1, command: Some("omp"), model: "claude-sonnet-5-5")`; `args` may use `{model}`, `{prompt}`, `{project}`; the default arguments are `-p --model {model} {prompt}`). With no `agent.ron`, or no `command` in it, Send only queues the notes, and a hand-started agent works them the same way.

1. **List.** `grimoire_state` has a notes section: the sent and claimed notes with `id`, `holder` and `lease_until`. Pass `note=<id>` for a note's bundle: text, area and margin, nearby pieces, the palette, the camera and earlier notes.
2. **Claim.** `note_claim` with your name as `agent` holds the note for a lease. `note_renew` keeps it while you work, `note_release` gives it back. A note another agent holds is refused.
3. **Work inside the fence.** Every `grimoire_apply_operation` you make with `agent` set is checked against your note: an operation reaching outside the note's area plus one cell is refused before anything is written. Removing a piece a person placed is refused until you have asked with `note_ask` and the person has answered with `note_answer`. Capture a before and an after still with `grimoire_capture_still`.
4. **Resolve.** `note_resolve` closes the note with a one-line `summary`, your `assumptions`, the `ops` (operation ids) you ran and `captures` `[before, after]`. The person then keeps the result or undoes the whole note in one step, which reopens it.

Worked example: the person's note `n1` covers cells 0,0 to 2,2 ("tidy the wall"), and the agent `omp` works it. Operations, one per line:

```json
{"operation": "note_add", "parameters": {"cells": [0, 0, 2, 2], "text": "tidy the wall"}}
{"operation": "note_send", "parameters": {}}
{"operation": "note_claim", "parameters": {"id": "n1", "agent": "omp"}}
{"operation": "placement_add", "parameters": {"piece": "site/wall", "at": [0.5, 0.5]}, "agent": "omp"}
{"operation": "note_resolve", "parameters": {"id": "n1", "agent": "omp", "summary": "added a wall", "captures": ["before.png", "after.png"], "assumptions": ["the wall belongs in the corner"]}}
```

The first two lines are the person's (the note box and Send); the rest are the agent's. `placement_add`'s piece and cell are the example's own: use pieces from `grimoire_list_pieces`.

