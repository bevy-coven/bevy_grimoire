# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 14:08Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 200 | 2026-11-01T13:55Z | 6 | 0 | 3 | - | 5 | 2 | - |
| 201 | 2026-11-01T14:08Z | 2 | 0 | 6 | - | 2 | 0 | demo? |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8             1      12.7        12.7
```

## The last 12 passes

```text
12 pass(es), 2969 steps, 8.2 h of pass time

category                       hours  share  steps  mean s
model                           2.88  35.2%   1348     7.7
wait: gate                      1.97  24.1%     38   186.2
wait: other                     0.71   8.8%     21   122.5
remote command                  0.65   8.0%    246     9.6
wait: review                    0.47   5.7%     17    98.7
wait: tests                     0.33   4.0%      9   131.2
phase mark                      0.17   2.1%     71     8.9
cargo test                      0.17   2.0%     37    16.1
remote wait                     0.13   1.6%     17    27.7
push gate                       0.11   1.4%     29    14.3
wait: release                   0.11   1.3%      3   126.7
loop tools                      0.08   1.0%     52     5.7
clip record/render              0.06   0.7%     25     8.7
shell                           0.06   0.7%    347     0.6
wait: build                     0.06   0.7%      4    51.3

phase                          hours  share  steps  mean s
gate                            1.70  20.8%    264    23.1
orient                          1.49  18.3%    601     8.9
build                           1.16  14.2%    464     9.0
choose                          1.14  14.0%    674     6.1
ship                            0.97  11.8%    222    15.7
research                        0.73   9.0%    405     6.5
prove                           0.73   8.9%    226    11.6
log                             0.18   2.2%     66     9.6
release                         0.08   1.0%     24    11.7

slowest single steps
  pass  195     648s  tool  wait: tests          Waiting for local core tests
  pass  200     507s  bg    shell                bg_11 failed: BEVY_BRP_PORT=15710 /tmp/p186/bin/bevy_grimoire_mapper . town --live > /tmp/
  pass  192     293s  tool  wait: gate           Waiting for gate output
  pass  192     292s  tool  wait: gate           Waiting for gate output
  pass  190     291s  tool  wait: gate           Waiting for gate
  pass  195     290s  tool  wait: gate           Waiting for gate
  pass  191     290s  tool  wait: gate           Waiting for the gate
  pass  195     290s  tool  wait: other          Waiting for before-run
  pass  195     289s  tool  wait: gate           Waiting for gate
  pass  190     286s  tool  wait: other          Waiting for after-run on big
  pass  200     286s  tool  wait: gate           Waiting for the gate
  pass  195     285s  tool  wait: gate           Waiting for gate
  pass  195     285s  tool  wait: gate           Waiting for gate
  pass  200     281s  tool  wait: gate           Waiting for the gate
  pass  200     279s  tool  wait: gate           Waiting for the gate
```

## Every pass recorded

```text
181 pass(es), 15068 steps, 92.5 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.57  35.2%    200   586.3
wait: tests                    19.23  20.8%    105   659.2
wait: gate                     14.80  16.0%    112   475.6
model                          12.25  13.3%   7035     6.3
cargo test                      2.71   2.9%    334    29.2
wait: build                     2.43   2.6%     21   416.2
remote command                  1.78   1.9%    547    11.7
wait: review                    1.17   1.3%     33   127.2
push gate                       1.05   1.1%    319    11.8
wait: clip                      0.71   0.8%     19   135.3
wait: release                   0.56   0.6%      6   336.8
shell                           0.55   0.6%   2805     0.7
cargo build                     0.46   0.5%     57    29.0
loop tools                      0.43   0.5%    452     3.5
remote wait                     0.43   0.5%     55    28.0

phase                          hours  share  steps  mean s
orient                         78.93  85.4%  10100    28.1
gate                            3.52   3.8%    467    27.1
build                           2.87   3.1%   1174     8.8
prove                           2.31   2.5%    830    10.0
choose                          1.81   2.0%   1181     5.5
ship                            1.54   1.7%    451    12.3
research                        0.90   1.0%    573     5.7
release                         0.32   0.3%    123     9.4
log                             0.25   0.3%     97     9.3
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

