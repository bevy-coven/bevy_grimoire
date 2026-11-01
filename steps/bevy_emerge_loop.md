# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 02:45Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 0 | 2026-11-01T00:32Z | 47 | 0 | 53 | bevy_grimoire_solver-v0.1.1 | 16 | 3 | release(bevy_grimoire bevy_grimoire_core) |
| 183 | 2026-11-01T02:03Z | 4 | 0 | 6 | bevy_grimoire_core-v0.2.1 | 2 | 0 | demo? |
| 184 | 2026-11-01T02:45Z | 3 | 0 | 6 | - | 2 | 0 | demo? |

## The last 12 passes

```text
12 pass(es), 2750 steps, 6.7 h of pass time

category                       hours  share  steps  mean s
model                           2.74  40.7%   1285     7.7
wait: gate                      1.15  17.0%     21   196.7
remote command                  0.68  10.2%    197    12.5
wait: review                    0.46   6.8%     11   150.4
wait: tests                     0.43   6.5%     13   120.3
cargo test                      0.25   3.7%     50    17.9
wait: other                     0.24   3.6%     13    66.9
push gate                       0.19   2.9%     39    17.8
remote wait                     0.11   1.7%     14    28.6
loop tools                      0.09   1.3%     60     5.3
wait: build                     0.08   1.2%      4    73.0
phase mark                      0.07   1.1%     46     5.7
github                          0.05   0.8%     49     3.9
shell                           0.04   0.6%    309     0.4
cargo build                     0.03   0.5%      2    57.6

phase                          hours  share  steps  mean s
orient                          2.77  41.1%   1161     8.6
prove                           1.19  17.7%    501     8.6
gate                            0.94  13.9%    121    27.8
build                           0.66   9.8%    244     9.7
choose                          0.53   7.9%    414     4.6
ship                            0.36   5.3%    148     8.7
release                         0.20   2.9%     77     9.2
research                        0.05   0.7%     50     3.3
log                             0.03   0.4%     12     8.8
bugs                            0.01   0.2%     13     4.1

slowest single steps
  pass  176     641s  tool  wait: gate           Waiting for gate result
  pass  183     432s  tool  wait: gate           Waiting for the gate result
  pass  174     340s  tool  wait: review         Waiting for reviewer result
  pass  172     336s  tool  wait: gate           Waiting for the gate result
  pass  176     331s  tool  wait: tests          Waiting for core test results or big build
  pass  177     320s  tool  wait: other          Waiting for the after-run
  pass  169     300s  bg    shell                bg_2 failed: cat > /tmp/p160/build1.sh <<'EOF' cd ~/p160/grimoire export CARGO_TARGET_DIR=
  pass  173     288s  model model                
  pass  184     280s  bg    subagents            NotesViewReview completed: NotesViewReview
  pass  172     275s  tool  wait: tests          Waiting for the build/test or the reviewer
  pass  175     231s  tool  wait: gate           Waiting for gate result
  pass  177     230s  tool  wait: review         Waiting for before-run and reviewer
  pass  184     230s  tool  wait: gate           Waiting for the gate result
  pass  184     229s  tool  wait: gate           Waiting for the gate result
  pass  175     228s  tool  wait: gate           Waiting for gate result
```

## Every pass recorded

```text
164 pass(es), 11028 steps, 81.3 h of pass time

category                       hours  share  steps  mean s
wait: other                    31.68  39.0%    172   663.1
wait: tests                    18.79  23.1%     92   735.2
wait: gate                     12.12  14.9%     65   671.1
model                           8.27  10.2%   5192     5.7
cargo test                      2.45   3.0%    278    31.8
wait: build                     2.31   2.8%     13   639.5
push gate                       0.88   1.1%    282    11.2
remote command                  0.84   1.0%    231    13.0
wait: clip                      0.66   0.8%     12   198.0
wait: review                    0.54   0.7%     13   150.9
shell                           0.48   0.6%   2345     0.7
wait: release                   0.41   0.5%      2   739.9
cargo build                     0.40   0.5%     42    34.2
loop tools                      0.29   0.4%    377     2.8
cargo clippy                    0.29   0.4%     31    33.7

phase                          hours  share  steps  mean s
orient                         77.33  95.1%   9415    29.6
prove                           1.19   1.5%    501     8.6
gate                            0.94   1.2%    121    27.8
build                           0.66   0.8%    244     9.7
choose                          0.53   0.7%    414     4.6
ship                            0.36   0.4%    148     8.7
release                         0.20   0.2%     77     9.2
research                        0.05   0.1%     50     3.3
log                             0.03   0.0%     12     8.8
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

