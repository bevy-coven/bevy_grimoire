# Review: 2026-10-21-03 — the editor's own eyes, over MCP

Enhancement: **an agent can see the editor, and can see when its own action landed.**
Evidence: `evidence/after/`, `evidence/crops/`, `video/the-sequence.mp4`.

---

## Pass 1 — Function: every state of the feature, at full frame

### The critique, before the verdict

**What I was looking for.** Three tools, and the question for each is the same: *does the answer
tell the agent what it needs, without the agent knowing anything about the project?* A tool that
returns a picture the agent cannot read is a tool that returns nothing; a tool that returns a
summary the agent cannot check is a tool that returns a claim.

**The first capture was the editor's own `Surface` image, and the whole editor was in it.** The
world, the inspector, the kit list, the status bar, the gizmo. That is the right frame — the
editor draws one screen, and an agent that saw only the world would miss the inspector, which is
*where the editor reports what it did*.

**The first failure I found was a race, and it was invisible in the image.** The sequence returned
**3 images for 4 frames**. The manifest said `frames: 4`, the files on disk were 4, and the answer
carried 3. The cause: the run finished when the frames were *asked for*, and the last PNG was
still being written. **A caller that trusted the answer and read the files would have found one
missing, for no reason it could see.** Fixed by finishing on the *write*, and the observer that
sees the write.

**The second failure was the headless case, and it was the editor's own doing.** The headless
harness runs with `WgpuSettings { backends: None }` — no GPU, no render world — so a screenshot
never produces a readback. My first fix made the run *wait* for a frame that could never exist,
and the test hung for 20 seconds and failed. **The fix was to settle the question once, at
startup**: `RenderPlugin` creates the `RenderApp` sub-app only when a backend was available, so
the sub-app's presence *is* the fact. The headless run now finishes and **says why** — `"this
editor has no render world, so no frame could be written"` — rather than reporting success with
no picture behind it.

**The third thing I checked was the most important one: does frame 0 really show the state before
the action?** The manifest says `operation_applied_t_s` and each frame's `actual_t_s`, and the
assertion is `applied_at > frame_0.actual_t_s`. In the live run: **the operation went in at
0.121 s, frame 0 was taken at 0.061 s.** And the pixels agree: frame 0's inspector reads
**Y 35**, frame 1's reads **Y 48**. The operation set Y to 48.

### The states, all captured

| state | what it shows | verdict |
|---|---|---|
| `emerge_state` | the map, the project, the armed piece, the problems | reads |
| `emerge_capture_still` | one image of the editor + the state in the same answer | reads |
| `emerge_capture_sequence`, frame 0 | the state **before** the action | reads |
| `emerge_capture_sequence`, frames 1..n | the state **after** the action | reads |
| the manifest | when each frame was due, when the action went in, what it said | reads |

### Verdict: **GREAT**

**Every state works, and the one that could have lied does not.** The frame that is supposed to
show the *absence* of the change shows the absence of the change, and the pixels say so.

---

## Pass 2 — Zoom: 3x on the panel that carries the answer

### The critique, before the verdict

**A full frame at 2560x1406 read as "the editor is fine".** The Y bound is a *number in a field*,
and a number in a field is where a screenshot's honesty is decided: at 1x it is a smudge, and a
smudge cannot be evidence.

**The crop is the inspector's `MAP SIZE` block** — the field the operation writes. If the change
is real, this field changes. If the sequence is lying about its ordering, this field does not.

**What the zoom showed:** frame 0's field reads **X 300 / Y 35**, frame 1's reads **X 300 / Y 48**.
The X is unchanged because the operation only touched the Y axis — which is the *right* kind of
control, because a change that moved X as well would be a change the author did not ask for, and
the picture would not say which was which.

**The typography holds up.** The digits are the editor's monospace, the field's background is
distinct from the panel's, and the label column's right-alignment puts the three numbers on the
same edge — so a reader comparing two frames is comparing two numbers on the same baseline, not
two shapes in different places.

### Verdict: **GREAT**

**The change is legible at 3x, and the crop is the evidence rather than a decoration.** The number
that the operation wrote is the number that the picture shows.

---

## Pass 3 — Fresh eyes: re-captured from scratch, judged as a developer who has never seen it

### The critique, before the verdict

**I re-ran the whole thing from a cold editor** — a fresh process, a fresh MCP server, a fresh
project — and drove it the way an agent would: list the tools, read the description, call the
tool, look at what comes back. **No project knowledge.** The questions a fresh agent actually has:

1. *What is this editor holding right now?* → `emerge_state` answers in one call, with the map's
   name, its path, its bounds, its placement count, the project, the armed piece, the door, and
   the problems. **No prior knowledge needed.**
2. *What does the editor look like?* → `emerge_capture_still` answers with the image **and** the
   state, in one call.
3. *Did my change land, and when?* → `emerge_capture_sequence` answers with four frames and the
   manifest's timings.

**The fresh-eyes problem I actually hit: the descriptions.** The brief's rule is *"read only the
description, call the tool, and check the result is usable with no project knowledge."* So the
description **is** the manual, and the test I wrote held it to that — and it **failed**:

> the tool `emerge_state` does not say what comes back: *"What the editor is holding right now: the
> open map and its bounds, how many pieces it has, what is armed, what is selected, which door and
> tab are open, and what the editor last said and what has gone wrong. A read; it changes nothing."*

**It described the editor's state and never said it *returned* it.** A reader could not tell a
result from a refusal. Fixed — *"Returns that summary as JSON, and changes nothing."*

**The second fresh-eyes problem: the annotations.** `emerge_capture_sequence` is marked
`destructiveHint: true` and `emerge_state` is marked `idempotentHint: true`. **Those are the
right answers, and they are not the obvious ones** — the sequence *is* destructive (it applies an
operation), the state *is* idempotent (it writes nothing). The test that checks them is worth
keeping, because the obvious answers would have been wrong.

**The third fresh-eyes problem: the tool list.** Seven tools, all prefixed `emerge_`, all unique,
each with a schema. A fresh agent sees them in one call and the names are self-describing.

**What a fresh agent *cannot* do, and should be told:** it cannot ask for a frame rate other than
4 (or 2 for a long operation), and it cannot ask for a sequence longer than 10 seconds. **Both
refusals are in the schema** (`enum: [4, 2]`, `maximum: 10`), so the agent learns the bound from
the contract rather than from a failure.

### Verdict: **GREAT**

**A developer who had never seen the editor could list the tools, read one description, call it,
and use the result.** The one place the description was not enough, the test caught it and the
description was fixed.

---

## The three verdicts

| pass | verdict |
|---|---|
| 1 — Function | **GREAT** |
| 2 — Zoom | **GREAT** |
| 3 — Fresh eyes | **GREAT** |

## What each pass changed

- **Pass 1** found the write race (3 images for 4 frames) and the headless hang. Both fixed; the
  second one made the headless run *say why* it had no picture.
- **Pass 2** found nothing. The panel that carries the answer is legible at 3x and the two frames
  differ in exactly the field the operation wrote.
- **Pass 3** found the missing "returns" in `emerge_state`'s description, and the test that will
  keep it there.

**A pass that finds nothing is a pass that looked.** The zoom pass compared two numbers on the
same baseline and the fresh-eyes pass read the descriptions the way an agent would — and the
descriptions were the thing that was wrong.

## The gate that was not green, and why

**Sanctuary's own suite is red: 1,037 pass, 8 fail.** **It was red before this run touched
anything**, and the measurement is the two rows:

| Sanctuary's state | result |
|---|---|
| its committed `HEAD`, its uncommitted work stashed | **1,040 pass, 5 fail** |
| with another agent's work in progress | **1,037 pass, 8 fail** |

The five at `HEAD` are pre-existing (`prototype::kit_lineup`, `prototype::town_map`,
`siege::flankers`, `siege::research`, and four `siege::isowall` tests failing on *"the chunks
never settled"*). The three extra are another agent's in-flight work in Sanctuary's own
`src/prototype/town_map.rs`.

**This run cannot be the cause, and the reason is structural rather than a guess:** Sanctuary
depends on **`bevy_emerge_core` alone**, and this run changed only `bevy_emerge_mapper` and
`bevy_emerge_mcp`. The diff over the shared crates is **empty**. **No consumer callsite is broken,
and `AGENTS_CHAT.md` needs no note.** The editor's own boot against Sanctuary's project is green.

**A review that says "green" here would be a lie with a table under it.**

## The one thing this review cannot settle

**Whether an agent's *vision model* reads the editor as well as a person does.** The frames are
the editor's own pixels at 2560x1406, which is the same thing the author sees, and the inspector's
digits are legible at 3x. **But this review judged the frames with a human eye, and the claim
that a model can read them is a claim about the model, not about the editor.** The editor's part
— *the pixels are the editor's own, and the change is in them* — is measured and shown.
