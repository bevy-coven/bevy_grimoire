# Plan: the editor's own eyes, over MCP

Run: **2026-10-21-03**.
Enhancement: **an agent can see the editor, and can see when its own action landed.**

## The shape of it

**One new module in the editor, three new tools on the server, one new crate for the frames.**

```
crates/bevy_emerge_mapper/src/sequencer.rs   (new)  the editor's side: the clock, the frames,
                                                     and the operation's moment
crates/bevy_emerge_mapper/src/ops_brp.rs     (edit) three BRP commands, so the server has
                                                     something to call
crates/bevy_emerge_mcp/src/main.rs           (edit) three tools, and the base64 of a PNG
crates/bevy_emerge_mcp/src/docs.rs           (edit) the tools' descriptions — the agent's manual
```

## The decision that shapes everything

**The sequencer lives in the editor, not in the server.**

The screenshot reads the **rendered** frame. So an operation applied before frame 0's readback
would be **in** frame 0 — and a sequence that begins with the thing it was supposed to show the
absence of is worse than no sequence, because it says the action was invisible when it was not.

**The server cannot promise the ordering.** It would take two round trips — capture, then apply
— with no ordering between them, and the readback lands a frame or two after the request. So the
editor takes the frames on its own clock and applies the operation **after frame 0 has been asked
for**. The ordering is then a property of the editor's code, and it is testable.

## The three tools

### `emerge_state` — what the editor is holding

A **read**. The map (name, path, bounds, placements, dirty), the project (kit, kits,
descriptors, compositions), the editor (tool, armed piece, brush yaw, the last thing it said,
the problems), the door, the mode, the selection.

**It returns JSON, and it changes nothing.** An agent calls it before a change to know what it is
changing, and after one to check that it landed.

### `emerge_capture_still` — a picture of the editor as it is now

**The editor's own `Surface` image** — the same pixels the author sees, world and interface
together, because the editor draws both into one image.

**The answer carries the image** (an MCP `image` content block, base64 PNG) **and the state**, so
a caller gets the picture and the machine-readable answer in one call.

### `emerge_capture_sequence` — a sequence of frames, with the action in the middle

Parameters: `label`, `seconds` (≤ 10), `fps` (4, or 2 for a long slow operation), and an
optional `operation`.

**The frames are named `f{index:03}_t{seconds:.2}s.png`**, in a directory the editor owns.

**The operation goes in after frame 0.** The manifest records *when* — `operation_applied_t_s` —
and the operation's own result.

**The run finishes when every frame has been WRITTEN**, not when they were asked for. The
readback lands a frame or two after the spawn, and a run that finished on the ask would report
itself done while its last PNG was still on its way — a caller that read the files then would
find one missing, for no reason it could see.

## The refusals

**Refusal over degradation.** Each one is a real thing the tool cannot do:

- **A second sequence while one is running** — the two would interleave their file names.
- **A name that is not an id** (`../escape`) — the name names a directory, and that one is
  outside the capture area.
- **A length over the cap** (10 s) — an unbounded sequence is an unbounded number of files.
- **A rate over the cap** (4 fps, or 2) — a fast sequence is a disk-filling sequence.
- **An operation that is not in the registry** — refused by the registry, not by a second list.

## The tests

### `crates/bevy_emerge_mapper/tests/a_sequence_starts_before_the_action.rs`

**Three tests, and each one can fail.**

1. **`a_sequence_starts_before_the_action_and_continues_after_it`** — runs the real sequencer in
   a real editor, applies a real operation through the real registry, and asserts that
   `operation_applied_t_s > frames[0].actual_t_s` **and** that the map's Y bound changed.
   **The assertion is on the algorithm, not on the string**: it fails if the operation is applied
   before the first frame is asked for, whatever the message says.
2. **`a_second_sequence_while_one_is_running_is_refused`**
3. **`a_sequence_refuses_a_bad_name_a_bad_length_and_a_bad_rate`**

**And the test says what it cannot measure.** The headless harness runs with
`WgpuSettings { backends: None }` — no GPU, no render world — so a screenshot never produces a
readback and **no PNG is written**. The headless test proves the *ordering*; **the pixels are
proved in a real windowed editor**, and the run's evidence is the capture.

### `crates/bevy_emerge_mapper/tests/operations_have_two_front_ends.rs`

**A new check: `every_brp_method_the_server_calls_is_registered_in_the_bridge`.** The two lists
are written down in two places — the server calls `emerge/…`, the bridge registers `emerge/…` —
and **a method the server calls and the bridge does not register is a tool that fails at the
call**, which reads like the editor being down. The test walks the server's source for every
`"emerge/…"` string and checks the bridge registers it.

### `crates/bevy_emerge_mcp/tests/an_agent_calls_the_server.rs`

**`the_capture_and_state_tools_say_what_they_do`** — reads the tool list and checks each new
tool's description says **what comes back**, and that its annotations are the ones its behaviour
earns (`emerge_state` is a read and is idempotent; the two captures are not idempotent, because
each writes a file).

**This test found a real gap**: `emerge_state`'s description did not say it *returns* the
summary. The brief's rule — *"read only the description, call the tool, and check the result is
usable with no project knowledge"* — is exactly what the description failed.

## The gates

`cargo test --workspace` · `cargo clippy --workspace --all-targets -- -D warnings` ·
`cargo fmt --check` · `cargo doc --no-deps` · the headless boot against Sanctuary's project ·
Sanctuary's own suite.
