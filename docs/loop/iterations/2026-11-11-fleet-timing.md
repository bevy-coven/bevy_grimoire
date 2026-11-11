# Fleet timing: where the time went (2026-11-09 to 2026-11-11)

Read from the omp session JSONL of every worker (big, bmb, mac_air: 38 sessions), every `task` subagent (67: reviewers and fix agents) and the manager (1 session, 46 h wall). The tool is `tools/loop/fleet/timing.py`, built on `steps.py`'s timeline (`python3 tools/loop/fleet/timing.py --workers <dirs> --manager <session.jsonl>`); the raw output is below. Measured unless marked `[INFERENCE]`.

## Two corrections to the raw numbers

- **A long "model" row is parked time, not thinking.** omp disposes a finished session and revives it on the next message; `steps.py` charges the whole gap to the model. A reviewer's 14.1 h "model" row is `session_exit`, then an IRC message 14 h later. Rows over 10 minutes are counted apart as PARKED. Real stalls are the worker's 429s: one 17.1 h (`gdcuttwo`), and 8.2 h in two sessions that herdr's restart killed (`followfive`, `gooeythree`) [INFERENCE for the herdr cause: both die at the same time as a herdr restart].
- **The manager's `edit files` 12.2 h is the owner's night.** One row of 729 min is a session paused while the owner slept; it is not an edit.

## 1. Workers: two thirds of the active time is waiting on cargo

65.8 active worker-hours (after 25.3 h parked).

| share | what |
|---|---|
| 30% | the model (12,268 turns, median 2.4 s) |
| 48% | cargo, counted by name: test 14.8 h, waits on tests 9.4 h, build 3.4 h + 1.7 h, clippy 1.3 h + 0.7 h |
| 19% | `wait: other`: the worker's own intents on 8 of the largest are all test runs ("Waiting for touched tests", "for the ratchet run") [INFERENCE for the rest] |
| 3% | shell, sleep, remote wait |

A worker typed 1,566 filtered test commands, 247 clippy runs and 71 unfiltered runs. Each filtered run relinks the one 1,832-test editor binary (`bevy_cauldron` `all.rs`); a test run's foreground time has a median of 40 s, and the ones that go to the background average 3.8 min. **The cost per iteration is the build and link of that binary, not the test**: 1,566 runs at about 40 s is 17 h. [INFERENCE: the link share was not isolated; it is the first thing to measure.]

## 2. The push gate: 43% of runs were refused

61 gate runs: 30 passed (7.9 min mean, 4.0 h), 26 refused (10.1 min, 4.4 h), 5 other. 4.4 h of serial gate time bought nothing, and each refusal also cost a fix and another gate run.

Why they were refused (22 failing tests across the 26 refusals; some refusals named none that the log kept):

| cause | failing tests | a worker would have seen it if |
|---|---|---|
| wall-clock tests failing under load or by order: `keeping_a_set_over_an_existing_name` (5), the two UI budget tests (2 + 2) | 9 | they ran on a quiet host; this is the gate's own noise |
| fast static protected tests: `one_prompt_one_vocabulary` (2), `no_source_file_reaches_for_an_engine` (2), `every_resource_is_classified` (2), `no_project_file_is_written_the_plain_way` (2), and one each for the guide, the frozen data kinds, the samples, the chrome census, the reload contract | 13 | the worker had run the full suite once: 71 unfiltered runs in 38 sessions |

The first row is not a worker's fault: three wall-clock tests failed 9 times because big runs CI jobs and two or three agents' builds beside the gate (load 40 to 50). Fixed this pass at the test (`f72fa05c`, `1fb939a4`, `deaa2414`); one more (`the_frame_after_a_change...ten_thousand_nodes`, best of 5 at 12 ms) still fails when the host is busy, so I stopped the workers' processes during a gate push (`/tmp/fleet-mgr/pause-workers.sh`). **That deadlocked for 35 minutes**: a stopped worker's `cargo` holds the shared target's build lock, and the gate's clippy and nextest waited on it (load 1.7 on a 24-thread host). Stop only the workers' test binaries, never a cargo that holds the lock; or give the gate its own target dir.

## 3. The manager's 46 hours

| h | what |
|---|---|
| 14.9 | waiting on workers and subagents (`wait: other`) |
| 8.4 | waiting on the gate |
| 3.3 | waiting on the owner (8 questions, median 17.5 min) |
| 1.8 + 0.7 | waiting on reviews and subagents |
| 2.0 | the manager's own model time |
| 1.1 | waiting on tests |

The manager is the serial resource: one gate at a time, one worktree, and the gate cannot overlap a worker's build without risking a wall-clock refusal. Workers' wall time is mostly spent finished and waiting for it: `yaml` 42.3 h wall for 2.4 h active, `346` 44.8 h for 1.2 h, `355` 43.4 h for 0.3 h.

## What to change (each tied to a number above)

1. **The brief forbids the full suite, and that caused the static refusals.** `brief.md` rule 6 says "Never run ... the whole editor binary"; workers obeyed (71 unfiltered runs in 38 sessions). **A worker's DONE requires the full editor suite, once, from its own script** (`tools/loop/fleet/predone.sh`, one command): the fast static protected tests were most of the avoidable refusals, and 71 unfiltered runs in 38 sessions shows the brief's instruction is not followed. Gate side: refuse a worker's DONE line without the suite's `Summary` text in the result file.
2. **Measure the link share of a filtered run on big** (`cargo nextest run -p bevy_cauldron -E 'test(x)'` after touching one file, with `-Z timings` or `cargo build --timings`). If linking is over half, a faster linker (`mold`; `agents/TESTING.md` names it) or a split `bevy_cauldron` test binary is the single largest lever on 48% of the workers' time.
3. **Quiet the gate or the test, not both ways.** Stop agent builds during a gate push (done by hand now; put it in `tools/loop/fleet/gate.sh`), or move wall-clock budget tests behind `--run-ignored` and run them once on a quiet host in the release gate. Two refusal causes disappear either way.
4. **Do not let a worker sit finished.** Workers wait 40 h on a 2 h job because the manager reviews and gates one cluster at a time. Review while the next cluster is built, and land two clusters per gate push when their files do not overlap (the gate's cost is per push, 8 min, not per cluster).
5. **Fall back to Claude at the first 429**, not after 17 h: `gdcuttwo` lost 17.1 h to a credential block (30 min after a 429, then retries).

## Found while measuring

- **A push from bmb skipped the gate.** bmb's clone had no `core.hooksPath`, so `git push origin HEAD:main` landed `26a0a91d` in 4.8 s with no `pre-push gate` line. It was not `--no-verify`; the hook was simply not installed. It was checked afterwards (four runs on big of the same code, whose only failures were timing tests, and the bmb run, whose failures were the pty terminal tests that hang there). `core.hooksPath` is now set on bmb. The gate cannot run on bmb anyway: the pty terminal tests hang there (`the_terminal_*`, `terminal::tests::*`), so a bmb run needs `-E 'not test(terminal)'`, which the hook does not pass.
- **Model choice on a hard fix.** DeepSeek `navtwo` spent two rounds (1 h 8 min, $0.16) and left 4 of 9 tests red, three of the four product defects it had diagnosed but not fixed. A Claude agent fixed all four and ran the full suite in 17 min ($1.2). Put the fall back to Claude after one failed fix round, not two.
- **Three workers on one shared target contend before they test.** A single measured run alone took 578 s because other agents' cargo held the build lock; the same test ran in 4 s. The gate's 7.9 min mean is mostly that queue.
