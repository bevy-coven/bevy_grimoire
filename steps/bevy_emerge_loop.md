# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 10:21Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 184 | 2026-11-01T02:45Z | 3 | 0 | 6 | - | 2 | 0 | demo? |
| 185 | 2026-11-01T03:32Z | 6 | 0 | 10 | bevy_grimoire-v0.1.2,bevy_grimoire_anim-v0.1.2,bevy_grimoire_core-v0.3.0 | 3 | 0 | demo? |
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

## The last 12 passes

```text
12 pass(es), 2846 steps, 8.2 h of pass time

category                       hours  share  steps  mean s
model                           2.86  35.0%   1299     7.9
wait: gate                      1.97  24.2%     32   222.0
wait: other                     0.81   9.9%     25   116.1
remote command                  0.61   7.5%    210    10.5
wait: review                    0.43   5.3%     10   154.6
wait: tests                     0.39   4.8%      8   174.5
cargo test                      0.19   2.4%     45    15.4
push gate                       0.14   1.7%     22    22.7
wait: release                   0.13   1.6%      3   155.5
wait: build                     0.12   1.5%      7    64.3
phase mark                      0.12   1.5%     83     5.3
remote wait                     0.11   1.3%     11    35.5
loop tools                      0.10   1.2%     61     5.8
github                          0.04   0.5%     45     3.0
cargo build                     0.03   0.4%      4    30.4

phase                          hours  share  steps  mean s
gate                            2.10  25.7%    255    29.7
build                           2.05  25.1%    811     9.1
choose                          1.06  12.9%    572     6.6
prove                           0.94  11.5%    315    10.7
ship                            0.89  10.9%    250    12.8
research                        0.55   6.8%    309     6.4
orient                          0.26   3.2%    202     4.7
log                             0.18   2.1%     72     8.8
release                         0.13   1.5%     46     9.8
bugs                            0.01   0.1%      4     5.5

slowest single steps
  pass  195     648s  tool  wait: tests          Waiting for local core tests
  pass  186     592s  tool  wait: gate           Waiting for gate to finish
  pass  188     541s  tool  wait: gate           Waiting for gate/release/accept result
  pass  185     408s  tool  wait: gate           Waiting for gate and release to finish
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
```

## Every pass recorded

```text
175 pass(es), 13583 steps, 88.7 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.45  36.6%    195   599.1
wait: tests                    19.10  21.5%     98   701.6
wait: gate                     13.96  15.7%     93   540.4
model                          10.88  12.3%   6359     6.2
cargo test                      2.64   3.0%    318    29.9
wait: build                     2.42   2.7%     19   458.4
remote command                  1.30   1.5%    416    11.3
push gate                       1.00   1.1%    303    11.9
wait: review                    0.97   1.1%     23   152.5
wait: clip                      0.66   0.7%     13   183.7
wait: release                   0.54   0.6%      5   389.2
shell                           0.51   0.6%   2618     0.7
cargo build                     0.43   0.5%     46    33.9
remote wait                     0.39   0.4%     45    30.8
loop tools                      0.38   0.4%    432     3.2

phase                          hours  share  steps  mean s
orient                         77.58  87.4%   9609    29.1
gate                            2.84   3.2%    354    28.9
build                           2.55   2.9%   1027     8.9
prove                           1.99   2.2%    726     9.9
choose                          1.43   1.6%    865     6.0
ship                            1.21   1.4%    383    11.4
research                        0.60   0.7%    359     6.0
release                         0.32   0.4%    123     9.4
log                             0.20   0.2%     82     8.8
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

