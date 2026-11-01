# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 17:08Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 213 | 2026-11-01T17:08Z | 2 | 0 | 4 | - | 3 | 2 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            13      14.5        16.1
```

## The last 12 passes

```text
12 pass(es), 1523 steps, 2.9 h of pass time

category                       hours  share  steps  mean s
model                           1.14  38.9%    722     5.7
cargo test                      0.41  13.9%     42    34.9
wait: other                     0.39  13.2%     23    60.3
wait: gate                      0.25   8.6%      6   151.2
phase mark                      0.23   8.0%     81    10.4
push gate                       0.13   4.4%     19    24.1
shell                           0.12   4.2%    292     1.5
remote wait                     0.06   2.1%     10    21.7
github                          0.06   2.0%     72     2.9
clip record/render              0.05   1.6%     22     7.7
wait: tests                     0.04   1.3%      3    44.0
cargo build                     0.02   0.6%      6    10.3
cargo clippy                    0.01   0.4%      2    20.0
eval                            0.01   0.3%      1    30.3
loop tools                      0.01   0.2%     29     0.8

phase                          hours  share  steps  mean s
build                           0.64  21.8%    429     5.4
prove                           0.62  21.1%    286     7.8
ship                            0.47  16.0%    142    11.8
log                             0.34  11.7%     60    20.5
choose                          0.26   9.0%    231     4.1
gate                            0.26   8.9%     50    18.7
research                        0.20   6.7%    169     4.2
orient                          0.10   3.3%    104     3.3
bugs                            0.03   0.9%     35     2.6
release                         0.02   0.7%     10     7.6

slowest single steps
  pass  208     240s  tool  wait: gate           Waiting for tag push
  pass  206     214s  tool  wait: gate           Waiting for push gate
  pass  208     187s  tool  wait: gate           Waiting for push gate
  pass  202     175s  tool  wait: other          {}
  pass  202     166s  tool  wait: other          {}
  pass  209     131s  tool  wait: other          {}
  pass  212     118s  tool  wait: other          {}
  pass  206     103s  tool  wait: gate           Waiting for push
  pass  203     102s  tool  wait: other          {}
  pass  206     101s  tool  wait: gate           Waiting for push
  pass  212      94s  tool  wait: other          {}
  pass  204      92s  tool  wait: other          {}
  pass  213      91s  tool  wait: other          {}
  pass  212      86s  bg    cargo test           bg_3 completed: cd /home/ladvien/bevy_emerge_loop; cargo test -p bevy_grimoire_mapper --te
  pass  206      79s  bg    shell                bg_3 completed: cd crates/bevy_grimoire_mapper; sed -i '48i mod a_key_set_in_both_settings
```

## Every pass recorded

```text
193 pass(es), 16591 steps, 95.4 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.96  34.6%    223   532.1
wait: tests                    19.26  20.2%    108   642.2
wait: gate                     15.05  15.8%    118   459.1
model                          13.39  14.0%   7757     6.2
cargo test                      3.12   3.3%    376    29.9
wait: build                     2.43   2.5%     21   416.2
remote command                  1.78   1.9%    547    11.7
push gate                       1.18   1.2%    338    12.5
wait: review                    1.17   1.2%     33   127.2
wait: clip                      0.71   0.7%     19   135.3
shell                           0.68   0.7%   3097     0.8
wait: release                   0.56   0.6%      6   336.8
phase mark                      0.53   0.6%    233     8.2
remote wait                     0.49   0.5%     65    27.0
cargo build                     0.48   0.5%     63    27.2

phase                          hours  share  steps  mean s
orient                         79.02  82.8%  10204    27.9
gate                            3.78   4.0%    517    26.3
build                           3.51   3.7%   1603     7.9
prove                           2.92   3.1%   1116     9.4
choose                          2.08   2.2%   1412     5.3
ship                            2.00   2.1%    593    12.2
research                        1.10   1.1%    742     5.3
log                             0.59   0.6%    157    13.6
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

