# Pass 8 (mac_air clone), 2026-11-08: the release that failed, the tool light that slides, the hand-back answered in the block

Focus: the editor's UI on bevy_gooey · Commits: ed8565e1..the pass's last · Machine: mac_air edits, big builds and tests · Outcome: shipped (see the log line for what is owed)

Log number 481 (pass 8 of this clone).

## First job: the audit's `release`

The 0.5.1 release run (`release-macos`, run 42470951199) **failed**: the first timestamped `codesign` got "A timestamp was expected but was not found" from Apple's timestamp server, 15 s in, and nothing was built to upload. Nothing about the bundle was wrong. `build_app.sh` now retries a timestamped codesign up to four times, 20 s apart, on that message and on "timestamp service is not available" only; any other codesign failure is a failure at once. `tools/release/macos/test_stamped_codesign.py` (3 tests, a stub `codesign` on PATH) holds both halves. The workflow was not re-run: the fix goes forward in the next tag (0.5.2), which carries 0.5.1's content.

## The cluster

| item | state |
|---|---|
| #336 UI-R06 the shell moves | **Slice 1 committed, left open.** The bevy_gooey pin moved to `04a3cdf` (v0.43.1: G-61, the sliding selection indicator, and the release). The tool modes are already one `toggle_group`, so the tool light now slides from the armed tool to the new one on `motion.snappy`, hidden at rest. `the_toolbars_tool_light_slides_to_the_armed_tool` (2 tests, motion put back on a manual 60 Hz clock); two mutations (reduced motion always on; the group never following the armed tool) fail it. Clip `2026-11-08-ui-r06-tool-light`. Still waiting on gooey #69 (split panes that open and close, a sidebar that peeks), #70 (tab reorder) and #71 (menus from their anchor). |
| #340 UI-60b the plan and the hand-back inline | **Slice 1 committed, left open.** **Answer hand-backs here** (a switch in the AGENT block, off by default): with it on and the block open, `grimoire/present` asks in a card under the HAND-BACK line (what was made, what the person said, what the agent assumed, where to look; Accept, Undo N operations, Dismiss) instead of the dialog, and a block closed on a waiting card hands the question to the dialog. It is a switch and not the default because the existing presentation tests pin the dialog; they pass unchanged. 6 tests. Clip `2026-11-08-ui-60b-hand-back`. **The agent's plan is not built** (the editor has no plan an agent declares). |
| the eight standing GIFs and `editor.png` | Retaken after the tool light (all passed `check.py`; `review-labels` on take 2; `place-and-undo` on take 1 with last pass's tighter framing) and looked at. The hand-back card is a second visible change since then: see the log line for `owed demos`. |
| #337, #210, #237 to #245, #247, #251, #254, #257, #258, #260 | Not taken: the same reasons as pass 7's report (a person has to run the update; #210 waits on the owner's word about two protected tests; the rest are rebuilds larger than a slice or wait on #210). |

## What this pass learned

1. **A release that dies on infrastructure is read from the run, not assumed to be the code.** The log named the call and the message in one read; the fix is a retry that gives up at once on anything else, so a real signing fault is not hidden behind waiting.
2. **A re-pin is avoided by making the new behaviour a choice, not the default.** The inline hand-back first replaced the dialog whenever the block was open; that would have changed what `an_agent_presents_its_result` and my own pass-7 test pin, and a pass may not. A switch kept every existing assertion as it was and gave the owner the decision.
3. **A control that is below the fold is not on screen.** The first card put its buttons after four text rows; in the capture the panel ended above them and the click landed on nothing. The answers now come first, right under the line that says a question is waiting.

## Evidence

- `test_stamped_codesign.py` 3 tests; `the_toolbars_tool_light_slides_to_the_armed_tool` 2 tests (mutations: 2 of 2 caught); `the_hand_back_can_be_answered_in_the_agent_block` 6 tests, with the 17 related presentation, block and pause tests (23 in all) passing; clippy clean on every target.
- Gates: pass 8's first push (the codesign retry) 99 s, the checkpoint push (the pin move and the tool light) 320 s.
- Every clip and contact sheet looked at: the tool light, the eight GIFs and editor.png, the hand-back clip (two recordings: the first showed the card's answers below the fold).

## Not done, said plainly

- The agent's plan (#340), and #336's remaining parts. #337's last step is a person's.
- See the log line for what could not be finished if big was down.

## Cleanup

Worktree `~/p8-wt` and `/tmp/p8_*` on big were this pass's and are removed by `tools/loop/remote/clean.sh`; the `p8-*` branches in the loop clone and the local `p6dbg` and `p7dbg` are listed for the owner.
