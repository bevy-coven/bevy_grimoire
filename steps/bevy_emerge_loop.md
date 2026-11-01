# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 11:31Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 186 | 2026-11-01T04:01Z | 3 | 0 | 0 | - | 2 | 0 | - |
| 187 | 2026-11-01T04:29Z | 3 | 0 | 5 | - | 2 | 0 | demo? |
| 188 | 2026-11-01T05:10Z | 4 | 0 | 9 | bevy_grimoire_core-v0.3.1 | 2 | 0 | demo? |
| 189 | 2026-11-01T05:50Z | 4 | 0 | 9 | bevy_grimoire_core-v0.3.2 | 2 | 0 | demo? |
| 190 | 2026-11-01T06:29Z | 4 | 0 | 7 | bevy_grimoire_core-v0.3.3 | 2 | 0 | demo? |
| 191 | 2026-11-01T07:24Z | 4 | 0 | 10 | bevy_grimoire_core-v0.3.4 | 2 | 0 | demo? |
| 192 | 2026-11-01T08:09Z | 5 | 0 | 5 | bevy_grimoire-v0.2.0,bevy_grimoire_anim-v0.1.3,bevy_grimoire_core-v0.4.0 | 3 | 0 | demo? |
| 193 | 2026-11-01T08:44Z | 3 | 0 | 3 | bevy_grimoire-v0.2.1 | 3 | 0 | demo? |
| 194 | 2026-11-01T09:14Z | 3 | 0 | 2 | bevy_grimoire-v0.3.0 | 3 | 0 | demo? |
| 195 | 2026-11-01T10:21Z | 5 | 0 | 5 | bevy_grimoire_core-v0.4.1 | 2 | 0 | demo? |
| 196 | 2026-11-01T11:01Z | 3 | 0 | 12 | bevy_grimoire-v0.3.1,bevy_grimoire_anim-v0.1.4,bevy_grimoire_core-v0.5.0 | 2 | 0 | demo? |
| 197 | 2026-11-01T11:31Z | 2 | 0 | 5 | - | 2 | 0 | demo? |

## The last 12 passes

```text
12 pass(es), 2791 steps, 7.8 h of pass time

category                       hours  share  steps  mean s
model                           2.76  35.2%   1270     7.8
wait: gate                      1.87  23.8%     32   209.9
wait: other                     0.75   9.5%     23   117.1
remote command                  0.62   7.9%    211    10.6
wait: review                    0.44   5.6%     12   131.7
wait: tests                     0.32   4.1%      8   146.2
cargo test                      0.20   2.6%     40    18.4
push gate                       0.13   1.7%     22    21.5
phase mark                      0.13   1.6%     79     5.8
remote wait                     0.11   1.4%     13    30.9
loop tools                      0.11   1.4%     56     6.9
wait: release                   0.11   1.3%      3   126.7
wait: build                     0.10   1.2%      5    70.0
github                          0.04   0.5%     47     3.2
cargo build                     0.04   0.4%      5    25.4

phase                          hours  share  steps  mean s
build                           2.11  26.9%    864     8.8
gate                            1.88  24.0%    227    29.8
ship                            1.01  12.9%    271    13.5
choose                          0.96  12.2%    507     6.8
prove                           0.69   8.8%    212    11.7
research                        0.60   7.6%    353     6.1
orient                          0.29   3.7%    230     4.5
log                             0.18   2.3%     72     8.9
release                         0.13   1.6%     46     9.8

slowest single steps
  pass  195     648s  tool  wait: tests          Waiting for local core tests
  pass  186     592s  tool  wait: gate           Waiting for gate to finish
  pass  188     541s  tool  wait: gate           Waiting for gate/release/accept result
  pass  189     303s  tool  wait: review         Waiting for reviewer and the after run
  pass  192     293s  tool  wait: gate           Waiting for gate output
  pass  192     292s  tool  wait: gate           Waiting for gate output
  pass  190     291s  tool  wait: gate           Waiting for gate
  pass  195     290s  tool  wait: gate           Waiting for gate
  pass  191     290s  tool  wait: gate           Waiting for the gate
  pass  195     290s  tool  wait: other          Waiting for before-run
  pass  195     289s  tool  wait: gate           Waiting for gate
  pass  190     286s  tool  wait: other          Waiting for after-run on big
  pass  195     285s  tool  wait: gate           Waiting for gate
  pass  188     285s  bg    subagents            OplogReview completed: OplogReview
  pass  195     285s  tool  wait: gate           Waiting for gate
```

## Every pass recorded

```text
177 pass(es), 14055 steps, 89.9 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.49  36.1%    197   593.7
wait: tests                    19.11  21.3%    100   688.0
wait: gate                     14.16  15.8%     99   514.9
model                          11.29  12.6%   6575     6.2
cargo test                      2.67   3.0%    323    29.8
wait: build                     2.42   2.7%     19   458.4
remote command                  1.58   1.8%    460    12.3
push gate                       1.02   1.1%    307    12.0
wait: review                    1.02   1.1%     26   140.5
wait: clip                      0.68   0.8%     14   175.8
wait: release                   0.56   0.6%      6   336.8
shell                           0.52   0.6%   2673     0.7
cargo build                     0.43   0.5%     47    33.3
loop tools                      0.40   0.5%    438     3.3
remote wait                     0.39   0.4%     47    29.8

phase                          hours  share  steps  mean s
orient                         77.63  86.4%   9652    29.0
gate                            3.08   3.4%    386    28.7
build                           2.87   3.2%   1174     8.8
prove                           2.11   2.4%    780     9.8
choose                          1.54   1.7%    965     5.8
ship                            1.45   1.6%    428    12.2
research                        0.64   0.7%    403     5.7
release                         0.32   0.4%    123     9.4
log                             0.22   0.2%     88     9.0
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

