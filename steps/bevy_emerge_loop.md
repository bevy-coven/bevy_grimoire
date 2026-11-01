# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 22:54Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 208 | 2026-11-01T16:06Z | 3 | 0 | 4 | bevy_grimoire_core-v0.5.2 | 1 | 0 | demo? |
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

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4             1      88.0        88.0
```

## The last 12 passes

```text
12 pass(es), 2820 steps, 6.3 h of pass time

category                       hours  share  steps  mean s
model                           2.25  35.7%   1362     5.9
wait: other                     0.95  15.1%     32   107.1
cargo test                      0.60   9.5%     77    28.1
shell                           0.46   7.4%    624     2.7
wait: gate                      0.44   6.9%     14   112.3
wait: clip                      0.38   6.0%      6   226.9
phase mark                      0.27   4.3%     93    10.4
cargo build                     0.20   3.2%     26    27.8
clip record/render              0.17   2.7%     77     7.9
push gate                       0.13   2.1%     39    12.0
wait: build                     0.12   1.9%      3   141.6
remote wait                     0.09   1.4%     11    29.0
wait: tests                     0.08   1.2%      8    34.0
github                          0.06   1.0%     96     2.4
cargo publish                   0.03   0.5%      9    13.5

phase                          hours  share  steps  mean s
build                           2.44  38.8%    837    10.5
prove                           0.89  14.1%    485     6.6
ship                            0.66  10.5%    314     7.6
log                             0.65  10.4%    261     9.0
release                         0.53   8.4%    116    16.4
gate                            0.53   8.4%    215     8.8
choose                          0.31   5.0%    251     4.5
research                        0.18   2.8%    190     3.3
orient                          0.10   1.7%    121     3.1
bugs                            0.01   0.1%     12     2.7

slowest single steps
  pass  219     838s  tool  wait: other          Waiting for --help run
  pass  220     424s  tool  wait: clip           Wait for five demo recordings
  pass  217     361s  tool  wait: other          {}
  pass  220     329s  tool  wait: clip           Wait for hero/frame/move recordings
  pass  220     314s  tool  wait: clip           Wait for final recording batch
  pass  220     273s  tool  wait: gate           Wait for the push gate
  pass  216     266s  tool  wait: other          {}
  pass  219     266s  tool  wait: gate           Waiting for the gate
  pass  208     240s  tool  wait: gate           Waiting for tag push
  pass  216     229s  tool  wait: other          {}
  pass  216     216s  bg    shell                bg_4 completed: rm -rf root && mkdir root && cp -r /home/ladvien/bevy_emerge_loop/assets r
  pass  216     195s  tool  wait: other          {}
  pass  208     187s  tool  wait: gate           Waiting for push gate
  pass  217     186s  tool  wait: other          {}
  pass  217     171s  tool  wait: other          {}
```

## Every pass recorded

```text
199 pass(es), 18702 steps, 100.4 h of pass time

category                       hours  share  steps  mean s
wait: other                    33.74  33.6%    243   499.9
wait: tests                    19.33  19.3%    115   605.2
wait: gate                     15.37  15.3%    130   425.6
model                          15.15  15.1%   8778     6.2
cargo test                      3.58   3.6%    437    29.5
wait: build                     2.55   2.5%     24   381.9
remote command                  1.78   1.8%    547    11.7
push gate                       1.28   1.3%    369    12.5
wait: review                    1.17   1.2%     33   127.2
wait: clip                      1.09   1.1%     25   157.3
shell                           1.03   1.0%   3584     1.0
cargo build                     0.68   0.7%     85    28.6
phase mark                      0.66   0.7%    287     8.3
wait: release                   0.56   0.6%      6   336.8
remote wait                     0.53   0.5%     70    27.4

phase                          hours  share  steps  mean s
orient                         79.08  78.8%  10268    27.7
build                           5.68   5.7%   2283     9.0
gate                            4.22   4.2%    714    21.3
prove                           3.47   3.5%   1430     8.7
ship                            2.59   2.6%    858    10.9
choose                          2.21   2.2%   1511     5.3
research                        1.22   1.2%    888     5.0
log                             0.99   1.0%    380     9.3
release                         0.86   0.9%    241    12.8
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

