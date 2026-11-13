# The benchmark's runs

Each row is one committed run of `cauldron-bench` (BM-03, #413): a report beside the JSON `cauldron-bench compare` reads. The raw trial directories are not committed. This file is written by `cauldron-bench report`, newest first, so edit the reports and not this list.

```sh
cauldron-bench report <run directory> --label baseline   # writes docs/bench/runs/<date>-baseline.md
cauldron-bench compare docs/bench/runs/<a>.json docs/bench/runs/<b>.json
```

| run | date | agent and model | variant | commit | dev tasks: mean success (95% interval) | mean pass^k | cost per success | harness errors | held-out total |
|---|---|---|---|---|---|---|---|---|---|
