# bevy_grimoire_rhai

> ⚠️ **Vibe Coded** — written by an AI agent working from a human's direction. It is covered by tests, but it has had no line-by-line human audit. Read it before you trust it.

Rhai scripts on entities, reloaded live: the one Rhai host a `bevy_grimoire` game runs its scripts on.

> **Part of [`bevy-coven/bevy_grimoire`](https://github.com/bevy-coven/bevy_grimoire)** (`crates/bevy_grimoire_rhai/`), MIT OR Apache-2.0, Bevy 0.20. Issues and PRs belong in `bevy_grimoire`.

## What it does

A `Script` is a component naming a `.rhai` file. The file defines up to four hooks, and the plugin calls the ones it finds:

```rhai
// scripts/door.rhai
fn on_start()             { this.state.open = false; }
fn on_update(dt)          { this.state.frames = (this.state.frames ?? 0) + 1; }
fn on_event(name, data)   { if name == "pressed" { this.Door.open = true; } }
fn on_trigger(other)      { print(other); }
```

A script reads and writes its own entity's reflected components by name (`this.Health.current -= 1`), finds another entity by its `Name` (`find("guard")`, `world_set("guard", "Health", …)`), sends and receives events (`emit`, `on_event`), and starts timers (`timer("slow", 2.5)`).

```yaml
# <project>/game/room.entities.yaml
entities:
  door:
    Script: { file: "scripts/door.rhai" }
```

## Saving a script changes the running game

Saving the `.rhai` file recompiles it in place. **The state a script keeps in `this.state` survives the reload**, so a counter keeps counting. A file that does not compile leaves the last good version running and raises a `ScriptProblem` naming the file and the line. A call that runs past `SCRIPT_STEP_LIMIT` operations is stopped with a problem naming the file, so a runaway script costs one call rather than the game.

## Why its own crate

`bevy_society` embeds Rhai, but its engine registers only society types and keeps its entry points `pub(crate)`, so it is not a general host. Rather than build a second one beside it — the two-paths shape the repository's rules forbid — the general host lives here, and `bevy_society` can use it.

## The script API is reflection-driven

A script touches exactly what the game registered. `this` is built from the components the entity actually carries, so the set is the same one GD-04's `game.schema.json` documents. A component the game did not register cannot be named by a script any more than by a file.
