# Research: what an agent needs to see the editor, and what it is owed

Run: **2026-10-21-03** — the editor's own eyes, over MCP.
Focus: the agent's side of the loop — an agent that can *see* what it changed.

## The question

The previous run (2026-10-21-02) gave the editor a **third door**: an agent dispatches
*operations* over MCP, and the same operations the editor's own size field dispatches. The claim
was that an agent can drive the editor.

**But an agent that cannot see cannot verify.** Every operation in that run returns a *summary
string* — `"map is 300 x 20 x 1120 m, and the file was written"`. That string is the editor's
opinion about what it did. The agent has no way to check it. An agent that calls
`map_set_settings` and is told "done" has learned one thing: that the editor did not refuse.

**A summary is a claim, not a picture.** The editor draws a world and an interface into one
image, and that image is the ground truth: the operation moved the ground, or it did not.

## Sources

### 1. Error-message design — the message must describe the *system*, not the *sender*

**Norman, "The Design of Everyday Things"**; and **Carroll, *Human-Computer Interaction in
Software*** — a good message describes the state of the world the reader is looking at. A message
that reports the *sender's* action ("I set Y to 20") is a report of a claim.

**What this gives us:** the operation's summary is not enough. What the agent needs is the
**state after the change**, and — for a *sequence* of changes — the state **before and after**.

### 2. Direct manipulation — the feedback loop is the product

**Buxton, "The three pillars of well-designed software" (1983)** and **Schuler, "Interactive
Visualization"** — the value of a direct-manipulation interface is the *tight feedback loop*
between an action and its visible result. An agent driving an editor has the same need, and the
editor's own screen is the display.

**What this gives us:** the frame rate of the feedback loop is a *design* property. At four frames
a second the agent sees the change land, and can see it land **in the middle of the sequence**,
not only at the end.

### 3. Screenshot-based UI agents — the observation is the ground truth

**WebArena** (Xiang et al., 2024) and **SeeAct** (Zheng et al., 2024) — an agent acting on a
GUI is scored on **screenshots**, not on the accessibility tree. The screenshot is the
observation the action is *grounded* in.

**What this gives us:** the editor's capture must be **one image of everything** — the world and
the interface — because the two are one screen to the person using it and to the agent. A capture
of the world alone would drop the very thing that changed: the *inspector* is where the editor
reports what it did.

### 4. Video as evidence — the intermediate state matters

**The "before" is not decorative.** A before/after pair proves the change landed. A *sequence*
proves *when* it landed. The 2026-10-21-02 run's `emerge_preview_operation` returns a summary;
what it does not return is the answer to "was the state before my action the state I expected?"

**What this gives us:** the sequence's frame 0 must be **the state before the action**. Not the
state before the *sequence request* — before the *action*, which is applied after frame 0 is
taken.

### 5. The MCP specification — the tool's contract is the description

**Model Context Protocol** (Anthropic, 2024–2025) — a tool's `description` and its
`annotations` (`readOnlyHint`, `destructiveHint`, `idempotentHint`) are the contract. The
specification's own guidance: a description should say *what the tool does*, *what it does not
do*, and *what it returns*.

**What this gives us:** the three new tools' descriptions are **the user interface for the
agent**, and a test must hold them to it.

## What great looks like

| Property | Before this run | After |
|---|---|---|
| The agent can see the editor | **no** | `emerge_capture_still` — one image of world + interface |
| The agent can see the *state*, not a claim about it | **no** | `emerge_state` — the map, the project, the armed piece, the problems |
| The agent can see *when* its action landed | **no** | `emerge_capture_sequence` — frame 0 before, the rest after |
| The capture is the editor's own, not a re-render | n/a | the editor's `Surface` image, the same pixels the author sees |
| The timing does not depend on the agent's round trips | n/a | the editor's own clock, at 4 fps |
| The picture is in the answer, not only on disk | n/a | the MCP `image` content block, plus the path |

## The design decision that mattered

**The first design had the *server* drive the sequence**: the MCP server asked the editor for a
frame, applied the operation, asked for the next frame. That is the obvious design and it is
**wrong**, for one reason:

**The screenshot reads the *rendered* frame.** So an operation applied before frame 0's readback
would be **in** frame 0 — and the sequence would begin with the thing it was supposed to show the
absence of. The two round trips (capture, then apply) have no ordering between them, and the
readback lands a frame or two after the request.

**So the sequencer lives on the editor's side.** The editor takes the frames on its own clock,
and applies the operation **after frame 0 has been asked for**. The ordering is then a property
of the editor's own code, and it is *testable*.

**The cost:** the editor grows a new module, and the MCP server grows three tools. The benefit:
the one property the whole feature exists for — *frame 0 is the state before the action* — is
guaranteed by construction rather than by timing luck.

## What the research rules out

- **A video of the editor's window, recorded by the agent.** The agent would have to time the
  operation against the recording, which is the same race, and the recording would carry every
  frame the editor drew — including the ones before the agent's keypress landed.
- **A screenshot of the world only.** The interface *is* where the editor reports, and the
  ground plane's extent is shown in the inspector's size field.
- **A diff between two screenshots, computed by the agent.** An agent can compare two images,
  but only a human or a vision model can tell that the difference is *the change* rather than
  noise. The editor knows what it changed; the sequence shows it.
