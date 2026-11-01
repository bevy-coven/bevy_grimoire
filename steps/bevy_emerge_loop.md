# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 04:29Z.

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

## The last 12 passes

```text
12 pass(es), 2765 steps, 7.1 h of pass time

category                       hours  share  steps  mean s
model                           2.69  37.9%   1291     7.5
wait: gate                      1.39  19.6%     22   227.0
remote command                  0.75  10.6%    171    15.8
wait: tests                     0.52   7.4%     14   133.9
wait: review                    0.46   6.4%     11   149.4
wait: other                     0.29   4.2%     14    75.8
cargo test                      0.19   2.7%     51    13.8
push gate                       0.16   2.2%     32    17.6
loop tools                      0.11   1.6%     65     6.3
phase mark                      0.10   1.5%     66     5.7
wait: build                     0.09   1.3%      4    81.5
remote wait                     0.08   1.1%     11    26.2
wait: release                   0.05   0.6%      2    82.8
shell                           0.04   0.6%    320     0.5
cargo build                     0.03   0.5%      2    57.6

phase                          hours  share  steps  mean s
prove                           1.52  21.4%    590     9.3
gate                            1.49  21.0%    179    30.0
orient                          1.46  20.6%    607     8.7
build                           1.15  16.2%    529     7.8
choose                          0.67   9.5%    507     4.8
ship                            0.47   6.7%    179     9.5
release                         0.20   2.8%     77     9.2
log                             0.07   0.9%     26     9.3
research                        0.05   0.7%     50     3.3
bugs                            0.01   0.2%     13     4.1

slowest single steps
  pass  176     641s  tool  wait: gate           Waiting for gate result
  pass  186     592s  tool  wait: gate           Waiting for gate to finish
  pass  183     432s  tool  wait: gate           Waiting for the gate result
  pass  185     408s  tool  wait: gate           Waiting for gate and release to finish
  pass  174     340s  tool  wait: review         Waiting for reviewer result
  pass  172     336s  tool  wait: gate           Waiting for the gate result
  pass  176     331s  tool  wait: tests          Waiting for core test results or big build
  pass  177     320s  tool  wait: other          Waiting for the after-run
  pass  173     288s  model model                
  pass  184     280s  bg    subagents            NotesViewReview completed: NotesViewReview
  pass  172     275s  tool  wait: tests          Waiting for the build/test or the reviewer
  pass  187     271s  bg    subagents            ChecksViewReview-2 completed: ChecksViewReview-2
  pass  185     249s  bg    subagents            RigsVerdictReview completed: RigsVerdictReview
  pass  187     231s  tool  wait: gate           Waiting for the gate
  pass  175     231s  tool  wait: gate           Waiting for gate result
```

## Every pass recorded

```text
167 pass(es), 11627 steps, 83.0 h of pass time

category                       hours  share  steps  mean s
wait: other                    31.74  38.2%    175   652.9
wait: tests                    18.88  22.7%     94   723.0
wait: gate                     12.53  15.1%     70   644.2
model                           8.88  10.7%   5479     5.8
cargo test                      2.51   3.0%    292    30.9
wait: build                     2.32   2.8%     14   597.2
remote command                  1.06   1.3%    269    14.2
push gate                       0.90   1.1%    287    11.3
wait: clip                      0.66   0.8%     12   198.0
wait: review                    0.62   0.7%     15   147.9
shell                           0.49   0.6%   2431     0.7
wait: release                   0.46   0.5%      3   546.9
cargo build                     0.40   0.5%     42    34.2
loop tools                      0.33   0.4%    393     3.0
cargo clippy                    0.29   0.4%     31    33.7

phase                          hours  share  steps  mean s
orient                         77.36  93.2%   9442    29.5
prove                           1.52   1.8%    590     9.3
gate                            1.49   1.8%    179    30.0
build                           1.15   1.4%    529     7.8
choose                          0.67   0.8%    507     4.8
ship                            0.47   0.6%    179     9.5
release                         0.20   0.2%     77     9.2
log                             0.07   0.1%     26     9.3
research                        0.05   0.1%     50     3.3
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

