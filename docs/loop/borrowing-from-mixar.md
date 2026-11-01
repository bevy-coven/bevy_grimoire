# Borrowing an interaction from Mixar

**Who reads this:** an agent working on how a person and an agent work together in bevy_grimoire — the loop's Collaboration lens, a series step that names a Mixar beat, or any task that says "like Mixar".

Mixar (https://github.com/Mixar-AI/mixar-app) is a Blender fork with an in-app agent. bevy_grimoire copies how its agent collaborates, not how it is built. **Mixar's agent lives inside the app while bevy_grimoire's agent sits outside and drives the editor over MCP (`bevy_cauldron_mcp`).** So a Mixar chat element becomes two things here: a surface in the editor the person watches, and the value an `grimoire_*` tool returns to the agent.

## Licence: borrow behaviour, never expression

**Measured at the pin on 2026-10-25**, because the numbers decide how careful you must be:

| | |
|---|---|
| the repository's `LICENSE` | **GNU GPL version 3** |
| `SPDX-License-Identifier` in the chat module's sources | `GPL-3.0-or-later` (523 files) and `GPL-2.0-or-later` (96 files) — **both are present**, so a file's own header is the authority for that file |
| bevy_grimoire | MIT OR Apache-2.0, and `AGENTS.md` forbids copying GPL code |

Rules:

1. **Never copy, paste or line-by-line translate Mixar code into Rust or Python here.** That covers its identifiers, comments, UI strings, icons, images and file layout.
2. **Read Mixar's source to learn what the person sees, what the agent gets back and what state changes.** Write that down in your own words (the behaviour note below), **stop reading the Mixar files**, and build from the note and bevy_grimoire's own patterns.
3. The reviewer treats any hunk that reads as a translation of a Mixar function as a ship-blocker.

## Pinned source

- Pin: commit `39b9d276ed82a254c5a91515b1953f9401f687d1` (main, 2026-10-22, `VERSION` 4.3.4).
- Cite Mixar as `mixar@39b9d27:<path>`.
- Read a file at the pin with `read` on `https://raw.githubusercontent.com/Mixar-AI/mixar-app/39b9d276ed82a254c5a91515b1953f9401f687d1/<path>`.
- Find files by listing the tree at `https://api.github.com/repos/Mixar-AI/mixar-app/git/trees/39b9d276ed82a254c5a91515b1953f9401f687d1?recursive=1` (it answers `truncated: false`, 4,629 entries). **Never guess a path.**
- The chat agent's client code is under `src/scripts/mixar/modules/space_mixie_chat/` (`core/` logic, `ui/` properties and operators), and its native drawing is under `src/source/blender/editors/space_mixie_chat/`.
- **To move the pin:** re-check every catalogue row against the new commit, and change the pin and the rows in one commit.

## Procedure: one beat per iteration

1. **Pick.** Take the catalogue row the iteration selected, or the row its open series step names. One row per iteration. A beat you discover along the way gets a new `unexamined` row; do not build it in the same iteration.
2. **Read.** Read the row's files at the pin, and the files they import that the tree listing shows under the same module.
3. **Write the behaviour note.** Put it in the backlog item's `evidence`, and copy it into the iteration report under `## Borrowed from Mixar`. It has six lines:
   - `Trigger:` what starts the beat (an agent call, a key, a click).
   - `Person sees:` what the person sees, step by step.
   - `Agent gets:` the values that come back to the agent.
   - `State:` what state it creates or changes, and how that is undone.
   - `Refuses:` what it refuses, and how it says so.
   - `Source:` one `mixar@39b9d27:<path>` per file read.
4. **Translate.** Map every line of the note onto bevy_grimoire, reusing what exists:
   - A change to project data becomes a registered operation in `crates/bevy_cauldron/src/ops.rs` with both front ends. `tests/operations_have_two_front_ends.rs` enforces this.
   - A presentation or conversation element that changes no data becomes an `grimoire_*` MCP tool backed by an editor-side resource (the pattern of `grimoire_present` / `present.rs`).
   - Anything the person sees is shown on an existing surface: `Status::problem` for refusals, `Status::note` for answers, the confirm modal (`confirm.rs`) for a yes/no, or a dock panel for a persistent feed. Name the surface in the note.
   - Undo goes through the oplog (`Undo::Inverse` or `Undo::Logged`).
   - Where Mixar's choice conflicts with a bevy_grimoire rule (refusal over degradation, problem versus note, one plugin per domain, data-driven with no rebuild), bevy_grimoire wins, and the note records the difference as `Differs:`.
5. **Build** per the loop brief. The failing-first test asserts the behaviour from the note (what the person sees, or what the agent gets), **never a string Mixar uses**.
6. **Prove it as a person sees it.** Make a capture sequence of the beat end to end through MCP and a contact sheet, with captions that are literally true. Phase 4 of the loop brief applies unchanged.
7. **Update the catalogue** in the same commit as the feature: the row's status, the bevy_grimoire symbol or tool, and the PR number.

## Catalogue

Paths are relative to `src/scripts/mixar/modules/space_mixie_chat/` unless they start with `src/`.

**Two source citations in this table were corrected on 2026-10-25 after reading the files at the pin**, and the corrections are the reason the procedure says "never guess a path":

- The action buttons' native file is **copy-to-clipboard only** — `mixie_chat_handle_action_button_click` reads the cached text of a clicked button and puts it on the clipboard, and the only action it acts on is `CHAT_ACTION_COPY`. It draws nothing.
- `chat_payloads.py`'s `approval_required` is **a flag the client sends to the backend** for a *fresh* turn, not a review surface. The review surface is `MixieChatActionItem` — a button's `label`, `value` and `style` (`PRIMARY` / `DEFAULT` / `DANGER`) — and the answer travels back on the next turn as an input answer with `action='respond'` and a `question_ref` (`core/composer_send.py`).

| Beat | What it gives the person | Mixar source (at the pin) | bevy_grimoire | Status |
|---|---|---|---|---|
| Present a result for review | The agent ends with what it made, what it assumed and a Keep / Undo all answer that returns to the agent | `core/slot_processor.py` (`MixieChatActionItem` replacement), `ui/properties/chat_slot_types.py` (`MixieChatActionItem`), `core/composer_send.py` (`action='respond'`, `question_ref`) | `grimoire_present`, `present.rs` | `planned (S1.7)` |
| Steps narrated by measured effect | Each step is one line saying what changed, never a raw log | `core/steps_format.py`, `core/steps_recorder.py`, `core/slot_processor.py` | `steps::from_oplog`, drawn in the journal | `shipped (E-068)` |
| Captures under their step | The picture sits on the step that made it | `core/capture_store.py` | `steps::Capture`, marked on its row | `shipped (E-068)` |
| Plan checklist | The person sees the agent's plan and each step's status | `ui/properties/chat_slot_types.py` (`MixieChatTodoItem`) | `grimoire_plan` | `backlog (E-069)` |
| Auto mode | The person lets the agent decide open choices instead of asking | `core/chat_payloads.py` (`auto_mode`), `core/composer_send.py` | — | `backlog (E-070)` |
| Turn checkpoints | One action takes back, or re-applies, everything an agent turn did | `core/turn_checkpoints.py`, `core/checkpoint_timeline.py`, `ui/operators/checkpoint_ops.py` | `EditorState::group_turn`, called by `grimoire/present` | `shipped (E-071)` |
| Retry a step | The person re-runs a failed step without restarting the turn | `core/retry_action.py` | — | `unexamined` |
| Moodboard references | Reference images the agent reads and cites | `src/scripts/mixar/modules/moodboard/core/chat_sync.py` | — | `backlog (E-075)` |
