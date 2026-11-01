# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 06:29Z.

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

## The last 12 passes

```text
12 pass(es), 3039 steps, 7.8 h of pass time

category                       hours  share  steps  mean s
model                           2.90  37.3%   1390     7.5
wait: gate                      1.61  20.8%     25   232.0
remote command                  0.68   8.7%    201    12.2
wait: review                    0.54   7.0%     12   163.1
wait: other                     0.47   6.1%     16   106.8
wait: tests                     0.40   5.1%     13   109.4
cargo test                      0.21   2.7%     53    14.4
push gate                       0.17   2.2%     31    19.6
wait: build                     0.17   2.2%      8    75.9
phase mark                      0.15   1.9%     86     6.2
loop tools                      0.12   1.5%     71     5.9
remote wait                     0.09   1.1%     12    26.5
cargo build                     0.05   0.6%      3    58.4
wait: release                   0.05   0.6%      2    82.8
github                          0.03   0.4%     41     2.7

phase                          hours  share  steps  mean s
build                           2.05  26.5%    893     8.3
gate                            1.99  25.7%    220    32.6
prove                           1.58  20.3%    604     9.4
choose                          0.67   8.7%    507     4.8
ship                            0.62   8.0%    248     9.1
orient                          0.31   3.9%    242     4.5
release                         0.24   3.1%     99     8.9
research                        0.17   2.1%    168     3.6
log                             0.11   1.4%     37    10.6
bugs                            0.01   0.2%     13     4.1

slowest single steps
  pass  176     641s  tool  wait: gate           Waiting for gate result
  pass  186     592s  tool  wait: gate           Waiting for gate to finish
  pass  188     541s  tool  wait: gate           Waiting for gate/release/accept result
  pass  183     432s  tool  wait: gate           Waiting for the gate result
  pass  185     408s  tool  wait: gate           Waiting for gate and release to finish
  pass  174     340s  tool  wait: review         Waiting for reviewer result
  pass  176     331s  tool  wait: tests          Waiting for core test results or big build
  pass  177     320s  tool  wait: other          Waiting for the after-run
  pass  189     303s  tool  wait: review         Waiting for reviewer and the after run
  pass  190     291s  tool  wait: gate           Waiting for gate
  pass  190     286s  tool  wait: other          Waiting for after-run on big
  pass  188     285s  bg    subagents            OplogReview completed: OplogReview
  pass  184     280s  bg    subagents            NotesViewReview completed: NotesViewReview
  pass  189     274s  tool  wait: gate           Waiting for the gate result
  pass  187     271s  bg    subagents            ChecksViewReview-2 completed: ChecksViewReview-2
```

## Every pass recorded

```text
170 pass(es), 12361 steps, 84.9 h of pass time

category                       hours  share  steps  mean s
wait: other                    31.94  37.6%    180   638.7
wait: tests                    18.90  22.3%     96   708.7
wait: gate                     12.97  15.3%     78   598.8
model                           9.60  11.3%   5797     6.0
cargo test                      2.55   3.0%    301    30.5
wait: build                     2.40   2.8%     18   480.1
remote command                  1.16   1.4%    321    13.1
push gate                       0.95   1.1%    292    11.7
wait: review                    0.73   0.9%     17   155.6
wait: clip                      0.66   0.8%     12   198.0
shell                           0.50   0.6%   2484     0.7
wait: release                   0.46   0.5%      3   546.9
cargo build                     0.42   0.5%     43    34.8
loop tools                      0.36   0.4%    408     3.1
remote wait                     0.32   0.4%     40    28.6

phase                          hours  share  steps  mean s
orient                         77.48  91.2%   9535    29.3
build                           2.05   2.4%    893     8.3
gate                            1.99   2.3%    220    32.6
prove                           1.58   1.9%    604     9.4
choose                          0.67   0.8%    507     4.8
ship                            0.62   0.7%    248     9.1
release                         0.24   0.3%     99     8.9
research                        0.17   0.2%    168     3.6
log                             0.11   0.1%     37    10.6
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

