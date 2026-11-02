# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-02 04:10Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 212 | 2026-11-01T16:47Z | 3 | 0 | 5 | - | 4 | 0 | demo? |
| 213 | 2026-11-01T17:08Z | 2 | 0 | 4 | - | 3 | 2 | - |
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

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4             5      80.6        83.9
```

## The last 12 passes

```text
12 pass(es), 4634 steps, 10.9 h of pass time

category                       hours  share  steps  mean s
model                           4.04  37.1%   2209     6.6
wait: other                     2.66  24.4%     77   124.3
cargo test                      0.96   8.8%    135    25.5
wait: gate                      0.64   5.9%     19   122.2
shell                           0.53   4.8%   1051     1.8
wait: clip                      0.46   4.2%      8   205.2
clip record/render              0.35   3.2%    118    10.6
cargo build                     0.32   3.0%     41    28.2
phase mark                      0.23   2.1%    105     8.0
wait: tests                     0.13   1.2%     12    37.8
wait: build                     0.12   1.1%      3   141.6
push gate                       0.11   1.0%     43     9.4
github                          0.11   1.0%    132     3.0
remote wait                     0.06   0.5%     11    18.8
cargo clippy                    0.05   0.4%      8    21.8

phase                          hours  share  steps  mean s
build                           3.43  31.5%   1402     8.8
ship                            3.09  28.4%    869    12.8
prove                           1.44  13.2%    763     6.8
gate                            1.05   9.7%    388     9.8
release                         0.54   5.0%    120    16.3
log                             0.47   4.3%    243     7.0
choose                          0.36   3.3%    311     4.2
research                        0.31   2.9%    343     3.3
orient                          0.18   1.6%    157     4.0
bugs                            0.01   0.1%     12     2.7

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
203 pass(es), 20822 steps, 105.6 h of pass time

category                       hours  share  steps  mean s
wait: other                    35.50  33.6%    291   439.2
wait: tests                    19.39  18.4%    120   581.7
model                          17.15  16.2%   9774     6.3
wait: gate                     15.69  14.9%    137   412.4
cargo test                      4.00   3.8%    504    28.6
wait: build                     2.55   2.4%     24   381.9
remote command                  1.78   1.7%    547    11.7
push gate                       1.29   1.2%    377    12.3
wait: clip                      1.17   1.1%     27   156.0
wait: review                    1.17   1.1%     33   127.2
shell                           1.13   1.1%   4058     1.0
cargo build                     0.80   0.8%    102    28.1
phase mark                      0.72   0.7%    323     8.1
wait: release                   0.56   0.5%      6   336.8
remote wait                     0.55   0.5%     76    25.9

phase                          hours  share  steps  mean s
orient                         79.17  74.9%  10337    27.6
build                           6.82   6.5%   2916     8.4
ship                            5.04   4.8%   1432    12.7
gate                            4.75   4.5%    887    19.3
prove                           4.13   3.9%   1759     8.5
choose                          2.34   2.2%   1623     5.2
research                        1.41   1.3%   1085     4.7
log                             1.05   1.0%    394     9.6
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

