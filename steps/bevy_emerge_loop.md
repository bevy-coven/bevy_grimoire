# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 12:39Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 198 | 2026-11-01T12:04Z | 2 | 0 | 1 | - | 2 | 0 | demo? |
| 199 | 2026-11-01T12:39Z | 4 | 0 | 7 | - | 3 | 0 | demo? |

## The last 12 passes

```text
12 pass(es), 2912 steps, 8.0 h of pass time

category                       hours  share  steps  mean s
model                           2.88  36.0%   1316     7.9
wait: gate                      1.86  23.3%     33   203.1
wait: other                     0.75   9.4%     23   117.8
remote command                  0.63   7.9%    232     9.8
wait: review                    0.49   6.2%     14   127.3
wait: tests                     0.24   3.0%      8   109.1
cargo test                      0.20   2.5%     39    18.7
phase mark                      0.14   1.8%     77     6.6
push gate                       0.14   1.8%     26    19.5
remote wait                     0.13   1.6%     15    31.3
wait: release                   0.11   1.3%      3   126.7
wait: build                     0.11   1.3%      7    54.2
loop tools                      0.10   1.2%     50     7.2
github                          0.04   0.6%     46     3.5
cargo build                     0.04   0.4%      5    25.4

phase                          hours  share  steps  mean s
gate                            2.01  25.1%    282    25.6
build                           1.72  21.5%    645     9.6
choose                          1.14  14.3%    674     6.1
ship                            1.00  12.4%    254    14.1
research                        0.82  10.2%    484     6.1
prove                           0.71   8.9%    226    11.4
orient                          0.29   3.7%    224     4.7
log                             0.18   2.3%     68     9.6
release                         0.13   1.6%     46     9.8

slowest single steps
  pass  195     648s  tool  wait: tests          Waiting for local core tests
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
  pass  193     278s  tool  wait: gate           Waiting for the gate on big
```

## Every pass recorded

```text
179 pass(es), 14539 steps, 91.0 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.49  35.7%    198   590.8
wait: tests                    19.12  21.0%    102   674.9
wait: gate                     14.39  15.8%    103   502.9
model                          11.76  12.9%   6795     6.2
cargo test                      2.71   3.0%    331    29.5
wait: build                     2.43   2.7%     21   416.2
remote command                  1.69   1.9%    501    12.2
wait: review                    1.11   1.2%     29   137.9
push gate                       1.04   1.1%    313    12.0
wait: clip                      0.68   0.8%     14   175.8
wait: release                   0.56   0.6%      6   336.8
shell                           0.52   0.6%   2733     0.7
cargo build                     0.43   0.5%     47    33.3
loop tools                      0.43   0.5%    443     3.5
remote wait                     0.41   0.4%     50    29.3

phase                          hours  share  steps  mean s
orient                         77.65  85.3%   9666    28.9
gate                            3.50   3.8%    461    27.3
build                           2.87   3.2%   1174     8.8
prove                           2.23   2.5%    816     9.8
choose                          1.81   2.0%   1181     5.5
ship                            1.47   1.6%    433    12.2
research                        0.87   1.0%    534     5.8
release                         0.32   0.4%    123     9.4
log                             0.25   0.3%     94     9.5
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

