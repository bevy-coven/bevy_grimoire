# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 09:14Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 183 | 2026-11-01T02:03Z | 4 | 0 | 6 | bevy_grimoire_core-v0.2.1 | 2 | 0 | demo? |
| 184 | 2026-11-01T02:45Z | 3 | 0 | 6 | - | 2 | 0 | demo? |
| 185 | 2026-11-01T03:32Z | 6 | 0 | 10 | bevy_grimoire-v0.1.2,bevy_grimoire_anim-v0.1.2,bevy_grimoire_core-v0.3.0 | 3 | 0 | demo? |
| 186 | 2026-11-01T04:01Z | 3 | 0 | 0 | - | 2 | 0 | - |
| 187 | 2026-11-01T04:29Z | 3 | 0 | 5 | - | 2 | 0 | demo? |
| 188 | 2026-11-01T05:10Z | 4 | 0 | 9 | bevy_grimoire_core-v0.3.1 | 2 | 0 | demo? |
| 189 | 2026-11-01T05:50Z | 4 | 0 | 9 | bevy_grimoire_core-v0.3.2 | 2 | 0 | demo? |
| 190 | 2026-11-01T06:29Z | 4 | 0 | 7 | bevy_grimoire_core-v0.3.3 | 2 | 0 | demo? |
| 191 | 2026-11-01T07:24Z | 4 | 0 | 10 | bevy_grimoire_core-v0.3.4 | 2 | 0 | demo? |
| 192 | 2026-11-01T08:09Z | 5 | 0 | 5 | bevy_grimoire-v0.2.0,bevy_grimoire_anim-v0.1.3,bevy_grimoire_core-v0.4.0 | 3 | 0 | demo? |
| 193 | 2026-11-01T08:44Z | 3 | 0 | 3 | bevy_grimoire-v0.2.1 | 3 | 0 | demo? |
| 194 | 2026-11-01T09:14Z | 3 | 0 | 2 | bevy_grimoire-v0.3.0 | 3 | 0 | demo? |

## The last 12 passes

```text
12 pass(es), 2819 steps, 7.6 h of pass time

category                       hours  share  steps  mean s
model                           2.80  36.9%   1282     7.9
wait: gate                      1.80  23.7%     29   223.0
wait: other                     0.70   9.2%     24   104.9
remote command                  0.60   7.9%    209    10.3
wait: review                    0.43   5.7%     11   142.3
wait: tests                     0.21   2.7%      7   106.9
cargo test                      0.15   1.9%     43    12.3
push gate                       0.13   1.7%     23    20.3
wait: release                   0.13   1.7%      3   155.5
phase mark                      0.13   1.7%     83     5.5
remote wait                     0.13   1.6%     12    37.5
wait: build                     0.12   1.6%      7    64.3
loop tools                      0.10   1.3%     62     5.6
github                          0.04   0.5%     45     3.0
cargo build                     0.03   0.4%      4    30.4

phase                          hours  share  steps  mean s
build                           1.94  25.5%    824     8.5
gate                            1.86  24.5%    224    30.0
choose                          1.08  14.3%    611     6.4
prove                           0.94  12.4%    320    10.6
ship                            0.63   8.3%    181    12.5
research                        0.55   7.3%    309     6.4
orient                          0.26   3.5%    201     4.7
log                             0.17   2.3%     72     8.7
release                         0.15   1.9%     62     8.5
bugs                            0.01   0.1%      4     5.5

slowest single steps
  pass  186     592s  tool  wait: gate           Waiting for gate to finish
  pass  188     541s  tool  wait: gate           Waiting for gate/release/accept result
  pass  183     432s  tool  wait: gate           Waiting for the gate result
  pass  185     408s  tool  wait: gate           Waiting for gate and release to finish
  pass  189     303s  tool  wait: review         Waiting for reviewer and the after run
  pass  192     293s  tool  wait: gate           Waiting for gate output
  pass  192     292s  tool  wait: gate           Waiting for gate output
  pass  190     291s  tool  wait: gate           Waiting for gate
  pass  191     290s  tool  wait: gate           Waiting for the gate
  pass  190     286s  tool  wait: other          Waiting for after-run on big
  pass  188     285s  bg    subagents            OplogReview completed: OplogReview
  pass  184     280s  bg    subagents            NotesViewReview completed: NotesViewReview
  pass  193     278s  tool  wait: gate           Waiting for the gate on big
  pass  189     274s  tool  wait: gate           Waiting for the gate result
  pass  187     271s  bg    subagents            ChecksViewReview-2 completed: ChecksViewReview-2
```

## Every pass recorded

```text
174 pass(es), 13296 steps, 87.6 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.31  36.9%    191   609.0
wait: tests                    18.92  21.6%     97   702.2
wait: gate                     13.63  15.6%     88   557.5
model                          10.60  12.1%   6223     6.1
cargo test                      2.59   3.0%    312    29.9
wait: build                     2.42   2.8%     19   458.4
remote command                  1.27   1.5%    397    11.5
push gate                       0.99   1.1%    301    11.8
wait: review                    0.92   1.0%     22   149.9
wait: clip                      0.66   0.8%     13   183.7
wait: release                   0.54   0.6%      5   389.2
shell                           0.51   0.6%   2578     0.7
cargo build                     0.43   0.5%     46    33.9
remote wait                     0.39   0.4%     45    30.8
loop tools                      0.38   0.4%    426     3.2

phase                          hours  share  steps  mean s
orient                         77.57  88.5%   9598    29.1
gate                            2.42   2.8%    301    28.9
build                           2.27   2.6%    973     8.4
prove                           1.97   2.2%    708    10.0
choose                          1.32   1.5%    794     6.0
ship                            0.94   1.1%    306    11.1
research                        0.60   0.7%    359     6.0
release                         0.32   0.4%    123     9.4
log                             0.20   0.2%     79     9.0
bugs                            0.01   0.0%     13     4.1

slowest single steps
  pass  130    1801s  tool  wait: other          {}
  pass  129    1801s  tool  wait: other          {}
  pass  128    1801s  tool  wait: other          {}
  pass  165    1801s  tool  wait: tests          Waiting for the slice 3 test build
  pass  132    1800s  tool  wait: other          {}
  pass  131    1800s  tool  wait: other          {}
  pass  136    1800s  tool  wait: other          {}
  pass   21    1800s  tool  wait: gate           Waiting for the workspace gate
  pass  128    1800s  tool  wait: other          {}
  pass  133    1800s  tool  wait: other          {}
  pass  152    1800s  tool  wait: tests          Waiting for slice 3 test build
  pass    5    1800s  tool  wait: tests          Waiting for test build to finish
  pass   15    1800s  tool  wait: tests          Waiting for the workspace test run
  pass   19    1800s  tool  wait: tests          Waiting on workspace tests
  pass   20    1800s  tool  wait: tests          Waiting on the full test suite
```

