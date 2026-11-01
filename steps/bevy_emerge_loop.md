# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-01 16:30Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 210 | 2026-11-01T16:21Z | 2 | 0 | 0 | - | 2 | 0 | - |
| 211 | 2026-11-01T16:30Z | 1 | 0 | 0 | - | 3 | 2 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            11      13.7        13.1
```

## The last 12 passes

```text
12 pass(es), 1649 steps, 3.8 h of pass time

category                       hours  share  steps  mean s
model                           1.35  35.9%    770     6.3
wait: gate                      0.66  17.5%     15   158.7
wait: other                     0.35   9.4%     16    79.3
cargo test                      0.33   8.9%     38    31.7
phase mark                      0.24   6.5%     75    11.8
wait: tests                     0.14   3.8%      6    86.0
push gate                       0.13   3.5%     21    22.4
remote command                  0.08   2.2%     46     6.5
remote wait                     0.08   2.1%     15    19.4
shell                           0.08   2.1%    274     1.1
clip record/render              0.07   2.0%     33     8.2
wait: review                    0.06   1.5%      4    49.8
github                          0.05   1.4%     66     2.8
cargo build                     0.04   1.1%     14    10.7
wait: clip                      0.03   0.8%      5    21.9

phase                          hours  share  steps  mean s
orient                          1.34  35.7%    514     9.4
build                           0.51  13.6%    340     5.4
ship                            0.48  12.8%    130    13.4
prove                           0.47  12.3%    180     9.3
log                             0.34   8.9%     57    21.3
research                        0.23   6.1%    208     4.0
gate                            0.20   5.3%     38    19.0
choose                          0.16   4.2%    131     4.4
release                         0.02   0.5%      8     8.2
bugs                            0.02   0.4%     23     2.6

slowest single steps
  pass  200     507s  bg    shell                bg_11 failed: BEVY_BRP_PORT=15710 /tmp/p186/bin/bevy_grimoire_mapper . town --live > /tmp/
  pass  200     286s  tool  wait: gate           Waiting for the gate
  pass  200     281s  tool  wait: gate           Waiting for the gate
  pass  200     279s  tool  wait: gate           Waiting for the gate
  pass  200     266s  tool  wait: other          Waiting for the Linux rerun
  pass  200     249s  tool  wait: gate           Waiting for the gate result
  pass  208     240s  tool  wait: gate           Waiting for tag push
  pass  200     222s  bg    shell                bg_9 completed: python3 - <<'EOF' p='crates/bevy_grimoire_mapper/examples/asset_spike_meas
  pass  206     214s  tool  wait: gate           Waiting for push gate
  pass  200     202s  tool  wait: gate           Waiting for the gate
  pass  200     190s  bg    shell                bg_16 failed: kill 26041; sleep 2; pgrep -f "/tmp/p186/bin/bevy_grimoire_mapper" | wc -l; 
  pass  201     190s  tool  wait: tests          Waiting for test build
  pass  208     187s  tool  wait: gate           Waiting for push gate
  pass  202     175s  tool  wait: other          {}
  pass  202     166s  tool  wait: other          {}
```

## Every pass recorded

```text
191 pass(es), 16188 steps, 94.8 h of pass time

category                       hours  share  steps  mean s
wait: other                    32.84  34.7%    214   552.5
wait: tests                    19.26  20.3%    108   642.2
wait: gate                     15.05  15.9%    118   459.1
model                          13.11  13.8%   7565     6.2
cargo test                      3.05   3.2%    369    29.7
wait: build                     2.43   2.6%     21   416.2
remote command                  1.78   1.9%    547    11.7
push gate                       1.17   1.2%    334    12.6
wait: review                    1.17   1.2%     33   127.2
wait: clip                      0.71   0.8%     19   135.3
shell                           0.60   0.6%   3007     0.7
wait: release                   0.56   0.6%      6   336.8
phase mark                      0.49   0.5%    218     8.1
remote wait                     0.49   0.5%     65    27.0
cargo build                     0.48   0.5%     61    28.1

phase                          hours  share  steps  mean s
orient                         79.00  83.4%  10180    27.9
gate                            3.70   3.9%    499    26.7
build                           3.38   3.6%   1514     8.0
prove                           2.69   2.8%    996     9.7
choose                          1.97   2.1%   1312     5.4
ship                            1.95   2.1%    563    12.5
research                        1.10   1.2%    742     5.3
log                             0.58   0.6%    151    13.9
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

