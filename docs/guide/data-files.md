# Data files

This page lists every file in a bevy_grimoire project. Each table says where the file lives, what it holds, who writes it, whether the editor reloads it when it changes, and which code reads it.

"Live reload" means the editor watches the file and reloads it when its content changes. A game reads the project when it starts, and reloads it when it changes only if it adds `GrimoireLivePlugin` ([your game](your-game.md)). [The reload contract](reload-contract.md) lists every kind of file, with what a change to it costs a running program.

## kits.ron

| | |
|---|---|
| Where | `assets/grimoire/kits.ron` |
| What it holds | The kit folders the project binds (`bind`), each as a namespace and a folder. Named groups of bound kits (`bash`). The lattice (optional, default `face_bands` 1 in the range 1 to 8, `snap_divisor` 3 in the range 2 to 8, `cell_height` 4.0 m). The `authoring` kit (optional). |
| Written by | The editor's kit chooser adds, removes and points bindings and `authoring`. Other fields are edited by hand. |
| Live reload | Yes. |
| Read by | The game's loader (`project_data::load`) and the editor. |
| Format version | 3 |

## vocab.ron

| | |
|---|---|
| Where | `assets/grimoire/vocab.ron` |
| What it holds | The token tables that every piece is described with. Each axis lists its tokens, up to 64 per axis. The axes are `kind`, `effects`, `look` and `surfaces`, and three optional axes: `capabilities` (what a person can do), `edge` (what a cell presents to its neighbour) and `slot` (what may fill a hole in a tile). |
| Written by | Edited by hand. The editor's labelling writes suggested tokens to `slop/llm/vocab_proposals.ron`, which nothing loads. |
| Live reload | Yes. A change reloads the whole file or none of it. |
| Read by | The game's loader and the editor. Every token a piece uses must be in this file. |
| Format version | None. |

## library.ron

| | |
|---|---|
| Where | Each bound kit's folder, such as `assets/grimoire/<kit>/library.ron` |
| What it holds | The pieces a kit defines, one entry per piece. Each entry carries the piece's id, mesh, footprint, extent, alignment and tokens. |
| Written by | The editor. Committing a piece in the kit door writes it, and measuring a mesh writes its measured values. |
| Live reload | Yes. |
| Read by | The game's loader and the editor, through the layered library. |
| Format version | 1 |

## project.ron

| | |
|---|---|
| Where | Each bound kit's folder, such as `assets/grimoire/<kit>/project.ron` |
| What it holds | How this project uses the kit. `exclude` lists path prefixes the project does not use. `patches` are changes to a kit's pieces that apply to this project only. `note` is free text. |
| Written by | The editor's kit chooser (patches) and its exclude action. |
| Live reload | Yes. |
| Read by | The game's loader and the editor, through the layered library. |
| Format version | 2 |

## compositions.ron

| | |
|---|---|
| Where | `assets/grimoire/compositions.ron`. Optional: the game treats an absent file as no compositions. |
| What it holds | Named groups of pieces. A map stamps a composition, and the game expands the stamp into placements at load. |
| Written by | The editor, when a composition is saved or renamed. |
| Live reload | Yes. |
| Read by | The game's loader, which checks it against the bound library, and the editor. |
| Format version | 1 |

## rules.ron

| | |
|---|---|
| Where | `assets/grimoire/rules.ron`. Optional: an absent file means no rules. |
| What it holds | Rules the editor checks against the bound library. |
| Written by | The editor's rule operations. |
| Live reload | Yes. |
| Read by | The game's loader, which checks it against the bound library, and the editor. |
| Format version | 1 |

## standards.ron

| | |
|---|---|
| Where | `assets/grimoire/standards.ron`. Optional: an absent file means no standards. |
| What it holds | The project's kit standards: rules on a piece's width, depth or height (`min`, `max`, `multiple_of`), each for the pieces whose `kind` holds a token, or every piece. |
| Written by | Hand. |
| Live reload | Yes. The Meshes pane's `standards` row names the rules the selected piece breaks, and every pair of modules that do not nest where a rule covering that piece is one of the two; `grimoire_state.standards` lists every breach, every rule that covers no piece, and in `unnested` every pair of rules whose `multiple_of` modules do not nest (neither a whole multiple of the other, so a 2 m wall run and a 1.5 m fence run share a grid line only every 6 m; width and depth compare with each other, height with height), with where they meet again; `piece_import` reports a new piece's breaches. A breach is reported, not refused. |
| Read by | The editor. |
| Format version | 1 |

## repetition.ron

| | |
|---|---|
| Where | `assets/grimoire/repetition.ron`. Optional: an absent file means the default rules (6 m, structure kinds `building-module`, `fence`, `tile`, `landmark`, `grave`, `door`, weight 0.1, no round kinds). |
| What it holds | What counts as a visible repeat and what it costs: `radius_m`, the `kind` tokens that make a piece `structure` (meant to repeat), `structure_weight` against clutter's 1.0, and `round`, the `kind` tokens of pieces that look the same at any yaw (a barrel, a pot): their copies repeat whatever their yaw, and `repetition_vary` never turns one but counts it as left. A field left out keeps its default. |
| Written by | Hand. |
| Live reload | Yes. `grimoire_state.repetition` reads it on every call and names a bad file in its `error`; the status bar's `REPEATS` line re-reads it when it changes. The `repetition_vary` operation (or the Terrain field line `repeats vary`) reads it on every run: it turns each clutter copy (never a round one) that looks like an earlier one in range by a half or quarter turn until none does, never turns structure, keeps the first copy's yaw, and one undo puts every turned row back. A piece whose floor footprint is longer one way than the other (a bench, a cart) gets the half turn only, since a quarter turn would swap its width and depth onto cells it never covered; a copy the half turn cannot end is counted as left. A bad file is refused, never replaced by the defaults. |
| Read by | The editor. |

## rigs.ron

| | |
|---|---|
| Where | `assets/grimoire/rigs.ron` |
| What it holds | The animation rigs: which clips a character plays and the timing values measured for them. |
| Written by | The editor's animation tab, when a person adopts measured values. The write replaces the whole file. |
| Live reload | No. The editor does not watch this file. |
| Read by | The editor's animation tab. |
| Format version | 3 |

## maps/<name>.map.ron

| | |
|---|---|
| Where | `assets/grimoire/maps/<name>.map.ron`, one file per map |
| What it holds | One map: its name, its origin and bounds, its placements, its stamps, its locations and its `enclosure` wish. The file keeps each stamp as a stamp. The game expands stamps when it loads. |
| Written by | The editor, when a map is saved and when a new empty map is made. The save **checks** the map by expanding its stamps — the same `Map::expanded` the game uses, so the gate and the loader agree — and then writes the map **unexpanded**, so each stamp stays a stamp. |
| Live reload | Yes, for the map the editor has open. |
| Read by | The game's loader, which refuses a map whose `name` does not match its file, and the editor. |
| Format version | 7 |

### What a save leaves out

A placement is written without the fields that hold their defaults: `on`, `owned`, `owned_because`, `patch` and `note` are omitted when they are `None` or `false`. An absent field deserializes to exactly that value, so nothing is lost and the file says only what is true. On the town map this is 226,923 bytes down to 134,320 — **41 %** — and a second save is byte-identical, so a file no longer grows each time it is written.

A stated value is still written: `owned: true`, a `note`, a `patch`, a `paint` layer and `on:` all appear when they mean something. Only the map's own header `note` is written even when absent — it is one line per file, and having it visible is why the field exists.

### `enclosure`

How much of a generated region should close into rooms, and what that wish is worth:

```ron
enclosure: (wish: 0.25, weight: 200),
```

`wish` is a fraction, `0.0` to `1.0`, and only the **composition** source uses it — the learned and declared sources derive no interfaces, so they have no walls to close. The wish is *soft*: a region that cannot reach the target returns its best arrangement and says so, rather than refusing. `weight` is what the wish is worth against the other terms of the solve; any positive value makes it decisive, because it is the only soft constraint in play.

**Absent means those two numbers**, which is what the editor shipped before the field existed (E-005), so a map written earlier generates exactly as it did. Set it through `map_set_settings` (`enclosure_wish`, `enclosure_weight`) or by editing the file; both write the same field, and neither needs a rebuild.

## forest/<map>.ron

| | |
|---|---|
| Where | `assets/grimoire/forest/<map>.ron`, one file per map, beside its terrain. Needs the editor built with `--features bevy_flora`. |
| What it holds | The map's trees: each one's `id`, `species` (a `bevy_flora` preset name such as `oak`, or a `.tree.ron` path under `assets/grimoire/` such as `trees/tall.tree.ron`), its trunk's `at: (x, z)`, the `ground_m` it stands on, `yaw` (radians), `scale` and pool `variant`; a tree placed by hand also has `age_years` and `seed`. |
| Written by | `forest_scatter`: a `plant` stroke (`bevy_flora`'s own Scatter, kept only within `radius_m` of the stroke, on the terrain) or one `place`d tree; the Terrain field's `forest <x> <z> <radius_m> <spacing_m> <seed> <species>` line sends the same operation. Each is one undo step. |
| Live reload | Yes. The viewport draws every tree with `bevy_flora`'s plugin and redraws within half a second of a change to this file or to a `.tree.ron` it names: a scattered tree is a `PooledTree` of its species' pool, built in the background, and a placed tree is a `ProceduralTree` grown from its species with its own seed and age, the tree the game grows. With `bevy_elements` too, the trees wear the map's date. |
| Read by | The editor. A game that lists `bevy_flora` reads it to spawn the same trees through the crate's pools. |
| Format version | None. |

## trees/<name>.tree.ron

| | |
|---|---|
| Where | `assets/grimoire/trees/<name>.tree.ron` |
| What it holds | One tree species: `bevy_flora`'s own `TreeParams`, in the format its asset loader reads. |
| Written by | `tree_edit` (`preset` starts it from a crate preset, `set` changes fields by dotted path, an unknown field is refused), or the Terrain field's `tree <name> [<preset>] [<field.path>=<value> ...]` line. One undo step per edit. |
| Live reload | Yes. Every drawn tree of this species regrows within half a second of an edit. |
| Read by | The editor and the game. |
| Format version | `bevy_flora`'s. |

## family/bevy_elements.ron

| | |
|---|---|
| Where | `<project root>/family/bevy_elements.ron`, beside `family.ron`, which must list `bevy_elements`. Optional: an absent file means the editor's own light and plain ground. |
| What it holds | A map's environment in `bevy_elements`' own serde types: `clock`, `location`, `climate`, `forced_weather`, and the crate's `snow`, `wetness`, `rocks` and `dust` settings. `volume_cover` is the crate's `SurfaceCover` (a `snow` and a `wetness` band: `amount`, `max_slope_deg`, `slope_soft_deg`, `min_height_m`, `max_height_m`, `height_soft_m`) that the carved ground wears whatever the weather is; `None`, the default, lets the weather decide. Every field but `clock` and `climate` may be left out and takes the crate's default. |
| Written by | `family_set` (the ENVIRONMENT buttons, the FAMILY SETTINGS field, an agent), one undo per change, or by hand. |
| Live reload | Yes, with `--features bevy_elements`. The preview's clock, climate, weather and ground cover follow the file. With `isomesh` too (`--features volume_snow` has both), the carved ground's chunks are drawn with the crate's `WeatheredMaterial` and wear `volume_cover` in the next frame, with no remesh. A file that does not parse is named on the status line and the previous values stay. |
| Read by | The editor; a game reads it with the same crate types. |

## oplog.jsonl

| | |
|---|---|
| Where | `assets/grimoire/oplog.jsonl` |
| What it holds | The editor's history of changes, one JSON object per line. It is only appended to. |
| Written by | The editor, one line per operation. |
| Live reload | No. The editor does not watch this file. |
| Read by | The MCP server's oplog read tools. The game does not read it. |
| Format version | None. |

## Snip images

| | |
|---|---|
| Where | `target/loop/captures/snips/`, under the project root. Each snip is a `snip-<ms>.png` with a `.json` beside it. |
| What it holds | A cut-out of the editor's window, written by the editor and the MCP server. |
| Written by | The editor and `bevy_grimoire_snip`. Each file is created new, so an existing file is never replaced. |
| Live reload | No. |
| Read by | Nothing in the project. The game does not read it. |
| Format version | None. |

## recent.ron

Not a project file: one per user, so a project folder never carries where an author opened it from.

| | |
|---|---|
| Where | `<data_dir>/Grimoire/recent.ron`: `~/.local/share` on Linux, `~/Library/Application Support` on macOS, `%APPDATA%` on Windows. |
| What it holds | The projects the editor has opened, most recent first: each one's `path`, whether it is `pinned`, and `last_opened` (seconds since the Unix epoch). A folder that has gone stays listed and is marked missing; `Recent::locate` points the entry at where it went. |
| Written by | The editor, each time a project is opened by the menu, the command line or an agent's `map_open`. Each write goes through a temporary file, then a rename. |
| Live reload | No. |
| Read by | The editor only. |
| Format version | 1. A file of another version is refused by name and left as it is, and the project still opens. |
