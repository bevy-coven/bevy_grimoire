# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 01:02Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 0 | 2026-11-01T00:32Z | 47 | 0 | 53 | bevy_grimoire_solver-v0.1.1 | 16 | 3 | release(bevy_grimoire bevy_grimoire_core) |

## The last 12 passes

```text
12 pass(es), 2634 steps, 6.9 h of pass time

category                       hours  share  steps  mean s
model                           2.66  38.5%   1239     7.7
wait: gate                      1.31  18.9%     21   224.1
remote command                  0.65   9.4%    183    12.8
wait: review                    0.47   6.8%     10   169.8
wait: tests                     0.44   6.4%     14   113.9
wait: other                     0.32   4.6%     12    95.2
cargo test                      0.24   3.5%     46    19.2
push gate                       0.24   3.5%     49    17.7
remote wait                     0.13   1.9%     17    27.2
loop tools                      0.09   1.3%     56     5.7
wait: build                     0.07   1.0%      3    79.5
github                          0.06   0.9%     53     4.0
phase mark                      0.05   0.7%     29     6.3
cargo build                     0.04   0.6%      4    40.0
shell                           0.04   0.6%    300     0.5

phase                          hours  share  steps  mean s
orient                          4.20  60.6%   1576     9.6
prove                           1.03  14.9%    388     9.6
gate                            0.56   8.0%     77    26.0
build                           0.33   4.8%    149     8.1
ship                            0.31   4.5%    125     9.0
choose                          0.24   3.5%    183     4.7
release                         0.18   2.5%     61    10.4
research                        0.05   0.7%     50     3.3
log                             0.02   0.3%      7    12.4
bugs                            0.01   0.1%      9     3.4

slowest single steps
  pass  166     793s  tool  wait: gate           Waiting for the push gate
  pass  176     641s  tool  wait: gate           Waiting for gate result
  pass  166     389s  tool  wait: gate           Waiting for the #179 gated push
  pass  174     340s  tool  wait: review         Waiting for reviewer result
  pass  172     336s  tool  wait: gate           Waiting for the gate result
  pass  176     331s  tool  wait: tests          Waiting for core test results or big build
  pass  177     320s  tool  wait: other          Waiting for the after-run
  pass  169     300s  bg    shell                bg_2 failed: cat > /tmp/p160/build1.sh <<'EOF' cd ~/p160/grimoire export CARGO_TARGET_DIR=
  pass  173     288s  model model                
  pass  166     276s  tool  wait: tests          Waiting for slice 4 test run on big
  pass  172     275s  tool  wait: tests          Waiting for the build/test or the reviewer
  pass  166     272s  tool  wait: review         Waiting for the reviewer
  pass  166     251s  tool  wait: other          Waiting for slice 4 re-run and accept.py
  pass  175     231s  tool  wait: gate           Waiting for gate result
  pass  177     230s  tool  wait: review         Waiting for before-run and reviewer
```

## Every pass recorded

```text
162 pass(es), 10477 steps, 80.0 h of pass time

category                       hours  share  steps  mean s
wait: other                    31.61  39.5%    167   681.4
wait: tests                    18.71  23.4%     90   748.5
wait: gate                     11.83  14.8%     59   721.9
model                           7.80   9.7%   4941     5.7
cargo test                      2.45   3.1%    269    32.7
wait: build                     2.29   2.9%     12   688.4
push gate                       0.86   1.1%    278    11.1
remote command                  0.67   0.8%    188    12.9
wait: clip                      0.65   0.8%     10   233.2
wait: review                    0.48   0.6%     11   157.5
shell                           0.48   0.6%   2281     0.8
wait: release                   0.41   0.5%      2   739.9
cargo build                     0.40   0.5%     42    34.2
cargo clippy                    0.29   0.4%     30    34.8
loop tools                      0.28   0.4%    364     2.8

phase                          hours  share  steps  mean s
orient                         77.31  96.6%   9397    29.6
prove                           1.03   1.3%    388     9.6
gate                            0.56   0.7%     77    26.0
build                           0.33   0.4%    149     8.1
ship                            0.31   0.4%    125     9.0
choose                          0.24   0.3%    183     4.7
release                         0.18   0.2%     61    10.4
research                        0.05   0.1%     50     3.3
log                             0.02   0.0%      7    12.4
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

