# `cauldron-bench`: a real agent against a real editor

```sh
cargo run -p bevy_cauldron_bench -- run --suite dev --trials 5
cargo run -p bevy_cauldron_bench -- run --agent oracle     # no model, must pass every task
cargo run -p bevy_cauldron_bench -- run --agent nothing    # connects and stops, must pass none
cargo run -p bevy_cauldron_bench -- run --variant no-frame-view --agent oracle
```

`run` takes every task of a suite `n` times. One trial:

1. **copies the fixture project** into a directory of its own, lays the arm's overlay over the copy, and
   applies the task's `setup` through the editor;
2. **starts the editor binary built at the commit under test**, on its own BRP port and (on Linux) its own
   `Xvfb` display, and waits for `grimoire/state` to answer;
3. **starts the agent connected exactly as a person's agent is** — the MCP config comes from the editor's
   own `grimoire/agent_setup`, which is what the terminal's agent tab writes, with a fresh token — and
   gives it the task's `prompt` and nothing else. The guide reaches it over MCP (`grimoire_guide`) as it
   reaches every agent;
4. **stops the agent at `budget.wall_s`** and closes the editor;
5. **grades what is left** (BM-01, `crates/bevy_cauldron_bench`) and writes `trial.json`: the outcome, every
   check with the value it measured, the agent's tool calls, tokens and wall time, the tools its
   `tools/list` held, and the wishes it made.

## The four agents

| `--agent` | What it does | What it must score |
|---|---|---|
| `omp` (default) | a real model harness, `omp -p --mode json --model <model>` | whatever it scores — that is the measurement |
| `claude-code` | Claude Code, `claude -p --output-format stream-json --verbose --mcp-config <the editor's config> --strict-mcp-config --model <model>` (default `sonnet`) | whatever it scores — that is the measurement |
| `oracle` | calls the task's own `solution` through the real MCP server | **passes every task** |
| `nothing` | connects, reads `tools/list`, and stops | **passes none** |

`oracle` and `nothing` need no model, and they are what proves the pipe: the editor started, the server
answered, the config worked, the changes landed in the project, and the grader read them. A suite where
the oracle fails a task, or `nothing` passes one, is a harness problem rather than a result.

### Claude Code is isolated the way `omp` is (BM-06)

- **`CLAUDE_CONFIG_DIR` is a scratch directory of the trial's own** and so is `HOME`: no user `CLAUDE.md`, no
  memory, no user settings, no session store. `--strict-mcp-config` leaves the Dark Factory's server the
  only one, and `--disable-slash-commands` loads no skill.
- **The credential is carried in and nothing else**: `ANTHROPIC_API_KEY` if the environment has it, else the
  login file `~/.claude/.credentials.json`. A host with neither is a harness error that says to log in.
- **The transcript is the measurement.** `tokens_in/out/cache_*` are the `result` event's usage, the calls are
  the `tool_use` blocks and whether each was refused is its `tool_result`, and the cost Claude Code reports
  (`total_cost_usd`) is kept for a model with no listed price.
- **A transcript that shows anything else is a harness error**: an MCP server other than the editor's, a tool
  outside the editor's and Claude Code's own set (`claude::BUILTIN_TOOLS`), or a server that did not connect.

## Agents and models compared: `matrix` (BM-06)

```sh
cauldron-bench matrix --trials 5 --label agents \
  --pairs omp:ollama-cloud/deepseek-v4.1-flash:1017,omp:llama.cpp/bonsai-2-27b,claude-code:sonnet
```

One suite, once per pair, **on the same tasks, trials, variant and editor commit**: the only difference
between two rows is the agent and model, and a matrix of runs at different commits, guides, suites or trial
counts is refused. It writes **one report with a row per pair** (success with its interval, `pass^k`, wall
time, tool calls, edits behind the editor's back, cost per trial, per success and for the run), marks the
pairs **on the cost-versus-success frontier** (no other pair succeeds at least as often for no more cost),
and per pair lists the tools it called most, its refusals by tool and its wishes, then says which tools were
refused to **every** pair (the Dark Factory's hard spot) and which to **one** pair only (that agent's).
A model with an unlisted price costs `unknown` and is not placed on the frontier; a local model's listed
price is often 0, and its wall time is in the table. With `--label`, the matrix is published as
`docs/bench/matrices/<date>-<label>.md` and `.json` and listed in `docs/bench/README.md`.

## Outcomes

`pass`, `fail`, and **`harness_error`** — and a harness error is never scored as either of the other two.
An editor that did not start, a port that did not open, a transcript that could not be read, a `--hide-tools`
name this build has no tool for: those are facts about the harness, with the reason in the record, and the
run exits non-zero so a script notices.

## The report, and what changed between two runs (BM-03)

Every run ends with a report, `report.md` and `report.json` beside its trials. **Name a run with `--label`**
and it is also published as `docs/bench/runs/<date>-<label>.md` and `.json` and listed in
`docs/bench/README.md`, newest first; the raw trial directories are never committed.

```sh
cauldron-bench run --trials 5 --label baseline         # trials, then the report
cauldron-bench report <run directory> --label baseline # the same report, from trials already run
cauldron-bench compare docs/bench/runs/<a>.json docs/bench/runs/<b>.json
cauldron-bench compare <a> <b> --across                # a different suite, agent or trial count
```

- **The header** names the editor and MCP server commit (`+dirty` when the tree had changes), the SHA-256 of
  the shipped guide, the agent and its version, the model and its listed price, the variant, the trials per
  task, the host and the date, so a reader can reproduce the run.
- **Per task**: trials `n`, successes `c`, `c/n`, **`pass^k`** for `k = 1..n` (`C(c,k)/C(n,k)`, τ-bench,
  `10.48550/arXiv.2406.12045`), median and p90 wall time, mean tool calls, refusals, undos, **edits behind the
  editor's back** (files changed that no operation-log line explains: the agent went around the tools),
  tokens, cost and wishes; and for each failed trial the **first failing check** with what it wanted and
  measured, and the agent's **last five tool calls**.
- **Per suite and per tag** (dev tasks): mean success with a 95% bootstrap interval (tasks, then trials within
  tasks, resampled `BOOTSTRAP_RESAMPLES = 10_000` times from a fixed seed), mean `pass^k`, cost, cost per
  success and the harness-error count.
- **Held-out tasks are a total and nothing else**: never named, never with a prompt, never by tag.
- **Cost is the model's listed price times the tokens the harness reported** (`omp models find --json`).
  A model with no fixed price costs `unknown`, never `0`; a harness that reports its own cost (Claude Code)
  is used only when no price is listed.
- **A run is unfit for comparison** when more than `HARNESS_ERROR_MAX_FRACTION = 0.10` of its trials are
  harness errors. The report says so at the top and `compare` refuses it.
- **`compare <a> <b>`** pairs the tasks both runs scored and answers `better`, `worse` or `no measurable
  change` with a paired 95% bootstrap interval of the difference; the interval including zero is no
  measurable change. It lists every task whose success moved with both counts, each marked as a single task
  that is not significant alone. Runs with different suites, agents or trial counts are refused unless
  `--across`, which says which differ.

## Variants

`<suite>/variants.yaml` names arms to compare:

```yaml
version: 1
variants:
  - name: no-frame-view
    hide_tools: [grimoire_frame_view]      # served as --hide-tools: absent from tools/list
    overlay: [.dark_factory/AGENTS.md]     # project files laid over the copy
```

`hide_tools` reaches the MCP server as `--hide-tools`, so the tool is **absent from the agent's tool list**
rather than refused when called — an arm asking "what does it do without this tool" is not answered by a
tool that is listed and refuses. **Comparing two editor versions needs no variant**: that is two runs at two
commits.

**The tool list is read from the agent, not from the server.** `omp --mode json`'s stream carries no `tools`
key at all (measured: a whole 810-event transcript, 2026-11-13), so the extension omp already loads to find
`.mcp.json` records what `pi.getAllTools()` answers at the start of the agent's first turn, into
`tools-offered.jsonl` beside the transcript. omp spells the editor's tools `mcp__cauldron_grimoire_state`;
the record spells them as the server does (`grimoire_state`), so a `hide_tools` name is looked up in one
spelling. A list that cannot be read (no line, a registry that threw, an empty one, none of the editor's
tools) is a harness error, never `tools_offered: []`.

Measured on this host, 2026-11-13, one trial per arm on `count-what-the-bedroom-holds` with
`anthropic/claude-haiku-4-5`: the bare run was offered **46** tools (26 of the editor's, 20 of omp's own)
with `grimoire_frame_view` among them; the `no-frame-view` run was offered **45**, the same list less exactly
`grimoire_frame_view`. Neither agent called it.

## What the agent is allowed to know

The benchmarked agent is given the task's prompt and what the project states, and nothing else:

- `--no-context-files --no-skills --no-rules --no-extensions -e <the trial's mcp folder>`, so no `AGENTS.md`,
  no skill, no rule and no extension beyond the one that carries the editor's `.mcp.json`;
- **`HOME` set to the trial's own**, so the harness reads no user `~/.omp/agent/mcp.json`, no memory and no
  session store. Without it the developer's own MCP servers load into every trial and the agent spends its
  turn on `bash` — measured, 2026-11-13;
- **the credential carried into that `HOME`** by `omp auth export` and `omp auth import`, because an isolated
  `HOME` has no login either. It carries credentials only, never an MCP server, and a host with none is a
  harness error naming what to do (`omp` once).

A trial whose agent was **offered** a tool outside that set (the editor's `grimoire_*` and omp's own,
`run::BUILTIN_TOOLS`) is a harness error naming the tool, as is one that was offered none of the
editor's, and so is one whose model provider refused a turn (an HTTP 429 or 402): the provider's answer is
not a result about the agent.

## Held-out tasks (BM-04)

The suite is twenty dev tasks (`bench/tasks/`, to iterate against) and eight **held-out** ones, which a change is
scored on and never tuned against, so they are **never in the repository, an issue, a commit or a log**. They
live outside every checkout, at `/mnt/codex_fs/bench/dark_factory/held_out/`, one folder per task holding its
`task.yaml` and its own scene (`fixture/assets/grimoire/maps/…`).

```sh
cauldron-bench run --agent oracle  --held-out /mnt/codex_fs/bench/dark_factory/held_out   # all 28: must pass every one
cauldron-bench run --agent nothing --held-out /mnt/codex_fs/bench/dark_factory/held_out   # must pass none
cauldron-bench run --trials 5 --label baseline --held-out /mnt/codex_fs/bench/dark_factory/held_out
```

- **The runner proves them valid first.** Before a model is run on a held-out task, `nothing` must fail it and
  `oracle` must pass it, on the real editor: BM-01's two runs, which the push gate cannot make for tasks it
  cannot read. A failure says `held-out task 3 of 8`, never the id or the checks.
- **A trial is given its own scene and nothing of the task**: the task's `fixture/` is laid over the trial's
  copy, and its prompt, solution and checks are not written into a trial's folder.
- **A held-out trial is filed by ordinal and recorded as a count.** Its folder is `held-out-<n>/trial-<k>/`,
  never the task's id, and its `trial.json` holds the outcome and how many checks held and nothing of the task: no id,
  prompt, title, tags or check text. The folder is an ancestor of every later trial's working directory, and a
  model that reads `../trial-1/trial.json` learns what a score is and nothing about what it scored. The validity
  proofs (whose records hold the solution) are written to a scratch directory and deleted before any model trial
  starts.
- **The separation is a path nobody is told, and that is all it is.** The share is world-readable on the hosts
  that mount it and `omp` has no sandbox, so an agent that searched the filesystem for task files could find them.
  The fixes above remove every place a *run* writes the tasks; they do not make the share private. A benchmark
  run on a host that mounts it trusts the agent not to go looking, the way a student is trusted with an exam in
  the next room.
- **Nothing prints them by name**: a console line is `(a held-out task) trial 2: PASS`, and a report states
  them as a total (`held-out (total only)`), never by task, tag or prompt.
- **The directory is refused if it is inside a repository**, and so is a task there that does not say
  `split: held_out`.

## Another project's suite

`--project <root> --suite <dir>` runs a game's own suite (its `.dark_factory/bench/`) against that game's
project. The repository's own suite is the default, and so is this checkout's editor and server binary.

## Where the files go

`~/.local/state/cauldron-bench/<run>/<task>/trial-<n>/` holds the trial's project, its `editor.log`, the
agent's transcript and its `trial.json`. **Never the repository**: a benchmark is not the project it
measures, and its raw output must not appear in the project's `git status`.

## What it refuses before any trial runs

An editor or server binary that is not there, an `--agent` this build does not have, a `--variant` the suite
does not state, a suite with one task that will not load, and — for `--agent omp` — a model this host does
not list (`omp models find`). Each of those would otherwise become a run of harness errors that says
nothing about the editor.
