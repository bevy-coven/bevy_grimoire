# Where bevy_emerge_loop's time goes

Written by `tools/loop/steps.py publish` after every pass; the rows are in [`bevy_emerge_loop.tsv`](bevy_emerge_loop.tsv). Updated 2026-11-02 12:05Z.

## What the last passes shipped

From [`bevy_emerge_loop-audit.tsv`](bevy_emerge_loop-audit.tsv): commits on main, unpushed commits, package files changed, release tags, doc and demo files, and what was missing.

| pass | at | commits | unpushed | package_files | released | docs_files | demo_files | missing |
|---|---|---|---|---|---|---|---|---|
| 0 | 2026-11-02T12:05Z | 22 | 2 | 14 | - | 26 | 21 | push |

## How each configuration did

From [`bevy_emerge_loop-config.tsv`](bevy_emerge_loop-config.tsv): one row per pass with its host, wall time, model, thinking level and build settings.

```text
host       model                        think    jobs  threads  passes  mean min  median min
big        -                            -        -     -             1 29893685.6  29893685.6
```

## The last 12 passes

```text
1 pass(es), 123 steps, 0.3 h of pass time

category                       hours  share  steps  mean s
wait: tests                     0.11  41.1%      1   382.8
wait: other                     0.05  20.9%      1   194.8
model                           0.05  18.0%     53     3.2
cargo test                      0.03  12.9%      2    60.2
cargo build                     0.02   6.4%      1    60.0
shell                           0.00   0.6%     51     0.1
git                             0.00   0.0%      1     0.2
loop tools                      0.00   0.0%      3     0.0
edit files                      0.00   0.0%      8     0.0
read files                      0.00   0.0%      2     0.1

phase                          hours  share  steps  mean s
orient                          0.26 100.0%    123     7.6

slowest single steps
  pass    0     383s  tool  wait: tests          Waiting for the first test run to report
  pass    0     195s  tool  wait: other          Waiting on the rerun
  pass    0      60s  tool  cargo test           [launched bg_14] Rerunning regression test :: cd /home/ladvien/bevy_emerge_loop && cargo t
  pass    0      60s  tool  cargo test           [launched bg_12] Running new test to watch it fail :: cd /home/ladvien/bevy_emerge_loop &&
  pass    0      60s  tool  cargo build          [launched bg_13] Building after fix :: cd /home/ladvien/bevy_emerge_loop && cargo build -p
  pass    0      18s  model model                
  pass    0      11s  model model                
  pass    0      10s  model model                
  pass    0       6s  model model                
  pass    0       6s  model model                
  pass    0       6s  model model                
  pass    0       5s  model model                
  pass    0       5s  model model                
  pass    0       4s  model model                
  pass    0       4s  model model                
```

## Every pass recorded

```text
1 pass(es), 123 steps, 0.3 h of pass time

category                       hours  share  steps  mean s
wait: tests                     0.11  41.1%      1   382.8
wait: other                     0.05  20.9%      1   194.8
model                           0.05  18.0%     53     3.2
cargo test                      0.03  12.9%      2    60.2
cargo build                     0.02   6.4%      1    60.0
shell                           0.00   0.6%     51     0.1
git                             0.00   0.0%      1     0.2
loop tools                      0.00   0.0%      3     0.0
edit files                      0.00   0.0%      8     0.0
read files                      0.00   0.0%      2     0.1

phase                          hours  share  steps  mean s
orient                          0.26 100.0%    123     7.6

slowest single steps
  pass    0     383s  tool  wait: tests          Waiting for the first test run to report
  pass    0     195s  tool  wait: other          Waiting on the rerun
  pass    0      60s  tool  cargo test           [launched bg_14] Rerunning regression test :: cd /home/ladvien/bevy_emerge_loop && cargo t
  pass    0      60s  tool  cargo test           [launched bg_12] Running new test to watch it fail :: cd /home/ladvien/bevy_emerge_loop &&
  pass    0      60s  tool  cargo build          [launched bg_13] Building after fix :: cd /home/ladvien/bevy_emerge_loop && cargo build -p
  pass    0      18s  model model                
  pass    0      11s  model model                
  pass    0      10s  model model                
  pass    0       6s  model model                
  pass    0       6s  model model                
  pass    0       6s  model model                
  pass    0       5s  model model                
  pass    0       5s  model model                
  pass    0       4s  model model                
  pass    0       4s  model model                
```

