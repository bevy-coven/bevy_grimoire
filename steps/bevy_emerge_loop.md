# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 11:01Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 196 | 2026-11-01T11:01Z | 3 | 0 | 12 | bevy_grimoire-v0.3.1,bevy_grimoire_anim-v0.1.4,bevy_grimoire_core-v0.5.0 | 2 | 0 | demo? |

## The last 12 passes

```text
12 pass(es), 2814 steps, 8.1 h of pass time

category                       hours  share  steps  mean s
model                           2.84  35.0%   1285     8.0
wait: gate                      1.94  23.9%     31   224.9
wait: other                     0.81   9.9%     25   116.0
remote command                  0.63   7.8%    210    10.8
wait: review                    0.44   5.4%     11   144.1
wait: tests                     0.32   4.0%      8   146.2
cargo test                      0.19   2.4%     43    16.0
wait: release                   0.15   1.9%      4   135.3
push gate                       0.14   1.7%     23    22.0
phase mark                      0.12   1.5%     80     5.5
remote wait                     0.11   1.4%     12    33.1
wait: build                     0.11   1.4%      6    66.0
loop tools                      0.10   1.2%     58     6.3
github                          0.04   0.5%     46     3.2
cargo build                     0.03   0.4%      4    30.4

phase                          hours  share  steps  mean s
build                           2.15  26.5%    879     8.8
gate                            2.14  26.4%    265    29.1
choose                          1.01  12.5%    551     6.6
ship                            0.86  10.5%    235    13.1
prove                           0.82  10.1%    241    12.3
research                        0.55   6.8%    309     6.4
orient                          0.27   3.4%    207     4.7
log                             0.18   2.2%     72     8.8
release                         0.13   1.5%     46     9.8

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
176 pass(es), 13842 steps, 89.4 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.49  36.3%    197   593.7
wait: tests                    19.11  21.4%    100   688.0
wait: gate                     14.05  15.7%     96   527.0
model                          11.12  12.4%   6477     6.2
cargo test                      2.65   3.0%    321    29.7
wait: build                     2.42   2.7%     19   458.4
remote command                  1.47   1.6%    441    12.0
push gate                       1.02   1.1%    305    12.0
wait: review                    0.99   1.1%     24   147.8
wait: clip                      0.68   0.8%     14   175.8
wait: release                   0.56   0.6%      6   336.8
shell                           0.51   0.6%   2654     0.7
cargo build                     0.43   0.5%     46    33.9
loop tools                      0.39   0.4%    435     3.3
remote wait                     0.39   0.4%     46    30.3

phase                          hours  share  steps  mean s
orient                         77.60  86.8%   9622    29.0
gate                            3.08   3.4%    386    28.7
build                           2.81   3.1%   1123     9.0
prove                           2.02   2.3%    742     9.8
choose                          1.54   1.7%    965     5.8
ship                            1.21   1.4%    383    11.4
research                        0.60   0.7%    359     6.0
release                         0.32   0.4%    123     9.4
log                             0.20   0.2%     84     8.8
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

