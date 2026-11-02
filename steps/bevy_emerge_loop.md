# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-02 08:13Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 227 | 2026-11-02T08:13Z | 5 | 0 | 8 | - | 15 | 11 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4             8      80.5        84.7
```

## The last 12 passes

```text
12 pass(es), 5707 steps, 13.9 h of pass time

category                       hours  share  steps  mean s
model                           5.13  37.0%   2732     6.8
wait: other                     2.86  20.7%     82   125.7
cargo test                      1.38   9.9%    186    26.7
wait: clip                      0.90   6.5%     10   323.4
wait: gate                      0.83   6.0%     21   142.0
shell                           0.50   3.6%   1279     1.4
clip record/render              0.47   3.4%    155    10.8
cargo build                     0.43   3.1%     53    29.1
phase mark                      0.26   1.9%    101     9.4
wait: tests                     0.25   1.8%     22    41.4
remote wait                     0.14   1.0%     22    23.2
wait: build                     0.13   0.9%      7    65.6
github                          0.13   0.9%    175     2.6
cargo clippy                    0.11   0.8%     23    16.7
push gate                       0.09   0.7%     39     8.4

phase                          hours  share  steps  mean s
build                           4.52  32.6%   1991     8.2
ship                            3.06  22.1%    832    13.2
prove                           2.25  16.2%   1040     7.8
gate                            1.71  12.3%    464    13.2
research                        0.78   5.6%    547     5.1
release                         0.54   3.9%    118    16.5
log                             0.46   3.3%    236     7.1
choose                          0.34   2.5%    277     4.5
orient                          0.20   1.4%    166     4.3

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
  pass  223     289s  tool  wait: other          Waiting for retries
  pass  220     273s  tool  wait: gate           Wait for the push gate
  pass  216     266s  tool  wait: other          {}
```

## Every pass recorded

```text
206 pass(es), 22620 steps, 109.7 h of pass time

category                       hours  share  steps  mean s
wait: other                    35.82  32.7%    305   422.8
wait: tests                    19.53  17.8%    133   528.6
model                          18.72  17.1%  10645     6.3
wait: gate                     15.92  14.5%    142   403.7
cargo test                      4.55   4.1%    566    28.9
wait: build                     2.56   2.3%     28   328.5
remote command                  1.78   1.6%    547    11.7
wait: clip                      1.61   1.5%     29   200.2
push gate                       1.29   1.2%    382    12.1
wait: review                    1.22   1.1%     34   129.4
shell                           1.18   1.1%   4455     1.0
cargo build                     0.93   0.8%    120    27.8
phase mark                      0.80   0.7%    345     8.4
remote wait                     0.65   0.6%     90    26.2
clip record/render              0.58   0.5%    218     9.6

phase                          hours  share  steps  mean s
orient                         79.23  72.2%  10380    27.5
build                           8.12   7.4%   3665     8.0
gate                            5.58   5.1%   1074    18.7
prove                           5.30   4.8%   2241     8.5
ship                            5.11   4.7%   1436    12.8
choose                          2.42   2.2%   1697     5.1
research                        1.91   1.7%   1329     5.2
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

