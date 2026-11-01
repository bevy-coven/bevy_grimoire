# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 19:24Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 217 | 2026-11-01T19:24Z | 8 | 0 | 10 | bevy_grimoire_core-v0.6.2 | 5 | 2 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            17      19.0        16.6
```

## The last 12 passes

```text
12 pass(es), 2205 steps, 4.2 h of pass time

category                       hours  share  steps  mean s
model                           1.66  39.5%   1056     5.7
wait: other                     0.67  16.0%     27    89.9
cargo test                      0.53  12.6%     64    29.7
wait: gate                      0.31   7.4%     10   111.8
phase mark                      0.23   5.5%     91     9.2
shell                           0.19   4.5%    465     1.5
clip record/render              0.14   3.3%     64     7.7
cargo build                     0.11   2.5%     14    27.2
remote wait                     0.09   2.1%     13    24.6
push gate                       0.08   2.0%     25    11.9
github                          0.06   1.4%     87     2.4
wait: tests                     0.05   1.1%      6    28.1
cargo publish                   0.03   0.7%      7    14.2
read files                      0.01   0.3%    111     0.4
cargo clippy                    0.01   0.3%      2    23.4

phase                          hours  share  steps  mean s
prove                           0.98  23.4%    528     6.7
ship                            0.65  15.4%    238     9.8
build                           0.61  14.6%    327     6.8
gate                            0.53  12.5%    215     8.8
log                             0.50  11.9%    183     9.8
choose                          0.30   7.1%    232     4.6
research                        0.28   6.7%    283     3.6
release                         0.25   6.1%     64    14.3
orient                          0.08   2.0%    105     2.9
bugs                            0.01   0.3%     18     2.7

slowest single steps
  pass  217     361s  tool  wait: other          {}
  pass  216     266s  tool  wait: other          {}
  pass  208     240s  tool  wait: gate           Waiting for tag push
  pass  216     229s  tool  wait: other          {}
  pass  216     216s  bg    shell                bg_4 completed: rm -rf root && mkdir root && cp -r /home/ladvien/bevy_emerge_loop/assets r
  pass  206     214s  tool  wait: gate           Waiting for push gate
  pass  216     195s  tool  wait: other          {}
  pass  208     187s  tool  wait: gate           Waiting for push gate
  pass  217     186s  tool  wait: other          {}
  pass  217     171s  tool  wait: other          {}
  pass  217     153s  tool  wait: other          {}
  pass  209     131s  tool  wait: other          {}
  pass  212     118s  tool  wait: other          {}
  pass  217     104s  bg    shell                bg_4 completed: sed -i '/eprintln!("DBG/d' tests/a_person_finds_and_runs_a_command_in_the_
  pass  206     103s  tool  wait: gate           Waiting for push
```

## Every pass recorded

```text
197 pass(es), 17794 steps, 97.7 h of pass time

category                       hours  share  steps  mean s
wait: other                    33.46  34.3%    238   506.2
wait: tests                    19.27  19.7%    111   625.1
wait: gate                     15.11  15.5%    122   445.8
model                          14.28  14.6%   8334     6.2
cargo test                      3.41   3.5%    417    29.5
wait: build                     2.43   2.5%     21   416.2
remote command                  1.78   1.8%    547    11.7
push gate                       1.22   1.2%    350    12.5
wait: review                    1.17   1.2%     33   127.2
shell                           0.74   0.8%   3363     0.8
wait: clip                      0.71   0.7%     19   135.3
phase mark                      0.61   0.6%    271     8.2
cargo build                     0.57   0.6%     72    28.4
wait: release                   0.56   0.6%      6   336.8
remote wait                     0.53   0.5%     70    27.4

phase                          hours  share  steps  mean s
orient                         79.05  81.0%  10240    27.8
gate                            4.22   4.3%    714    21.3
build                           3.71   3.8%   1704     7.8
prove                           3.45   3.5%   1416     8.8
ship                            2.35   2.4%    734    11.5
choose                          2.19   2.2%   1488     5.3
research                        1.22   1.3%    888     5.0
log                             0.83   0.8%    300     9.9
release                         0.59   0.6%    189    11.2
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

