# needs-decision

Structural decisions this loop found but must not make. Each file is one decision: the options,
what each costs, and a recommendation. The loop moves to the next item while one waits here.

Two came out of the 2026-10-20 technical review
(`/Users/ladvien/sanctuary/docs/design/emerge-mapper-review.md`). Both are **the owner's call**.

- `the-runtime-half.md` — retire `bevy_emerge`, or let the game adopt it.
- `the-placement-stack.md` — delete `placement/`, or finish it.
- `the-selection-rule.md` (2026-10-24) — one item per iteration cannot reach the trunk: 11 of 135
  verbs have an agent path, and none of them generate. Does trunk work get a series exemption?
