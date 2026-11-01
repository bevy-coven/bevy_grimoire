# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 15:30Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 206 | 2026-11-01T15:30Z | 5 | 0 | 7 | - | 3 | 0 | release(bevy_grimoire_core),demo? |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8             6      15.5        14.6
```

## The last 12 passes

```text
12 pass(es), 2421 steps, 6.2 h of pass time

category                       hours  share  steps  mean s
model                           2.15  35.0%   1117     6.9
wait: gate                      1.29  20.9%     27   171.5
remote command                  0.50   8.2%    150    12.1
wait: other                     0.48   7.8%     20    86.4
cargo test                      0.36   5.8%     46    28.0
wait: tests                     0.32   5.2%      9   126.8
wait: review                    0.25   4.1%     11    81.9
phase mark                      0.21   3.4%     75     9.9
push gate                       0.15   2.4%     28    18.8
clip record/render              0.06   1.0%     25     8.7
remote wait                     0.06   1.0%     14    15.2
github                          0.06   1.0%     63     3.3
loop tools                      0.06   0.9%     34     6.2
wait: clip                      0.05   0.8%      6    30.4
shell                           0.05   0.8%    344     0.5

phase                          hours  share  steps  mean s
orient                          1.40  22.7%    543     9.3
gate                            1.28  20.8%    198    23.3
ship                            0.94  15.3%    226    15.0
build                           0.88  14.4%    429     7.4
choose                          0.57   9.2%    462     4.4
prove                           0.53   8.6%    190    10.0
research                        0.40   6.4%    290     4.9
log                             0.13   2.1%     38    12.5
bugs                            0.02   0.3%     23     2.6
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
  pass  200     266s  tool  wait: other          Waiting for the Linux rerun
  pass  200     249s  tool  wait: gate           Waiting for the gate result
  pass  198     231s  tool  wait: review         Waiting for reviewer
  pass  197     228s  tool  wait: gate           Waiting for gate wait job
```

## Every pass recorded

```text
186 pass(es), 15717 steps, 93.8 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.79  35.0%    211   559.5
wait: tests                    19.24  20.5%    106   653.3
wait: gate                     14.91  15.9%    115   466.9
model                          12.75  13.6%   7340     6.3
cargo test                      2.95   3.1%    358    29.7
wait: build                     2.43   2.6%     21   416.2
remote command                  1.78   1.9%    547    11.7
wait: review                    1.17   1.2%     33   127.2
push gate                       1.13   1.2%    329    12.4
wait: clip                      0.71   0.8%     19   135.3
wait: release                   0.56   0.6%      6   336.8
shell                           0.55   0.6%   2922     0.7
cargo build                     0.46   0.5%     58    28.6
remote wait                     0.44   0.5%     59    27.1
loop tools                      0.44   0.5%    460     3.4

phase                          hours  share  steps  mean s
orient                         78.97  84.2%  10141    28.0
gate                            3.70   3.9%    499    26.7
build                           3.16   3.4%   1402     8.1
prove                           2.50   2.7%    898    10.0
choose                          1.89   2.0%   1256     5.4
ship                            1.89   2.0%    532    12.8
research                        0.99   1.1%    649     5.5
release                         0.33   0.4%    125     9.5
log                             0.33   0.4%    117    10.1
bugs                            0.03   0.0%     36     3.1

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

