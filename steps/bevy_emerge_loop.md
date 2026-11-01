# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 05:50Z.

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

## The last 12 passes

```text
12 pass(es), 2808 steps, 7.2 h of pass time

category                       hours  share  steps  mean s
model                           2.78  38.4%   1295     7.7
wait: gate                      1.47  20.3%     21   251.7
remote command                  0.64   8.8%    182    12.6
wait: review                    0.51   7.0%     11   166.7
wait: tests                     0.40   5.5%     13   109.4
wait: other                     0.40   5.5%     15    94.8
cargo test                      0.21   2.9%     49    15.3
push gate                       0.15   2.1%     29    18.8
wait: build                     0.14   1.9%      7    71.4
phase mark                      0.12   1.7%     81     5.5
loop tools                      0.11   1.5%     64     6.3
remote wait                     0.07   0.9%     10    24.5
cargo build                     0.05   0.7%      3    58.4
wait: release                   0.05   0.6%      2    82.8
github                          0.03   0.4%     41     2.7

phase                          hours  share  steps  mean s
gate                            1.83  25.2%    203    32.4
build                           1.72  23.7%    710     8.7
prove                           1.58  21.8%    604     9.4
choose                          0.67   9.3%    507     4.8
ship                            0.57   7.9%    229     9.0
orient                          0.38   5.2%    237     5.7
release                         0.24   3.4%     99     8.9
research                        0.17   2.3%    168     3.6
log                             0.07   1.0%     31     8.6
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
  pass  173     288s  model model                
  pass  188     285s  bg    subagents            OplogReview completed: OplogReview
  pass  184     280s  bg    subagents            NotesViewReview completed: NotesViewReview
  pass  189     274s  tool  wait: gate           Waiting for the gate result
  pass  187     271s  bg    subagents            ChecksViewReview-2 completed: ChecksViewReview-2
  pass  189     256s  tool  wait: gate           Waiting for the gate result
```

## Every pass recorded

```text
169 pass(es), 12099 steps, 84.3 h of pass time

category                       hours  share  steps  mean s
wait: other                    31.86  37.8%    179   640.7
wait: tests                    18.90  22.4%     96   708.7
wait: gate                     12.83  15.2%     74   624.3
model                           9.38  11.1%   5687     5.9
cargo test                      2.55   3.0%    297    30.9
wait: build                     2.37   2.8%     17   502.0
remote command                  1.12   1.3%    301    13.4
push gate                       0.93   1.1%    290    11.6
wait: review                    0.70   0.8%     16   157.6
wait: clip                      0.66   0.8%     12   198.0
shell                           0.49   0.6%   2458     0.7
wait: release                   0.46   0.5%      3   546.9
cargo build                     0.42   0.5%     43    34.8
loop tools                      0.35   0.4%    400     3.2
remote wait                     0.30   0.4%     38    28.1

phase                          hours  share  steps  mean s
orient                         77.43  91.9%   9499    29.3
gate                            1.83   2.2%    203    32.4
build                           1.72   2.0%    710     8.7
prove                           1.58   1.9%    604     9.4
choose                          0.67   0.8%    507     4.8
ship                            0.57   0.7%    229     9.0
release                         0.24   0.3%     99     8.9
research                        0.17   0.2%    168     3.6
log                             0.07   0.1%     31     8.6
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

