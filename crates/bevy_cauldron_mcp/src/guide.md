**Read this before you call anything else.** The tool descriptions say what one tool does; this says what order they come in and what the person on the other end expects. Cauldron is a world editor a person is looking at while you work — every change you make is on their screen before they read your answer.

## Orient

**Start with the state, not with a change.** `grimoire_state` is the whole picture: the open map and its bounds, how many pieces it has, what is armed, what is selected, which door and tab are open, the undo and redo stacks, and what has gone wrong. Read it first and often; it is cheap and it never changes anything.

**Then read what the person already told you about this project.** `grimoire_project_feedback_list` returns the reports they filed about this project from the editor itself (Cmd+Shift+D): each one's note, the interface nodes and map points they pointed at, and a picture of the screen at that moment. If they filed a report saying the wall tool draws over the road, that is the ground truth of what bothers them — read `grimoire_feedback_list` for the reports about Cauldron itself, and `grimoire_list_snips` for the pictures they cut out to show you something.

**Before you change a map, learn what it is made of.** `grimoire_list_pieces` is the library, a page at a time with each piece's labels; `grimoire_list_rules` is the grammar a generated layout comes from. `grimoire_read_oplog` says what has already been done to this project, by anyone, and whether it was taken back.

## Know the style before you shape a place

**Call `grimoire_style` for a place before you shape it.** The game's look and feel is written down in `design/`, one folder per place: the whole game, a region, a level (named after its map) and an area of a level. The tool returns the chain an agent reads, the game's files first and the most specific place last, each file under a heading naming its path. Pass `map` for a level, `at` (`[x, z]` in map metres) for the area that holds that point, and `topic` (`dialogue`, `sound`, `ui`) when the task is about one of them; with no `map` you get the game's files alone. The pictures the text cites come back as images, each after its caption, at most eight; one it left out is named with its path and its caption, so the words are still yours.

**A style file that says it is waiting for the person is not yours to read.** The person allows a project's style the way they allow its guide: that exact content, or the whole project, and one changed byte asks again. Until then the tool gives you one line in the file's place and never its text, so do not go round it with a shell: say in your hand-back that you worked without that file.

**Change a level's or an area's style with `style_edit`, and propose a change to the game's or a region's.** `style_edit` is an operation, applied with `grimoire_apply_operation`: `{path, edits: [{find, replace}]}` makes small edits to a page that is there, or `{path, create}` makes a new topic page. Each `find` must be in the page exactly once, so make it longer until it is; a `find` that matches twice or not at all is refused with the count and nothing is written. The edit is logged and can be undone, and the view goes to the page. It is refused when the person has not allowed the page (you cannot change what you were never shown), when the page or its chain would pass the style limits, or when it would cite a picture that is not in the project. A board is changed only by `board_set` and an area's rectangle only by `design_area_set`, so `style_edit` refuses those files.

**The game's pages and a region's are the person's to decide.** Call `grimoire_style_propose` with `path`, `edits` and `why` instead: the editor queues the change as a row in the AGENT block, shown as the change to the page, and the person's **Keep** writes it and **Discard** drops it. The call returns at once with the proposal's id; read `grimoire_state.style_proposals` for `kept` or `discarded`, and do not propose the same page again while one waits. A page that was changed after your proposal cannot be kept against, so propose again from the page as it is.

**Cite the style lines you followed.** If you called `grimoire_style`, give `grimoire_present` a `followed` list of `{path, line}`: the lines of the chain it returned that you worked to, a few and not the whole style. The person's hand-back card lists each line's text with a link that opens it on the Design page, so they can see whether the style was used and argue with a specific line. Each entry must be a non-empty line of a page `grimoire_style` served you in this session, and the page must still say it; a line from a page you were never served, past a page's end, blank, or on a page that has changed since is refused with the entry and the reason, and nothing is handed back. A hand-back with no style involved (a terrain fix, a test run) cites nothing.

**Say in your wish what the style did not tell you.** After the hand-back, `grimoire_wish` is where a question about the place that the style left unanswered belongs: it tells the person which file to write next. Name what the style left you guessing, so the lines you cited and the lines you wished for are the two halves of one answer.

## Change

**Four tools, in this order.** `grimoire_list_operations` says what operations exist and what parameters each takes — call it first, because a parameter name you guessed is a refused operation. `grimoire_preview_operation` runs the operation's own check and returns what would change while changing nothing. `grimoire_apply_operation` makes the change and records it in the operation log, returning the id you cite later. `grimoire_undo_operation` takes one change back by that id, and `dry_run` on either of the middle two previews without writing.

**Your own default is `dry_run: true` for a change to the map's shape.** A `map_set_settings`, a `map_create`, a `fill` or a `layout_accept` moves ground the person is standing on, so preview it before you apply it and say in your summary that you did. The tool's own `dry_run` defaults to false; it is your habit, not the tool's, that has to be safe.

**An operation that replaces values is destructive, not additive.** `grimoire_apply_operation` overwrites the fields it names, so an undo is the only way back. Take one when a proposal is uncertain: the log line is cheap and the person's map is not.

**An operation refused is an operation that changed nothing**, and the refusal names the field and the reason. Read it, fix the parameter, call again — do not reach for `grimoire_run_command` to do the same thing by a different door. That tool runs a registered editor command by its id, exactly as its keyboard shortcut would, and it is for what has no operation.

**Before you hand-write a project file, ask for its schema.** `grimoire_schema` returns one kind's JSON Schema (`kind` is a file kind's id: `map`, `library`, `kits` and the rest), generated from the type the reader reads, so it lists every field the file may have, each field's description, and which other file's value it names.

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

**Then propose a lesson, if the task taught one.** A wish is about what the editor lacked; a lesson is about what the *work* taught, and it goes into this project's own guide so the next agent reads it before touching anything. Call `grimoire_lesson` with `after` (the same hand-back the wish cites), `lesson` — the one line the guide would gain, 280 characters at most and **with no newline, because a lesson is one line** — and `why`, also one line, under the same limit.

```text
{"after": {"presentation": "present-1"}, "lesson": "Measure the walls before placing a door: a door placed blind overlapped one.", "why": "the overlap was found by rendering, three calls after the placement"}
```

**Only the person writes the guide.** Your call queues the line as a row in the AGENT block; **Keep** appends it to `.dark_factory/AGENTS.md` under `## Lessons`, and **Discard** drops it. `grimoire_lesson` returns at once with the line it would add, and `grimoire_state.lessons` reports `waiting`, `kept` or `discarded` — do not poll for the answer. **A project with no guide is refused**, and the person creates one on the project page; say so in your hand-back rather than retrying.

**One lesson per hand-back, and a lesson is optional.** A second lesson citing the same hand-back is refused, as is a newline or a line past its limit — nothing is trimmed. If the task taught nothing the next agent needs, propose none: a filler line in someone's guide is worse than a gap.

**Wish and move on.** `grimoire_wish` returns as soon as the wish is queued — it does not wait for a person to read it, because they may be away and your turn may be the last one. Do not poll `grimoire_state` for its answer, and do not repeat the wish because nothing happened yet. **If nothing was hard, make no wish at all**: an empty wish costs the person a row to read and reject, and it teaches them that the queue is not worth reading.

## Working a note

A person leaves notes on the map (a rectangle or picked pieces, with an instruction) and presses Send. Any agent connected to this server can work a sent note; the editor holds no agent logic. If the project's `agent.yaml` names a command, Send also starts it once per Send (`version: 1`, `command: omp`, `model: claude-sonnet-5-5`, one field a line; `args` may use `{model}`, `{prompt}`, `{project}`; the default arguments are `-p --model {model} {prompt}`). With no `agent.yaml`, or no `command` in it, Send only queues the notes, and a hand-started agent works them the same way.

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

