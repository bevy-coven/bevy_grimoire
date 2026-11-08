# Pass 6 (mac_air clone), 2026-11-08: the agent can be paused, the outliner has a budget, the Invoke Agent clip is recorded

Focus: the editor's UI on bevy_gooey, plus the debts of pass 4 and 5 · Commits: 9f79df01..878942fd and the docs and release that follow · Machine: mac_air edits, big builds and tests · Outcome: shipped

The same number as bevy_emerge_loop's pass 479 (both write to `docs/loop/log.md`).

## The cluster

| item | state |
|---|---|
| owed: the Invoke Agent clip (CHANGELOG `invoke-agent`) | **Recorded and in the line.** The earlier takes failed for two reasons that were mine: the tools server must be named `bevy_cauldron_mcp` beside the editor or the agent self-test fails, and the empty pane with the button only appears when the agent is chosen in the **user's** settings. Clip `2026-11-08-invoke-agent`. |
| owed: `place-and-undo` | **Retaken, passes `check.py` on take 1**, with the other seven GIFs and `editor.png`. Pass 5's eight takes failed `check.py` (3.0 % against 5 % at the place) and this pass's passed on take 1 on the same code. Load is not the whole explanation (four of pass 5's takes ran alone), and the cause is not understood; the check is a race between the placed mesh and its 0.2 s window, and it went the right way this time. |
| #246 UI-60 the Familiar | **Slice committed, left open.** View > Pause agent writes: while on, `grimoire/operation_apply` and `grimoire/operation_undo` refuse every write with the reason (in the answer and on the status line as a problem), nothing changes or is logged, a `dry_run` preview still answers, a person's own edits are untouched, nothing is saved. Clip `2026-11-08-ui-60-agent-pause`. Not built: the panel (connection, current operation, plan, hand-back with Accept, Undo, Dismiss). |
| #259 UI-93 the performance budget | **Slice committed, left open.** The outliner took a fingerprint of every piece on every frame; it now looks only when the map, the library or the search moved. Held by counts (two thousand pieces: built once, 120 settled frames read no piece and rebuild nothing, an edit is one rebuild, the widget is 68 entities). The acceptance's "2 ms per frame" is not met as written: a wall-clock assertion in a gate that runs under load is a flake (#322, #195). |
| #339 (new, bug) a project-level agent setting opens a plain shell | **Fixed (0.5.0), closed with the fix.** Two fixes reasoned from the code failed in a real editor and were reverted; a log of the first frames found the cause (below); the third fix is verified by the same probe: an Invoke Agent button at 18 s and at 58 s where a shell had been. |
| #237, #238, #239, #240 to #243, #247, #258 | Not taken this pass; their comments from pass 5 stand. |

## What this pass learned

1. **The first fixes of #339 were reasoned from the code and did not change the real editor; a log did.** The headless harness opens the project before its first frame, so no test shows the gap; only a real editor under Xvfb does. Two attempts (wait in `take_the_terminal_settings`, wait in `pump_the_terminal`) each passed 83 tests and each left the shell open; they were reverted in their own commit rather than shipped. Then `warn!` lines in the first eight frames said: frame 0, the pane is drawn and `EditorSettings` is taken (Off) with no `Project`; frame 1, the first shell spawns, still no `Project`; frame 2, the `Project` exists and the agent the project chose (Custom) arrives. The root is pointed at before frame 0 and the `Project` is built in the editor's first frame, so `pump_the_terminal` now waits for a root with no `Project` yet and then for the look at the project's file. Cost of getting there: four builds of about six minutes; the log lines would have been the first move.
2. **A test that counts the wrong thing misses the defect.** The budget test first counted rebuilds, and the original per-frame cost (reading every piece to compare a fingerprint) builds nothing, so the mutation that puts it back passed. The test now also counts how many times every piece was read.
3. **A system with more than sixteen parameters is not a system.** `operation_apply` was at sixteen, so the pause switch went into one `SystemParam` with the question a person has open (`Gates`).

## Evidence

- `the_agent_can_be_paused`: 2 tests. Three mutations, each failing the intended test: a paused agent still writes, a paused agent can still undo, a preview is refused too.
- `the_outliner_does_no_work_on_a_settled_frame`: 2 tests. One mutation (the outliner reads every piece every frame) fails the intended test.
- Gate: passed in 279 s on the push that landed.
- Every clip and contact sheet looked at (the pause clip, the Invoke Agent clip, eight GIFs, `editor.png`).

## Release

0.4.9 (the pause switch, the outliner budget, the clips) and then 0.5.0 (the fix of #339), each tagged after its gate.

## Not done, said plainly

- #246 has no panel; #259's millisecond line is not met; #339 has no regression test (nothing headless can be two frames early) and its proof is the probe.
- #210 UI-02 waits on the owner's word about two protected tests; #336 UR06 waits on gooey #69, #70 and #71.

## Cleanup

Worktree `~/p6-wt` and `/tmp/p6_*` were this pass's and are removed; the `p6-*` branches in the loop clone are listed for the owner.
