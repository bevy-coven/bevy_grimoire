# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-02 10:48Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 230 | 2026-11-02T10:48Z | 5 | 0 | 8 | - | 14 | 11 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4            10      77.0        83.6
```

## The last 12 passes

```text
12 pass(es), 6052 steps, 14.8 h of pass time

category                       hours  share  steps  mean s
model                           5.37  36.2%   2893     6.7
wait: other                     2.97  20.0%     92   116.4
cargo test                      1.37   9.2%    180    27.5
wait: clip                      1.01   6.8%     13   280.4
wait: gate                      0.85   5.7%     21   146.3
shell                           0.62   4.1%   1342     1.7
clip record/render              0.52   3.5%    151    12.5
cargo build                     0.51   3.5%     66    28.0
wait: tests                     0.28   1.9%     26    39.0
remote wait                     0.23   1.6%     31    27.1
phase mark                      0.22   1.5%     94     8.3
eval                            0.16   1.1%     53    11.0
github                          0.14   0.9%    183     2.7
cargo clippy                    0.13   0.9%     27    18.0
wait: build                     0.13   0.9%      7    65.6

phase                          hours  share  steps  mean s
build                           4.70  31.7%   2157     7.8
ship                            3.63  24.5%    949    13.8
prove                           3.09  20.8%   1372     8.1
gate                            1.47   9.9%    385    13.8
research                        0.70   4.7%    451     5.6
release                         0.38   2.6%     74    18.4
choose                          0.34   2.3%    269     4.5
orient                          0.30   2.0%    258     4.1
log                             0.24   1.6%    100     8.5

slowest single steps
  pass  227    1424s  tool  wait: clip           Waiting for the demo re-record
  pass  224     840s  tool  wait: other          {}
  pass  219     838s  tool  wait: other          Waiting for --help run
  pass  221     498s  tool  wait: other          {}
  pass  220     424s  tool  wait: clip           Wait for five demo recordings
  pass  222     416s  tool  wait: other          {}
  pass  221     401s  tool  wait: other          {}
  pass  224     399s  tool  wait: other          {}
  pass  230     390s  tool  wait: clip           Waiting for demo re-recording
  pass  217     361s  tool  wait: other          {}
  pass  220     329s  tool  wait: clip           Wait for hero/frame/move recordings
  pass  220     314s  tool  wait: clip           Wait for final recording batch
  pass  221     310s  tool  wait: other          {}
  pass  228     300s  bg    shell                bg_4 failed: grep -rn "bevy_elements.ron" docs/guide/*.md | head -5; ls /tmp/*/family/bevy
  pass  223     289s  tool  wait: other          Waiting for retries
```

## Every pass recorded

```text
208 pass(es), 23543 steps, 111.8 h of pass time

category                       hours  share  steps  mean s
wait: other                    36.19  32.4%    324   402.1
wait: tests                    19.56  17.5%    137   513.9
model                          19.36  17.3%  11081     6.3
wait: gate                     15.96  14.3%    143   401.8
cargo test                      4.67   4.2%    577    29.2
wait: build                     2.56   2.3%     28   328.5
remote command                  1.78   1.6%    547    11.7
wait: clip                      1.73   1.5%     32   194.3
shell                           1.35   1.2%   4629     1.0
push gate                       1.29   1.2%    385    12.1
wait: review                    1.27   1.1%     35   130.8
cargo build                     1.06   1.0%    137    27.9
phase mark                      0.83   0.7%    359     8.3
remote wait                     0.77   0.7%    101    27.3
clip record/render              0.70   0.6%    236    10.7

phase                          hours  share  steps  mean s
orient                         79.34  71.0%  10487    27.2
build                           8.41   7.5%   3861     7.8
prove                           6.35   5.7%   2702     8.5
ship                            5.70   5.1%   1563    13.1
gate                            5.64   5.0%   1087    18.7
choose                          2.42   2.2%   1697     5.1
research                        1.92   1.7%   1339     5.2
log                             1.06   1.0%    400     9.6
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

