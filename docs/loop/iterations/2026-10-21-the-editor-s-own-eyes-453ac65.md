# 2026-10-21 the-editor's-own-eyes: an agent can see the editor, and can see when its own action landed

Focus: the agent's side of the loop · Commits: `453ac65..e2be78c` · Outcome: **shipped**

## Summary

**An agent can now see the editor.** Three new tools on the MCP server — `emerge_state`,
`emerge_capture_still`, `emerge_capture_sequence` — and one new module in the editor,
`crates/bevy_emerge_mapper/src/sequencer.rs`.

**The claim, stated exactly:** an agent can call one tool and receive **the editor's own pixels**
— world and interface together, because the editor draws both into one image and the inspector is
where the editor reports what it did — and it can ask for a **sequence** in which the operation it
dispatched lands **after the first frame**, so the sequence shows the state *before* and the state
*after*.

**The previous run could not check anything.** Every answer was a summary string the editor wrote
about itself, so an agent that applied a change and was told "done" had learned one thing: that
the editor did not refuse. **A summary is a claim; the editor's own pixels are the ground truth.**

## Baseline

- **Gate:** `cargo test --workspace` → **1,320 pass, 0 fail** (from the previous run).
- **The MCP server had four tools**: `emerge_list_operations`, `emerge_preview_operation`,
  `emerge_apply_operation`, `emerge_read_oplog`. **None of them returned an image**, and none of
  them returned the editor's state. *Evidence: `git show HEAD:crates/bevy_emerge_mcp/src/main.rs |
  grep -oE 'name = "emerge_[a-z_]+"'`.*
- **Latency:** the paths did not exist, so there was nothing to measure.

## The design decision that shaped everything

**The sequencer lives in the editor, not in the MCP server.**

The obvious design puts the sequencing in the server: capture a frame, apply the operation,
capture the next. **It is wrong**, for one reason: **the screenshot reads the *rendered* frame**,
so an operation applied before frame 0's readback would be **in** frame 0 — and a sequence that
begins with the thing it was supposed to show the absence of is *worse than no sequence*, because
it says the action was invisible when it was not.

**The server cannot promise the ordering.** It would take two round trips — capture, then apply —
with no ordering between them, and the readback lands a frame or two after the request.

**So the editor takes the frames on its own clock** and applies the operation **after frame 0 has
been asked for**. The ordering is then a property of the editor's code, and it is testable:
`operation_applied_t_s > frames[0].actual_t_s`.

## Bugs found and fixed while building it

Three, each caught by the thing that measures it.

### B-001: the sequence reported 3 images for 4 frames

**The run finished when the frames were *asked for*, and the last PNG was still being written.** A
caller that trusted the answer and read the files would have found one missing, for no reason it
could see. **Fixed by finishing on the *write*** — an observer records the write, and the run's
finish condition is `all asked && all written`.

*Measured:* the live sequence returned **4 images for 4 frames**, and all four files were on disk.

### B-002: the headless editor hung for 20 seconds, waiting for a frame that cannot exist

With `WgpuSettings { backends: None }` there is no render world, so a screenshot never produces a
readback. My first fix made the run *wait* for a frame that could never arrive, and the test
hung.

**Whether a frame can be written is now settled once, at startup** — `RenderPlugin` creates the
`RenderApp` sub-app only when a backend was available, so **the sub-app's presence is the fact**,
and it is a public, measurable one. A headless run **ends and says why**: *"this editor has no
render world, so no frame could be written"*, rather than reporting success with no picture
behind it.

### B-003: `emerge_state`'s description never said it returned anything

The test that holds the descriptions to the brief's rule — *"read only the description, call the
tool, and check the result is usable with no project knowledge"* — failed on it:

> the tool `emerge_state` does not say what comes back: *"What the editor is holding right now: the
> open map and its bounds, how many pieces it has, what is armed, what is selected, which door and
> tab are open, and what the editor last said and what has gone wrong. A read; it changes nothing."*

It described the editor's state and never said it *returned* it. **A reader could not tell a result
from a refusal.** Fixed: *"Returns that summary as JSON, and changes nothing."*

## And a ratchet that caught the new resources

`every_resource_is_classified` failed on `sequencer::Sequencer` and `sequencer::CanRender`, and
**its own failure message said what to do** — add each to `screen::OWNERSHIP`. Both are
`Ownership::Session`: a capture in flight is bound to this application, and **a door change does
not make a headless editor able to render**.

## The seam is now checked too

`every_brp_method_the_server_calls_is_registered_in_the_bridge` walks the server's source for
every `"emerge/…"` it calls and checks the bridge registers it. **A method the server calls and
the bridge does not register is a tool that fails at the call**, and the failure reads like the
editor being down.

## The measurements

Live, against Sanctuary's `town` map — 727 placements, 300 × 20 × 1120 m.

| | |
|---|---|
| `emerge_capture_still` | **206 ms** — one PNG (1,363,909 bytes, 2560×1406) plus the state |
| `emerge_capture_sequence` | **880 ms** — four frames and the action between them |
| the action's moment | **0.121 s**; frame 0's moment: **0.061 s** |
| the pixels | frame 0 reads **Y 35**, frame 1 reads **Y 48** — the operation set Y to 48 |

**The editor's clock drove the timing.** The server made one call and waited.

## The tests

- `crates/bevy_emerge_mapper/tests/a_sequence_starts_before_the_action.rs` — **three tests.** The
  first runs the real sequencer in a real editor, applies a real operation through the real
  registry, and asserts `applied_at > frame_0.actual_t_s` **and** that the map's Y bound changed.
  **The assertion is on the algorithm, not on the string.**
- `crates/bevy_emerge_mapper/tests/operations_have_two_front_ends.rs` — **one new test**, the seam
  check above.
- `crates/bevy_emerge_mcp/tests/an_agent_calls_the_server.rs` — **one new test**, holding the three
  tools' descriptions and annotations to the brief's rule.

**And the test says what it cannot measure.** The headless harness has no render world, so **no PNG
is written there**; the headless test proves the *ordering*, and the run says why it has no
picture. **The pixels are proved in a real windowed editor** — and that is the run's evidence.

## The gates

| gate | result |
|---|---|
| `cargo test --workspace` | **1,325 pass, 0 fail** (up from 1,320) |
| clippy | **zero findings in any file this run touched**; the workspace's pre-existing findings are in other crates and unchanged |
| `cargo fmt` | **every file this run touched is rustfmt-clean** under edition 2024; the repository as a whole is not (thousands of pre-existing diffs elsewhere) |
| `cargo doc --no-deps` | clean |
| the headless boot, Sanctuary's project | **12 frames, 4096 entities, no panic** |
| Sanctuary's own suite | **1,037 pass, 8 fail — red before this run, and not this run's** |

### On Sanctuary's red

**Measured, not assumed.** Stashing Sanctuary's own uncommitted work and re-running its suite at
its committed `HEAD` gives **1,040 pass, 5 fail**; with another agent's work in progress it is
**1,037 pass, 8 fail**. The five at `HEAD` are pre-existing — `prototype::kit_lineup`,
`prototype::town_map`, `siege::flankers`, `siege::research`, and four `siege::isowall` tests
failing on *"the chunks never settled"*. The three extra are another agent's in-flight work in
Sanctuary's own `src/prototype/town_map.rs`.

**This run cannot be the cause, and the reason is structural:** Sanctuary's `Cargo.toml` depends on
**`bevy_emerge_core` alone**, and this run changed **only** `bevy_emerge_mapper` and
`bevy_emerge_mcp` — the diff over `bevy_emerge_core/`, `bevy_emerge/`, `bevy_emerge_rng/`,
`bevy_emerge_solver/`, `bevy_emerge_anim/` and `bevy_devshot/` is **empty**. **No consumer
callsite is broken, and `AGENTS_CHAT.md` needs no note.**

**The editor's own boot against Sanctuary's project is green** — 12 frames, 4096 entities, no
panic.

## The three verdicts

| pass | verdict | what it changed |
|---|---|---|
| 1 — Function | **GREAT** | found B-001 and B-002 |
| 2 — Zoom | **GREAT** | nothing — the panel that carries the answer is legible at 3× |
| 3 — Fresh eyes | **GREAT** | found B-003 |

**A pass that finds nothing is a pass that looked.** The zoom pass compared two numbers on the
same baseline; the fresh-eyes pass read the descriptions the way an agent would, and the
descriptions were the thing that was wrong.

## The one thing this review cannot settle

**Whether an agent's *vision model* reads the editor as well as a person does.** The frames are
the editor's own pixels at 2560×1406 and the inspector's digits are legible at 3× — but **this
review judged the frames with a human eye**, and the claim that a model can read them is a claim
about the model, not about the editor. The editor's part — *the pixels are the editor's own, and
the change is in them* — is measured and shown.

## The run

`docs/loop/runs/2026-10-21-03/` — the research, the plan, the perf numbers, the review, the
frames, the 3× crops and the mp4.
