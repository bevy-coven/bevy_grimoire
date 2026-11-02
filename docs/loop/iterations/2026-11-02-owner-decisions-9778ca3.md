# 2026-11-02 ui: the five blockers are decided, and K now reveals the overlay through gooey

Focus: the owner's six questions, all answered · Commits: cc60d05..HEAD · Machine: big · Outcome: shipped

## Summary
The owner asked what the `blocked` labels actually wait on and then answered all five decisions. This
pass records each where it is read, files the one request the research proved was missing (**G-33**),
and lands the first slice that needed no exception: **`K` reveals the shortcut overlay through gooey's
`ShortcutBadges`**, which G-32's `Reveal::Key` made possible.

## The measurement that changed the picture
The survey said "one protected exception unblocks 29 issues". Classifying **every** `chrome::`
reference in the protected tests by whether gooey can carry it says otherwise:

| the tests reference | refs | gooey |
|---|---|---|
| 12 colour tones (`KEY`, `VEIL`, `SUGGEST`, `SNIP_WASH`, `LABEL`, `MUTED`, …) | 14 | **yes** — `GooeySkin::tones` holds all 12 |
| 4 skin fields (`ACCENT`, `DANGER`, `SCRIM`, `TEXT`) | 7 | **yes** |
| 5 layout numbers (`PAD`, `COL_WIDE`, `BODY_LINE_H`, `BODY_CHAR_W`, `GAP_ROW`) | 10 | partly |
| **ten state colours with no home** | **24** | **no** |

`UNCHECKED` (5), `VOID` (3), `ROW_SELECTED` (3), `GROUND_HELD` (3), `DIM` (3), `PANEL_BG` (4), `ROW_BG`
(2), `WORLD_HELD`, `THUMB_SLOT`, `SLOT_BG`. So #210 needed a **gooey request**, not just a re-pin
exception. Filed as **bevy-coven/bevy_gooey#37 (G-33)**, added to UI-00.

## Enhancement: K reveals the overlay, in the crate
Files: `crates/bevy_cauldron/src/ui.rs`, `tests/the_shortcut_overlay_answers_to_k.rs` (new).
`EditorUiPlugin` installs `ShortcutBadges { hold: Reveal::Key(KeyCode::KeyK), delay_seconds: 0.0 }`.
Zero delay because `K` is not a chord prefix: the 0.3 s default exists to stop a modifier flashing the
screen on the way to `Ctrl+S`, and nothing types `K` to reach a chord.

**Proof, measured:** with the resource removed both tests fail —
`holding K must raise 'BadgesShown', or a host cannot tell the overlay is up` and
`the editor must install 'ShortcutBadges', or holding K reveals nothing` — and with it both pass in
0.40 s. That is the chip/reveal half of #235; `badges.rs` keeps its leader-line layout engine.

## The five decisions, recorded on their issues
| # | Decision | State |
|---|---|---|
| #235 | chip-only slice, keep the layout engine | **slice landed**; clip owed |
| #210 | file G-33, then the theme in one clean slice | G-33 filed; issue needs #37 |
| #152, #201 | exceptions granted for the fixture text and the `assert_ne!` | next pass |
| #112, #113 | serve clef-flash on big:8093 | **cannot be served from here**: `:8093` refuses, `~/clef_serve` is an empty venv, no `clef-flash` on PATH, GPU idle (`tenant=none, vram=7658/24576`) |
| #193 | leave to bevy_elements' loop | unchanged |

## Review
`reviewer` not spawned. Self-review: (1) *is the reveal really gooey's now* — the resource is asserted
in a test and the flag rises and falls with `K`; (2) *did I weaken anything* — no test file edited except
the one `mod` line, and no assertion loosened; (3) *is G-33 honest* — it says "we cannot express these",
not "you should design it this way", and offers the `problems()` check; (4) *could the decisions mislead
the next pass* — each is on its own issue with the limits restated; (5) *what did I get wrong* — I first
reported the model as servable, then found `~/clef_serve` empty.

## Test diffs
New `tests/the_shortcut_overlay_answers_to_k.rs` + its one `mod` line (owner exception, 2026-10-29).
No existing assertion touched.

## Release
None: `bevy_cauldron` is `publish = false`; no packaged crate changed.

## Next focus
#152's fixture text and #201's `assert_ne!`, both now granted.
