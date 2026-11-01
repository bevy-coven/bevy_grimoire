# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 15:09Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 204 | 2026-11-01T15:02Z | 4 | 0 | 4 | - | 1 | 0 | demo? |
| 205 | 2026-11-01T15:09Z | 3 | 0 | 2 | - | 1 | 0 | demo? |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8             5      14.5        13.1
```

## The last 12 passes

```text
12 pass(es), 2461 steps, 6.3 h of pass time

category                       hours  share  steps  mean s
model                           2.18  34.6%   1136     6.9
wait: gate                      1.32  20.9%     26   182.8
remote command                  0.55   8.7%    164    12.0
wait: other                     0.48   7.6%     20    86.4
wait: tests                     0.33   5.2%      9   131.2
cargo test                      0.33   5.2%     45    26.2
wait: review                    0.29   4.6%     12    86.3
phase mark                      0.21   3.3%     74    10.1
push gate                       0.16   2.5%     27    20.8
loop tools                      0.06   1.0%     35     6.3
clip record/render              0.06   1.0%     25     8.7
remote wait                     0.06   0.9%     12    17.7
github                          0.05   0.9%     57     3.5
wait: clip                      0.05   0.8%      6    30.4
shell                           0.05   0.8%    340     0.5

phase                          hours  share  steps  mean s
orient                          1.40  22.3%    545     9.3
gate                            1.28  20.3%    198    23.3
build                           0.87  13.8%    412     7.6
prove                           0.81  12.9%    257    11.4
ship                            0.76  12.1%    190    14.5
choose                          0.60   9.5%    480     4.5
research                        0.40   6.4%    284     5.1
log                             0.16   2.6%     56    10.4
bugs                            0.01   0.2%     17     2.6
release                         0.01   0.1%      2    16.3

slowest single steps
  pass  195     648s  tool  wait: tests          Waiting for local core tests
  pass  200     507s  bg    shell                bg_11 failed: BEVY_BRP_PORT=15710 /tmp/p186/bin/bevy_grimoire_mapper . town --live > /tmp/
  pass  195     290s  tool  wait: gate           Waiting for gate
  pass  195     290s  tool  wait: other          Waiting for before-run
  pass  195     289s  tool  wait: gate           Waiting for gate
  pass  200     286s  tool  wait: gate           Waiting for the gate
  pass  195     285s  tool  wait: gate           Waiting for gate
  pass  195     285s  tool  wait: gate           Waiting for gate
  pass  200     281s  tool  wait: gate           Waiting for the gate
  pass  200     279s  tool  wait: gate           Waiting for the gate
  pass  199     277s  tool  wait: gate           Waiting for the gate
  pass  194     271s  tool  wait: gate           Waiting for gate result
  pass  194     270s  tool  wait: gate           Waiting for gate result
  pass  200     266s  tool  wait: other          Waiting for the Linux rerun
  pass  200     249s  tool  wait: gate           Waiting for the gate result
```

## Every pass recorded

```text
185 pass(es), 15589 steps, 93.5 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.79  35.1%    211   559.5
wait: tests                    19.23  20.6%    105   659.2
wait: gate                     14.80  15.8%    112   475.6
model                          12.62  13.5%   7278     6.2
cargo test                      2.89   3.1%    353    29.4
wait: build                     2.43   2.6%     21   416.2
remote command                  1.78   1.9%    547    11.7
wait: review                    1.17   1.2%     33   127.2
push gate                       1.13   1.2%    325    12.6
wait: clip                      0.71   0.8%     19   135.3
wait: release                   0.56   0.6%      6   336.8
shell                           0.55   0.6%   2898     0.7
cargo build                     0.46   0.5%     58    28.6
remote wait                     0.44   0.5%     57    28.1
loop tools                      0.44   0.5%    459     3.4

phase                          hours  share  steps  mean s
orient                         78.97  84.5%  10135    28.0
gate                            3.70   4.0%    499    26.7
build                           3.10   3.3%   1377     8.1
prove                           2.47   2.6%    888    10.0
choose                          1.89   2.0%   1256     5.4
ship                            1.70   1.8%    496    12.4
research                        0.94   1.0%    605     5.6
release                         0.33   0.4%    125     9.5
log                             0.33   0.4%    117    10.1
bugs                            0.03   0.0%     30     3.2

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

