# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-02 12:25Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 220 | 2026-11-01T22:54Z | 6 | 0 | 4 | bevy_grimoire_core-v0.6.4 | 17 | 9 | - |
| 221 | 2026-11-02T00:03Z | 5 | 0 | 4 | - | 2 | 0 | demo? |
| 222 | 2026-11-02T01:17Z | 10 | 0 | 11 | - | 2 | 0 | demo? |
| 223 | 2026-11-02T02:42Z | 15 | 0 | 10 | bevy_grimoire-v0.4.0,bevy_grimoire_core-v0.6.5 | 17 | 14 | - |
| 224 | 2026-11-02T04:10Z | 8 | 0 | 12 | - | 16 | 12 | - |
| 225 | 2026-11-02T05:38Z | 15 | 0 | 8 | - | 17 | 13 | - |
| 226 | 2026-11-02T07:04Z | 9 | 0 | 11 | - | 20 | 15 | - |
| 227 | 2026-11-02T08:13Z | 5 | 0 | 8 | - | 15 | 11 | - |
| 228 | 2026-11-02T09:36Z | 8 | 0 | 5 | - | 18 | 13 | - |
| 230 | 2026-11-02T10:48Z | 5 | 0 | 8 | - | 14 | 11 | - |
| 231 | 2026-11-02T11:41Z | 7 | 0 | 2 | - | 16 | 14 | - |
| 232 | 2026-11-02T12:25Z | 9 | 0 | 3 | - | 15 | 13 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4            12      72.3        78.8
```

## The last 12 passes

```text
12 pass(es), 5705 steps, 14.5 h of pass time

category                       hours  share  steps  mean s
model                           4.93  34.1%   2713     6.5
wait: other                     2.80  19.4%     96   105.2
wait: clip                      1.62  11.2%     16   364.8
cargo test                      1.19   8.2%    152    28.3
wait: gate                      0.71   4.9%     16   160.4
shell                           0.60   4.2%   1230     1.8
clip record/render              0.50   3.4%    139    12.9
cargo build                     0.45   3.1%     58    28.1
remote wait                     0.31   2.2%     37    30.6
wait: tests                     0.28   1.9%     25    40.0
phase mark                      0.24   1.6%     94     9.1
eval                            0.16   1.1%     52    11.1
cargo clippy                    0.15   1.1%     29    19.2
github                          0.14   1.0%    176     2.9
wait: build                     0.13   0.9%      7    65.6

phase                          hours  share  steps  mean s
build                           4.00  27.6%   1939     7.4
prove                           3.79  26.1%   1507     9.0
ship                            3.22  22.2%    803    14.4
gate                            1.54  10.7%    391    14.2
choose                          0.72   5.0%    253    10.2
research                        0.70   4.8%    451     5.6
orient                          0.27   1.9%    240     4.0
release                         0.13   0.9%     26    17.5
log                             0.08   0.5%     23    12.5
bugs                            0.04   0.3%     37     3.5

slowest single steps
  pass  227    1424s  tool  wait: clip           Waiting for the demo re-record
  pass  231    1353s  tool  wait: clip           Waiting for clip recorder
  pass  224     840s  tool  wait: other          {}
  pass  232     676s  bg    subagents            RerecordDemos completed: RerecordDemos
  pass  231     675s  tool  wait: clip           Waiting for demo re-recording
  pass  232     549s  tool  wait: other          {}
  pass  221     498s  tool  wait: other          {}
  pass  220     424s  tool  wait: clip           Wait for five demo recordings
  pass  222     416s  tool  wait: other          {}
  pass  221     401s  tool  wait: other          {}
  pass  224     399s  tool  wait: other          {}
  pass  230     390s  tool  wait: clip           Waiting for demo re-recording
  pass  220     329s  tool  wait: clip           Wait for hero/frame/move recordings
  pass  220     314s  tool  wait: clip           Wait for final recording batch
  pass  221     310s  tool  wait: other          {}
```

## Every pass recorded

```text
210 pass(es), 23941 steps, 113.4 h of pass time

category                       hours  share  steps  mean s
wait: other                    36.53  32.2%    336   391.4
model                          19.64  17.3%  11265     6.3
wait: tests                    19.56  17.3%    138   510.4
wait: gate                     15.99  14.1%    144   399.8
cargo test                      4.76   4.2%    586    29.2
wait: build                     2.56   2.3%     28   328.5
wait: clip                      2.34   2.1%     35   240.2
remote command                  1.78   1.6%    547    11.7
shell                           1.35   1.2%   4690     1.0
push gate                       1.31   1.2%    388    12.2
wait: review                    1.27   1.1%     35   130.8
cargo build                     1.13   1.0%    142    28.6
phase mark                      0.87   0.8%    373     8.4
remote wait                     0.85   0.7%    107    28.5
clip record/render              0.72   0.6%    240    10.8

phase                          hours  share  steps  mean s
orient                         79.34  70.0%  10498    27.2
build                           8.47   7.5%   3913     7.8
prove                           7.23   6.4%   2923     8.9
gate                            5.77   5.1%   1105    18.8
ship                            5.70   5.0%   1563    13.1
choose                          2.91   2.6%   1750     6.0
research                        1.92   1.7%   1339     5.2
log                             1.06   0.9%    403     9.5
release                         0.88   0.8%    251    12.7
bugs                            0.08   0.1%     85     3.2

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

