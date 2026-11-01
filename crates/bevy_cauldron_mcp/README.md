# bevy_cauldron_mcp

**The [bevy_grimoire](https://github.com/bevy-coven/bevy_grimoire) world editor, over the Model Context
Protocol.**

An MCP server that lets an agent drive the editor through **typed operations** — the same
operations a person at the keyboard uses — instead of synthesising keystrokes.

## What it is

The server speaks MCP over **stdio** and reaches the **running editor** over the Bevy Remote
Protocol. The editor is a separate process and owns the world; the server asks it questions and
tells it what to do, and **it never writes to the project directory itself** — the editor owns
that.

## Running it

```sh
# The editor, with its BRP transport up (it is on by default).
cargo run -p bevy_cauldron -- <project>

# The server. BEVY_BRP_PORT is the same variable the editor reads.
cargo run -p bevy_cauldron_mcp -- --project <project>
```

`BEVY_BRP_PORT` defaults to **15702** on both sides, so one variable points both ends at the same
socket. Running the editor and the server against different projects is the one mistake that
produces a confusing answer, and `--project` says which project the server *thinks* it is on.

## The tools

| tool | what it does |
|---|---|
| `grimoire_list_operations` | every operation the editor knows, with its parameters' schema; `query` searches them, best first |
| `grimoire_preview_operation` | what an operation would change; nothing changes |
| `grimoire_apply_operation` | run one, and record it in the operation log |
| `grimoire_read_oplog` | the log, newest last |

**Four tools, and not one per operation**, because the always-loaded toolset is a prompt an agent
pays for on every turn. The apply tool dispatches by name, and the list tool says what the names
are.

The reference is generated: `cargo run -q -p bevy_cauldron_mcp -- --emit-docs > docs/mcp-tools.md`.

## Licence

MIT OR Apache-2.0.
