# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 15:02Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 204 | 2026-11-01T15:02Z | 4 | 0 | 4 | - | 1 | 0 | demo? |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8             4      16.4        14.6
```

## The last 12 passes

```text
12 pass(es), 2581 steps, 6.8 h of pass time

category                       hours  share  steps  mean s
model                           2.33  34.4%   1184     7.1
wait: gate                      1.47  21.8%     28   189.3
remote command                  0.56   8.3%    177    11.4
wait: other                     0.55   8.2%     20    99.8
wait: review                    0.33   4.9%     13    91.4
wait: tests                     0.33   4.8%      9   131.2
cargo test                      0.30   4.4%     42    25.6
phase mark                      0.19   2.8%     77     8.9
push gate                       0.16   2.4%     29    20.2
remote wait                     0.08   1.1%     14    19.8
loop tools                      0.07   1.0%     41     6.0
wait: release                   0.06   1.0%      2   116.3
clip record/render              0.06   0.9%     25     8.7
wait: clip                      0.05   0.8%      7    27.9
github                          0.05   0.8%     54     3.5

phase                          hours  share  steps  mean s
orient                          1.46  21.6%    579     9.1
gate                            1.42  21.0%    209    24.4
build                           1.03  15.2%    464     8.0
prove                           0.84  12.4%    270    11.2
ship                            0.82  12.1%    197    15.0
choose                          0.60   8.8%    480     4.5
research                        0.42   6.1%    295     5.1
log                             0.17   2.6%     62    10.1
release                         0.01   0.1%      2    16.3
bugs                            0.00   0.0%      2     2.3

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
  pass  193     278s  tool  wait: gate           Waiting for the gate on big
  pass  199     277s  tool  wait: gate           Waiting for the gate
  pass  193     271s  tool  wait: gate           Waiting for the gate on big
  pass  194     271s  tool  wait: gate           Waiting for gate result
  pass  194     270s  tool  wait: gate           Waiting for gate result
```

## Every pass recorded

```text
184 pass(es), 15527 steps, 93.3 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.77  35.1%    209   564.5
wait: tests                    19.23  20.6%    105   659.2
wait: gate                     14.80  15.9%    112   475.6
model                          12.58  13.5%   7248     6.2
cargo test                      2.85   3.1%    349    29.4
wait: build                     2.43   2.6%     21   416.2
remote command                  1.78   1.9%    547    11.7
wait: review                    1.17   1.2%     33   127.2
push gate                       1.13   1.2%    325    12.6
wait: clip                      0.71   0.8%     19   135.3
wait: release                   0.56   0.6%      6   336.8
shell                           0.55   0.6%   2884     0.7
cargo build                     0.46   0.5%     58    28.6
remote wait                     0.44   0.5%     57    28.1
loop tools                      0.44   0.5%    459     3.4

phase                          hours  share  steps  mean s
orient                         78.96  84.6%  10130    28.1
gate                            3.66   3.9%    493    26.7
build                           3.08   3.3%   1357     8.2
prove                           2.42   2.6%    874    10.0
choose                          1.89   2.0%   1256     5.4
ship                            1.70   1.8%    494    12.4
research                        0.94   1.0%    605     5.6
release                         0.33   0.4%    125     9.5
log                             0.33   0.4%    117    10.1
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

