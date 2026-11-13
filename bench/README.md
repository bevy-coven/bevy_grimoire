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

## The three agents

| `--agent` | What it does | What it must score |
|---|---|---|
| `omp` (default) | a real model harness, `omp -p --mode json --model <model>` | whatever it scores — that is the measurement |
| `oracle` | calls the task's own `solution` through the real MCP server | **passes every task** |
| `nothing` | connects, reads `tools/list`, and stops | **passes none** |

`oracle` and `nothing` need no model, and they are what proves the pipe: the editor started, the server
answered, the config worked, the changes landed in the project, and the grader read them. A suite where
the oracle fails a task, or `nothing` passes one, is a harness problem rather than a result.

## Outcomes

`pass`, `fail`, and **`harness_error`** — and a harness error is never scored as either of the other two.
An editor that did not start, a port that did not open, a transcript that could not be read, a `--hide-tools`
name this build has no tool for: those are facts about the harness, with the reason in the record, and the
run exits non-zero so a script notices.

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
