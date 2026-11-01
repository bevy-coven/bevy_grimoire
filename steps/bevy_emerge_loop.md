# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 08:09Z.

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

## The last 12 passes

```text
12 pass(es), 3070 steps, 8.2 h of pass time

category                       hours  share  steps  mean s
model                           3.03  37.1%   1412     7.7
wait: gate                      1.77  21.6%     28   227.7
wait: other                     0.75   9.2%     25   108.5
remote command                  0.61   7.4%    211    10.3
wait: review                    0.55   6.7%     14   141.6
wait: tests                     0.38   4.6%     11   123.3
push gate                       0.17   2.0%     27    22.0
phase mark                      0.16   1.9%     85     6.6
cargo test                      0.13   1.6%     45    10.7
loop tools                      0.11   1.4%     69     6.0
remote wait                     0.11   1.3%     12    32.4
wait: build                     0.11   1.3%      7    54.5
wait: release                   0.09   1.1%      3   104.4
cargo build                     0.05   0.6%      5    36.3
github                          0.04   0.5%     44     3.1

phase                          hours  share  steps  mean s
gate                            2.01  24.6%    245    29.5
build                           1.84  22.5%    799     8.3
prove                           1.14  13.9%    391    10.5
choose                          1.08  13.2%    621     6.3
ship                            0.86  10.5%    287    10.7
research                        0.52   6.4%    310     6.1
release                         0.32   3.9%    123     9.4
orient                          0.25   3.0%    220     4.1
log                             0.15   1.8%     51    10.5
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
  pass  192     293s  tool  wait: gate           Waiting for gate output
  pass  192     292s  tool  wait: gate           Waiting for gate output
  pass  190     291s  tool  wait: gate           Waiting for gate
  pass  191     290s  tool  wait: gate           Waiting for the gate
  pass  190     286s  tool  wait: other          Waiting for after-run on big
  pass  188     285s  bg    subagents            OplogReview completed: OplogReview
  pass  184     280s  bg    subagents            NotesViewReview completed: NotesViewReview
```

## Every pass recorded

```text
172 pass(es), 12946 steps, 86.6 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.22  37.2%    189   613.6
wait: tests                    18.90  21.8%     96   708.7
wait: gate                     13.32  15.4%     84   571.1
model                          10.25  11.8%   6064     6.1
cargo test                      2.56   3.0%    307    30.0
wait: build                     2.40   2.8%     18   480.1
remote command                  1.22   1.4%    370    11.9
push gate                       0.97   1.1%    296    11.8
wait: review                    0.84   1.0%     20   150.5
wait: clip                      0.66   0.8%     12   198.0
shell                           0.50   0.6%   2544     0.7
wait: release                   0.50   0.6%      4   447.1
cargo build                     0.43   0.5%     46    33.9
remote wait                     0.37   0.4%     43    30.8
loop tools                      0.37   0.4%    418     3.2

phase                          hours  share  steps  mean s
orient                         77.50  89.5%   9551    29.2
gate                            2.24   2.6%    284    28.4
build                           2.05   2.4%    893     8.3
prove                           1.58   1.8%    604     9.4
choose                          1.30   1.5%    776     6.0
ship                            0.88   1.0%    297    10.7
research                        0.52   0.6%    310     6.1
release                         0.32   0.4%    123     9.4
log                             0.16   0.2%     55    10.2
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

