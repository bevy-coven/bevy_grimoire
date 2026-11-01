# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 02:03Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 0 | 2026-11-01T00:32Z | 47 | 0 | 53 | bevy_grimoire_solver-v0.1.1 | 16 | 3 | release(bevy_grimoire bevy_grimoire_core) |
| 183 | 2026-11-01T02:03Z | 4 | 0 | 6 | bevy_grimoire_core-v0.2.1 | 2 | 0 | demo? |

## The last 12 passes

```text
12 pass(es), 2685 steps, 6.5 h of pass time

category                       hours  share  steps  mean s
model                           2.68  40.9%   1257     7.7
wait: gate                      1.13  17.3%     20   203.5
remote command                  0.63   9.7%    192    11.8
wait: review                    0.46   7.0%     11   150.4
wait: tests                     0.36   5.6%     12   109.1
cargo test                      0.25   3.8%     47    18.9
wait: other                     0.22   3.3%     13    60.3
push gate                       0.21   3.1%     44    16.8
remote wait                     0.15   2.2%     18    29.0
loop tools                      0.09   1.4%     58     5.5
wait: build                     0.07   1.0%      3    79.5
phase mark                      0.06   1.0%     37     6.2
github                          0.06   0.9%     52     3.9
cargo build                     0.04   0.7%      4    40.0
shell                           0.04   0.6%    314     0.4

phase                          hours  share  steps  mean s
orient                          3.27  50.0%   1378     8.5
prove                           1.05  16.1%    411     9.2
gate                            0.74  11.3%     99    26.8
build                           0.50   7.7%    216     8.3
choose                          0.38   5.8%    293     4.6
ship                            0.32   5.0%    133     8.8
release                         0.20   3.0%     77     9.2
research                        0.05   0.7%     50     3.3
log                             0.03   0.4%     10     9.5
bugs                            0.01   0.1%      9     3.4

slowest single steps
  pass  176     641s  tool  wait: gate           Waiting for gate result
  pass  183     432s  tool  wait: gate           Waiting for the gate result
  pass  174     340s  tool  wait: review         Waiting for reviewer result
  pass  172     336s  tool  wait: gate           Waiting for the gate result
  pass  176     331s  tool  wait: tests          Waiting for core test results or big build
  pass  177     320s  tool  wait: other          Waiting for the after-run
  pass  169     300s  bg    shell                bg_2 failed: cat > /tmp/p160/build1.sh <<'EOF' cd ~/p160/grimoire export CARGO_TARGET_DIR=
  pass  173     288s  model model                
  pass  172     275s  tool  wait: tests          Waiting for the build/test or the reviewer
  pass  175     231s  tool  wait: gate           Waiting for gate result
  pass  177     230s  tool  wait: review         Waiting for before-run and reviewer
  pass  175     228s  tool  wait: gate           Waiting for gate result
  pass  175     226s  tool  wait: build          Waiting for mapper build and reviewer
  pass  174     225s  tool  wait: gate           Waiting for gate result
  pass  170     225s  tool  wait: gate           Waiting for gate to finish
```

## Every pass recorded

```text
163 pass(es), 10737 steps, 80.6 h of pass time

category                       hours  share  steps  mean s
wait: other                    31.64  39.3%    170   670.1
wait: tests                    18.71  23.2%     90   748.5
wait: gate                     11.99  14.9%     61   707.4
model                           8.02  10.0%   5060     5.7
cargo test                      2.45   3.0%    273    32.3
wait: build                     2.29   2.8%     12   688.4
push gate                       0.87   1.1%    281    11.1
remote command                  0.69   0.9%    206    12.0
wait: clip                      0.66   0.8%     12   198.0
wait: review                    0.54   0.7%     13   150.9
shell                           0.48   0.6%   2319     0.7
wait: release                   0.41   0.5%      2   739.9
cargo build                     0.40   0.5%     42    34.2
cargo clippy                    0.29   0.4%     31    33.7
loop tools                      0.28   0.4%    371     2.8

phase                          hours  share  steps  mean s
orient                         77.32  95.9%   9407    29.6
prove                           1.05   1.3%    411     9.2
gate                            0.74   0.9%     99    26.8
build                           0.50   0.6%    216     8.3
choose                          0.38   0.5%    293     4.6
ship                            0.32   0.4%    133     8.8
release                         0.20   0.2%     77     9.2
research                        0.05   0.1%     50     3.3
log                             0.03   0.0%     10     9.5
bugs                            0.01   0.0%      9     3.4

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

