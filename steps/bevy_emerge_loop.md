# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 18:40Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 216 | 2026-11-01T18:40Z | 13 | 0 | 8 | bevy_grimoire-v0.3.2,bevy_grimoire_anim-v0.1.5,bevy_grimoire_core-v0.6.0,bevy_grimoire_core-v0.6.1 | 8 | 4 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            16      17.5        16.5
```

## The last 12 passes

```text
12 pass(es), 1964 steps, 3.6 h of pass time

category                       hours  share  steps  mean s
model                           1.41  39.4%    940     5.4
wait: other                     0.45  12.5%     23    70.2
cargo test                      0.45  12.4%     48    33.4
wait: gate                      0.31   8.7%     10   111.8
phase mark                      0.25   7.0%     91     9.9
shell                           0.18   5.0%    403     1.6
clip record/render              0.12   3.2%     54     7.7
cargo build                     0.09   2.5%     13    24.7
remote wait                     0.09   2.5%     13    24.6
push gate                       0.06   1.8%     22    10.6
github                          0.06   1.6%     82     2.5
wait: tests                     0.05   1.3%      6    28.1
cargo publish                   0.02   0.7%      6    14.8
cargo clippy                    0.01   0.4%      2    23.4
sleep                           0.01   0.3%      5     6.7

phase                          hours  share  steps  mean s
prove                           0.84  23.5%    456     6.6
build                           0.63  17.6%    347     6.5
gate                            0.50  14.0%    209     8.6
log                             0.50  13.9%    183     9.8
ship                            0.36  10.1%    120    10.8
research                        0.28   7.9%    283     3.6
choose                          0.19   5.4%    172     4.1
release                         0.17   4.8%     52    12.0
orient                          0.08   2.1%     99     2.8
bugs                            0.02   0.7%     33     2.6

slowest single steps
  pass  216     266s  tool  wait: other          {}
  pass  208     240s  tool  wait: gate           Waiting for tag push
  pass  216     229s  tool  wait: other          {}
  pass  216     216s  bg    shell                bg_4 completed: rm -rf root && mkdir root && cp -r /home/ladvien/bevy_emerge_loop/assets r
  pass  206     214s  tool  wait: gate           Waiting for push gate
  pass  216     195s  tool  wait: other          {}
  pass  208     187s  tool  wait: gate           Waiting for push gate
  pass  209     131s  tool  wait: other          {}
  pass  212     118s  tool  wait: other          {}
  pass  206     103s  tool  wait: gate           Waiting for push
  pass  206     101s  tool  wait: gate           Waiting for push
  pass  214      95s  tool  wait: gate           Waiting for the push gate
  pass  212      94s  tool  wait: other          {}
  pass  213      91s  tool  wait: other          {}
  pass  212      86s  bg    cargo test           bg_3 completed: cd /home/ladvien/bevy_emerge_loop; cargo test -p bevy_grimoire_mapper --te
```

## Every pass recorded

```text
196 pass(es), 17491 steps, 96.9 h of pass time

category                       hours  share  steps  mean s
wait: other                    33.22  34.3%    232   515.5
wait: tests                    19.27  19.9%    111   625.1
wait: gate                     15.11  15.6%    122   445.8
model                          13.99  14.4%   8188     6.2
cargo test                      3.30   3.4%    397    29.9
wait: build                     2.43   2.5%     21   416.2
remote command                  1.78   1.8%    547    11.7
push gate                       1.20   1.2%    347    12.4
wait: review                    1.17   1.2%     33   127.2
shell                           0.73   0.8%   3287     0.8
wait: clip                      0.71   0.7%     19   135.3
phase mark                      0.61   0.6%    265     8.3
wait: release                   0.56   0.6%      6   336.8
cargo build                     0.55   0.6%     71    27.9
remote wait                     0.53   0.6%     70    27.4

phase                          hours  share  steps  mean s
orient                         79.04  81.6%  10229    27.8
gate                            4.16   4.3%    702    21.4
build                           3.71   3.8%   1704     7.8
prove                           3.26   3.4%   1330     8.8
choose                          2.09   2.2%   1428     5.3
ship                            2.06   2.1%    614    12.1
research                        1.22   1.3%    888     5.0
log                             0.83   0.9%    300     9.9
release                         0.50   0.5%    177    10.3
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

