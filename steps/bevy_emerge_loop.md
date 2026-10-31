# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-10-31 23:21Z.

## The last 12 passes

```text
12 pass(es), 2334 steps, 6.9 h of pass time

category                       hours  share  steps  mean s
model                           2.39  34.9%   1095     7.9
wait: gate                      1.14  16.6%     20   205.5
wait: tests                     1.13  16.5%     12   340.0
remote command                  0.59   8.6%    154    13.8
wait: review                    0.28   4.0%      5   198.5
push gate                       0.24   3.5%     46    18.7
cargo test                      0.22   3.2%     42    19.0
wait: other                     0.17   2.5%     11    55.9
remote wait                     0.16   2.3%     17    33.2
shell                           0.14   2.0%    285     1.7
loop tools                      0.09   1.3%     59     5.5
wait: build                     0.07   1.0%      2   119.2
github                          0.06   0.9%     58     3.9
git                             0.05   0.7%     67     2.5
sleep                           0.05   0.7%      5    33.2

phase                          hours  share  steps  mean s
orient                          5.73  83.5%   1811    11.4
prove                           0.44   6.4%    213     7.5
gate                            0.23   3.4%     39    21.4
choose                          0.21   3.1%    155     5.0
build                           0.21   3.1%     94     8.1
ship                            0.02   0.4%     10     8.7
log                             0.01   0.1%      4     6.7

slowest single steps
  pass  165    1801s  tool  wait: tests          Waiting for the slice 3 test build
  pass  165    1184s  tool  wait: tests          Waiting for the slice 3 test build
  pass  166     793s  tool  wait: gate           Waiting for the push gate
  pass  165     400s  tool  wait: gate           Waiting for gate result on big
  pass  166     389s  tool  wait: gate           Waiting for the #179 gated push
  pass  174     340s  tool  wait: review         Waiting for reviewer result
  pass  172     336s  tool  wait: gate           Waiting for the gate result
  pass  169     300s  bg    shell                bg_2 failed: cat > /tmp/p160/build1.sh <<'EOF' cd ~/p160/grimoire export CARGO_TARGET_DIR=
  pass  173     288s  model model                
  pass  166     276s  tool  wait: tests          Waiting for slice 4 test run on big
  pass  172     275s  tool  wait: tests          Waiting for the build/test or the reviewer
  pass  166     272s  tool  wait: review         Waiting for the reviewer
  pass  166     251s  tool  wait: other          Waiting for slice 4 re-run and accept.py
  pass  175     231s  tool  wait: gate           Waiting for gate result
  pass  175     228s  tool  wait: gate           Waiting for gate result
```

## Every pass recorded

```text
160 pass(es), 9876 steps, 78.4 h of pass time

category                       hours  share  steps  mean s
wait: other                    31.46  40.1%    164   690.6
wait: tests                    18.52  23.6%     85   784.5
wait: gate                     11.55  14.7%     56   742.7
model                           7.22   9.2%   4652     5.6
cargo test                      2.42   3.1%    262    33.3
wait: build                     2.29   2.9%     11   750.9
push gate                       0.81   1.0%    269    10.8
wait: clip                      0.65   0.8%     10   233.2
remote command                  0.61   0.8%    159    13.9
shell                           0.47   0.6%   2216     0.8
wait: release                   0.41   0.5%      1  1475.2
cargo build                     0.38   0.5%     41    33.6
cargo clippy                    0.29   0.4%     30    34.8
wait: review                    0.29   0.4%      6   171.3
remote wait                     0.26   0.3%     31    30.1

phase                          hours  share  steps  mean s
orient                         77.25  98.6%   9331    29.8
prove                           0.44   0.6%    213     7.5
gate                            0.23   0.3%     39    21.4
choose                          0.21   0.3%    155     5.0
build                           0.21   0.3%     94     8.1
ship                            0.02   0.0%     10     8.7
log                             0.01   0.0%      4     6.7

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

