# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-02 01:17Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 210 | 2026-11-01T16:21Z | 2 | 0 | 0 | - | 2 | 0 | - |
| 211 | 2026-11-01T16:30Z | 1 | 0 | 0 | - | 3 | 2 | - |
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

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4             3      76.8        74.2
```

## The last 12 passes

```text
12 pass(es), 3591 steps, 8.2 h of pass time

category                       hours  share  steps  mean s
model                           3.05  37.2%   1723     6.4
wait: other                     1.95  23.7%     55   127.5
cargo test                      0.79   9.6%    112    25.4
shell                           0.47   5.7%    820     2.1
wait: clip                      0.38   4.6%      6   226.9
wait: gate                      0.32   3.9%     12    95.5
phase mark                      0.23   2.8%     95     8.9
cargo build                     0.22   2.7%     27    29.9
clip record/render              0.21   2.6%     87     8.7
wait: build                     0.12   1.4%      3   141.6
push gate                       0.11   1.4%     42     9.6
remote wait                     0.09   1.1%     11    29.0
github                          0.08   1.0%    113     2.6
wait: tests                     0.07   0.8%      7    35.3
cargo publish                   0.03   0.4%      7    14.9

phase                          hours  share  steps  mean s
build                           3.02  36.8%   1190     9.1
ship                            1.94  23.7%    614    11.4
prove                           0.97  11.9%    503     7.0
gate                            0.53   6.4%    215     8.8
release                         0.52   6.3%    110    17.0
log                             0.50   6.1%    251     7.2
choose                          0.32   3.9%    263     4.3
research                        0.26   3.2%    287     3.3
orient                          0.13   1.6%    125     3.7
bugs                            0.01   0.1%     12     2.7

slowest single steps
  pass  219     838s  tool  wait: other          Waiting for --help run
  pass  221     498s  tool  wait: other          {}
  pass  220     424s  tool  wait: clip           Wait for five demo recordings
  pass  222     416s  tool  wait: other          {}
  pass  221     401s  tool  wait: other          {}
  pass  217     361s  tool  wait: other          {}
  pass  220     329s  tool  wait: clip           Wait for hero/frame/move recordings
  pass  220     314s  tool  wait: clip           Wait for final recording batch
  pass  221     310s  tool  wait: other          {}
  pass  220     273s  tool  wait: gate           Wait for the push gate
  pass  216     266s  tool  wait: other          {}
  pass  219     266s  tool  wait: gate           Waiting for the gate
  pass  216     229s  tool  wait: other          {}
  pass  221     219s  tool  wait: other          {}
  pass  216     216s  bg    shell                bg_4 completed: rm -rf root && mkdir root && cp -r /home/ladvien/bevy_emerge_loop/assets r
```

## Every pass recorded

```text
201 pass(es), 19655 steps, 102.7 h of pass time

category                       hours  share  steps  mean s
wait: other                    34.78  33.8%    268   467.2
wait: tests                    19.33  18.8%    115   605.2
model                          16.08  15.6%   9228     6.3
wait: gate                     15.37  15.0%    130   425.6
cargo test                      3.83   3.7%    480    28.7
wait: build                     2.55   2.5%     24   381.9
remote command                  1.78   1.7%    547    11.7
push gate                       1.28   1.2%    375    12.3
wait: review                    1.17   1.1%     33   127.2
wait: clip                      1.09   1.1%     25   157.3
shell                           1.06   1.0%   3811     1.0
cargo build                     0.70   0.7%     88    28.6
phase mark                      0.68   0.7%    302     8.2
wait: release                   0.56   0.5%      6   336.8
remote wait                     0.53   0.5%     71    27.0

phase                          hours  share  steps  mean s
orient                         79.12  77.0%  10291    27.7
build                           6.39   6.2%   2700     8.5
gate                            4.22   4.1%    714    21.3
ship                            3.88   3.8%   1160    12.0
prove                           3.60   3.5%   1464     8.9
choose                          2.24   2.2%   1545     5.2
research                        1.35   1.3%   1013     4.8
log                             1.05   1.0%    394     9.6
release                         0.86   0.8%    241    12.8
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

