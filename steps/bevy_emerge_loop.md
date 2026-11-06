# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-06 16:15Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
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
| 421 | 2026-11-06T16:15Z | 1 | 0 | 0 | - | 0 | 0 | - |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
Cs-MacBook-Pro anthropic/claude-sonnet-5-5  xhigh    4     -             1      75.8        75.8
big        anthropic/claude-sonnet-5-5  high     10    8            18      22.1        17.1
big        anthropic/claude-sonnet-5-5  high     6     4            12      72.3        78.8
big        -                            -        -     -            60      56.6        25.9
big        claude-sonnet-5-5            medium   -     -            15       8.4         0.8
```

## The last 12 passes

```text
12 pass(es), 204 steps, 0.6 h of pass time

category                       hours  share  steps  mean s
wait: other                     0.39  64.4%      6   235.6
phase mark                      0.10  17.2%     21    18.0
model                           0.09  14.0%    108     2.8
push gate                       0.02   3.9%      8    10.8
loop tools                      0.00   0.4%     23     0.4
shell                           0.00   0.0%     36     0.0
git                             0.00   0.0%      2     0.0

phase                          hours  share  steps  mean s
gate                            0.34  55.7%      8   152.9
prove                           0.10  17.2%      8    47.1
orient                          0.09  13.9%    148     2.1
build                           0.06   9.9%      8    27.2
repro                           0.01   2.4%     28     1.9
ship                            0.00   0.8%      4     4.2

slowest single steps
  pass  411     541s  tool  wait: other          {}
  pass  410     541s  tool  wait: other          {}
  pass  411     125s  tool  wait: other          {}
  pass  410     125s  tool  wait: other          {}
  pass  411      60s  tool  phase mark           [launched bg_1] cd crates/bevy_cauldron/tests; cat > capture_feedback_1793968919654.rs <<'
  pass  411      60s  tool  phase mark           [launched bg_2] cd crates/bevy_cauldron && python3 - <<'EOF' p="src/tiles.rs" s=open(p).re
  pass  411      60s  tool  phase mark           [launched bg_3] python3 tools/changelog/changelog.py add --id feedback-1793968919654 --sha
  pass  410      60s  tool  phase mark           [launched bg_1] cd crates/bevy_cauldron/tests; cat > capture_feedback_1793968919654.rs <<'
  pass  410      60s  tool  phase mark           [launched bg_2] cd crates/bevy_cauldron && python3 - <<'EOF' p="src/tiles.rs" s=open(p).re
  pass  410      60s  tool  phase mark           [launched bg_3] python3 tools/changelog/changelog.py add --id feedback-1793968919654 --sha
  pass  411      41s  tool  wait: other          {}
  pass  410      41s  tool  wait: other          {}
  pass  416      24s  tool  push gate            echo "- 2026-11-06 **pass 417** — nothing left to answer (queue empty; 3 new cases since r
  pass  414      14s  tool  push gate            git commit -qm "log: pass 415 nothing left to answer" -- docs/loop/log.md; git push -q ori
  pass  418      12s  tool  push gate            python3 - <<'EOF' p="docs/loop/log.md" s=open(p).read().splitlines() s[-1]="- 2026-11-06 *
```

## Every pass recorded

```text
322 pass(es), 45617 steps, 158.6 h of pass time

category                       hours  share  steps  mean s
wait: other                    40.27  25.4%    507   286.0
model                          28.97  18.3%  21858     4.8
wait: tests                    22.92  14.5%    329   250.8
wait: gate                     21.54  13.6%    279   277.9
cargo test                     11.64   7.3%   1285    32.6
shell                           4.69   3.0%  10301     1.6
wait: build                     3.85   2.4%    147    94.3
cargo build                     3.72   2.3%    455    29.4
push gate                       3.58   2.3%    700    18.4
clip record/render              3.14   2.0%   1049    10.8
wait: clip                      2.56   1.6%     51   181.0
remote wait                     1.87   1.2%    188    35.7
phase mark                      1.80   1.1%    704     9.2
remote command                  1.78   1.1%    549    11.7
wait: review                    1.28   0.8%     37   125.0

phase                          hours  share  steps  mean s
orient                         96.67  60.9%  18299    19.0
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

