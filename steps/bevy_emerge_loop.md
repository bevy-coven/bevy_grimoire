# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 16:47Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 201 | 2026-11-01T14:08Z | 2 | 0 | 6 | - | 2 | 0 | demo? |
| 202 | 2026-11-01T14:32Z | 2 | 0 | 13 | bevy_grimoire_core-v0.5.1 | 2 | 0 | demo? |
| 203 | 2026-11-01T14:46Z | 4 | 0 | 5 | - | 2 | 0 | demo? |
| 204 | 2026-11-01T15:02Z | 4 | 0 | 4 | - | 1 | 0 | demo? |
| 205 | 2026-11-01T15:09Z | 3 | 0 | 2 | - | 1 | 0 | demo? |
| 206 | 2026-11-01T15:30Z | 5 | 0 | 7 | - | 3 | 0 | release(bevy_grimoire_core),demo? |
| 207 | 2026-11-01T15:47Z | 3 | 0 | 4 | - | 1 | 0 | demo? |
| 208 | 2026-11-01T16:06Z | 3 | 0 | 4 | bevy_grimoire_core-v0.5.2 | 1 | 0 | demo? |
| 209 | 2026-11-01T16:16Z | 5 | 0 | 3 | - | 2 | 0 | demo? |
| 210 | 2026-11-01T16:21Z | 2 | 0 | 0 | - | 2 | 0 | - |
| 211 | 2026-11-01T16:30Z | 1 | 0 | 0 | - | 3 | 2 | - |
| 212 | 2026-11-01T16:47Z | 3 | 0 | 5 | - | 4 | 0 | demo? |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            12      14.0        14.6
```

## The last 12 passes

```text
12 pass(es), 1395 steps, 2.8 h of pass time

category                       hours  share  steps  mean s
model                           1.05  37.6%    658     5.7
cargo test                      0.37  13.4%     41    32.9
wait: other                     0.35  12.7%     20    63.9
wait: gate                      0.29  10.4%      7   149.7
phase mark                      0.25   9.1%     78    11.7
push gate                       0.12   4.5%     17    26.3
wait: tests                     0.09   3.2%      4    80.4
shell                           0.07   2.4%    254     1.0
remote wait                     0.06   2.2%     10    21.7
github                          0.05   1.9%     68     2.8
cargo build                     0.02   0.6%      5    12.4
clip record/render              0.01   0.5%      9     5.9
cargo clippy                    0.01   0.4%      2    20.0
eval                            0.01   0.3%      1    30.3
loop tools                      0.01   0.3%     28     0.9

phase                          hours  share  steps  mean s
build                           0.61  21.8%    415     5.3
prove                           0.56  20.2%    216     9.4
ship                            0.48  17.4%    130    13.4
log                             0.34  12.2%     58    21.1
gate                            0.23   8.3%     48    17.4
research                        0.23   8.3%    208     4.0
choose                          0.20   7.3%    180     4.1
orient                          0.09   3.2%    102     3.2
release                         0.02   0.7%      8     8.2
bugs                            0.02   0.6%     23     2.6

slowest single steps
  pass  208     240s  tool  wait: gate           Waiting for tag push
  pass  206     214s  tool  wait: gate           Waiting for push gate
  pass  201     190s  tool  wait: tests          Waiting for test build
  pass  208     187s  tool  wait: gate           Waiting for push gate
  pass  202     175s  tool  wait: other          {}
  pass  202     166s  tool  wait: other          {}
  pass  201     140s  tool  wait: gate           Waiting for push gate
  pass  209     131s  tool  wait: other          {}
  pass  212     118s  tool  wait: other          {}
  pass  206     103s  tool  wait: gate           Waiting for push
  pass  203     102s  tool  wait: other          {}
  pass  206     101s  tool  wait: gate           Waiting for push
  pass  212      94s  tool  wait: other          {}
  pass  204      92s  tool  wait: other          {}
  pass  212      86s  bg    cargo test           bg_3 completed: cd /home/ladvien/bevy_emerge_loop; cargo test -p bevy_grimoire_mapper --te
```

## Every pass recorded

```text
192 pass(es), 16375 steps, 95.0 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.93  34.6%    220   538.8
wait: tests                    19.26  20.3%    108   642.2
wait: gate                     15.05  15.8%    118   459.1
model                          13.24  13.9%   7651     6.2
cargo test                      3.09   3.2%    374    29.7
wait: build                     2.43   2.6%     21   416.2
remote command                  1.78   1.9%    547    11.7
push gate                       1.17   1.2%    336    12.6
wait: review                    1.17   1.2%     33   127.2
wait: clip                      0.71   0.8%     19   135.3
shell                           0.62   0.7%   3049     0.7
wait: release                   0.56   0.6%      6   336.8
phase mark                      0.50   0.5%    224     8.0
remote wait                     0.49   0.5%     65    27.0
cargo build                     0.48   0.5%     62    27.6

phase                          hours  share  steps  mean s
orient                         79.01  83.1%  10192    27.9
gate                            3.73   3.9%    509    26.4
build                           3.48   3.7%   1589     7.9
prove                           2.79   2.9%   1032     9.7
choose                          2.02   2.1%   1361     5.3
ship                            1.95   2.1%    563    12.5
research                        1.10   1.2%    742     5.3
log                             0.59   0.6%    155    13.7
release                         0.34   0.4%    131     9.4
bugs                            0.03   0.0%     36     3.1

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

