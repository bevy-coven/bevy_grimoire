# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 17:50Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 204 | 2026-11-01T15:02Z | 4 | 0 | 4 | - | 1 | 0 | demo? |
| 205 | 2026-11-01T15:09Z | 3 | 0 | 2 | - | 1 | 0 | demo? |
| 206 | 2026-11-01T15:30Z | 5 | 0 | 7 | - | 3 | 0 | release(bevy_grimoire_core),demo? |
| 207 | 2026-11-01T15:47Z | 3 | 0 | 4 | - | 1 | 0 | demo? |
| 208 | 2026-11-01T16:06Z | 3 | 0 | 4 | bevy_grimoire_core-v0.5.2 | 1 | 0 | demo? |
| 209 | 2026-11-01T16:16Z | 5 | 0 | 3 | - | 2 | 0 | demo? |
| 210 | 2026-11-01T16:21Z | 2 | 0 | 0 | - | 2 | 0 | - |
| 211 | 2026-11-01T16:30Z | 1 | 0 | 0 | - | 3 | 2 | - |
| 212 | 2026-11-01T16:47Z | 3 | 0 | 5 | - | 4 | 0 | demo? |
| 213 | 2026-11-01T17:08Z | 2 | 0 | 4 | - | 3 | 2 | - |
| 214 | 2026-11-01T17:33Z | 3 | 0 | 7 | - | 4 | 2 | - |
| 215 | 2026-11-01T17:50Z | 2 | 0 | 4 | - | 2 | 0 | demo? |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            15      15.3        16.4
```

## The last 12 passes

```text
12 pass(es), 1670 steps, 3.0 h of pass time

category                       hours  share  steps  mean s
model                           1.23  41.0%    801     5.5
cargo test                      0.40  13.5%     42    34.7
wait: gate                      0.31  10.3%     10   111.8
phase mark                      0.25   8.4%     86    10.5
wait: other                     0.23   7.7%     17    49.1
shell                           0.14   4.8%    348     1.5
push gate                       0.10   3.2%     20    17.2
remote wait                     0.07   2.3%     11    22.2
clip record/render              0.06   2.0%     42     5.3
github                          0.06   1.9%     74     2.8
cargo build                     0.05   1.7%     10    18.8
wait: tests                     0.05   1.6%      6    28.1
cargo clippy                    0.01   0.4%      2    23.4
sleep                           0.01   0.3%      5     6.7
eval                            0.01   0.3%      3    10.6

phase                          hours  share  steps  mean s
prove                           0.76  25.4%    415     6.6
build                           0.65  21.8%    357     6.6
ship                            0.41  13.8%    126    11.8
log                             0.34  11.2%     57    21.3
research                        0.29   9.5%    269     3.8
gate                            0.24   8.1%    123     7.1
choose                          0.19   6.5%    172     4.1
orient                          0.08   2.6%    103     2.7
bugs                            0.02   0.8%     33     2.6
release                         0.01   0.4%      8     5.4

slowest single steps
  pass  208     240s  tool  wait: gate           Waiting for tag push
  pass  206     214s  tool  wait: gate           Waiting for push gate
  pass  208     187s  tool  wait: gate           Waiting for push gate
  pass  209     131s  tool  wait: other          {}
  pass  212     118s  tool  wait: other          {}
  pass  206     103s  tool  wait: gate           Waiting for push
  pass  206     101s  tool  wait: gate           Waiting for push
  pass  214      95s  tool  wait: gate           Waiting for the push gate
  pass  212      94s  tool  wait: other          {}
  pass  204      92s  tool  wait: other          {}
  pass  213      91s  tool  wait: other          {}
  pass  212      86s  bg    cargo test           bg_3 completed: cd /home/ladvien/bevy_emerge_loop; cargo test -p bevy_grimoire_mapper --te
  pass  206      79s  bg    shell                bg_3 completed: cd crates/bevy_grimoire_mapper; sed -i '48i mod a_key_set_in_both_settings
  pass  207      75s  tool  wait: tests          Waiting for clippy and mapper tests
  pass  207      68s  bg    cargo test           bg_2 completed: cargo test -p bevy_grimoire_mapper --test editor a_question_is_asked_in_a_
```

## Every pass recorded

```text
195 pass(es), 17097 steps, 96.1 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.96  34.3%    223   532.1
wait: tests                    19.27  20.1%    111   625.1
wait: gate                     15.11  15.7%    122   445.8
model                          13.72  14.3%   8000     6.2
cargo test                      3.20   3.3%    385    30.0
wait: build                     2.43   2.5%     21   416.2
remote command                  1.78   1.8%    547    11.7
push gate                       1.20   1.2%    343    12.6
wait: review                    1.17   1.2%     33   127.2
wait: clip                      0.71   0.7%     19   135.3
shell                           0.70   0.7%   3214     0.8
phase mark                      0.59   0.6%    253     8.4
wait: release                   0.56   0.6%      6   336.8
cargo build                     0.51   0.5%     68    27.2
remote wait                     0.51   0.5%     68    27.1

phase                          hours  share  steps  mean s
orient                         79.03  82.3%  10223    27.8
gate                            3.91   4.1%    616    22.8
build                           3.66   3.8%   1686     7.8
prove                           3.16   3.3%   1281     8.9
choose                          2.09   2.2%   1428     5.3
ship                            2.06   2.1%    610    12.2
research                        1.19   1.2%    842     5.1
log                             0.60   0.6%    162    13.3
release                         0.34   0.4%    133     9.3
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

