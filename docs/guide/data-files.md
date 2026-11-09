# Data files

This page lists every file in a bevy_emerge project. Each table says where the file lives, what it holds, who writes it, whether the editor reloads it when it changes, and which code reads it.

"Live reload" means the editor watches the file and reloads it when its content changes. The game never reloads a file. It reads the project once, when it starts.

## kits.ron

| | |
|---|---|
| Where | `assets/emerge/kits.ron` |
| What it holds | The kit folders the project binds (`bind`), each as a namespace and a folder. Named groups of bound kits (`bash`). The lattice (optional, default `face_bands` 1 in the range 1 to 8, `snap_divisor` 3 in the range 2 to 8, `cell_height` 4.0 m). The `authoring` kit (optional). |
| Written by | The editor's kit chooser adds, removes and points bindings and `authoring`. Other fields are edited by hand. |
| Live reload | Yes. |
| Read by | The game's loader (`project_data::load`) and the editor. |
| Format version | 3 |

## vocab.ron

| | |
|---|---|
| Where | `assets/emerge/vocab.ron` |
| What it holds | The token tables that every piece is described with. Each axis lists its tokens, up to 64 per axis. The axes are `kind`, `effects`, `look` and `surfaces`, and three optional axes: `capabilities` (what a person can do), `edge` (what a cell presents to its neighbour) and `slot` (what may fill a hole in a tile). |
| Written by | Edited by hand. The editor's labelling writes suggested tokens to `slop/llm/vocab_proposals.ron`, which nothing loads. |
| Live reload | Yes. A change reloads the whole file or none of it. |
| Read by | The game's loader and the editor. Every token a piece uses must be in this file. |
| Format version | None. |

## library.ron

| | |
|---|---|
| Where | Each bound kit's folder, such as `assets/emerge/<kit>/library.ron` |
| What it holds | The pieces a kit defines, one entry per piece. Each entry carries the piece's id, mesh, footprint, extent, alignment and tokens. |
| Written by | The editor. Committing a piece in the kit door writes it, and measuring a mesh writes its measured values. |
| Live reload | Yes. |
| Read by | The game's loader and the editor, through the layered library. |
| Format version | 1 |

## project.ron

| | |
|---|---|
| Where | Each bound kit's folder, such as `assets/emerge/<kit>/project.ron` |
| What it holds | How this project uses the kit. `exclude` lists path prefixes the project does not use. `patches` are changes to a kit's pieces that apply to this project only. `note` is free text. |
| Written by | The editor's kit chooser (patches) and its exclude action. |
| Live reload | Yes. |
| Read by | The game's loader and the editor, through the layered library. |
| Format version | 2 |

## compositions.ron

| | |
|---|---|
| Where | `assets/emerge/compositions.ron`. Optional: the game treats an absent file as no compositions. |
| What it holds | Named groups of pieces. A map stamps a composition, and the game expands the stamp into placements at load. |
| Written by | The editor, when a composition is saved or renamed. |
| Live reload | Yes. |
| Read by | The game's loader, which checks it against the bound library, and the editor. |
| Format version | 1 |

## rules.ron

| | |
|---|---|
| Where | `assets/emerge/rules.ron`. Optional: an absent file means no rules. |
| What it holds | Rules the editor checks against the bound library. |
| Written by | The editor's rule operations. |
| Live reload | Yes. |
| Read by | The game's loader, which checks it against the bound library, and the editor. |
| Format version | 1 |

## rigs.ron

| | |
|---|---|
| Where | `assets/emerge/rigs.ron` |
| What it holds | The animation rigs: which clips a character plays and the timing values measured for them. |
| Written by | The editor's animation tab, when a person adopts measured values. The write replaces the whole file. |
| Live reload | No. The editor does not watch this file. |
| Read by | The editor's animation tab. |
| Format version | 3 |

## maps/<name>.map.ron

| | |
|---|---|
| Where | `assets/emerge/maps/<name>.map.ron`, one file per map |
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

## oplog.jsonl

| | |
|---|---|
| Where | `assets/emerge/oplog.jsonl` |
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
| Written by | The editor and `bevy_emerge_snip`. Each file is created new, so an existing file is never replaced. |
| Live reload | No. |
| Read by | Nothing in the project. The game does not read it. |
| Format version | None. |
