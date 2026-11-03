# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-03 03:56Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 251 | 2026-11-03T01:10Z | 5 | 0 | 0 | - | 2 | 0 | - |
| 252 | 2026-11-03T01:25Z | 2 | 0 | 5 | - | 1 | 0 | demo? |
| 253 | 2026-11-03T01:34Z | 3 | 0 | 3 | - | 2 | 0 | demo? |
| 254 | 2026-11-03T01:36Z | 2 | 0 | 0 | - | 2 | 0 | - |
| 255 | 2026-11-03T01:54Z | 2 | 0 | 9 | - | 1 | 0 | demo? |
| 256 | 2026-11-03T02:08Z | 2 | 0 | 5 | - | 1 | 0 | demo? |
| 257 | 2026-11-03T02:31Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 258 | 2026-11-03T02:47Z | 2 | 0 | 2 | - | 1 | 0 | demo? |
| 259 | 2026-11-03T03:02Z | 2 | 0 | 4 | - | 1 | 0 | demo? |
| 260 | 2026-11-03T03:16Z | 2 | 0 | 4 | - | 1 | 0 | demo? |
| 261 | 2026-11-03T03:32Z | 2 | 0 | 0 | - | 1 | 0 | - |
| 262 | 2026-11-03T03:56Z | 2 | 0 | 1 | - | 1 | 0 | demo? |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4            12      72.3        78.8
big        -                            -        -     -            12     154.2       102.5
```

## The last 12 passes

```text
12 pass(es), 12415 steps, 25.7 h of pass time

category                       hours  share  steps  mean s
model                           7.41  28.8%   5989     4.5
wait: other                     3.11  12.1%    132    84.9
cargo test                      2.92  11.3%    348    30.2
wait: gate                      2.32   9.0%     61   137.0
shell                           1.49   5.8%   3009     1.8
clip record/render              1.41   5.5%    461    11.0
wait: clip                      1.29   5.0%     15   309.0
wait: tests                     1.13   4.4%     80    51.0
cargo build                     0.97   3.8%    145    24.2
push gate                       0.86   3.4%    136    22.8
remote wait                     0.76   3.0%     68    40.4
phase mark                      0.37   1.5%    185     7.3
cargo clippy                    0.37   1.5%     79    17.0
sleep                           0.31   1.2%     38    29.7
wait: build                     0.27   1.0%     27    35.4

phase                          hours  share  steps  mean s
build                           6.42  25.0%   3813     6.1
ship                            5.15  20.0%   1276    14.5
prove                           4.40  17.1%   2171     7.3
orient                          3.53  13.7%   2094     6.1
gate                            1.91   7.4%    526    13.1
bugs                            1.43   5.6%    901     5.7
choose                          1.16   4.5%    709     5.9
log                             0.83   3.2%    269    11.1
research                        0.75   2.9%    560     4.8
release                         0.14   0.6%     39    13.1

slowest single steps
  pass  227    1424s  tool  wait: clip           Waiting for the demo re-record
  pass  231    1353s  tool  wait: clip           Waiting for clip recorder
  pass  224     840s  tool  wait: other          {}
  pass  232     676s  bg    subagents            RerecordDemos completed: RerecordDemos
  pass  231     675s  tool  wait: clip           Waiting for demo re-recording
  pass  251     567s  tool  wait: tests          Waiting for the base commit test
  pass  232     549s  tool  wait: other          {}
  pass  251     542s  tool  wait: gate           Waiting for the push loop
  pass  251     538s  model model                
  pass  221     498s  tool  wait: other          {}
  pass  222     416s  tool  wait: other          {}
  pass  221     401s  tool  wait: other          {}
  pass  224     399s  tool  wait: other          {}
  pass  230     390s  tool  wait: clip           Waiting for demo re-recording
  pass  221     310s  tool  wait: other          {}
```

## Every pass recorded

```text
211 pass(es), 31117 steps, 126.1 h of pass time

category                       hours  share  steps  mean s
wait: other                    36.86  29.2%    375   353.8
model                          22.56  17.9%  14767     5.5
wait: tests                    20.47  16.2%    195   377.9
wait: gate                     17.69  14.0%    191   333.4
cargo test                      6.50   5.2%    785    29.8
wait: build                     2.81   2.2%     51   198.5
shell                           2.52   2.0%   6593     1.4
wait: clip                      2.38   1.9%     40   214.2
push gate                       2.14   1.7%    505    15.3
remote command                  1.78   1.4%    547    11.7
cargo build                     1.65   1.3%    230    25.8
clip record/render              1.65   1.3%    573    10.4
remote wait                     1.30   1.0%    138    33.8
wait: review                    1.28   1.0%     36   127.8
phase mark                      1.03   0.8%    472     7.9

phase                          hours  share  steps  mean s
orient                         82.61  65.5%  12362    24.1
build                          12.10   9.6%   6096     7.1
prove                           7.87   6.2%   3601     7.9
ship                            7.74   6.1%   2134    13.1
gate                            6.13   4.9%   1240    17.8
choose                          3.37   2.7%   2220     5.5
research                        1.97   1.6%   1448     4.9
log                             1.82   1.4%    649    10.1
bugs                            1.47   1.2%    949     5.6
release                         1.00   0.8%    280    12.9

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

