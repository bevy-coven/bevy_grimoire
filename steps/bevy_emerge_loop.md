# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 14:32Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 191 | 2026-11-01T07:24Z | 4 | 0 | 10 | bevy_grimoire_core-v0.3.4 | 2 | 0 | demo? |
| 192 | 2026-11-01T08:09Z | 5 | 0 | 5 | bevy_grimoire-v0.2.0,bevy_grimoire_anim-v0.1.3,bevy_grimoire_core-v0.4.0 | 3 | 0 | demo? |
| 193 | 2026-11-01T08:44Z | 3 | 0 | 3 | bevy_grimoire-v0.2.1 | 3 | 0 | demo? |
| 194 | 2026-11-01T09:14Z | 3 | 0 | 2 | bevy_grimoire-v0.3.0 | 3 | 0 | demo? |
| 195 | 2026-11-01T10:21Z | 5 | 0 | 5 | bevy_grimoire_core-v0.4.1 | 2 | 0 | demo? |
| 196 | 2026-11-01T11:01Z | 3 | 0 | 12 | bevy_grimoire-v0.3.1,bevy_grimoire_anim-v0.1.4,bevy_grimoire_core-v0.5.0 | 2 | 0 | demo? |
| 197 | 2026-11-01T11:31Z | 2 | 0 | 5 | - | 2 | 0 | demo? |
| 198 | 2026-11-01T12:04Z | 2 | 0 | 1 | - | 2 | 0 | demo? |
| 199 | 2026-11-01T12:39Z | 4 | 0 | 7 | - | 3 | 0 | demo? |
| 200 | 2026-11-01T13:55Z | 6 | 0 | 3 | - | 5 | 2 | - |
| 201 | 2026-11-01T14:08Z | 2 | 0 | 6 | - | 2 | 0 | demo? |
| 202 | 2026-11-01T14:32Z | 2 | 0 | 13 | bevy_grimoire_core-v0.5.1 | 2 | 0 | demo? |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8             2      18.2        18.2
```

## The last 12 passes

```text
12 pass(es), 2941 steps, 7.9 h of pass time

category                       hours  share  steps  mean s
model                           2.82  35.6%   1344     7.6
wait: gate                      1.82  23.0%     34   193.0
wait: other                     0.74   9.3%     23   115.2
remote command                  0.61   7.7%    226     9.8
wait: review                    0.43   5.4%     16    97.1
wait: tests                     0.33   4.1%      9   131.2
cargo test                      0.22   2.7%     38    20.5
phase mark                      0.18   2.2%     74     8.6
push gate                       0.11   1.4%     28    14.6
remote wait                     0.11   1.4%     15    26.5
wait: release                   0.11   1.3%      3   126.7
loop tools                      0.08   1.0%     48     5.8
clip record/render              0.06   0.8%     25     8.7
github                          0.06   0.7%     52     4.0
shell                           0.05   0.7%    358     0.5

phase                          hours  share  steps  mean s
gate                            1.67  21.1%    273    22.0
orient                          1.47  18.6%    577     9.2
choose                          1.19  15.0%    713     6.0
ship                            0.92  11.6%    216    15.3
build                           0.91  11.5%    399     8.3
prove                           0.79  10.0%    248    11.5
research                        0.73   9.3%    405     6.5
log                             0.15   1.8%     62     8.5
release                         0.09   1.1%     26    12.1

slowest single steps
  pass  195     648s  tool  wait: tests          Waiting for local core tests
  pass  200     507s  bg    shell                bg_11 failed: BEVY_BRP_PORT=15710 /tmp/p186/bin/bevy_grimoire_mapper . town --live > /tmp/
  pass  192     293s  tool  wait: gate           Waiting for gate output
  pass  192     292s  tool  wait: gate           Waiting for gate output
  pass  195     290s  tool  wait: gate           Waiting for gate
  pass  191     290s  tool  wait: gate           Waiting for the gate
  pass  195     290s  tool  wait: other          Waiting for before-run
  pass  195     289s  tool  wait: gate           Waiting for gate
  pass  200     286s  tool  wait: gate           Waiting for the gate
  pass  195     285s  tool  wait: gate           Waiting for gate
  pass  195     285s  tool  wait: gate           Waiting for gate
  pass  200     281s  tool  wait: gate           Waiting for the gate
  pass  200     279s  tool  wait: gate           Waiting for the gate
  pass  193     278s  tool  wait: gate           Waiting for the gate on big
  pass  199     277s  tool  wait: gate           Waiting for the gate
```

## Every pass recorded

```text
182 pass(es), 15302 steps, 92.9 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.67  35.2%    203   579.4
wait: tests                    19.23  20.7%    105   659.2
wait: gate                     14.80  15.9%    112   475.6
model                          12.42  13.4%   7141     6.3
cargo test                      2.77   3.0%    339    29.4
wait: build                     2.43   2.6%     21   416.2
remote command                  1.78   1.9%    547    11.7
wait: review                    1.17   1.3%     33   127.2
push gate                       1.07   1.1%    320    12.0
wait: clip                      0.71   0.8%     19   135.3
wait: release                   0.56   0.6%      6   336.8
shell                           0.55   0.6%   2842     0.7
cargo build                     0.46   0.5%     58    28.6
loop tools                      0.43   0.5%    456     3.4
remote wait                     0.43   0.5%     55    28.0

phase                          hours  share  steps  mean s
orient                         78.95  85.0%  10112    28.1
gate                            3.66   3.9%    493    26.7
build                           2.97   3.2%   1292     8.3
prove                           2.37   2.6%    852    10.0
choose                          1.86   2.0%   1220     5.5
ship                            1.54   1.7%    464    12.0
research                        0.90   1.0%    573     5.7
release                         0.33   0.4%    125     9.5
log                             0.25   0.3%     99     9.3
bugs                            0.01   0.0%     13     4.1

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

