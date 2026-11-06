# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-06 15:23Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 406 | 2026-11-06T14:55Z | 2 | 0 | 5 | - | 1 | 0 | demo? |
| 408 | 2026-11-06T15:23Z | 3 | 0 | 1 | - | 1 | 0 | demo? |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4            12      72.3        78.8
big        -                            -        -     -            59      57.5        26.0
big        claude-sonnet-5-5            medium   -     -             4      22.5        22.4
```

## The last 12 passes

```text
12 pass(es), 2800 steps, 6.1 h of pass time

category                       hours  share  steps  mean s
wait: other                     1.46  24.0%     29   181.3
model                           1.24  20.4%   1327     3.4
cargo test                      0.96  15.7%     71    48.5
wait: tests                     0.47   7.6%     24    69.8
wait: gate                      0.37   6.1%      9   149.2
shell                           0.33   5.4%    793     1.5
cargo build                     0.30   5.0%     26    42.0
clip record/render              0.27   4.4%     65    14.7
push gate                       0.19   3.1%     25    27.3
phase mark                      0.13   2.1%     52     9.0
wait: build                     0.12   2.0%     12    35.9
cargo clippy                    0.12   1.9%     21    20.3
github                          0.05   0.9%     81     2.4
remote wait                     0.05   0.8%      5    35.1
loop tools                      0.03   0.4%     71     1.3

phase                          hours  share  steps  mean s
gate                            1.93  31.8%    580    12.0
build                           1.29  21.1%    601     7.7
ship                            0.76  12.5%     40    68.4
prove                           0.68  11.1%    366     6.7
orient                          0.62  10.2%    534     4.2
bugs                            0.55   9.0%    351     5.6
repro                           0.13   2.2%    148     3.2
choose                          0.07   1.2%     95     2.8
research                        0.04   0.6%     40     3.2
log                             0.02   0.4%     34     2.3

slowest single steps
  pass  409     939s  tool  wait: other          {}
  pass  408     939s  tool  wait: other          {}
  pass  373     390s  bg    cargo test           bg_17 completed: timeout 900 cargo nextest run -p bevy_cauldron --test editor -E 'test(the
  pass  409     341s  tool  wait: other          {}
  pass  408     341s  tool  wait: other          {}
  pass  407     293s  tool  wait: other          {}
  pass  406     293s  tool  wait: other          {}
  pass  407     284s  tool  wait: other          {}
  pass  406     284s  tool  wait: other          {}
  pass  373     278s  bg    cargo test           bg_18 completed: timeout 900 cargo nextest run -p bevy_cauldron --test editor -E 'test(the
  pass  372     253s  tool  wait: tests          Waiting for baseline test
  pass  372     240s  tool  wait: other          Waiting for tree to clear
  pass  356     221s  tool  wait: gate           Waiting for the gate
  pass  372     198s  tool  wait: gate           Waiting for the push gate
  pass  358     192s  bg    push gate            bg_48 completed: timeout 2400 git push origin HEAD:main 2>&1 | tail -8
```

## Every pass recorded

```text
310 pass(es), 45413 steps, 158.0 h of pass time

category                       hours  share  steps  mean s
wait: other                    39.88  25.2%    501   286.6
model                          28.89  18.3%  21750     4.8
wait: tests                    22.92  14.5%    329   250.8
wait: gate                     21.54  13.6%    279   277.9
cargo test                     11.64   7.4%   1285    32.6
shell                           4.69   3.0%  10265     1.6
wait: build                     3.85   2.4%    147    94.3
cargo build                     3.72   2.4%    455    29.4
push gate                       3.55   2.2%    692    18.5
clip record/render              3.14   2.0%   1049    10.8
wait: clip                      2.56   1.6%     51   181.0
remote wait                     1.87   1.2%    188    35.7
remote command                  1.78   1.1%    549    11.7
phase mark                      1.70   1.1%    683     8.9
wait: review                    1.28   0.8%     37   125.0

phase                          hours  share  steps  mean s
orient                         96.58  61.1%  18151    19.2
build                          17.76  11.2%   9056     7.1
prove                          10.45   6.6%   5307     7.1
ship                           10.31   6.5%   2652    14.0
gate                            9.62   6.1%   2210    15.7
choose                          3.90   2.5%   2761     5.1
bugs                            3.44   2.2%   2047     6.1
log                             2.58   1.6%    921    10.1
research                        2.12   1.3%   1672     4.6
release                         1.12   0.7%    308    13.1
repro                           0.13   0.1%    148     3.2

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

