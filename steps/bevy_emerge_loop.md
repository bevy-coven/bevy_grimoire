# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 07:24Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 0 | 2026-11-01T00:32Z | 47 | 0 | 53 | bevy_grimoire_solver-v0.1.1 | 16 | 3 | release(bevy_grimoire bevy_grimoire_core) |
| 183 | 2026-11-01T02:03Z | 4 | 0 | 6 | bevy_grimoire_core-v0.2.1 | 2 | 0 | demo? |
| 184 | 2026-11-01T02:45Z | 3 | 0 | 6 | - | 2 | 0 | demo? |
| 185 | 2026-11-01T03:32Z | 6 | 0 | 10 | bevy_grimoire-v0.1.2,bevy_grimoire_anim-v0.1.2,bevy_grimoire_core-v0.3.0 | 3 | 0 | demo? |
| 186 | 2026-11-01T04:01Z | 3 | 0 | 0 | - | 2 | 0 | - |
| 187 | 2026-11-01T04:29Z | 3 | 0 | 5 | - | 2 | 0 | demo? |
| 188 | 2026-11-01T05:10Z | 4 | 0 | 9 | bevy_grimoire_core-v0.3.1 | 2 | 0 | demo? |
| 189 | 2026-11-01T05:50Z | 4 | 0 | 9 | bevy_grimoire_core-v0.3.2 | 2 | 0 | demo? |
| 190 | 2026-11-01T06:29Z | 4 | 0 | 7 | bevy_grimoire_core-v0.3.3 | 2 | 0 | demo? |
| 191 | 2026-11-01T07:24Z | 4 | 0 | 10 | bevy_grimoire_core-v0.3.4 | 2 | 0 | demo? |

## The last 12 passes

```text
12 pass(es), 3098 steps, 8.1 h of pass time

category                       hours  share  steps  mean s
model                           3.05  37.7%   1424     7.7
wait: gate                      1.70  21.1%     27   227.0
remote command                  0.65   8.0%    201    11.6
wait: other                     0.62   7.6%     21   105.7
wait: review                    0.52   6.4%     12   154.8
wait: tests                     0.40   4.9%     13   109.4
cargo test                      0.17   2.1%     52    12.0
wait: build                     0.17   2.1%      8    75.9
push gate                       0.16   2.0%     27    21.9
phase mark                      0.16   2.0%     86     6.6
loop tools                      0.11   1.4%     71     5.8
remote wait                     0.09   1.1%     12    27.7
cargo build                     0.07   0.8%      6    39.5
wait: release                   0.05   0.6%      2    82.8
github                          0.03   0.4%     43     2.8

phase                          hours  share  steps  mean s
gate                            2.24  27.8%    284    28.4
build                           1.99  24.6%    839     8.5
choose                          1.24  15.4%    742     6.0
prove                           1.17  14.5%    422    10.0
ship                            0.63   7.7%    255     8.8
orient                          0.25   3.1%    220     4.0
release                         0.24   3.0%     99     8.9
research                        0.17   2.1%    168     3.6
log                             0.14   1.7%     45    11.1
bugs                            0.01   0.2%     13     4.1

slowest single steps
  pass  176     641s  tool  wait: gate           Waiting for gate result
  pass  186     592s  tool  wait: gate           Waiting for gate to finish
  pass  188     541s  tool  wait: gate           Waiting for gate/release/accept result
  pass  183     432s  tool  wait: gate           Waiting for the gate result
  pass  185     408s  tool  wait: gate           Waiting for gate and release to finish
  pass  176     331s  tool  wait: tests          Waiting for core test results or big build
  pass  177     320s  tool  wait: other          Waiting for the after-run
  pass  189     303s  tool  wait: review         Waiting for reviewer and the after run
  pass  190     291s  tool  wait: gate           Waiting for gate
  pass  191     290s  tool  wait: gate           Waiting for the gate
  pass  190     286s  tool  wait: other          Waiting for after-run on big
  pass  188     285s  bg    subagents            OplogReview completed: OplogReview
  pass  184     280s  bg    subagents            NotesViewReview completed: NotesViewReview
  pass  189     274s  tool  wait: gate           Waiting for the gate result
  pass  187     271s  bg    subagents            ChecksViewReview-2 completed: ChecksViewReview-2
```

## Every pass recorded

```text
171 pass(es), 12695 steps, 85.8 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.08  37.4%    185   624.3
wait: tests                    18.90  22.0%     96   708.7
wait: gate                     13.13  15.3%     81   583.5
model                          10.02  11.7%   5950     6.1
cargo test                      2.56   3.0%    307    30.0
wait: build                     2.40   2.8%     18   480.1
remote command                  1.18   1.4%    338    12.5
push gate                       0.96   1.1%    293    11.9
wait: review                    0.80   0.9%     18   160.3
wait: clip                      0.66   0.8%     12   198.0
shell                           0.50   0.6%   2518     0.7
wait: release                   0.46   0.5%      3   546.9
cargo build                     0.43   0.5%     46    33.9
loop tools                      0.36   0.4%    414     3.2
remote wait                     0.33   0.4%     41    29.3

phase                          hours  share  steps  mean s
orient                         77.49  90.3%   9544    29.2
gate                            2.24   2.6%    284    28.4
build                           2.05   2.4%    893     8.3
prove                           1.58   1.8%    604     9.4
choose                          1.24   1.4%    742     6.0
ship                            0.65   0.8%    263     8.9
release                         0.24   0.3%     99     8.9
research                        0.17   0.2%    168     3.6
log                             0.14   0.2%     45    11.1
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

