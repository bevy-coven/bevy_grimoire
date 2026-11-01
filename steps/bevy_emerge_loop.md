# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 13:55Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 200 | 2026-11-01T13:55Z | 6 | 0 | 3 | - | 5 | 2 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
```

## The last 12 passes

```text
12 pass(es), 3120 steps, 8.6 h of pass time

category                       hours  share  steps  mean s
model                           3.10  36.0%   1411     7.9
wait: gate                      2.07  24.1%     39   191.5
wait: other                     0.76   8.9%     23   119.7
remote command                  0.67   7.8%    267     9.0
wait: review                    0.55   6.4%     18   110.0
wait: tests                     0.28   3.2%      8   123.9
cargo test                      0.17   1.9%     37    16.2
remote wait                     0.15   1.8%     20    27.2
phase mark                      0.13   1.5%     73     6.6
push gate                       0.13   1.5%     30    15.2
wait: release                   0.11   1.2%      3   126.7
loop tools                      0.09   1.1%     53     6.4
shell                           0.06   0.7%    352     0.6
clip record/render              0.06   0.7%     25     8.7
wait: build                     0.06   0.7%      5    41.0

phase                          hours  share  steps  mean s
gate                            1.84  21.4%    272    24.3
orient                          1.55  18.0%    638     8.7
build                           1.42  16.5%    577     8.8
choose                          1.14  13.3%    674     6.1
ship                            0.99  11.5%    252    14.2
prove                           0.71   8.3%    226    11.4
research                        0.70   8.1%    366     6.9
log                             0.18   2.1%     69     9.4
release                         0.08   0.9%     24    11.7

slowest single steps
  pass  195     648s  tool  wait: tests          Waiting for local core tests
  pass  200     507s  bg    shell                bg_11 failed: BEVY_BRP_PORT=15710 /tmp/p186/bin/bevy_grimoire_mapper . town --live > /tmp/
  pass  189     303s  tool  wait: review         Waiting for reviewer and the after run
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
```

## Every pass recorded

```text
180 pass(es), 14980 steps, 92.3 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.57  35.3%    200   586.3
wait: tests                    19.18  20.8%    104   663.8
wait: gate                     14.76  16.0%    111   478.7
model                          12.19  13.2%   6993     6.3
cargo test                      2.71   2.9%    333    29.3
wait: build                     2.43   2.6%     21   416.2
remote command                  1.78   1.9%    547    11.7
wait: review                    1.17   1.3%     33   127.2
push gate                       1.05   1.1%    319    11.8
wait: clip                      0.71   0.8%     18   142.6
wait: release                   0.56   0.6%      6   336.8
shell                           0.55   0.6%   2795     0.7
cargo build                     0.46   0.5%     57    29.0
loop tools                      0.43   0.5%    448     3.5
remote wait                     0.43   0.5%     55    28.0

phase                          hours  share  steps  mean s
orient                         78.92  85.5%  10090    28.2
gate                            3.50   3.8%    461    27.3
build                           2.87   3.1%   1174     8.8
prove                           2.23   2.4%    816     9.8
choose                          1.81   2.0%   1181     5.5
ship                            1.47   1.6%    433    12.2
research                        0.87   0.9%    534     5.8
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

