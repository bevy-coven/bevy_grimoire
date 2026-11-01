# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 15:47Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 207 | 2026-11-01T15:47Z | 3 | 0 | 4 | - | 1 | 0 | demo? |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8             7      15.6        16.1
```

## The last 12 passes

```text
12 pass(es), 2299 steps, 5.3 h of pass time

category                       hours  share  steps  mean s
model                           2.02  37.9%   1057     6.9
wait: gate                      0.97  18.3%     23   152.1
remote command                  0.48   8.9%    131    13.1
wait: other                     0.34   6.4%     16    76.8
cargo test                      0.34   6.3%     42    28.9
phase mark                      0.21   3.9%     74    10.0
wait: review                    0.19   3.6%     10    69.1
wait: tests                     0.16   3.0%      9    63.1
push gate                       0.15   2.8%     27    19.6
clip record/render              0.07   1.3%     29     8.3
remote wait                     0.06   1.1%     14    15.2
shell                           0.06   1.1%    342     0.6
github                          0.06   1.1%     62     3.3
loop tools                      0.05   1.0%     33     5.9
wait: clip                      0.05   1.0%      6    30.4

phase                          hours  share  steps  mean s
orient                          1.39  26.2%    538     9.3
gate                            0.86  16.2%    145    21.4
ship                            0.72  13.5%    161    16.0
build                           0.69  12.9%    419     5.9
prove                           0.60  11.2%    219     9.8
choose                          0.46   8.7%    395     4.2
research                        0.45   8.4%    339     4.8
log                             0.13   2.4%     37    12.7
bugs                            0.02   0.3%     23     2.6
release                         0.01   0.2%      2    16.3

slowest single steps
  pass  200     507s  bg    shell                bg_11 failed: BEVY_BRP_PORT=15710 /tmp/p186/bin/bevy_grimoire_mapper . town --live > /tmp/
  pass  200     286s  tool  wait: gate           Waiting for the gate
  pass  200     281s  tool  wait: gate           Waiting for the gate
  pass  200     279s  tool  wait: gate           Waiting for the gate
  pass  199     277s  tool  wait: gate           Waiting for the gate
  pass  200     266s  tool  wait: other          Waiting for the Linux rerun
  pass  200     249s  tool  wait: gate           Waiting for the gate result
  pass  198     231s  tool  wait: review         Waiting for reviewer
  pass  197     228s  tool  wait: gate           Waiting for gate wait job
  pass  198     226s  tool  wait: gate           Waiting for gate on big
  pass  200     222s  bg    shell                bg_9 completed: python3 - <<'EOF' p='crates/bevy_grimoire_mapper/examples/asset_spike_meas
  pass  206     214s  tool  wait: gate           Waiting for push gate
  pass  196     205s  tool  wait: gate           Waiting for the gate
  pass  200     202s  tool  wait: gate           Waiting for the gate
  pass  198     201s  tool  wait: gate           Waiting for gate on big
```

## Every pass recorded

```text
187 pass(es), 15882 steps, 94.1 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.79  34.9%    211   559.5
wait: tests                    19.26  20.5%    107   647.9
wait: gate                     14.93  15.9%    116   463.4
model                          12.90  13.7%   7416     6.3
cargo test                      2.98   3.2%    360    29.8
wait: build                     2.43   2.6%     21   416.2
remote command                  1.78   1.9%    547    11.7
wait: review                    1.17   1.2%     33   127.2
push gate                       1.15   1.2%    330    12.6
wait: clip                      0.71   0.8%     19   135.3
shell                           0.57   0.6%   2960     0.7
wait: release                   0.56   0.6%      6   336.8
cargo build                     0.48   0.5%     59    29.0
remote wait                     0.44   0.5%     59    27.1
loop tools                      0.44   0.5%    465     3.4

phase                          hours  share  steps  mean s
orient                         78.98  84.0%  10147    28.0
gate                            3.70   3.9%    499    26.7
build                           3.24   3.4%   1446     8.1
prove                           2.59   2.8%    945     9.9
ship                            1.93   2.1%    544    12.8
choose                          1.89   2.0%   1260     5.4
research                        1.05   1.1%    698     5.4
log                             0.33   0.4%    119    10.0
release                         0.33   0.4%    125     9.5
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

