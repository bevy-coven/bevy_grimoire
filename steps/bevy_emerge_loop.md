# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 05:10Z.

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

## The last 12 passes

```text
12 pass(es), 2816 steps, 7.2 h of pass time

category                       hours  share  steps  mean s
model                           2.70  37.6%   1309     7.4
wait: gate                      1.43  19.9%     22   233.9
remote command                  0.71   9.9%    174    14.6
wait: tests                     0.47   6.6%     14   121.3
wait: review                    0.43   5.9%     10   153.1
wait: other                     0.36   5.1%     16    81.8
cargo test                      0.21   2.9%     51    14.9
push gate                       0.15   2.1%     31    17.8
wait: build                     0.14   1.9%      6    83.3
phase mark                      0.11   1.6%     73     5.6
loop tools                      0.11   1.5%     62     6.4
remote wait                     0.08   1.1%     11    26.2
cargo build                     0.05   0.7%      3    58.4
wait: release                   0.05   0.6%      2    82.8
shell                           0.04   0.6%    312     0.5

phase                          hours  share  steps  mean s
gate                            1.66  23.1%    189    31.6
prove                           1.52  21.1%    590     9.3
build                           1.46  20.3%    597     8.8
orient                          0.90  12.6%    436     7.5
choose                          0.67   9.4%    507     4.8
ship                            0.48   6.6%    181     9.5
release                         0.24   3.4%     99     8.9
research                        0.17   2.3%    168     3.6
log                             0.07   1.0%     28     9.0
bugs                            0.01   0.2%     13     4.1

slowest single steps
  pass  176     641s  tool  wait: gate           Waiting for gate result
  pass  186     592s  tool  wait: gate           Waiting for gate to finish
  pass  188     541s  tool  wait: gate           Waiting for gate/release/accept result
  pass  183     432s  tool  wait: gate           Waiting for the gate result
  pass  185     408s  tool  wait: gate           Waiting for gate and release to finish
  pass  174     340s  tool  wait: review         Waiting for reviewer result
  pass  172     336s  tool  wait: gate           Waiting for the gate result
  pass  176     331s  tool  wait: tests          Waiting for core test results or big build
  pass  177     320s  tool  wait: other          Waiting for the after-run
  pass  173     288s  model model                
  pass  188     285s  bg    subagents            OplogReview completed: OplogReview
  pass  184     280s  bg    subagents            NotesViewReview completed: NotesViewReview
  pass  172     275s  tool  wait: tests          Waiting for the build/test or the reviewer
  pass  187     271s  bg    subagents            ChecksViewReview-2 completed: ChecksViewReview-2
  pass  185     249s  bg    subagents            RigsVerdictReview completed: RigsVerdictReview
```

## Every pass recorded

```text
168 pass(es), 11860 steps, 83.6 h of pass time

category                       hours  share  steps  mean s
wait: other                    31.81  38.0%    177   646.9
wait: tests                    18.90  22.6%     96   708.7
wait: gate                     12.68  15.2%     72   634.2
model                           9.09  10.9%   5582     5.9
cargo test                      2.54   3.0%    296    30.9
wait: build                     2.37   2.8%     16   533.4
remote command                  1.11   1.3%    280    14.2
push gate                       0.92   1.1%    289    11.5
wait: clip                      0.66   0.8%     12   198.0
wait: review                    0.62   0.7%     15   147.9
shell                           0.49   0.6%   2443     0.7
wait: release                   0.46   0.5%      3   546.9
cargo build                     0.42   0.5%     43    34.8
loop tools                      0.34   0.4%    395     3.1
cargo clippy                    0.29   0.3%     31    33.7

phase                          hours  share  steps  mean s
orient                         77.37  92.5%   9452    29.5
gate                            1.66   2.0%    189    31.6
prove                           1.52   1.8%    590     9.3
build                           1.46   1.7%    597     8.8
choose                          0.67   0.8%    507     4.8
ship                            0.48   0.6%    181     9.5
release                         0.24   0.3%     99     8.9
research                        0.17   0.2%    168     3.6
log                             0.07   0.1%     28     9.0
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

