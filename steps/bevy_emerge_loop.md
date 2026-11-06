# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-06 15:45Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 264 | 2026-11-03T04:30Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 265 | 2026-11-03T04:45Z | 1 | 0 | 0 | - | 1 | 0 | - |
| 266 | 2026-11-03T04:53Z | 1 | 0 | 0 | - | 1 | 0 | - |
| 267 | 2026-11-03T05:11Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 268 | 2026-11-03T05:33Z | 2 | 0 | 6 | - | 1 | 0 | demo? |
| 269 | 2026-11-03T05:45Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 270 | 2026-11-03T05:57Z | 1 | 0 | 0 | - | 1 | 0 | - |
| 406 | 2026-11-06T14:55Z | 2 | 0 | 5 | - | 1 | 0 | demo? |
| 408 | 2026-11-06T15:23Z | 3 | 0 | 1 | - | 1 | 0 | demo? |
| 410 | 2026-11-06T15:40Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 412 | 2026-11-06T15:42Z | 2 | 0 | 0 | - | 0 | 0 | - |
| 414 | 2026-11-06T15:45Z | 1 | 0 | 0 | - | 0 | 0 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4            12      72.3        78.8
big        -                            -        -     -            59      57.5        26.0
big        claude-sonnet-5-5            medium   -     -            10      12.4        16.1
```

## The last 12 passes

```text
12 pass(es), 760 steps, 2.4 h of pass time

category                       hours  share  steps  mean s
wait: other                     1.53  62.9%     21   261.8
model                           0.42  17.2%    360     4.2
cargo test                      0.22   9.1%     16    49.5
phase mark                      0.14   5.8%     39    12.9
cargo build                     0.07   2.7%      4    60.0
shell                           0.03   1.0%    166     0.5
push gate                       0.01   0.5%      9     4.8
loop tools                      0.01   0.4%     49     0.7
github                          0.01   0.4%     21     1.6
git                             0.00   0.1%     16     0.4
read files                      0.00   0.0%     19     0.1
edit files                      0.00   0.0%     12     0.1
remote command                  0.00   0.0%      2     0.2
eval                            0.00   0.0%     10     0.0
search                          0.00   0.0%      2     0.1

phase                          hours  share  steps  mean s
ship                            0.74  30.5%     32    83.4
build                           0.67  27.5%    226    10.6
prove                           0.36  14.8%     36    36.0
gate                            0.36  14.7%     32    40.1
repro                           0.15   6.1%    176     3.0
orient                          0.11   4.5%    194     2.0
research                        0.04   1.5%     40     3.2
choose                          0.01   0.4%     17     1.8

slowest single steps
  pass  409     939s  tool  wait: other          {}
  pass  408     939s  tool  wait: other          {}
  pass  411     541s  tool  wait: other          {}
  pass  410     541s  tool  wait: other          {}
  pass  373     390s  bg    cargo test           bg_17 completed: timeout 900 cargo nextest run -p bevy_cauldron --test editor -E 'test(the
  pass  409     341s  tool  wait: other          {}
  pass  408     341s  tool  wait: other          {}
  pass  407     293s  tool  wait: other          {}
  pass  406     293s  tool  wait: other          {}
  pass  407     284s  tool  wait: other          {}
  pass  406     284s  tool  wait: other          {}
  pass  373     278s  bg    cargo test           bg_18 completed: timeout 900 cargo nextest run -p bevy_cauldron --test editor -E 'test(the
  pass  373     190s  bg    cargo test           bg_20 completed: timeout 900 cargo nextest run -p bevy_cauldron --test editor -E 'test(the
  pass  411     125s  tool  wait: other          {}
  pass  410     125s  tool  wait: other          {}
```

## Every pass recorded

```text
316 pass(es), 45561 steps, 158.6 h of pass time

category                       hours  share  steps  mean s
wait: other                    40.27  25.4%    507   286.0
model                          28.95  18.3%  21827     4.8
wait: tests                    22.92  14.5%    329   250.8
wait: gate                     21.54  13.6%    279   277.9
cargo test                     11.64   7.3%   1285    32.6
shell                           4.69   3.0%  10293     1.6
wait: build                     3.85   2.4%    147    94.3
cargo build                     3.72   2.3%    455    29.4
push gate                       3.56   2.2%    697    18.4
clip record/render              3.14   2.0%   1049    10.8
wait: clip                      2.56   1.6%     51   181.0
remote wait                     1.87   1.2%    188    35.7
phase mark                      1.80   1.1%    698     9.3
remote command                  1.78   1.1%    549    11.7
wait: review                    1.28   0.8%     37   125.0

phase                          hours  share  steps  mean s
orient                         96.63  60.9%  18243    19.1
build                          17.82  11.2%   9064     7.1
prove                          10.55   6.7%   5315     7.1
ship                           10.31   6.5%   2656    14.0
gate                            9.96   6.3%   2218    16.2
choose                          3.90   2.5%   2761     5.1
bugs                            3.44   2.2%   2047     6.1
log                             2.58   1.6%    921    10.1
research                        2.12   1.3%   1672     4.6
release                         1.12   0.7%    308    13.1
repro                           0.15   0.1%    176     3.0

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

