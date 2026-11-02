# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-02 00:03Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 209 | 2026-11-01T16:16Z | 5 | 0 | 3 | - | 2 | 0 | demo? |
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

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4             2      78.1        78.1
```

## The last 12 passes

```text
12 pass(es), 3108 steps, 7.1 h of pass time

category                       hours  share  steps  mean s
model                           2.51  35.2%   1495     6.0
wait: other                     1.58  22.2%     45   126.6
cargo test                      0.66   9.2%     92    25.8
shell                           0.46   6.5%    685     2.4
wait: clip                      0.38   5.3%      6   226.9
wait: gate                      0.32   4.5%     12    95.5
phase mark                      0.24   3.4%     92     9.4
clip record/render              0.21   3.0%     84     9.0
cargo build                     0.20   2.8%     26    27.8
wait: build                     0.12   1.7%      3   141.6
push gate                       0.11   1.6%     40    10.2
remote wait                     0.09   1.2%     10    31.9
github                          0.07   1.0%    102     2.6
wait: tests                     0.07   1.0%      7    35.3
cargo publish                   0.03   0.4%      7    14.9

phase                          hours  share  steps  mean s
build                           2.59  36.3%    963     9.7
ship                            1.37  19.2%    465    10.6
prove                           0.99  13.8%    509     7.0
log                             0.56   7.8%    259     7.8
gate                            0.53   7.4%    215     8.8
release                         0.52   7.3%    110    17.0
choose                          0.32   4.5%    265     4.3
research                        0.14   2.0%    170     3.0
orient                          0.12   1.6%    122     3.5
bugs                            0.01   0.1%     12     2.7

slowest single steps
  pass  219     838s  tool  wait: other          Waiting for --help run
  pass  221     498s  tool  wait: other          {}
  pass  220     424s  tool  wait: clip           Wait for five demo recordings
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
  pass  216     195s  tool  wait: other          {}
```

## Every pass recorded

```text
200 pass(es), 19086 steps, 101.5 h of pass time

category                       hours  share  steps  mean s
wait: other                    34.37  33.9%    256   483.4
wait: tests                    19.33  19.0%    115   605.2
model                          15.47  15.2%   8958     6.2
wait: gate                     15.37  15.1%    130   425.6
cargo test                      3.67   3.6%    456    29.0
wait: build                     2.55   2.5%     24   381.9
remote command                  1.78   1.8%    547    11.7
push gate                       1.28   1.3%    372    12.4
wait: review                    1.17   1.1%     33   127.2
wait: clip                      1.09   1.1%     25   157.3
shell                           1.05   1.0%   3660     1.0
cargo build                     0.68   0.7%     85    28.6
phase mark                      0.67   0.7%    294     8.2
wait: release                   0.56   0.6%      6   336.8
remote wait                     0.53   0.5%     70    27.4

phase                          hours  share  steps  mean s
orient                         79.10  77.9%  10280    27.7
build                           5.87   5.8%   2419     8.7
gate                            4.22   4.2%    714    21.3
prove                           3.60   3.5%   1464     8.9
ship                            3.31   3.3%   1011    11.8
choose                          2.24   2.2%   1539     5.2
research                        1.23   1.2%    896     4.9
log                             1.05   1.0%    392     9.6
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

