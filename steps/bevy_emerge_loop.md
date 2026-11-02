# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-02 07:04Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 214 | 2026-11-01T17:33Z | 3 | 0 | 7 | - | 4 | 2 | - |
| 215 | 2026-11-01T17:50Z | 2 | 0 | 4 | - | 2 | 0 | demo? |
| 216 | 2026-11-01T18:40Z | 13 | 0 | 8 | bevy_grimoire-v0.3.2,bevy_grimoire_anim-v0.1.5,bevy_grimoire_core-v0.6.0,bevy_grimoire_core-v0.6.1 | 8 | 4 | - |
| 217 | 2026-11-01T19:24Z | 8 | 0 | 10 | bevy_grimoire_core-v0.6.2 | 5 | 2 | - |
| 219 | 2026-11-01T21:24Z | 21 | 0 | 128 | bevy_grimoire-v0.3.3,bevy_grimoire_anim-v0.1.6,bevy_grimoire_core-v0.6.3 | 18 | 2 | - |
| 220 | 2026-11-01T22:54Z | 6 | 0 | 4 | bevy_grimoire_core-v0.6.4 | 17 | 9 | - |
| 221 | 2026-11-02T00:03Z | 5 | 0 | 4 | - | 2 | 0 | demo? |
| 222 | 2026-11-02T01:17Z | 10 | 0 | 11 | - | 2 | 0 | demo? |
| 223 | 2026-11-02T02:42Z | 15 | 0 | 10 | bevy_grimoire-v0.4.0,bevy_grimoire_core-v0.6.5 | 17 | 14 | - |
| 224 | 2026-11-02T04:10Z | 8 | 0 | 12 | - | 16 | 12 | - |
| 225 | 2026-11-02T05:38Z | 15 | 0 | 8 | - | 17 | 13 | - |
| 226 | 2026-11-02T07:04Z | 9 | 0 | 11 | - | 20 | 15 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4             7      82.2        85.5
```

## The last 12 passes

```text
12 pass(es), 5629 steps, 13.1 h of pass time

category                       hours  share  steps  mean s
model                           5.00  38.1%   2697     6.7
wait: other                     2.86  21.8%     82   125.7
cargo test                      1.33  10.1%    178    26.9
wait: gate                      0.76   5.8%     21   130.5
shell                           0.50   3.8%   1274     1.4
wait: clip                      0.46   3.5%      8   205.2
clip record/render              0.45   3.4%    157    10.3
cargo build                     0.42   3.2%     55    27.4
phase mark                      0.27   2.1%    107     9.1
wait: tests                     0.20   1.5%     21    34.6
remote wait                     0.14   1.1%     21    24.1
github                          0.14   1.0%    159     3.1
wait: build                     0.13   1.0%      6    76.5
cargo clippy                    0.11   0.9%     24    17.1
push gate                       0.11   0.8%     43     9.3

phase                          hours  share  steps  mean s
build                           4.40  33.5%   1933     8.2
ship                            3.11  23.7%    843    13.3
prove                           1.75  13.3%    980     6.4
gate                            1.61  12.2%    521    11.1
research                        0.74   5.6%    521     5.1
release                         0.54   4.1%    118    16.5
log                             0.46   3.5%    239     7.0
choose                          0.35   2.7%    285     4.4
orient                          0.18   1.4%    155     4.1

slowest single steps
  pass  224     840s  tool  wait: other          {}
  pass  219     838s  tool  wait: other          Waiting for --help run
  pass  221     498s  tool  wait: other          {}
  pass  220     424s  tool  wait: clip           Wait for five demo recordings
  pass  222     416s  tool  wait: other          {}
  pass  221     401s  tool  wait: other          {}
  pass  224     399s  tool  wait: other          {}
  pass  217     361s  tool  wait: other          {}
  pass  220     329s  tool  wait: clip           Wait for hero/frame/move recordings
  pass  220     314s  tool  wait: clip           Wait for final recording batch
  pass  221     310s  tool  wait: other          {}
  pass  223     289s  tool  wait: other          Waiting for retries
  pass  220     273s  tool  wait: gate           Wait for the push gate
  pass  216     266s  tool  wait: other          {}
  pass  219     266s  tool  wait: gate           Waiting for the gate
```

## Every pass recorded

```text
205 pass(es), 22220 steps, 108.5 h of pass time

category                       hours  share  steps  mean s
wait: other                    35.82  33.0%    305   422.8
wait: tests                    19.47  17.9%    129   543.3
model                          18.39  16.9%  10454     6.3
wait: gate                     15.81  14.6%    139   409.5
cargo test                      4.45   4.1%    554    28.9
wait: build                     2.56   2.4%     27   340.7
remote command                  1.78   1.6%    547    11.7
push gate                       1.29   1.2%    381    12.2
wait: review                    1.22   1.1%     34   129.4
shell                           1.17   1.1%   4371     1.0
wait: clip                      1.17   1.1%     27   156.0
cargo build                     0.89   0.8%    118    27.3
phase mark                      0.80   0.7%    340     8.5
remote wait                     0.63   0.6%     86    26.3
wait: release                   0.56   0.5%      6   336.8

phase                          hours  share  steps  mean s
orient                         79.20  73.0%  10359    27.5
build                           7.91   7.3%   3536     8.1
gate                            5.39   5.0%   1038    18.7
ship                            5.11   4.7%   1436    12.8
prove                           4.67   4.3%   2096     8.0
choose                          2.42   2.2%   1697     5.1
research                        1.83   1.7%   1263     5.2
log                             1.06   1.0%    396     9.6
release                         0.88   0.8%    251    12.7
bugs                            0.04   0.0%     48     3.0

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

