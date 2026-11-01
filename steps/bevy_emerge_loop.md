# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 17:33Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 213 | 2026-11-01T17:08Z | 2 | 0 | 4 | - | 3 | 2 | - |
| 214 | 2026-11-01T17:33Z | 3 | 0 | 7 | - | 4 | 2 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            14      15.2        16.4
```

## The last 12 passes

```text
12 pass(es), 1611 steps, 2.9 h of pass time

category                       hours  share  steps  mean s
model                           1.17  39.7%    772     5.5
cargo test                      0.40  13.7%     41    35.5
wait: gate                      0.30  10.0%      9   118.3
wait: other                     0.28   9.6%     20    51.2
phase mark                      0.21   7.1%     84     9.0
push gate                       0.13   4.4%     23    20.5
shell                           0.13   4.4%    334     1.4
remote wait                     0.08   2.9%     13    23.4
github                          0.06   2.0%     72     3.0
clip record/render              0.05   1.7%     32     5.6
wait: tests                     0.05   1.6%      6    28.1
cargo build                     0.04   1.3%      9    15.1
cargo clippy                    0.01   0.4%      2    23.4
sleep                           0.01   0.3%      5     6.7
eval                            0.01   0.3%      1    30.3

phase                          hours  share  steps  mean s
prove                           0.68  23.2%    349     7.1
build                           0.64  21.7%    382     6.0
ship                            0.51  17.3%    140    13.1
log                             0.34  11.5%     61    20.1
research                        0.23   7.8%    209     4.0
choose                          0.22   7.3%    200     3.9
gate                            0.21   7.1%    117     6.5
orient                          0.08   2.7%    102     2.8
bugs                            0.03   0.9%     35     2.6
release                         0.01   0.4%      8     5.4

slowest single steps
  pass  208     240s  tool  wait: gate           Waiting for tag push
  pass  206     214s  tool  wait: gate           Waiting for push gate
  pass  208     187s  tool  wait: gate           Waiting for push gate
  pass  209     131s  tool  wait: other          {}
  pass  212     118s  tool  wait: other          {}
  pass  206     103s  tool  wait: gate           Waiting for push
  pass  203     102s  tool  wait: other          {}
  pass  206     101s  tool  wait: gate           Waiting for push
  pass  214      95s  tool  wait: gate           Waiting for the push gate
  pass  212      94s  tool  wait: other          {}
  pass  204      92s  tool  wait: other          {}
  pass  213      91s  tool  wait: other          {}
  pass  212      86s  bg    cargo test           bg_3 completed: cd /home/ladvien/bevy_emerge_loop; cargo test -p bevy_grimoire_mapper --te
  pass  206      79s  bg    shell                bg_3 completed: cd crates/bevy_grimoire_mapper; sed -i '48i mod a_key_set_in_both_settings
  pass  207      75s  tool  wait: tests          Waiting for clippy and mapper tests
```

## Every pass recorded

```text
194 pass(es), 16913 steps, 95.8 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.96  34.4%    223   532.1
wait: tests                    19.27  20.1%    111   625.1
wait: gate                     15.09  15.8%    121   449.1
model                          13.59  14.2%   7913     6.2
cargo test                      3.17   3.3%    380    30.0
wait: build                     2.43   2.5%     21   416.2
remote command                  1.78   1.9%    547    11.7
push gate                       1.20   1.2%    343    12.6
wait: review                    1.17   1.2%     33   127.2
wait: clip                      0.71   0.7%     19   135.3
shell                           0.68   0.7%   3176     0.8
wait: release                   0.56   0.6%      6   336.8
phase mark                      0.54   0.6%    244     7.9
remote wait                     0.51   0.5%     68    27.1
cargo build                     0.50   0.5%     67    26.8

phase                          hours  share  steps  mean s
orient                         79.03  82.5%  10214    27.9
gate                            3.87   4.0%    610    22.8
build                           3.61   3.8%   1674     7.8
prove                           3.05   3.2%   1201     9.2
choose                          2.08   2.2%   1420     5.3
ship                            2.05   2.1%    604    12.2
research                        1.13   1.2%    782     5.2
log                             0.59   0.6%    160    13.4
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

