# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-02 02:42Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 223 | 2026-11-02T02:42Z | 15 | 0 | 10 | bevy_grimoire-v0.4.0,bevy_grimoire_core-v0.6.5 | 17 | 14 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4             4      78.6        79.1
```

## The last 12 passes

```text
12 pass(es), 4161 steps, 9.5 h of pass time

category                       hours  share  steps  mean s
model                           3.58  37.5%   1998     6.4
wait: other                     2.05  21.5%     58   127.2
cargo test                      0.92   9.7%    126    26.4
wait: gate                      0.64   6.8%     19   122.2
shell                           0.52   5.4%    921     2.0
wait: clip                      0.46   4.8%      8   205.2
phase mark                      0.24   2.5%    105     8.1
cargo build                     0.23   2.4%     29    28.1
clip record/render              0.22   2.3%     96     8.1
wait: tests                     0.13   1.3%     12    37.8
wait: build                     0.12   1.2%      3   141.6
push gate                       0.12   1.2%     43     9.6
github                          0.09   0.9%    126     2.6
remote wait                     0.09   0.9%     14    22.8
read files                      0.04   0.5%    230     0.7

phase                          hours  share  steps  mean s
build                           3.36  35.2%   1373     8.8
ship                            2.34  24.6%    725    11.6
gate                            1.05  11.0%    388     9.8
prove                           0.98  10.3%    517     6.8
release                         0.54   5.7%    120    16.3
log                             0.47   4.9%    245     6.9
choose                          0.33   3.5%    283     4.2
research                        0.31   3.3%    343     3.3
orient                          0.14   1.5%    134     3.7
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
  pass  223     289s  tool  wait: other          Waiting for retries
  pass  220     273s  tool  wait: gate           Wait for the push gate
  pass  216     266s  tool  wait: other          {}
  pass  219     266s  tool  wait: gate           Waiting for the gate
  pass  223     262s  tool  wait: gate           Waiting for release push
  pass  223     232s  tool  wait: gate           Waiting for gate push
```

## Every pass recorded

```text
202 pass(es), 20263 steps, 104.2 h of pass time

category                       hours  share  steps  mean s
wait: other                    34.89  33.5%    272   461.8
wait: tests                    19.39  18.6%    120   581.7
model                          16.63  16.0%   9521     6.3
wait: gate                     15.69  15.1%    137   412.4
cargo test                      3.97   3.8%    495    28.9
wait: build                     2.55   2.4%     24   381.9
remote command                  1.78   1.7%    547    11.7
push gate                       1.29   1.2%    376    12.3
wait: clip                      1.17   1.1%     27   156.0
wait: review                    1.17   1.1%     33   127.2
shell                           1.10   1.1%   3916     1.0
phase mark                      0.72   0.7%    317     8.2
cargo build                     0.70   0.7%     90    28.1
wait: release                   0.56   0.5%      6   336.8
remote wait                     0.53   0.5%     74    25.9

phase                          hours  share  steps  mean s
orient                         79.13  76.0%  10306    27.6
build                           6.74   6.5%   2887     8.4
gate                            4.75   4.6%    887    19.3
ship                            4.28   4.1%   1271    12.1
prove                           3.62   3.5%   1484     8.8
choose                          2.26   2.2%   1565     5.2
research                        1.41   1.4%   1085     4.7
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

