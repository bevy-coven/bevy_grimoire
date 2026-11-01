# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 14:46Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 203 | 2026-11-01T14:46Z | 4 | 0 | 5 | - | 2 | 0 | demo? |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8             3      16.5        13.1
```

## The last 12 passes

```text
12 pass(es), 2732 steps, 7.2 h of pass time

category                       hours  share  steps  mean s
model                           2.46  33.9%   1249     7.1
wait: gate                      1.67  23.0%     31   193.8
wait: other                     0.65   8.9%     21   110.7
remote command                  0.60   8.3%    209    10.4
wait: review                    0.36   5.0%     15    87.6
wait: tests                     0.33   4.5%      9   131.2
cargo test                      0.24   3.3%     36    24.2
phase mark                      0.17   2.4%     76     8.2
push gate                       0.14   1.9%     30    16.3
remote wait                     0.11   1.5%     16    24.9
wait: release                   0.11   1.5%      3   126.7
loop tools                      0.07   1.0%     44     5.8
clip record/render              0.06   0.8%     25     8.7
wait: clip                      0.05   0.7%      7    27.9
github                          0.05   0.7%     49     3.9

phase                          hours  share  steps  mean s
orient                          1.46  20.2%    576     9.1
gate                            1.42  19.6%    209    24.4
ship                            1.00  13.8%    221    16.3
build                           0.96  13.2%    436     7.9
prove                           0.82  11.3%    262    11.3
research                        0.73  10.1%    405     6.5
choose                          0.65   9.0%    514     4.5
log                             0.12   1.7%     60     7.3
release                         0.09   1.2%     26    12.1
bugs                            0.00   0.0%      2     2.3

slowest single steps
  pass  195     648s  tool  wait: tests          Waiting for local core tests
  pass  200     507s  bg    shell                bg_11 failed: BEVY_BRP_PORT=15710 /tmp/p186/bin/bevy_grimoire_mapper . town --live > /tmp/
  pass  192     293s  tool  wait: gate           Waiting for gate output
  pass  192     292s  tool  wait: gate           Waiting for gate output
  pass  195     290s  tool  wait: gate           Waiting for gate
  pass  195     290s  tool  wait: other          Waiting for before-run
  pass  195     289s  tool  wait: gate           Waiting for gate
  pass  200     286s  tool  wait: gate           Waiting for the gate
  pass  195     285s  tool  wait: gate           Waiting for gate
  pass  195     285s  tool  wait: gate           Waiting for gate
  pass  200     281s  tool  wait: gate           Waiting for the gate
  pass  200     279s  tool  wait: gate           Waiting for the gate
  pass  193     278s  tool  wait: gate           Waiting for the gate on big
  pass  199     277s  tool  wait: gate           Waiting for the gate
  pass  193     271s  tool  wait: gate           Waiting for the gate on big
```

## Every pass recorded

```text
183 pass(es), 15427 steps, 93.1 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.73  35.2%    206   571.9
wait: tests                    19.23  20.7%    105   659.2
wait: gate                     14.80  15.9%    112   475.6
model                          12.48  13.4%   7199     6.2
cargo test                      2.80   3.0%    343    29.4
wait: build                     2.43   2.6%     21   416.2
remote command                  1.78   1.9%    547    11.7
wait: review                    1.17   1.3%     33   127.2
push gate                       1.10   1.2%    323    12.3
wait: clip                      0.71   0.8%     19   135.3
wait: release                   0.56   0.6%      6   336.8
shell                           0.55   0.6%   2866     0.7
cargo build                     0.46   0.5%     58    28.6
remote wait                     0.44   0.5%     57    28.1
loop tools                      0.44   0.5%    458     3.4

phase                          hours  share  steps  mean s
orient                         78.96  84.8%  10120    28.1
gate                            3.66   3.9%    493    26.7
build                           3.01   3.2%   1329     8.2
prove                           2.40   2.6%    866    10.0
choose                          1.89   2.0%   1256     5.4
ship                            1.65   1.8%    484    12.2
research                        0.90   1.0%    573     5.7
release                         0.33   0.4%    125     9.5
log                             0.26   0.3%    105     8.9
bugs                            0.02   0.0%     15     3.9

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

