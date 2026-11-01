# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 08:44Z.

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
| 192 | 2026-11-01T08:09Z | 5 | 0 | 5 | bevy_grimoire-v0.2.0,bevy_grimoire_anim-v0.1.3,bevy_grimoire_core-v0.4.0 | 3 | 0 | demo? |
| 193 | 2026-11-01T08:44Z | 3 | 0 | 3 | bevy_grimoire-v0.2.1 | 3 | 0 | demo? |

## The last 12 passes

```text
12 pass(es), 2939 steps, 7.8 h of pass time

category                       hours  share  steps  mean s
model                           2.89  37.1%   1339     7.8
wait: gate                      1.71  21.9%     28   219.4
wait: other                     0.81  10.3%     26   111.6
remote command                  0.59   7.5%    207    10.2
wait: review                    0.52   6.6%     12   155.3
wait: tests                     0.19   2.4%      6   112.7
push gate                       0.15   1.9%     26    20.8
phase mark                      0.13   1.7%     85     5.6
wait: release                   0.13   1.7%      4   117.8
remote wait                     0.13   1.6%     12    37.5
cargo test                      0.12   1.6%     41    10.7
loop tools                      0.12   1.5%     66     6.4
wait: build                     0.11   1.4%      7    54.5
cargo build                     0.05   0.6%      5    36.3
github                          0.04   0.5%     44     3.1

phase                          hours  share  steps  mean s
build                           2.01  25.7%    859     8.4
gate                            1.86  23.9%    224    30.0
choose                          1.08  13.9%    621     6.3
prove                           0.81  10.4%    275    10.6
ship                            0.76   9.7%    237    11.5
research                        0.54   6.9%    321     6.0
release                         0.32   4.1%    123     9.4
orient                          0.26   3.4%    202     4.7
log                             0.14   1.8%     54     9.5
bugs                            0.01   0.2%     13     4.1

slowest single steps
  pass  186     592s  tool  wait: gate           Waiting for gate to finish
  pass  188     541s  tool  wait: gate           Waiting for gate/release/accept result
  pass  183     432s  tool  wait: gate           Waiting for the gate result
  pass  185     408s  tool  wait: gate           Waiting for gate and release to finish
  pass  177     320s  tool  wait: other          Waiting for the after-run
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
```

## Every pass recorded

```text
173 pass(es), 13128 steps, 87.1 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.31  37.1%    191   609.0
wait: tests                    18.90  21.7%     96   708.7
wait: gate                     13.48  15.5%     86   564.2
model                          10.44  12.0%   6142     6.1
cargo test                      2.56   2.9%    308    29.9
wait: build                     2.40   2.8%     18   480.1
remote command                  1.23   1.4%    383    11.6
push gate                       0.98   1.1%    298    11.8
wait: review                    0.88   1.0%     21   150.7
wait: clip                      0.66   0.8%     13   183.7
wait: release                   0.54   0.6%      5   389.2
shell                           0.50   0.6%   2558     0.7
cargo build                     0.43   0.5%     46    33.9
remote wait                     0.39   0.4%     45    30.8
loop tools                      0.37   0.4%    424     3.2

phase                          hours  share  steps  mean s
orient                         77.56  89.0%   9590    29.1
gate                            2.42   2.8%    301    28.9
build                           2.23   2.6%    965     8.3
prove                           1.65   1.9%    631     9.4
choose                          1.30   1.5%    776     6.0
ship                            0.94   1.1%    306    11.1
research                        0.54   0.6%    321     6.0
release                         0.32   0.4%    123     9.4
log                             0.17   0.2%     61     9.9
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

