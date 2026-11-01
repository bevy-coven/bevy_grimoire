# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 04:01Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 0 | 2026-11-01T00:32Z | 47 | 0 | 53 | bevy_grimoire_solver-v0.1.1 | 16 | 3 | release(bevy_grimoire bevy_grimoire_core) |
| 183 | 2026-11-01T02:03Z | 4 | 0 | 6 | bevy_grimoire_core-v0.2.1 | 2 | 0 | demo? |
| 184 | 2026-11-01T02:45Z | 3 | 0 | 6 | - | 2 | 0 | demo? |
| 185 | 2026-11-01T03:32Z | 6 | 0 | 10 | bevy_grimoire-v0.1.2,bevy_grimoire_anim-v0.1.2,bevy_grimoire_core-v0.3.0 | 3 | 0 | demo? |
| 186 | 2026-11-01T04:01Z | 3 | 0 | 0 | - | 2 | 0 | - |

## The last 12 passes

```text
12 pass(es), 2820 steps, 7.2 h of pass time

category                       hours  share  steps  mean s
model                           2.83  39.0%   1320     7.7
wait: gate                      1.38  19.1%     22   226.4
remote command                  0.73  10.1%    181    14.5
wait: review                    0.49   6.8%     12   147.5
wait: tests                     0.43   6.0%     13   120.3
wait: other                     0.30   4.1%     15    71.8
cargo test                      0.28   3.9%     58    17.6
push gate                       0.18   2.4%     37    17.2
loop tools                      0.10   1.4%     66     5.6
wait: build                     0.09   1.3%      5    67.7
phase mark                      0.09   1.3%     59     5.6
remote wait                     0.08   1.1%     11    26.5
wait: release                   0.05   0.6%      2    82.8
shell                           0.04   0.6%    336     0.4
github                          0.03   0.5%     41     2.9

phase                          hours  share  steps  mean s
orient                          2.08  28.7%    845     8.8
prove                           1.42  19.7%    568     9.0
gate                            1.37  18.9%    167    29.6
build                           0.97  13.4%    414     8.4
choose                          0.63   8.7%    483     4.7
ship                            0.47   6.5%    177     9.5
release                         0.20   2.7%     77     9.2
research                        0.05   0.6%     50     3.3
log                             0.04   0.6%     18     8.9
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
  pass  185     249s  bg    subagents            RigsVerdictReview completed: RigsVerdictReview
  pass  175     231s  tool  wait: gate           Waiting for gate result
  pass  177     230s  tool  wait: review         Waiting for before-run and reviewer
  pass  184     230s  tool  wait: gate           Waiting for the gate result
```

## Every pass recorded

```text
166 pass(es), 11432 steps, 82.5 h of pass time

category                       hours  share  steps  mean s
wait: other                    31.74  38.5%    174   656.7
wait: tests                    18.79  22.8%     92   735.2
wait: gate                     12.46  15.1%     68   659.5
model                           8.71  10.6%   5385     5.8
cargo test                      2.51   3.0%    290    31.1
wait: build                     2.32   2.8%     14   597.2
remote command                  0.97   1.2%    258    13.6
push gate                       0.89   1.1%    286    11.2
wait: clip                      0.66   0.8%     12   198.0
wait: review                    0.62   0.7%     15   147.9
shell                           0.49   0.6%   2408     0.7
wait: release                   0.46   0.6%      3   546.9
cargo build                     0.40   0.5%     42    34.2
loop tools                      0.31   0.4%    389     2.9
cargo clippy                    0.29   0.4%     31    33.7

phase                          hours  share  steps  mean s
orient                         77.35  93.7%   9431    29.5
prove                           1.42   1.7%    568     9.0
gate                            1.37   1.7%    167    29.6
build                           0.97   1.2%    414     8.4
choose                          0.63   0.8%    483     4.7
ship                            0.47   0.6%    177     9.5
release                         0.20   0.2%     77     9.2
research                        0.05   0.1%     50     3.3
log                             0.04   0.1%     18     8.9
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

