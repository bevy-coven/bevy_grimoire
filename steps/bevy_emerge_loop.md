# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-06 15:40Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 262 | 2026-11-03T03:56Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 263 | 2026-11-03T04:09Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
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

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4            12      72.3        78.8
big        -                            -        -     -            59      57.5        26.0
big        claude-sonnet-5-5            medium   -     -             6      20.3        18.7
```

## The last 12 passes

```text
12 pass(es), 2283 steps, 4.7 h of pass time

category                       hours  share  steps  mean s
wait: other                     1.71  36.2%     27   227.5
model                           1.11  23.5%   1069     3.7
cargo test                      0.52  11.1%     39    48.2
clip record/render              0.24   5.2%     57    15.4
shell                           0.23   4.9%    685     1.2
wait: tests                     0.18   3.9%     10    66.2
phase mark                      0.16   3.5%     53    11.1
cargo build                     0.15   3.2%     14    38.9
wait: gate                      0.11   2.3%      3   128.3
push gate                       0.10   2.1%     18    20.2
cargo clippy                    0.05   1.1%      9    19.9
remote wait                     0.05   1.0%      4    43.8
github                          0.04   0.9%     65     2.3
wait: build                     0.04   0.8%      4    33.1
loop tools                      0.02   0.4%     61     1.0

phase                          hours  share  steps  mean s
gate                            1.16  24.6%    356    11.7
build                           1.01  21.4%    479     7.6
ship                            0.77  16.2%     44    62.6
prove                           0.72  15.2%    352     7.3
bugs                            0.55  11.6%    351     5.6
orient                          0.24   5.1%    348     2.5
repro                           0.15   3.1%    176     3.0
choose                          0.07   1.6%     95     2.8
research                        0.04   0.8%     40     3.2
log                             0.02   0.4%     32     2.4

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
  pass  372     253s  tool  wait: tests          Waiting for baseline test
  pass  372     240s  tool  wait: other          Waiting for tree to clear
  pass  372     198s  tool  wait: gate           Waiting for the push gate
```

## Every pass recorded

```text
312 pass(es), 45495 steps, 158.5 h of pass time

category                       hours  share  steps  mean s
wait: other                    40.27  25.4%    507   286.0
model                          28.92  18.2%  21792     4.8
wait: tests                    22.92  14.5%    329   250.8
wait: gate                     21.54  13.6%    279   277.9
cargo test                     11.64   7.3%   1285    32.6
shell                           4.69   3.0%  10283     1.6
wait: build                     3.85   2.4%    147    94.3
cargo build                     3.72   2.3%    455    29.4
push gate                       3.55   2.2%    692    18.5
clip record/render              3.14   2.0%   1049    10.8
wait: clip                      2.56   1.6%     51   181.0
remote wait                     1.87   1.2%    188    35.7
phase mark                      1.80   1.1%    694     9.3
remote command                  1.78   1.1%    549    11.7
wait: review                    1.28   0.8%     37   125.0

phase                          hours  share  steps  mean s
orient                         96.59  60.9%  18177    19.1
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

