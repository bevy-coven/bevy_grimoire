# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 03:32Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 0 | 2026-11-01T00:32Z | 47 | 0 | 53 | bevy_grimoire_solver-v0.1.1 | 16 | 3 | release(bevy_grimoire bevy_grimoire_core) |
| 183 | 2026-11-01T02:03Z | 4 | 0 | 6 | bevy_grimoire_core-v0.2.1 | 2 | 0 | demo? |
| 184 | 2026-11-01T02:45Z | 3 | 0 | 6 | - | 2 | 0 | demo? |
| 185 | 2026-11-01T03:32Z | 6 | 0 | 10 | bevy_grimoire-v0.1.2,bevy_grimoire_anim-v0.1.2,bevy_grimoire_core-v0.3.0 | 3 | 0 | demo? |

## The last 12 passes

```text
12 pass(es), 2924 steps, 7.4 h of pass time

category                       hours  share  steps  mean s
model                           2.91  39.5%   1369     7.7
wait: gate                      1.32  17.9%     23   207.0
remote command                  0.80  10.8%    213    13.5
wait: review                    0.49   6.7%     12   147.2
wait: tests                     0.43   5.9%     13   120.3
wait: other                     0.30   4.1%     15    71.8
cargo test                      0.26   3.6%     55    17.2
push gate                       0.19   2.6%     40    17.0
remote wait                     0.11   1.5%     14    28.6
wait: build                     0.09   1.3%      5    67.7
loop tools                      0.09   1.2%     64     5.2
phase mark                      0.09   1.2%     53     5.8
github                          0.05   0.7%     43     4.2
wait: release                   0.05   0.6%      2    82.8
shell                           0.04   0.5%    339     0.4

phase                          hours  share  steps  mean s
orient                          2.67  36.2%   1106     8.7
prove                           1.42  19.3%    568     9.0
gate                            1.20  16.2%    159    27.1
build                           0.76  10.3%    310     8.9
choose                          0.59   7.9%    458     4.6
ship                            0.43   5.9%    157     9.9
release                         0.20   2.7%     77     9.2
research                        0.05   0.6%     50     3.3
log                             0.04   0.6%     16     9.4
bugs                            0.01   0.2%     13     4.1

slowest single steps
  pass  176     641s  tool  wait: gate           Waiting for gate result
  pass  183     432s  tool  wait: gate           Waiting for the gate result
  pass  185     408s  tool  wait: gate           Waiting for gate and release to finish
  pass  174     340s  tool  wait: review         Waiting for reviewer result
  pass  172     336s  tool  wait: gate           Waiting for the gate result
  pass  176     331s  tool  wait: tests          Waiting for core test results or big build
  pass  177     320s  tool  wait: other          Waiting for the after-run
  pass  169     300s  bg    shell                bg_2 failed: cat > /tmp/p160/build1.sh <<'EOF' cd ~/p160/grimoire export CARGO_TARGET_DIR=
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
165 pass(es), 11264 steps, 82.0 h of pass time

category                       hours  share  steps  mean s
wait: other                    31.74  38.7%    174   656.7
wait: tests                    18.79  22.9%     92   735.2
wait: gate                     12.29  15.0%     67   660.5
model                           8.54  10.4%   5305     5.8
cargo test                      2.47   3.0%    283    31.4
wait: build                     2.32   2.8%     14   597.2
remote command                  0.95   1.2%    249    13.8
push gate                       0.89   1.1%    285    11.2
wait: clip                      0.66   0.8%     12   198.0
wait: review                    0.58   0.7%     14   148.1
shell                           0.49   0.6%   2387     0.7
wait: release                   0.46   0.6%      3   546.9
cargo build                     0.40   0.5%     42    34.2
loop tools                      0.30   0.4%    382     2.8
cargo clippy                    0.29   0.4%     31    33.7

phase                          hours  share  steps  mean s
orient                         77.34  94.3%   9422    29.6
prove                           1.42   1.7%    568     9.0
gate                            1.20   1.5%    159    27.1
build                           0.76   0.9%    310     8.9
choose                          0.59   0.7%    458     4.6
ship                            0.43   0.5%    157     9.9
release                         0.20   0.2%     77     9.2
research                        0.05   0.1%     50     3.3
log                             0.04   0.1%     16     9.4
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

