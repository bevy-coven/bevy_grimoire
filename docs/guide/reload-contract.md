# The reload contract

This page lists every kind of file the editor writes and says what a running program does when one of them changes on disk. It is the audit that the hot-reload series (issues #198 to #204) builds on.

The list lives in code, in `bevy_grimoire_core::reload::KINDS`, and the table below is rendered from it and from the editor's operations. A test fails when the table is out of date.

## How to read the table

- **kind** is the id an operation names in `Operation::writes`.
- **path** is relative to `<grimoire>` (the directory holding `kits.ron`, `assets/grimoire` in a default project; for a kit's `library.ron` and `project.ron`, the kit's own directory) or to `<root>` (the game's root, the directory holding `assets/`). `<name>` stands for one word of letters, digits and underscores, and for a per-map kind it is the map's name.
- **parses to** is the Rust type the file is read as.
- **class** is what a change costs the programs this repository ships (the editor, and the game-side `GrimoireLivePlugin`), the dearest of them when they differ.
- **lifetime** is what the file belongs to.
- **written by** names the operations that write it. A kind no operation writes says who does.

### Classes

| class | meaning |
|---|---|
| `Live` | The new content replaces the old in place, or nothing holds the old content because the file is read fresh each time it is used. |
| `Respawn` | What was built from the file (entities, meshes, a world) is dropped and built again. The rest of the program carries on. |
| `Restart` | A running program that holds the kind cannot take a change to it. It keeps the old content until it is restarted. |

### Lifetimes

| lifetime | meaning |
|---|---|
| `global` | One per project, kit, species or crate. It outlives every map, and a change reaches everything that reads it. |
| `per map` | One per map, named after it. It loads with its map, and a change reaches only that map. |

## The table

<!-- reload-contract:start -->
| kind | path | parses to | class | lifetime | written by | how a change lands |
|---|---|---|---|---|---|---|
| `kits` | `<grimoire>/kits.ron` | Kits | Respawn | global | the project chooser (create, bind, unbind) | The editor reopens the whole project; the game's `GrimoireLivePlugin` reloads it whole. |
| `vocab` | `<grimoire>/vocab.ron` | Vocabularies | Respawn | global | the project chooser (create), the vocabulary editor | The editor re-resolves its libraries in place; the game reloads the project whole. |
| `library` | `<grimoire>/<kit>/library.ron` | Library | Respawn | global | `piece_import`, `piece_remove`, `label_set`, `piece_set_mount`, `label_apply`; also the project chooser (create a kit) | One per kit. The editor rereads the kit; the game rebuilds the maps that place a changed piece. |
| `policy` | `<grimoire>/<kit>/project.ron` | Policy | Respawn | global | `pack_set_excluded`; also the project chooser (create a kit, remove a policy), `policy::remove_patch` | One per kit: its exclusions, patches and mounts. Reloads with the kit's library. |
| `rules` | `<grimoire>/rules.ron` | Rules | Respawn | global | `rules_set`, `map_rename` | The editor reopens the project; the game revalidates it against the library. |
| `compositions` | `<grimoire>/compositions.ron` | Compositions | Respawn | global | `composition_define`, `composition_set_envelope`, `composition_rename` | A stamp expands to rows, so the maps that stamp a changed composition are rebuilt. |
| `standards` | `<grimoire>/standards.ron` | Standards | Live | global | by hand | Read fresh each time a kit is checked; the editor revalidates it when it changes. |
| `repetition` | `<grimoire>/repetition.ron` | RepetitionRules | Live | global | by hand | Read fresh on each `repetition_vary` run; the editor revalidates it when it changes. |
| `checks` | `<grimoire>/checks.ron` | Checks | Live | global | by hand | `Checks::for_project` reads it fresh on every run, so an edit takes effect at once. |
| `data_registry` | `<grimoire>/data.ron` | DataFiles | Live | global | by hand | The registry of a game's own tuning files, read at each `data_set`; nothing holds it. |
| `rigs` | `<grimoire>/rigs.ron` | Rigs | Restart | global | the animation tab | Read when the project opens and cached; nothing watches it (the guide says so). |
| `oplog` | `<grimoire>/oplog.jsonl` | JSON lines | Live | global | the dispatcher appends a line for every operation | Appended and read on request by the log tools; nothing holds it. |
| `reference_images` | `<grimoire>/references/<name>.png` | image bytes | Live | global | the editor, when a person drops an image on a map | Copied into the project and read when a map's reference is drawn. |
| `map` | `<grimoire>/maps/<name>.map.ron` | Map | Respawn | per map | `map_create`, `map_set_settings`, `layout_accept`, `fill`, `map_save`, `placement_add`, `placement_set_pin`, `placement_set_yaw`, `placement_set_tip`, `placement_set_pose`, `placement_set_lift`, `placement_remove`, `stamp_remove`, `stamp_add`, `relations_solve`, `repetition_vary`, `map_rename`, `placement_set_family`, `composition_rename`, `run_add`, `run_set`, `run_remove`, `placements_to_run`, `note_undo_work`; also the project chooser (create, settings, rename) | The editor adopts it into the open map, or keeps what it had when it does not parse; the game respawns the placements that changed. |
| `notes` | `<grimoire>/maps/<name>.notes.ron` | Notes | Live | per map | `note_add`, `note_edit`, `note_remove`, `note_send`, `note_claim`, `note_renew`, `note_release`, `note_ask`, `note_answer`, `note_resolve`, `note_keep`, `note_undo_work` | The editor rereads it when its modification time moves; the game has no reader. |
| `plan` | `<grimoire>/maps/<name>.plan.ron` | Plan | Live | per map | `plan_set` | Read on demand by `plan_set` and the plan view. |
| `references` | `<grimoire>/maps/<name>.references.ron` | References | Live | per map | `reference_set` | Read on demand by `reference_set` and the notes. |
| `relations` | `<grimoire>/maps/<name>.relations.ron` | Relations | Live | per map | by hand | Read on demand by `relations_solve`. |
| `terrain` | `<grimoire>/terrain/<name>.ron` | Heightfield | Respawn | per map | `terrain_stroke`, `terrain_import`; also `terrain_preset` (only in a build with the elements feature) | The editor redraws the ground when the file's modification time moves; a file that does not parse draws no ground. |
| `water_plan` | `<grimoire>/water/<name>.ron` | WaterPlan | Live | per map | `water_place` | The plan `water_run` reads; nothing draws it directly. |
| `water_field` | `<grimoire>/water/<name>.field.ron` | WaterField | Respawn | per map | `water_run` | The editor redraws the water surface when the file's modification time moves. |
| `volume` | `<grimoire>/volume/<name>.ron` | Volume | Respawn | per map | `volume_edit` | The editor rebuilds the carved ground when the file's modification time moves. |
| `forest` | `<grimoire>/forest/<name>.ron` | Forest | Respawn | per map | `forest_scatter` | The editor respawns the trees when the file's modification time moves. |
| `people` | `<grimoire>/people/<name>.ron` | People | Live | per map | `person_set` | Read at each `people_run`; nothing holds it. |
| `scripts` | `<grimoire>/scripts/<name>.ron` | Scripts | Live | per map | `trigger_set` | The map's triggers, read when they are run; nothing holds it. |
| `trees` | `<grimoire>/trees/<name>.tree.ron` | TreeParams | Respawn | global | `tree_edit` | A species a forest uses: the editor regrows every tree of it when the file's modification time moves. |
| `family` | `<root>/family/<name>.ron` | a family crate's settings | Live | global | `family_set` | One file per family crate: the editor and the game apply it in place, polled every 0.1 s. |
| `destructible_marks` | `<root>/destructible.ron` | Marks | Live | global | `destructible_set` (only in a build with the carnage feature) | Read at each `destructible_hit`; nothing holds it. |
| `destructible_bake` | `<root>/destructible/<name>.bake.ron` | a baked fracture | Live | global | `destructible_set` and `destructible_hit` (only in a build with the carnage feature) | A cache the next hit rebuilds when it is absent. |
| `game_data` | (any file registered in `data.ron`) | the game's own RON | Restart | global | `data_set` | Written by `data_set` into files the game owns. This repository ships no reader for them, so a running game is restarted unless it re-reads them itself. |
<!-- reload-contract:end -->

## Two ratchets keep the table honest

- `every_written_kind_has_a_reload_class` (`crates/bevy_grimoire_mapper/tests/reload_contract.rs`). Every operation declares the kinds it writes, and `Operation::writes` has no default, so a new operation cannot skip it. The test fails when an operation names a kind the list does not have. A second test runs an operation and checks that every file it changed is a kind it declares.
- `the_restart_list_only_shrinks` (`crates/bevy_grimoire_core/tests/reload_contract.rs`). Every `Restart` kind is on a checked-in list. A kind joins `Restart` only by editing that list and raising its ceiling, so the debt can be paid down and never grows unseen. Today it is `rigs` and `game_data`.

## A request for a kind that was never loaded

`reload_for(kind, loaded)` is the one policy. A change to a kind the program never loaded is the outcome `Reload::NotLoaded`: it is reported and nothing else happens. It is never an implicit load. A load in the middle of play is a hitch, and a program that did not ask for the file has no state to reload.

## What the audit found

These are the facts the later issues in the series start from.

- **The game's watcher sees every `.ron` under its directory** (`GrimoireLivePlugin`, the extension set `{ron}`) and runs the whole project load. It diffs placements, the library and masks. A change to a kind it does not read (notes, plans, references, forests) fires a reload that changes nothing, and a broken file of such a kind does not turn the verdict false, because nothing parses it. The class column names what each kind costs, not what the watcher notices.
- **The editor watches by name** (`watch.rs`): `kits.ron`, `vocab.ron`, `compositions.ron`, `rules.ron`, `standards.ron`, `repetition.ron`, each kit's `library.ron` and `project.ron`, and the open map. Terrain, water, volume, forest and tree files are polled by modification time in the views that draw them, and family settings by the family host every 0.1 s. Notes are polled by modification time every frame.
- **A file that does not read keeps the last good state in the project watcher and the game** (the editor says "kept what it had"). It does not everywhere: the terrain view despawns its ground and draws none. Making that uniform is #200.
- **Some writes are not atomic.** Maps, the library, compositions, rules, notes, plans and references are written to a temporary file and renamed. `family`, `game_data`, `people`, `forest`, `scripts`, `terrain`, `trees`, `volume`, `water_plan`, `water_field`, `destructible_marks`, `destructible_bake` and `reference_images` are plain writes, and so are `kits` and `vocab` when a project is created, so a reader can see half a file. Making them atomic is #199.
- **`game_data` is a `Restart` kind** because `data_set` writes files the game owns and this repository ships no reader for them. Whether a game re-reads them is the game's. #202 moves behaviour into data and scripts so the list shrinks.

## What is not a kind

These are written but are not project data, so they have no class. None is read back as project data.

- `<grimoire>/.live/game.ron`, the game's verdict on its latest reload. The game writes it, and the watcher skips every name that starts with a dot so it cannot trigger the reload it reports.
- `slop/llm/vocab_proposals.ron`, a VLM's label proposals, `target/grimoire_thumbs/`, `target/anim_bench_cache.ron` and `target/loop/captures/snips/`: caches and captures.
