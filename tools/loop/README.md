# The loop's measuring tools

Four scripts, one purpose: the loop stopped measuring because the tools lived outside the repo. They
live here now, are Python 3 standard library only, and never write to a person's project — every run
copies an `assets` directory to `/tmp` first and closes the editor it launched by its own pid.

**`common.py`** — what the others share: the repo path, `binary(name)` (a workspace binary looked up
through `cargo metadata`'s target directory, so the shared `~/.cargo-target-shared/global` is found
rather than assumed), `free_port()`, `make_scratch(source, dest)`, `class Editor` (one editor
process, `wait_ready(pred)` polling `emerge/state`, closed by pid), and `class Client` (the demo
recorder's `McpClient` plus `tools/list`). It imports `McpClient` and `brp` from `tools/demo/record.py`
rather than writing a second MCP client.

```
python3 tools/loop/accept.py --source ~/sanctuary/assets --out target/loop/<date>-<sha>/accept.json
python3 -m unittest tools/loop/test_accept.py
```

**`accept.py`** — the loop's headline metric. An outside MCP client is given the brief's own sentence
(*"a mossy ruined courtyard with two exits"*) and nothing else, and ten steps are recorded: tools
published, map made, map opened, pieces placed, layout proposed, still captured, layout kept, map
saved, result presented, result verified. Each step is `pass`, `fail` (with the refusal's own text) or
`skip` (its dependency did not pass — a step nobody could attempt has not been shown to be broken).
The report carries `reached`, `of` and `first_failure`. `--glb <path>` adds an eleventh step that
imports a mesh and places it, for measuring the Blender bridge. Exit 0 only when every step passes.

**`perf.py`** — responsiveness, in the five numbers the reports quote: `frame_ms_median` (from
`bevy_debugger/status`'s frame counter over five 2 s windows), `key_p95_ms` (a keystroke until
`emerge/state` says the armed tool changed), `file_ms` (a `yaw:` rewritten in the map file until the
watched placement's transform carries it), `mcp_ms` (the same value through `emerge_apply_operation`),
and `load_avg_1m` — because every number above is a statement about the machine as much as the code.
Exit 1 if a latency came back `None`: a report with a hole in it gets quoted as if it were a
measurement.

**`backlog.py`** — `check` (unique ids across the working list and the archive; `kind` in
`bug`/`enhancement`/`series`; `status` in the seven the brief lists; a series carrying its goal,
`approved_by`, acceptance steps and steps; exit 1 naming each violation), `next-id B|E|S` (one past
the highest number for that prefix across both files), `archive` (move every `fixed`/`shipped`/
`rejected` item to the end of the archive, in order).

**`test_accept.py`** — `summarize()`'s arithmetic, which is what the loop grades itself on: a skip is
neither a pass nor a failure, `first_failure` is the first failure in step order, a skip ahead of a
failure does not become the first failure, and a skip never satisfies a dependent.
