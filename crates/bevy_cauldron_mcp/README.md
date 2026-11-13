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
socket. With `--project`, the server refuses a change made while the editor has another project
open. **Without it the server follows the editor**: whatever it has open, including none (the
Projects home), and `grimoire_open_project` opens one.

## An installed Cauldron

Every macOS release carries this server beside the editor, signed with it:
`Cauldron.app/Contents/MacOS/bevy_cauldron_mcp`. An agent on another machine attaches over ssh with
no `--project`, so it follows the installed app from its first frame:

```sh
ssh <mac> /Applications/Cauldron.app/Contents/MacOS/bevy_cauldron_mcp
```

`grimoire_read_log` reads the editor's own log (`~/Library/Application Support/Grimoire/cauldron.log`)
from disk, so it answers when the editor does not, and `grimoire_capture_still` with
`"source": "window"` shows what the window itself shows rather than what the editor drew.

## The tools

| tool | what it does |
|---|---|
| `grimoire_guide` | how to work in Cauldron: which tools in what order, and what the person expects — **call it first** |
| `grimoire_list_operations` | every operation the editor knows, with its parameters' schema; `query` searches them, best first |
| `grimoire_preview_operation` | what an operation would change; nothing changes |
| `grimoire_apply_operation` | run one, and record it in the operation log |
| `grimoire_read_oplog` | the log, newest last |
| `grimoire_style` | the style of one place, game to area, behind the person's trust, with the pictures it cites |

**The operation tools are four, and not one per operation**, because the always-loaded toolset is a
prompt an agent pays for on every turn. The apply tool dispatches by name, and the list tool says what
the names are. `grimoire_guide` is the one prose tool: the shipped guide, `src/guide.md`, compiled in —
the same text `docs/mcp-tools.md` is rendered from, so an agent and a person read one guide, and the
server's `instructions` send an agent there before any other tool.

The reference is generated: `cargo run -q -p bevy_cauldron_mcp -- --emit-docs > docs/mcp-tools.md`.

## Licence

MIT OR Apache-2.0.
