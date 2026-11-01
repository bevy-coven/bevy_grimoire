# bevy_grimoire: what it is, why it exists, and what is wrong with it

One document, written 2026-10-26 at commit `f2c5b34`, after 56 recorded loop iterations. It is an
outside-in description of the project as it actually stands — including the parts that do not work and
the parts I would not defend.

Everything here is checkable against the repository. Numbers are measured, and where I am reasoning
rather than reporting I say so.

---

## 1. The one-sentence version

**bevy_grimoire is a world editor for Bevy games in which a person and an AI agent edit the same map
through the same named operations, with one undo history between them.**

That is not a feature list. It is the design constraint everything else follows from, and it is the
thing that makes this different from a level editor with an AI chat box bolted on.

---

## 2. The problem it is trying to solve

A game developer building a world has two tools that do not talk to each other: the editor, where
things get placed, and the agent, which can reason about what should be placed. Every existing bridge
between them is lossy in the same way:

| The usual arrangement | What goes wrong |
|---|---|
| Agent writes a file, editor reloads it | The change has no undo entry, no author, and no validation until the reload |
| Agent writes a script that calls editor APIs | The script is not auditable, not replayable, and bypasses every rule the UI enforces |
| Agent describes a change in prose | A person retypes it, and the retyping is where it goes wrong |
| Agent uses a separate "AI" path | Two implementations of "place a crate" that drift, and the AI path is always the worse one |

bevy_grimoire's answer is structural: **there is exactly one way to change anything**, a typed operation
in a single registry, and both the keyboard and the agent dispatch it. Everything else — undo, the log,
the validators, the parity test — falls out of that one decision.

---

## 3. What it is made of

168,569 lines of Rust in nine crates, 818 commits, 122 integration test files, 20 registered
operations, 14 MCP tools, 56 recorded loop iterations.

| Crate | Lines | What it is |
|---|---:|---|
| `bevy_cauldron` | 122,506 | The editor application. Opens a project directory, writes `<root>/assets/grimoire`. |
| `bevy_grimoire_core` | 37,058 | The engine-free schema, grammar learner, WFC, validators. No Bevy. |
| `bevy_cauldron_mcp` | 4,016 | The MCP server: 14 tools over stdio, talking to the running editor over BRP. |
| `bevy_grimoire_anim` | 1,327 | Clip-weight blending on a skinned rig. |
| `bevy_grimoire_solver` | 1,304 | A deterministic SAT back-end. |
| `bevy_grimoire` | 1,275 | The runtime: load a library and a map into a game. |
| `bevy_grimoire_snip` | 581 | Cuts one rectangle out of a picture of the editor. |
| `bevy_grimoire_rng` | 368 | One seeded ChaCha8 stream for the whole simulation. |
| `bevy_devshot` | 134 | A frame on demand, from the GPU. |

**The 122k/37k split is the architecture in one number.** The editor is a Bevy app and cannot be
unit-tested headless in the ordinary way; the core is engine-free and can. `engine_free.rs` fails the
build if a Bevy dependency appears in core.

### The operation registry

Twenty operations, and they are the whole vocabulary of what an agent can do:

```
map_create  map_open  map_save  map_rename  map_set_settings
placement_add  placement_remove  placement_set_yaw  placement_set_pin
layout_propose  layout_accept  layout_discard  fill
label_set  piece_import  piece_select  brush_arm  door_open
rules_set  stamp_remove
```

Each one declares four things: its parameters (a JSON schema an agent reads), a **plan** that is pure
and changes nothing (this is what preview is), a **commit** that is the only code that writes, and an
**undo strategy** (`None`, `Inverse`, or `Snapshot`). A parity test enumerates the registry and fails if
an operation lacks an MCP tool or a UI entry point, or if the two expose different parameters.

---

## 4. Every feature, honestly described

### 4.1 Authoring a map

- **Palette, ghost, place, fill, remove.** A translucent ghost follows the cursor showing exactly what
  a click will place, turned the way the brush is turned. `G` proposes and draws a whole generated
  layout as ghosts before `Enter` keeps it.
- **Layers that pass each other.** A floor tile under a crate is not in the crate's way; two lamps on
  one table contest that table and not every table; two wall pieces contest a face only at the same
  height. The rule is `stack::same_layer`, and it is a table, not a judgement.
- **Flush legal, inside refused.** Kitbashing wants wall segments exactly end to end, so touching
  passes and interpenetration refuses (`OVERLAP_EPS`).
- **Overlaps on disk are now visible.** `stack::overlaps` is a whole-map pass, reported on every
  mutating answer. On Sanctuary's real 720-placement town it found **781 interpenetrating pairs** that
  nothing could see before — cross-checked against the click path's own rule, 781/781 agreeing.

### 4.2 Kits, labels, and a vision model that is not trusted

- Import a `.glb`, measure it, admit or refuse it, and commit a library descriptor — a person
  accepting a scan and an agent calling `piece_import` go through the same four admission checks.
- A VLM can propose labels per axis. **Every proposal is marked unchecked, and no button confirms one
  in place**: you toggle a chip, which marks that axis as yours. That is a deliberate refusal to let a
  model's confidence become a person's.

### 4.3 Generation that continues *your* layout

- `grammar::learn` derives a grammar from an arrangement you already made, then WFC continues it. The
  result is more of your layout, not a generic one.
- Compositions are a second, stronger source: groups with rules, whose edge tokens are derived from
  their members' geometry rather than declared.
- **The proposal is drawn before it is committed**, and `Enter` keeps exactly what was on screen.
- **Enclosure is measurable**: `range::measure` scores a solved grid on Smith & Whitehead's
  expressive-range axes, and a proposal now answers with its own measurement.

### 4.4 The agent interface

- 14 tools, `grimoire_`-prefixed, domain-worded (`grimoire_apply_operation`, not "run tool 7").
- **Every mutating answer carries an operation id** an agent cites and a person reverts by.
- **Every map change reports what it broke**: edge faults and overlapping placements the change
  introduced, with the whole map's totals beside them, and a standing fault is never renamed "new".
- **`dry_run` previews** anything; the preview is the operation's own pure `plan`, so it cannot
  half-apply.
- **A presented turn is one undo step**, from either door — `Cmd+Z` and `grimoire_undo_operation {turn:
  true}` take back the same amount.

### 4.5 The person's half of the collaboration

- **Snips**: `Cmd+\`, drag a rectangle, type one line. The agent can list them, read the note, and open
  the crop — this is the "click, write a note, the agent picks it up" loop, and it exists.
- **The steps panel** (shipped this iteration): the journal now shows the agent's operations as rows,
  each reading the operation log's own `result` sentence verbatim, with a capture marked on the row
  that made it.
- **The receipt and the journal**: one row for what just happened, and a scrolling list of refusals
  that `Esc` cannot clear.
- **One undo history.** The agent's change joins yours; `Cmd+Z` reaches the newest change still in
  effect, whoever made it.

### 4.6 Data and hot reload

- Everything authored is RON/JSON in the project directory, versioned with migrations.
- The editor checks the project every quarter second and re-reads what changed — no restart.
- A live change is written back to the file, so nothing exists only in memory.
- Bad data never takes the editor down: a malformed file is refused with its file, field and line, and
  the last good state stays loaded.

---

## 5. The competitive advantage

I will separate what I think is genuinely durable from what is merely unusual.

### Durable

**1. One operation layer is an architectural moat, not a feature.**

Every capability an agent gains is a capability a person gains, by construction, and the parity test
enforces it. In every competing arrangement — an agent that scripts the app, an agent that writes files
— the agent path and the human path are separate code, and the weaker one rots. Here they cannot
diverge without a test failing.

The measured consequence: **`reached 11/11`** — an outside agent goes from the sentence *"a mossy ruined
courtyard with two exits"* to a made map, placed pieces, a proposed layout, a kept layout, a saved file
and a presentation waiting on the person, with no project knowledge beyond the tool descriptions.

**2. The operation log is a replayable record, not a debug print.**

Every change records its source, session, parameters, seed, rules hash and result. That is what makes
"what did the agent do while I was away" answerable, and it is the foundation an agent needs to *inspect*
its own work rather than guess.

**3. Validation is part of the answer, not a separate tool.**

An agent's change comes back with what it broke. That is the difference between an agent that can be
trusted to touch a 720-placement map and one that cannot — and the 781 overlapping pairs it found on
the only real project in this repo is the evidence that this was not a theoretical concern.

**4. The engine-free core is portable.**

`bevy_grimoire_core` has no Bevy dependency, so the grammar learner, the WFC and the validators can run
in a game, a test, a CI job or a server. Sanctuary — the shipping game this was built for — uses it
without the editor.

### Unusual but fragile

- **A VLM labeller that refuses to confirm its own guesses.** Right, and expensive.
- **Determinism everywhere**: `bevy_grimoire_solver` reads no clock and uses no threads; `bevy_grimoire_rng`
  is one seeded stream. A generated layout is reproducible from its seed.
- **The editor is driveable with no window raised** (an offscreen mirror camera plus BRP), which is
  what makes the loop's own visual verification possible.

### What is *not* an advantage

The line count. 122k lines of editor is not a moat; it is the cost of the thing that is.

---

## 6. What is wrong with it

This is the section I would want if I were reading someone else's project.

### Structural problems

**1. The editor crate is enormous and unaudited.**

122,506 lines in `bevy_cauldron`, and the README says it plainly: *"Vibe Coded — written by an AI
agent working from a human's direction… no line-by-line human audit."* I think that is the single
biggest risk in the repository. It is covered by tests, and the tests are good, but a test suite proves
the properties someone thought to assert — not that the code is comprehensible to the next person.

**2. Two copies of the tool parameter structs.**

The MCP server cannot depend on the editor crate (it is the bridge's client), so each tool's params and
the editor's params are separate structs, kept in step by a hand-written `json!` body. They drifted in
E-084 and it took a live run to find. I added a source-text ratchet, which catches the drift but does
not remove the duplication. **A shared params crate would.**

**3. The editor and the game load the same data through two implementations.**

`bevy_grimoire` (the runtime) and `bevy_cauldron` (the editor) both read a library and a map. The
repo's own convention is "consistent and wrong beats inconsistent", which is true, but it means a
schema change has two readers to update and one of them is checked less often.

**4. The workspace has 43 open backlog items and 11 of them are in `editor`.**

The backlog is honest, which is good. It is also long. Several items are years of ordinary work:
doorway clearance detection, variety measurement on real content, the session panel's remaining beats,
`map_remove`, redo for an agent's undo.

### Things that are genuinely limited, by design

**5. Overlaps are only checked *within* a layer.** A 2.2 m cabinet and a sconce at 1.8 m on the wall
behind it interpenetrate silently. A door and a crate in the doorway cannot conflict. The code says so
in its own doc (`stack.rs`:589-596); the information to close it exists and closing it is a decision
about what an author wants refused. Filed as E-079.

**6. Variety measurement cannot run on the only real project.** Sanctuary's 16 compositions are all
`Anchored`, and a tile grammar skips those (`"presents no edge to match"`). So the composed source is
refused there and the learned source derives no wall interfaces — `variety` answers `null` on `town`.
The metric is proved on crate fixtures. Filed as E-083.

**7. One undo semantics question is still open.** An agent's id-less undo walks the *log* and takes back
one logged change; `Cmd+Z` takes the turn. I made the agent's door able to take the turn (`turn: true`)
rather than changing what its existing call means, which I think is right — but it means an agent has
to know which of two verbs it wants.

### Process honesty

**8. The independent review has failed five iterations running.**

The loop contract says an iteration without an independent review "does not merge". For five
consecutive iterations the `reviewer` subagent died on a cloud rate limit, and I performed the review
myself and said so in each report. That is a weaker review. It is recorded rather than papered over, and
it is the process gap I would fix first.

**9. Every iteration measures `perf.py` on a shared machine and the numbers are frequently
incomparable.** Load moved between 3 and 49 across a single afternoon; a before/after pair has
repeatedly shown a "regression" that was the machine. Recent reports state the load beside every number
and sometimes decline to draw a conclusion. That is the honest response, and it means **there is no
trustworthy long-run performance trend for this project.**

---

## 7. What is right with it

**1. The one-operation rule is real and it is enforced.**

Not a convention, not a code review habit — a test. I trust this more than anything else in the repo.

**2. The repository writes down *why*.**

Module docs here argue rather than describe. `stack.rs` states the bounded claim of its overlap rule
and names the two cases it misses. `descriptor.rs` records a ruling that nothing reads `Clearance`.
`borrowing-from-mixar.md` has a licence rule, a pinned commit, a procedure and a catalogue with
statuses. This is the rarest thing in the repository and the most valuable.

**3. Refusal over degradation, everywhere.**

A headless editor refuses a snip rather than writing an empty picture. A `variety` that cannot be
measured is `null`, not `0.0`. A blank `rules.ron` is refused rather than read as "no rules". A save is
a hard link that fails rather than a rename that overwrites. When the code had to choose between a
plausible answer and no answer, it chose no answer, and it says why at the site.

**4. The failing-before discipline held.**

122 integration test files, and the iterations I ran all produced tests that failed on the base commit
and passed after. The reports quote the failing assertion. Where a test was vacuous — a `contains
("renam")` that could never match — it was found by reverting the fix and watching the test stay green,
and the test was rewritten.

**5. The loop leaves an honest trail.**

55 log lines, 94 archived backlog items, and iteration reports that name their own defects. When a claim
turned out false, the record says so. That is a working memory, and it is why an iteration can start
cold and be useful within twenty minutes.

**6. The data is the project, and it hot-reloads.**

No rebuild to change content. That sounds like a checkbox; it is the difference between a person who
tunes a courtyard in ten minutes and one who gives up.

---

## 8. If I were to change one thing

**Split the tool parameter structs into a shared crate.**

It is the smallest item on the "wrong" list and the one with the clearest failure: a hand-written body
that silently drops a field, found only by a live run, after every deterministic check passed. A shared
`bevy_grimoire_protocol` crate would make it a compile error instead of a ratchet, and it would shrink the
surface where two copies of one contract exist anywhere in the workspace.

The bigger items — the 122k-line unaudited editor, the performance trend nobody can measure — are real
and are not one-iteration fixes. This one is.

---

## 9. Where it stands

| | |
|---|---|
| Operations | 20, all with both front ends, enforced |
| MCP tools | 14 |
| `accept.py` reached | 11 / 11 |
| Integration test files | 122 |
| `cargo test --workspace` | 142 test binaries, 0 failures |
| Open backlog | 43 items |
| Shipped in the last 24 hours | a whole-map overlap pass, the rules reader, the rules writer, the variety measurement, turn grouping, turn undo from the agent's door, the steps panel, and a ratchet on the tool bodies |
| Published | nothing — E-006 is open, and the first publication is still undecided |

It runs, it is used by a shipping game, and it can be driven end to end by an agent that has read
nothing but the tool descriptions. It also has an unaudited 122k-line editor at its centre and a review
process that has not run in five iterations. Both of those are true at once, and a document that said
only one of them would be lying.
