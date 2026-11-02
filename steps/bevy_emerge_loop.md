# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-02 09:36Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 227 | 2026-11-02T08:13Z | 5 | 0 | 8 | - | 15 | 11 | - |
| 228 | 2026-11-02T09:36Z | 8 | 0 | 5 | - | 18 | 13 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4             9      80.8        83.9
```

## The last 12 passes

```text
12 pass(es), 6193 steps, 15.0 h of pass time

category                       hours  share  steps  mean s
model                           5.42  36.2%   2957     6.6
wait: other                     3.23  21.6%    101   115.3
cargo test                      1.39   9.3%    185    27.1
wait: clip                      0.90   6.0%     10   323.4
wait: gate                      0.81   5.4%     20   146.5
shell                           0.60   4.0%   1369     1.6
clip record/render              0.55   3.7%    158    12.6
cargo build                     0.53   3.6%     64    30.0
wait: tests                     0.25   1.7%     22    41.4
phase mark                      0.24   1.6%     99     8.6
remote wait                     0.20   1.3%     29    24.9
eval                            0.16   1.1%     53    11.0
github                          0.13   0.9%    187     2.5
wait: build                     0.13   0.9%      7    65.6
cargo clippy                    0.12   0.8%     25    16.9

phase                          hours  share  steps  mean s
build                           4.59  30.7%   2096     7.9
ship                            3.39  22.6%    910    13.4
prove                           3.01  20.1%   1349     8.0
gate                            1.67  11.2%    458    13.2
research                        0.72   4.8%    487     5.3
release                         0.54   3.6%    118    16.5
log                             0.46   3.1%    236     7.1
choose                          0.34   2.3%    269     4.5
orient                          0.25   1.7%    231     3.9

slowest single steps
  pass  227    1424s  tool  wait: clip           Waiting for the demo re-record
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
  pass  228     300s  bg    shell                bg_4 failed: grep -rn "bevy_elements.ron" docs/guide/*.md | head -5; ls /tmp/*/family/bevy
  pass  223     289s  tool  wait: other          Waiting for retries
  pass  220     273s  tool  wait: gate           Wait for the push gate
```

## Every pass recorded

```text
207 pass(es), 23290 steps, 111.0 h of pass time

category                       hours  share  steps  mean s
wait: other                    36.19  32.6%    324   402.1
wait: tests                    19.53  17.6%    133   528.6
model                          19.14  17.2%  10957     6.3
wait: gate                     15.92  14.3%    142   403.7
cargo test                      4.60   4.1%    570    29.0
wait: build                     2.56   2.3%     28   328.5
remote command                  1.78   1.6%    547    11.7
wait: clip                      1.61   1.5%     29   200.2
shell                           1.30   1.2%   4583     1.0
push gate                       1.29   1.2%    384    12.1
wait: review                    1.22   1.1%     34   129.4
cargo build                     1.05   0.9%    132    28.6
phase mark                      0.82   0.7%    352     8.4
remote wait                     0.71   0.6%     97    26.5
clip record/render              0.68   0.6%    231    10.6

phase                          hours  share  steps  mean s
orient                         79.28  71.4%  10454    27.3
build                           8.25   7.4%   3782     7.9
prove                           6.17   5.6%   2630     8.4
gate                            5.58   5.0%   1074    18.7
ship                            5.45   4.9%   1520    12.9
choose                          2.42   2.2%   1697     5.1
research                        1.91   1.7%   1329     5.2
log                             1.06   1.0%    398     9.6
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

