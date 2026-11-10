# Scripts

A **script** is behaviour that its fields cannot describe. Everything a component's fields can say lives in
[behaviours](behaviours.md); what is left — a conditional, a count, an arithmetic rule — is a `.rhai` file the game
runs on an entity.

```yaml
# <project>/game/room.entities.yaml
entities:
  door:
    Script: { file: "scripts/door.rhai" }
```

The path is relative to the game's root, so `scripts/door.rhai` is `<root>/scripts/door.rhai`. An entity a script
is put on is a **scripted entity**, and every hook it defines is called with that entity as `this`.

## The four hooks

```rhai
fn on_start()             { this.state.open = false; }        // once, when the script loads and after each save
fn on_update(dt)          { this.state.frames += 1; }         // every frame, with this frame's dt in seconds
fn on_event(name, data)   { if name == "pressed" { … } }      // every event this script may see
fn on_trigger(other)      { print(other); }                   // another entity came near this one
```

A script defines the ones it needs; a hook it leaves out simply never runs. **`on_start` runs first and once**, so
a script whose frame code assumes what its setup did is safe.

## What a script can touch

- **Its own entity's components, by name**: `this.Health.current -= 1`. `this` is built from the components the game
  registered that this entity actually carries, so the set is exactly what a YAML file could set — the same set
  `game.schema.json` documents, which is what an editor completes.
- **Its kept state**: `this.state`. A map this crate owns per entity. **It survives a reload**, so a script that
  counts frames keeps its count when you save a new version of it.
- **Another entity, by its `Name`**: `find("guard")` answers that entity's components as a value, and
  `world_get("guard", "Health")` answers one field. `world_set("guard", "Door", #{ open: true })` writes one back.
  **Two calls rather than one handle**, because a write through a map a function returned lands on a copy — measured
  against Rhai 1.27 — so `find("g").Health.current -= 1` would silently change nothing. `world_set` is one recorded
  write that lands on the entity.

## Events and timers

`emit("pressed")` sends an event every scripted entity may see; `emit("pressed", 3)` carries data with it. A game's
own code writes the same message (`ScriptEvent`) to address one script by putting its `Name` in `to`, and an empty
`to` goes to every script.

**`import` resolves nothing**: a script cannot load a file the game did not put in its project.

`timer("slow", 2.5)` fires, once, `2.5` seconds later, as an event named `timer` with `slow` as its data,
**addressed to the script that started it**: two doors running one script do not close each other. A timer needs its
entity to have a `Name`, because that is how the event is addressed; one without gets a problem saying so.

`on_trigger(other)` runs from an event named `trigger`, with the other entity's name as its data. A game's own code
writes it (or a `Trigger`'s crossing does, from the game side), so a script never has to know how the trigger volume
reports itself.

## Saving a script changes the running game

Saving the `.rhai` file recompiles it in the running game. **`this.state` carries across**, so the code changes and
the state stays.

- **A file that does not compile leaves the last good version running** and raises a problem naming the file and the
  line it is wrong at. A half-typed edit is not a reason to stop a door opening.
- **A call that runs past `SCRIPT_STEP_LIMIT` operations, or builds a string, array or map past its cap, is
  stopped**, with a problem naming the file. A runaway script costs one call, never the frame loop, and one
  fault is reported once per edit rather than once a frame.
- **A write at a component the entity does not carry, or at an immutable one, is refused** with a problem naming
  the script — the engine's own `apply` panics in both cases, so both are checked first.
- **A runtime error becomes a problem naming the file and the line** of the statement that raised it.

Every one of those is a `ScriptProblem`, which carries the same `Problem` shape a refused project file does, so a
game reports it in the same log.

## One host

There is one Rhai engine per game, not one per entity: an `Engine` carries the registered function set, and that set
*is* the script API. A game that needs a function of its own takes `ScriptEngine::engine_mut` and registers it there,
rather than building a second host.
