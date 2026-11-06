# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-06 15:59Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 266 | 2026-11-03T04:53Z | 1 | 0 | 0 | - | 1 | 0 | - |
| 267 | 2026-11-03T05:11Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 268 | 2026-11-03T05:33Z | 2 | 0 | 6 | - | 1 | 0 | demo? |
| 269 | 2026-11-03T05:45Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 270 | 2026-11-03T05:57Z | 1 | 0 | 0 | - | 1 | 0 | - |
| 406 | 2026-11-06T14:55Z | 2 | 0 | 5 | - | 1 | 0 | demo? |
| 408 | 2026-11-06T15:23Z | 3 | 0 | 1 | - | 1 | 0 | demo? |
| 410 | 2026-11-06T15:40Z | 2 | 0 | 1 | - | 1 | 0 | demo? |
| 412 | 2026-11-06T15:42Z | 2 | 0 | 0 | - | 0 | 0 | - |
| 414 | 2026-11-06T15:45Z | 1 | 0 | 0 | - | 0 | 0 | - |
| 416 | 2026-11-06T15:50Z | 1 | 0 | 0 | - | 0 | 0 | - |
| 418 | 2026-11-06T15:59Z | 1 | 0 | 0 | - | 0 | 0 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4            12      72.3        78.8
big        -                            -        -     -            59      57.5        26.0
big        claude-sonnet-5-5            medium   -     -            14       9.0         1.0
```

## The last 12 passes

```text
12 pass(es), 330 steps, 1.5 h of pass time

category                       hours  share  steps  mean s
wait: other                     1.10  75.1%     10   397.5
model                           0.14   9.8%    170     3.1
phase mark                      0.14   9.5%     31    16.3
cargo test                      0.05   3.6%      4    48.0
push gate                       0.02   1.5%      9     8.8
loop tools                      0.01   0.4%     32     0.7
shell                           0.00   0.1%     66     0.0
read files                      0.00   0.0%      4     0.1
git                             0.00   0.0%      4     0.0

phase                          hours  share  steps  mean s
ship                            0.56  38.3%     14   144.8
prove                           0.36  24.5%     36    36.0
gate                            0.36  24.3%     32    40.1
orient                          0.08   5.6%    154     1.9
build                           0.07   4.6%     12    20.2
repro                           0.04   2.7%     82     1.8

slowest single steps
  pass  409     939s  tool  wait: other          {}
  pass  408     939s  tool  wait: other          {}
  pass  411     541s  tool  wait: other          {}
  pass  410     541s  tool  wait: other          {}
  pass  409     341s  tool  wait: other          {}
  pass  408     341s  tool  wait: other          {}
  pass  411     125s  tool  wait: other          {}
  pass  410     125s  tool  wait: other          {}
  pass  409      60s  tool  phase mark           [launched bg_2] cat > docs/loop/iterations/2026-11-06-debug-feedback-1793928006124-5e2f033
  pass  408      60s  tool  phase mark           [launched bg_2] cat > docs/loop/iterations/2026-11-06-debug-feedback-1793928006124-5e2f033
  pass  409      60s  tool  cargo test           [launched bg_1] cd crates/bevy_cauldron; export CARGO_TARGET_DIR=$HOME/.cargo-target-share
  pass  408      60s  tool  cargo test           [launched bg_1] cd crates/bevy_cauldron; export CARGO_TARGET_DIR=$HOME/.cargo-target-share
  pass  411      60s  tool  phase mark           [launched bg_1] cd crates/bevy_cauldron/tests; cat > capture_feedback_1793968919654.rs <<'
  pass  411      60s  tool  phase mark           [launched bg_2] cd crates/bevy_cauldron && python3 - <<'EOF' p="src/tiles.rs" s=open(p).re
  pass  411      60s  tool  phase mark           [launched bg_3] python3 tools/changelog/changelog.py add --id feedback-1793968919654 --sha
```

## Every pass recorded

```text
320 pass(es), 45601 steps, 158.6 h of pass time

category                       hours  share  steps  mean s
wait: other                    40.27  25.4%    507   286.0
model                          28.97  18.3%  21849     4.8
wait: tests                    22.92  14.5%    329   250.8
wait: gate                     21.54  13.6%    279   277.9
cargo test                     11.64   7.3%   1285    32.6
shell                           4.69   3.0%  10301     1.6
wait: build                     3.85   2.4%    147    94.3
cargo build                     3.72   2.3%    455    29.4
push gate                       3.57   2.3%    699    18.4
clip record/render              3.14   2.0%   1049    10.8
wait: clip                      2.56   1.6%     51   181.0
remote wait                     1.87   1.2%    188    35.7
phase mark                      1.80   1.1%    702     9.2
remote command                  1.78   1.1%    549    11.7
wait: review                    1.28   0.8%     37   125.0

phase                          hours  share  steps  mean s
orient                         96.66  60.9%  18283    19.0
build                          17.82  11.2%   9064     7.1
prove                          10.55   6.7%   5315     7.1
ship                           10.31   6.5%   2656    14.0
gate                            9.96   6.3%   2218    16.2
choose                          3.90   2.5%   2761     5.1
bugs                            3.44   2.2%   2047     6.1
log                             2.58   1.6%    921    10.1
research                        2.12   1.3%   1672     4.6
release                         1.12   0.7%    308    13.1
repro                           0.15   0.1%    176     3.0

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

