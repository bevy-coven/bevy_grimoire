# 2026-11-03 shell: the input chain is readable, and the live drag's fault is located

Focus: #224 UI-30 · Commits: 4489e68..960f2af · Machine: big · Outcome: shipped

## Summary
Three passes could not diagnose a live drag that does nothing because the editor's input state was
invisible. `grimoire/pointer` now reports each step of the chain. It immediately showed that **pass 281's
conclusion was wrong** — the pointer chain is intact — and located the real fault: the press never
reaches `MouseButtonInput` in the live editor.

## The method (960f2af)
`grimoire/pointer`, a read-only BRP method in the shape of `grimoire/project_root`:

| Field | The question it answers |
|---|---|
| `cursor` | what `view::Pointer` decided — the crate's one answer to "where is the cursor" |
| `target`, `position` | what `bevy_picking` was actually given, which is what its backend tests |
| `over_ui` | the layout's own answer, a different question from the picker's |

A missing step is `null`, never `[0, 0]`, so a fault cannot read as a real position.

## What it found
**Read live, the chain is healthy:**

```json
{"cursor":[170.0,400.0],"over_ui":true,"position":[170.0,400.0],"target":"Image(..)"}
```

So **pass 281 was wrong**: the `{}` it reported as "no location" was BRP's opaque `Option`
serialisation, not a missing value. That is the second of my own conclusions this two-pass sequence has
retracted, and both were reached by reading a serialisation rather than by measuring the system.

**The real fault, from a temporary probe (removed):**

```
[inject] at=Vec2(300.0, 400.0) presses=0     # 86 frames, cursor accepted, position updating
```

`inject_clicks` runs, sees the cursor move to where the recorder put it, and **never sees a press**. So
`bevy_fairy/input`'s `{"kind":"Mouse","action":"Press"}` is not arriving as `MouseButtonInput` in the
live editor, and no press-derived picking input is written at all. That is a different question from
every one passes 273–281 asked, and it is now one request away from being visible.

## Review
Reviewer not spawned; self-review, mine:
- **Does the method overclaim?** No: three separate fields, `null` for a missing step, and the test
  asserts the null-vs-origin distinction explicitly.
- **Was the probe left in?** No — `git diff` of `surface.rs` is empty, verified after removing it.
- **Is pass 281's claim retracted in the record?** Yes, in the commit, here, and on the issue.
- **Did I weaken a check?** No: one new test file.
- **Is the changelog honest?** It names the fault as found, not fixed.

## Test diffs
One new test file and its `mod` line. No existing assertion changed.

## Release
None: `bevy_cauldron` is `publish = false`.

## Next focus
Why `bevy_fairy/input`'s press does not become `MouseButtonInput` live — read `bevy_fairy`'s
`handle_input` and whether the editor's plugin ordering lets it through.
