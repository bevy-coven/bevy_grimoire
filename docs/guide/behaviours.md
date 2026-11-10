# Behaviours

A behaviour is a component `bevy_grimoire` ships, so a game gets one by adding `BehaviourPlugin` and a person
or an agent configures it in a YAML file. Nothing is scripted: a behaviour whose fields can describe it lives
here, and anything else is a script (GD-06).

```yaml
# <project>/game/room.entities.yaml
entities:
  latch:    { Switch: { latch: north, on: false } }
  gate:     { Door: { link: north, open: false } }
  tripwire: { Trigger: { radius_m: 1.5, signal: gate_crossed } }
  bell:     { Countdown: { duration_s: 2.0, signal: dinner } }
  hero:     { Body: {}, Collector: {}, Transform: { translation: [0.0, 0.0, 0.0] } }
```

The file is read by the same reader a game's own entities are read by
([your game](your-game.md)), so the rules are the same ones: a type the game did not register is refused by
name, a key a type does not have is refused at its line, and a refusal changes nothing in the running game.

## The first set

|Behaviour|What it does|Writes|
|---|---|---|
|`Trigger`|A sphere of `radius_m` that says when a `Body` enters and when it leaves|`Triggered`|
|`Switch`|A named latch a `Signal` turns on and off|—|
|`Door`|Opens with the `Switch` whose `latch` is its `link`|—|
|`Spawner`|Asks for `of` every `interval_s`, at `at_m`|`SpawnRequested`|
|`Pickup`|Something a `Collector` that reaches it takes once, then it is gone|`PickedUp`|
|`Health`|`current` of `max`, clamped to that range whenever it is written|—|
|`Damage`|Takes `amount` off `Health` on the next frame, then goes|`Damaged`, `Died`|
|`Countdown`|Counts `duration_s`, then says `signal` once|`CountdownElapsed`|
|`PathFollow`|Walks `waypoints_m` at `speed_m_s`, stopping at the last|`WaypointReached`|
|`StateMachine`|`initial` and `state`, stepped along `transitions` by signals|`Transitioned`|
|`Body`|Marks an entity a `Trigger` notices|—|
|`Collector`|Marks an entity that takes a `Pickup`|—|

`Signal { name }` is the one message every named thing listens for, so a game's input writes one and every part
of a room that names it answers: a switch whose `latch` is that name flips, and a machine with a transition
`on` that name from its current state steps.

## What a behaviour may and may not do

- **It never spawns.** `spawn_descriptor` is this project's only spawner, so a `Spawner` writes a message and
  the game answers it with that one function. A behaviour that spawned would be the second path, and two
  spawners is how a piece comes to look one way in the editor and another in play.
- **Its fields name their unit** (`radius_m`, `duration_s`, `speed_m_s`), so a bounds check and a schema both
  address a number by a name that says what it is.
- **Its state is in the file**: `Switch.on`, `StateMachine.state`, `Countdown.elapsed_s`, `PathFollow.next`. A
  save can set where a behaviour is, so it is resumed rather than reset.
- **It acts in `PostUpdate`, after transform propagation**, so a body moved this frame is where it is this
  frame rather than where it was last one.
- **It needs a clock only if it counts**: a behaviour with a timer takes an optional `Time`, so a game without
  one ticks nothing rather than panicking.

## The name `Timer` was taken

A countdown is `Countdown`. `bevy_time::Timer` already owns the short type path `Timer`, and the reader refuses
an ambiguous short path — so a component called `Timer` here could never be written in a file by that name,
which is the whole point of it.

The same collision is why **the behaviours do not register themselves**. Bevy 0.20 auto-registers every
derived type process-wide, so a shipped `Health` would make a game's own `Health` ambiguous in that game's
files and the reader would refuse both. `BehaviourPlugin` is the opt-in: a game that does not add it has none
of these types in its registry, and a game that does add it and also has its own `Health` writes
`bevy_grimoire::behaviours::health::Health` for this one.
