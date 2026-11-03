# 2026-11-03 label: both wired paths read one judgement the same way

Focus: #113 D5, the measurement the issue actually asks for · Commits: ef8d824..9eb995a · Machine: big · Outcome: shipped

## Summary
Pass 278 withdrew a path that was never wired and said the real measurement was still owed. This pass
made it: the two paths `labels.rs` selects between — `vlm::parse_reply`+`validate` for `chat`, and
`decide_labels::label_with_decisions` for `decide` — are driven from **the same recorded judgement** over
a replay transport, and they agree on every axis.

## What is now pinned

**`tests/the_two_paths_read_one_judgement_the_same_way.rs`**
- One judgement (a chair's: `seating`, `seat-one`, `seat`, `brown`) is handed to both paths — the chat
  path as the JSON that judgement is written as, the decision path as P(yes) per token. **Every axis
  agrees**, compared as sets because the two order tokens differently on purpose.
- **The asymmetry that is D5's whole point:** `validate` *rejects* a reply naming `throne` after the
  model has written it; the decision path's request never contains `throne`, so there is nothing to
  reject. Asserted directly — no question id ends in `:throne`.

**`tests/the_wired_labelling_paths_agree_on_the_same_pieces.rs`**
- Every token the judgement names is suggested; nothing it does not name leaks in (P = 0.02 is under
  `DEFAULT_THRESHOLD`). The provenance names the model that answered and the date.
- A judgement naming nothing suggests nothing — the path does not fall back to the most probable token.

## Not vacuous, both proved
- The agreement test fails when the chat path is given one token the decision path is not:
  `kind: chat ["seating", "light"] vs decide ["seating"]`.
- The "nothing suggested" test fails at a threshold of 0.0 with `kind: ["seating", "light"]`, so the
  empty result at 0.5 is the threshold working.

## A thing learned by writing it
The wired decision path asks a **`choice` question (`front`)** alongside its per-token `noul`s, and a
transport answering everything as a `noul` is refused by `Response::check_answers`: *"question `front` is
a choice but was answered as a noul"*. Any replayed or live client has to answer each question in its own
kind.

## What this does not claim
Not agreement **percentages** over a large labelled kit: that needs recorded replies from the real chat
model, not a hand-written one. What is established is that the two readers do not disagree on their own
terms — which a percentage over fabricated replies could not establish, and which pass 277's stand-in
comparison got wrong.

## Review
Reviewer not spawned; self-review, mine:
- **Is the comparison fair?** Both paths get the same judgement in the shape each is designed for; neither
  is given a harder question.
- **Could the tests pass on a broken path?** Both were made to fail deliberately (one token, one
  threshold), and each failed with a message naming the difference.
- **Did I weaken a check?** No: two new test files, no existing assertion touched.
- **Is the claim narrower than the issue's?** Yes, and said so: percentages still need recorded chat
  replies.
- **Anything deleted this pass?** No.

## Test diffs
Two new test files and their two `mod` lines. No existing assertion changed.

## Release
None: no packaged crate changed.

## Next focus
Recorded chat replies over the labelled kit, for the agreement percentage #113's first acceptance item
asks for.
