# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-02 05:38Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 225 | 2026-11-02T05:38Z | 15 | 0 | 8 | - | 17 | 13 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4             6      81.7        85.7
```

## The last 12 passes

```text
12 pass(es), 5262 steps, 12.1 h of pass time

category                       hours  share  steps  mean s
model                           4.58  38.0%   2514     6.6
wait: other                     2.87  23.8%     84   123.1
cargo test                      1.09   9.0%    152    25.8
wait: gate                      0.64   5.3%     19   122.2
shell                           0.55   4.5%   1208     1.6
wait: clip                      0.46   3.8%      8   205.2
clip record/render              0.43   3.5%    149    10.3
cargo build                     0.37   3.1%     48    27.6
phase mark                      0.24   2.0%    108     8.1
wait: tests                     0.14   1.1%     13    38.0
github                          0.13   1.0%    152     3.0
wait: build                     0.12   1.0%      3   141.6
push gate                       0.11   0.9%     45     9.1
remote wait                     0.11   0.9%     19    20.4
cargo clippy                    0.08   0.7%     16    18.0

phase                          hours  share  steps  mean s
build                           4.31  35.8%   1881     8.3
ship                            3.09  25.6%    869    12.8
prove                           1.45  12.0%    832     6.3
gate                            1.32  10.9%    462    10.3
release                         0.54   4.5%    120    16.3
log                             0.46   3.8%    239     7.0
choose                          0.36   3.0%    300     4.4
research                        0.32   2.7%    355     3.3
orient                          0.19   1.6%    162     4.2
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
204 pass(es), 21637 steps, 107.1 h of pass time

category                       hours  share  steps  mean s
wait: other                    35.80  33.4%    304   423.9
wait: tests                    19.40  18.1%    121   577.2
model                          17.82  16.6%  10165     6.3
wait: gate                     15.69  14.7%    137   412.4
cargo test                      4.17   3.9%    526    28.6
wait: build                     2.55   2.4%     24   381.9
remote command                  1.78   1.7%    547    11.7
push gate                       1.29   1.2%    381    12.2
wait: clip                      1.17   1.1%     27   156.0
wait: review                    1.17   1.1%     33   127.2
shell                           1.17   1.1%   4257     1.0
cargo build                     0.84   0.8%    110    27.6
phase mark                      0.74   0.7%    332     8.1
remote wait                     0.60   0.6%     84    25.5
wait: release                   0.56   0.5%      6   336.8

phase                          hours  share  steps  mean s
orient                         79.20  73.9%  10354    27.5
build                           7.79   7.3%   3470     8.1
gate                            5.05   4.7%    971    18.7
ship                            5.04   4.7%   1432    12.7
prove                           4.24   4.0%   1864     8.2
choose                          2.38   2.2%   1661     5.2
research                        1.42   1.3%   1097     4.7
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

