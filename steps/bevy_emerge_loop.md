# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 21:24Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 219 | 2026-11-01T21:24Z | 21 | 0 | 128 | bevy_grimoire-v0.3.3,bevy_grimoire_anim-v0.1.6,bevy_grimoire_core-v0.6.3 | 18 | 2 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
```

## The last 12 passes

```text
12 pass(es), 2519 steps, 5.1 h of pass time

category                       hours  share  steps  mean s
model                           1.96  38.3%   1212     5.8
wait: other                     0.93  18.3%     29   115.9
cargo test                      0.62  12.0%     76    29.2
wait: gate                      0.37   7.2%     13   101.5
phase mark                      0.24   4.7%     92     9.5
cargo build                     0.21   4.2%     26    29.7
shell                           0.19   3.8%    538     1.3
clip record/render              0.16   3.1%     70     8.1
push gate                       0.14   2.8%     37    14.1
remote wait                     0.09   1.7%     11    29.0
github                          0.06   1.1%     89     2.3
wait: tests                     0.05   1.0%      7    25.1
cargo publish                   0.03   0.6%      8    14.6
read files                      0.01   0.3%    116     0.4
loop tools                      0.01   0.3%     48     1.0

phase                          hours  share  steps  mean s
build                           1.32  25.8%    572     8.3
prove                           0.95  18.6%    518     6.6
log                             0.66  12.8%    263     9.0
ship                            0.59  11.6%    228     9.4
gate                            0.53  10.3%    215     8.8
release                         0.43   8.3%    100    15.3
choose                          0.30   5.9%    241     4.5
research                        0.23   4.5%    239     3.4
orient                          0.10   2.0%    117     3.1
bugs                            0.01   0.2%     12     2.7

slowest single steps
  pass  219     838s  tool  wait: other          Waiting for --help run
  pass  217     361s  tool  wait: other          {}
  pass  216     266s  tool  wait: other          {}
  pass  219     266s  tool  wait: gate           Waiting for the gate
  pass  208     240s  tool  wait: gate           Waiting for tag push
  pass  216     229s  tool  wait: other          {}
  pass  216     216s  bg    shell                bg_4 completed: rm -rf root && mkdir root && cp -r /home/ladvien/bevy_emerge_loop/assets r
  pass  216     195s  tool  wait: other          {}
  pass  208     187s  tool  wait: gate           Waiting for push gate
  pass  217     186s  tool  wait: other          {}
  pass  217     171s  tool  wait: other          {}
  pass  217     153s  tool  wait: other          {}
  pass  219     148s  tool  wait: gate           Waiting for the push gate
  pass  209     131s  tool  wait: other          {}
  pass  212     118s  tool  wait: other          {}
```

## Every pass recorded

```text
198 pass(es), 18236 steps, 98.9 h of pass time

category                       hours  share  steps  mean s
wait: other                    33.72  34.1%    240   505.9
wait: tests                    19.29  19.5%    113   614.4
wait: gate                     15.28  15.5%    128   429.8
model                          14.71  14.9%   8552     6.2
cargo test                      3.57   3.6%    434    29.6
wait: build                     2.43   2.5%     21   416.2
remote command                  1.78   1.8%    547    11.7
push gate                       1.28   1.3%    366    12.6
wait: review                    1.17   1.2%     33   127.2
shell                           0.75   0.8%   3460     0.8
wait: clip                      0.71   0.7%     19   135.3
cargo build                     0.68   0.7%     84    29.0
phase mark                      0.63   0.6%    279     8.1
wait: release                   0.56   0.6%      6   336.8
remote wait                     0.53   0.5%     70    27.4

phase                          hours  share  steps  mean s
orient                         79.07  80.0%  10258    27.8
build                           4.48   4.5%   1974     8.2
gate                            4.22   4.3%    714    21.3
prove                           3.45   3.5%   1416     8.8
ship                            2.48   2.5%    760    11.7
choose                          2.19   2.2%   1497     5.3
research                        1.22   1.2%    888     5.0
log                             0.99   1.0%    380     9.3
release                         0.76   0.8%    225    12.1
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

