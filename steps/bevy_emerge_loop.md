# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 00:32Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 0 | 2026-11-01T00:32Z | 47 | 0 | 53 | bevy_grimoire_solver-v0.1.1 | 16 | 3 | release(bevy_grimoire bevy_grimoire_core) |

## The last 12 passes

```text
12 pass(es), 2535 steps, 7.5 h of pass time

category                       hours  share  steps  mean s
model                           2.58  34.2%   1193     7.8
wait: gate                      1.36  18.0%     21   232.9
wait: tests                     1.32  17.5%     17   280.1
remote command                  0.62   8.2%    171    13.1
wait: review                    0.35   4.7%      8   158.1
push gate                       0.24   3.2%     48    18.3
cargo test                      0.23   3.1%     46    18.4
wait: other                     0.21   2.8%     12    63.6
remote wait                     0.15   1.9%     18    29.0
loop tools                      0.07   1.0%     62     4.2
shell                           0.07   0.9%    293     0.9
wait: build                     0.07   0.9%      2   119.2
github                          0.06   0.8%     56     3.9
sleep                           0.05   0.6%      5    33.2
phase mark                      0.04   0.6%     21     7.4

phase                          hours  share  steps  mean s
orient                          5.50  73.0%   1756    11.3
prove                           0.84  11.1%    356     8.5
gate                            0.56   7.4%     77    26.0
build                           0.22   2.9%    106     7.5
choose                          0.21   2.8%    155     5.0
ship                            0.18   2.5%     69     9.6
log                             0.02   0.3%      7    12.4

slowest single steps
  pass  165    1801s  tool  wait: tests          Waiting for the slice 3 test build
  pass  165    1184s  tool  wait: tests          Waiting for the slice 3 test build
  pass  166     793s  tool  wait: gate           Waiting for the push gate
  pass  176     641s  tool  wait: gate           Waiting for gate result
  pass  165     400s  tool  wait: gate           Waiting for gate result on big
  pass  166     389s  tool  wait: gate           Waiting for the #179 gated push
  pass  174     340s  tool  wait: review         Waiting for reviewer result
  pass  172     336s  tool  wait: gate           Waiting for the gate result
  pass  176     331s  tool  wait: tests          Waiting for core test results or big build
  pass  169     300s  bg    shell                bg_2 failed: cat > /tmp/p160/build1.sh <<'EOF' cd ~/p160/grimoire export CARGO_TARGET_DIR=
  pass  173     288s  model model                
  pass  166     276s  tool  wait: tests          Waiting for slice 4 test run on big
  pass  172     275s  tool  wait: tests          Waiting for the build/test or the reviewer
  pass  166     272s  tool  wait: review         Waiting for the reviewer
  pass  166     251s  tool  wait: other          Waiting for slice 4 re-run and accept.py
```

## Every pass recorded

```text
161 pass(es), 10189 steps, 79.3 h of pass time

category                       hours  share  steps  mean s
wait: other                    31.50  39.7%    165   687.4
wait: tests                    18.71  23.6%     90   748.5
wait: gate                     11.77  14.8%     58   730.6
model                           7.55   9.5%   4803     5.7
cargo test                      2.44   3.1%    267    32.8
wait: build                     2.29   2.9%     11   750.9
push gate                       0.83   1.0%    272    11.0
wait: clip                      0.65   0.8%     10   233.2
remote command                  0.64   0.8%    176    13.1
shell                           0.47   0.6%   2252     0.8
wait: release                   0.41   0.5%      1  1475.2
cargo build                     0.38   0.5%     41    33.6
wait: review                    0.36   0.5%      9   144.4
cargo clippy                    0.29   0.4%     30    34.8
remote wait                     0.26   0.3%     33    28.4

phase                          hours  share  steps  mean s
orient                         77.30  97.4%   9388    29.6
prove                           0.84   1.1%    356     8.5
gate                            0.56   0.7%     77    26.0
build                           0.22   0.3%    106     7.5
choose                          0.21   0.3%    155     5.0
ship                            0.18   0.2%     69     9.6
log                             0.02   0.0%      7    12.4

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

