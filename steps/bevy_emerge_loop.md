# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-02 11:41Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 219 | 2026-11-01T21:24Z | 21 | 0 | 128 | bevy_grimoire-v0.3.3,bevy_grimoire_anim-v0.1.6,bevy_grimoire_core-v0.6.3 | 18 | 2 | - |
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

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4            11      74.9        83.3
```

## The last 12 passes

```text
12 pass(es), 5900 steps, 15.0 h of pass time

category                       hours  share  steps  mean s
model                           5.19  34.6%   2818     6.6
wait: other                     2.79  18.6%     87   115.5
wait: clip                      1.62  10.8%     16   364.8
cargo test                      1.27   8.5%    162    28.3
wait: gate                      0.89   5.9%     22   144.8
shell                           0.60   4.0%   1284     1.7
clip record/render              0.50   3.3%    142    12.7
cargo build                     0.50   3.3%     65    27.5
wait: tests                     0.29   1.9%     27    38.5
phase mark                      0.25   1.7%     95     9.5
remote wait                     0.23   1.6%     31    27.1
eval                            0.16   1.1%     53    11.0
cargo clippy                    0.15   1.0%     28    18.8
github                          0.14   0.9%    182     2.7
wait: build                     0.13   0.8%      7    65.6

phase                          hours  share  steps  mean s
build                           4.71  31.4%   2180     7.8
ship                            3.34  22.3%    829    14.5
prove                           3.22  21.4%   1336     8.7
gate                            1.47   9.8%    379    13.9
choose                          0.72   4.8%    262     9.9
research                        0.70   4.7%    451     5.6
release                         0.30   2.0%     62    17.3
orient                          0.29   1.9%    253     4.1
log                             0.24   1.6%    102     8.3
bugs                            0.01   0.1%     11     4.3

slowest single steps
  pass  227    1424s  tool  wait: clip           Waiting for the demo re-record
  pass  231    1353s  tool  wait: clip           Waiting for clip recorder
  pass  224     840s  tool  wait: other          {}
  pass  219     838s  tool  wait: other          Waiting for --help run
  pass  231     675s  tool  wait: clip           Waiting for demo re-recording
  pass  221     498s  tool  wait: other          {}
  pass  220     424s  tool  wait: clip           Wait for five demo recordings
  pass  222     416s  tool  wait: other          {}
  pass  221     401s  tool  wait: other          {}
  pass  224     399s  tool  wait: other          {}
  pass  230     390s  tool  wait: clip           Waiting for demo re-recording
  pass  220     329s  tool  wait: clip           Wait for hero/frame/move recordings
  pass  220     314s  tool  wait: clip           Wait for final recording batch
  pass  221     310s  tool  wait: other          {}
  pass  228     300s  bg    shell                bg_4 failed: grep -rn "bevy_elements.ron" docs/guide/*.md | head -5; ls /tmp/*/family/bevy
```

## Every pass recorded

```text
209 pass(es), 23694 steps, 112.7 h of pass time

category                       hours  share  steps  mean s
wait: other                    36.26  32.2%    325   401.6
wait: tests                    19.56  17.4%    138   510.4
model                          19.47  17.3%  11152     6.3
wait: gate                     15.99  14.2%    144   399.8
cargo test                      4.69   4.2%    579    29.1
wait: build                     2.56   2.3%     28   328.5
wait: clip                      2.34   2.1%     35   240.2
remote command                  1.78   1.6%    547    11.7
shell                           1.35   1.2%   4647     1.0
push gate                       1.29   1.1%    386    12.0
wait: review                    1.27   1.1%     35   130.8
cargo build                     1.06   0.9%    137    27.9
phase mark                      0.86   0.8%    366     8.5
remote wait                     0.77   0.7%    101    27.3
clip record/render              0.70   0.6%    237    10.7

phase                          hours  share  steps  mean s
orient                         79.34  70.4%  10493    27.2
build                           8.43   7.5%   3884     7.8
prove                           6.66   5.9%   2752     8.7
ship                            5.70   5.1%   1563    13.1
gate                            5.69   5.1%   1093    18.7
choose                          2.91   2.6%   1750     6.0
research                        1.92   1.7%   1339     5.2
log                             1.06   0.9%    402     9.5
release                         0.88   0.8%    251    12.7
bugs                            0.05   0.0%     59     3.3

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

