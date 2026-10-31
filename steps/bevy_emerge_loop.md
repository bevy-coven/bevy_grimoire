# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-10-31 22:29Z.

## The last 12 passes

```text
12 pass(es), 1957 steps, 6.8 h of pass time

category                       hours  share  steps  mean s
model                           2.10  30.7%    933     8.1
wait: tests                     1.77  25.9%     12   532.5
wait: gate                      0.95  13.9%     17   201.6
remote command                  0.46   6.8%    115    14.5
push gate                       0.24   3.5%     42    20.6
shell                           0.19   2.7%    280     2.4
wait: review                    0.18   2.6%      4   163.1
wait: build                     0.17   2.5%      3   209.4
wait: other                     0.17   2.5%     12    51.3
remote wait                     0.14   2.1%     15    34.4
cargo test                      0.14   2.0%     30    16.6
loop tools                      0.09   1.3%     52     6.3
git                             0.07   1.0%     74     3.5
github                          0.06   0.9%     62     3.7
sleep                           0.05   0.7%      5    33.2

phase                          hours  share  steps  mean s
orient                          6.85 100.0%   1950    12.6

slowest single steps
  pass  165    1801s  tool  wait: tests          Waiting for the slice 3 test build
  pass  157    1800s  tool  wait: tests          Waiting for slice 3 build and tests
  pass  165    1184s  tool  wait: tests          Waiting for the slice 3 test build
  pass  166     793s  tool  wait: gate           Waiting for the push gate
  pass  157     616s  tool  wait: build          Waiting for the target clone
  pass  157     575s  tool  wait: tests          Waiting for slice 3 build and tests
  pass  165     400s  tool  wait: gate           Waiting for gate result on big
  pass  166     389s  tool  wait: gate           Waiting for the #179 gated push
  pass  172     336s  tool  wait: gate           Waiting for the gate result
  pass  169     300s  bg    shell                bg_2 failed: cat > /tmp/p160/build1.sh <<'EOF' cd ~/p160/grimoire export CARGO_TARGET_DIR=
  pass  173     288s  model model                
  pass  166     276s  tool  wait: tests          Waiting for slice 4 test run on big
  pass  172     275s  tool  wait: tests          Waiting for the build/test or the reviewer
  pass  166     272s  tool  wait: review         Waiting for the reviewer
  pass  166     251s  tool  wait: other          Waiting for slice 4 re-run and accept.py
```

## Every pass recorded

```text
158 pass(es), 9322 steps, 77.2 h of pass time

category                       hours  share  steps  mean s
wait: other                    31.46  40.8%    164   690.6
wait: tests                    18.50  24.0%     83   802.6
wait: gate                     11.36  14.7%     53   771.9
model                           6.70   8.7%   4407     5.5
cargo test                      2.34   3.0%    248    33.9
wait: build                     2.23   2.9%     10   803.5
push gate                       0.78   1.0%    261    10.8
wait: clip                      0.65   0.8%     10   233.2
remote command                  0.49   0.6%    120    14.6
shell                           0.47   0.6%   2173     0.8
wait: release                   0.41   0.5%      1  1475.2
cargo build                     0.37   0.5%     40    33.0
cargo clippy                    0.29   0.4%     29    35.9
loop tools                      0.24   0.3%    337     2.6
remote wait                     0.23   0.3%     28    29.5

phase                          hours  share  steps  mean s
orient                         77.17 100.0%   9293    29.9

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

