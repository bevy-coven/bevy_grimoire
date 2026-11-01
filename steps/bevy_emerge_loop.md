# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 16:16Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 208 | 2026-11-01T16:06Z | 3 | 0 | 4 | bevy_grimoire_core-v0.5.2 | 1 | 0 | demo? |
| 209 | 2026-11-01T16:16Z | 5 | 0 | 3 | - | 2 | 0 | demo? |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8             9      15.3        16.1
```

## The last 12 passes

```text
12 pass(es), 2009 steps, 4.7 h of pass time

category                       hours  share  steps  mean s
model                           1.74  37.3%    930     6.7
wait: gate                      0.89  19.2%     19   168.9
cargo test                      0.37   7.9%     45    29.5
wait: other                     0.34   7.4%     16    77.2
phase mark                      0.24   5.1%     75    11.4
remote command                  0.20   4.3%     87     8.4
wait: tests                     0.15   3.3%      8    68.6
wait: review                    0.15   3.3%      7    77.9
push gate                       0.15   3.2%     26    20.8
shell                           0.07   1.5%    318     0.8
clip record/render              0.07   1.4%     29     8.3
remote wait                     0.06   1.2%     13    15.6
github                          0.05   1.2%     67     2.9
cargo build                     0.04   0.9%     14    10.7
loop tools                      0.03   0.7%     29     4.0

phase                          hours  share  steps  mean s
orient                          1.35  29.0%    514     9.5
gate                            0.62  13.3%    113    19.8
prove                           0.51  11.0%    181    10.2
build                           0.49  10.6%    336     5.3
ship                            0.49  10.4%    118    14.8
research                        0.44   9.5%    323     4.9
choose                          0.38   8.2%    317     4.3
log                             0.33   7.1%     55    21.5
release                         0.02   0.4%      8     8.2
bugs                            0.02   0.4%     23     2.6

slowest single steps
  pass  200     507s  bg    shell                bg_11 failed: BEVY_BRP_PORT=15710 /tmp/p186/bin/bevy_grimoire_mapper . town --live > /tmp/
  pass  200     286s  tool  wait: gate           Waiting for the gate
  pass  200     281s  tool  wait: gate           Waiting for the gate
  pass  200     279s  tool  wait: gate           Waiting for the gate
  pass  199     277s  tool  wait: gate           Waiting for the gate
  pass  200     266s  tool  wait: other          Waiting for the Linux rerun
  pass  200     249s  tool  wait: gate           Waiting for the gate result
  pass  208     240s  tool  wait: gate           Waiting for tag push
  pass  198     231s  tool  wait: review         Waiting for reviewer
  pass  198     226s  tool  wait: gate           Waiting for gate on big
  pass  200     222s  bg    shell                bg_9 completed: python3 - <<'EOF' p='crates/bevy_grimoire_mapper/examples/asset_spike_meas
  pass  206     214s  tool  wait: gate           Waiting for push gate
  pass  200     202s  tool  wait: gate           Waiting for the gate
  pass  198     201s  tool  wait: gate           Waiting for gate on big
  pass  200     190s  bg    shell                bg_16 failed: kill 26041; sleep 2; pgrep -f "/tmp/p186/bin/bevy_grimoire_mapper" | wc -l; 
```

## Every pass recorded

```text
189 pass(es), 16064 steps, 94.5 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.83  34.7%    213   554.9
wait: tests                    19.26  20.4%    108   642.2
wait: gate                     15.05  15.9%    118   459.1
model                          13.03  13.8%   7505     6.3
cargo test                      3.04   3.2%    368    29.7
wait: build                     2.43   2.6%     21   416.2
remote command                  1.78   1.9%    547    11.7
push gate                       1.17   1.2%    333    12.6
wait: review                    1.17   1.2%     33   127.2
wait: clip                      0.71   0.8%     19   135.3
shell                           0.59   0.6%   2991     0.7
wait: release                   0.56   0.6%      6   336.8
cargo build                     0.48   0.5%     61    28.1
phase mark                      0.45   0.5%    207     7.8
remote wait                     0.44   0.5%     60    26.7

phase                          hours  share  steps  mean s
orient                         78.99  83.5%  10166    28.0
gate                            3.70   3.9%    499    26.7
build                           3.37   3.6%   1510     8.0
prove                           2.63   2.8%    961     9.8
ship                            1.93   2.0%    546    12.7
choose                          1.92   2.0%   1282     5.4
research                        1.09   1.1%    726     5.4
log                             0.55   0.6%    143    13.8
release                         0.34   0.4%    131     9.4
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

