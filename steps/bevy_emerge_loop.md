# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-03 16:25Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 259 | 2026-11-03T03:02Z | 2 | 0 | 4 | - | 1 | 0 | demo? |
| 260 | 2026-11-03T03:16Z | 2 | 0 | 4 | - | 1 | 0 | demo? |
| 261 | 2026-11-03T03:32Z | 2 | 0 | 0 | - | 1 | 0 | - |
| 262 | 2026-11-03T03:56Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 263 | 2026-11-03T04:09Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 264 | 2026-11-03T04:30Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 265 | 2026-11-03T04:45Z | 1 | 0 | 0 | - | 1 | 0 | - |
| 266 | 2026-11-03T04:53Z | 1 | 0 | 0 | - | 1 | 0 | - |
| 267 | 2026-11-03T05:11Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 268 | 2026-11-03T05:33Z | 2 | 0 | 6 | - | 1 | 0 | demo? |
| 269 | 2026-11-03T05:45Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 270 | 2026-11-03T05:57Z | 1 | 0 | 0 | - | 1 | 0 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4            12      72.3        78.8
big        -                            -        -     -            23     117.4       105.0
```

## The last 12 passes

```text
12 pass(es), 2397 steps, 5.6 h of pass time

category                       hours  share  steps  mean s
model                           0.99  17.7%   1165     3.0
cargo test                      0.88  15.8%     67    47.2
wait: gate                      0.71  12.8%     11   233.6
wait: tests                     0.52   9.4%     28    67.0
cargo build                     0.51   9.1%     39    46.8
wait: other                     0.44   8.0%     22    72.7
shell                           0.38   6.9%    692     2.0
clip record/render              0.33   6.0%     75    16.0
push gate                       0.25   4.4%     24    36.8
wait: build                     0.18   3.3%     19    34.7
cargo clippy                    0.10   1.8%     18    20.6
phase mark                      0.09   1.6%     32    10.0
remote wait                     0.06   1.0%      6    33.4
github                          0.05   0.9%     62     2.9
wait: clip                      0.04   0.7%      1   137.3

phase                          hours  share  steps  mean s
orient                          2.23  40.1%    780    10.3
gate                            1.74  31.4%    512    12.3
build                           0.68  12.2%    383     6.4
prove                           0.42   7.6%    338     4.5
bugs                            0.37   6.7%    256     5.2
choose                          0.07   1.2%     78     3.1
ship                            0.02   0.4%     12     7.2
log                             0.02   0.4%     34     2.3

slowest single steps
  pass  350    1141s  tool  wait: gate           Waiting for the push gate
  pass  356     221s  tool  wait: gate           Waiting for the gate
  pass  358     192s  bg    push gate            bg_48 completed: timeout 2400 git push origin HEAD:main 2>&1 | tail -8
  pass  352     181s  tool  wait: tests          Waiting for the view_frame build and test
  pass  358     180s  tool  shell                Launching the editor on the scratch project :: cd /tmp/clipproj && DISPLAY=:98 BEVY_BRP_PO
  pass  360     168s  model model                
  pass  356     158s  tool  wait: gate           Waiting for the push gate
  pass  357     156s  tool  wait: gate           Waiting for the compose push
  pass  358     153s  tool  wait: gate           Waiting for the push gate
  pass  357     152s  tool  wait: tests          Waiting for the editor tests
  pass  351     143s  tool  wait: gate           Waiting for the push gate
  pass  357     142s  tool  wait: gate           Waiting for the gate
  pass  349     141s  tool  wait: gate           Waiting for the push gate
  pass  357     141s  tool  wait: gate           Waiting for the bench push
  pass  357     140s  tool  wait: gate           Waiting for the merge push
```

## Every pass recorded

```text
303 pass(es), 44529 steps, 155.7 h of pass time

category                       hours  share  steps  mean s
wait: other                    38.68  24.8%    485   287.1
model                          28.41  18.2%  21344     4.8
wait: tests                    22.85  14.7%    328   250.8
wait: gate                     21.49  13.8%    278   278.2
cargo test                     11.39   7.3%   1267    32.4
shell                           4.63   3.0%  10047     1.7
wait: build                     3.83   2.5%    146    94.4
cargo build                     3.65   2.3%    451    29.1
push gate                       3.55   2.3%    683    18.7
clip record/render              3.14   2.0%   1043    10.9
wait: clip                      2.56   1.6%     51   181.0
remote wait                     1.87   1.2%    188    35.7
remote command                  1.78   1.1%    547    11.7
phase mark                      1.64   1.1%    656     9.0
wait: review                    1.28   0.8%     37   125.0

phase                          hours  share  steps  mean s
orient                         96.42  61.9%  17916    19.4
build                          17.15  11.0%   8838     7.0
prove                          10.19   6.5%   5279     6.9
ship                            9.57   6.1%   2624    13.1
gate                            9.43   6.1%   2142    15.9
choose                          3.89   2.5%   2744     5.1
bugs                            3.27   2.1%   1952     6.0
log                             2.58   1.7%    921    10.1
research                        2.08   1.3%   1632     4.6
release                         1.12   0.7%    308    13.1

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

