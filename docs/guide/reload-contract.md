# The reload contract

This page lists every kind of file the editor writes and says what a running program does when one of them changes on disk. It is the audit that the hot-reload series (issues #198 to #204) builds on.

The list lives in code, in `bevy_grimoire_core::reload::KINDS`, and the table below is rendered from it and from the editor's operations. A test fails when the table is out of date.

## How to read the table

- **kind** is the id an operation names in `Operation::writes`.
- **path** is relative to `<grimoire>` (the project's directory, `assets/grimoire`, the one holding `kits.ron`) or to `<root>` (the game's root, the directory holding `assets/`). `<kit>` is a kit's directory under `<grimoire>`: the authoring kit's library, policy, terrain, water, volume, forest, people, scripts and trees live there. `<name>` and `<kit>` each stand for one word of letters, digits and underscores, and for a per-map kind `<name>` is the map's name. `<file>` is one file name of any characters.
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
| `data_registry` | `<grimoire>/<kit>/data.ron` | DataFiles | Live | global | by hand | The registry of a game's own tuning files, read at each `data_set`; nothing holds it. |
| `rigs` | `<grimoire>/rigs.ron` | Rigs | Restart | global | the animation tab | Read when the project opens and cached; nothing watches it (the guide says so). |
| `oplog` | `<grimoire>/oplog.jsonl` | JSON lines | Live | global | the dispatcher appends a line for every operation | Appended and read on request by the log tools; nothing holds it. |
| `reference_images` | `<root>/references/<file>` | image bytes | Live | global | the editor, when a person drops an image on a map | Copied into the project, where `reference_set` checks that the file exists; nothing watches it and nothing holds it. |
| `map` | `<grimoire>/maps/<name>.map.ron` | Map | Respawn | per map | `map_create`, `map_set_settings`, `layout_accept`, `fill`, `map_save`, `placement_add`, `placement_set_pin`, `placement_set_yaw`, `placement_set_tip`, `placement_set_pose`, `placement_set_lift`, `placement_remove`, `stamp_remove`, `stamp_add`, `relations_solve`, `repetition_vary`, `map_rename`, `placement_set_family`, `composition_rename`, `run_add`, `run_set`, `run_remove`, `placements_to_run`, `note_undo_work`; also the project chooser (create, settings, rename) | The editor adopts it into the open map, or keeps what it had when it does not parse; the game respawns the placements that changed. |
| `notes` | `<grimoire>/maps/<name>.notes.ron` | Notes | Live | per map | `note_add`, `note_edit`, `note_remove`, `note_send`, `note_claim`, `note_renew`, `note_release`, `note_ask`, `note_answer`, `note_resolve`, `note_keep`, `note_undo_work` | The editor rereads it when its modification time moves; the game has no reader. |
| `plan` | `<grimoire>/maps/<name>.plan.ron` | Plan | Live | per map | `plan_set` | Read on demand by `plan_set` and the plan view. |
| `references` | `<grimoire>/maps/<name>.references.ron` | References | Live | per map | `reference_set` | Read on demand by `reference_set` and the notes. |
| `relations` | `<grimoire>/maps/<name>.relations.ron` | Relations | Live | per map | by hand | Read on demand by `relations_solve`. |
| `terrain` | `<grimoire>/<kit>/terrain/<name>.ron` | Heightfield | Respawn | per map | `terrain_stroke`, `terrain_import`; also `terrain_preset` (only in a build with the elements feature) | The editor redraws the ground when the file's modification time moves. A file that does not parse is refused with its field named, and what is drawn stays; with nothing drawn yet, no ground is drawn. |
| `water_plan` | `<grimoire>/<kit>/water/<name>.ron` | WaterPlan | Live | per map | `water_place` | The plan `water_run` reads; nothing draws it directly. |
| `water_field` | `<grimoire>/<kit>/water/<name>.field.ron` | WaterField | Respawn | per map | `water_run` | The editor redraws the water surface when the file's modification time moves. A refused file keeps the water drawn before it. |
| `volume` | `<grimoire>/<kit>/volume/<name>.ron` | Volume | Respawn | per map | `volume_edit` | The editor rebuilds the carved ground when the file's modification time moves. A refused file keeps the ground drawn before it. |
| `forest` | `<grimoire>/<kit>/forest/<name>.ron` | Forest | Respawn | per map | `forest_scatter` | The editor respawns the trees when the file's modification time moves. |
| `people` | `<grimoire>/<kit>/people/<name>.ron` | People | Live | per map | `person_set` | Read at each `people_run`; nothing holds it. |
| `scripts` | `<grimoire>/<kit>/scripts/<name>.ron` | Scripts | Live | per map | `trigger_set` | The map's triggers, read when they are run; nothing holds it. |
| `trees` | `<grimoire>/<kit>/trees/<name>.tree.ron` | TreeParams | Respawn | global | `tree_edit` | A species a forest uses: the editor regrows every tree of it when the file's modification time moves. |
| `family` | `<root>/family/<name>.ron` | a family crate's settings | Live | global | `family_set` | One file per family crate: the editor applies it in place, polled every 0.1 s. The game-side `GrimoireLivePlugin` does not watch `<root>/family/`. |
| `family_list` | `<root>/family.ron` | Family | Live | global | by hand | Which family crates the project uses, read when the editor needs the list; nothing holds it. |
| `agent` | `<root>/agent.ron` | AgentSettings | Live | global | by hand | The note agent's model and prompt, read each time a note is sent; nothing holds it. |
| `play` | `<root>/play.ron` | PlayConfig | Live | global | by hand | How the editor launches the game, read each time `play` runs; nothing holds it. |
| `destructible_marks` | `<root>/destructible.ron` | Marks | Live | global | `destructible_set` (only in a build with the carnage feature) | Read at each `destructible_hit`; nothing holds it. |
| `destructible_bake` | `<root>/destructible/<name>.bake.ron` | a baked fracture | Live | global | `destructible_set` and `destructible_hit` (only in a build with the carnage feature) | A cache the next hit rebuilds when it is absent. |
| `game_data` | (any file registered in `data.ron`) | the game's own RON | Restart | global | `data_set` | Written by `data_set` into files the game owns. This repository ships no reader for them, so a running game is restarted unless it re-reads them itself. |
<!-- reload-contract:end -->

## Two ratchets keep the table honest

- `every_written_kind_has_a_reload_class` (`crates/bevy_grimoire_mapper/tests/reload_contract.rs`). Every operation declares the kinds it writes, and `Operation::writes` has no default, so a new operation cannot skip it. The test fails when an operation names a kind the list does not have. A second test runs an operation and checks that every file it changed is a kind it declares.
- `the_restart_list_only_shrinks` (`crates/bevy_grimoire_core/tests/reload_contract.rs`). Every `Restart` kind is on a checked-in list. A kind joins `Restart` only by editing that list and raising its ceiling, so the debt can be paid down and never grows unseen. Today it is `rigs` and `game_data`.

## A request for a kind that was never loaded

`reload_for(kind, loaded)` is the one policy. A change to a kind the program never loaded is the outcome `Reload::NotLoaded`: it is reported and nothing else happens. It is never an implicit load. A load in the middle of play is a hitch, and a program that did not ask for the file has no state to reload.

## A refusal names its file and its field (#200)

A reload validates before it applies. When a file is refused the previous state stays, and the answer says which file, which field and why. `bevy_grimoire_core::reload_check::check(kind, file, text)` parses a file as its kind with the type's own reader, the one a load uses, so the two cannot disagree. A refusal is a `Refusal { kind, file, field, reason }`: `field` is a path such as `version` or `placements[3].at[1]`, found by reading the file again with `serde_path_to_error` only after it was refused, and it is empty for a file that is not RON or that the type's own validation refused after it parsed (the reason says why). `check` covers 18 of the 33 kinds. The other 15 are on a checked-in list in `crates/bevy_grimoire_core/tests/reload_verdicts.rs` that may shrink and never grow unseen.

The game writes one `Verdict` to `.live/game.ron` for each edit it decides on. Version 2 of the file adds `seq`, which counts the game's decisions so an edit is one number however many frames pass, and `kinds`, one `KindVerdict { kind, file, outcome, field, reason }` for every file the load reads: `kits.ron`, each bound kit's `library.ron` and `project.ron`, `vocab.ron`, `rules.ron` and `compositions.ron` when present, and each map. `outcome` is `applied` or `refused`. The game still reloads the project as one unit, so one refused file keeps the whole previous project, and every other file's entry says `applied` because it is fine by itself. A refusal that is about how files fit together (a map that places a descriptor no library has) goes to the file whose path starts the error, and to a `project` entry when none does.

`grimoire_state.game` carries `seq` and `kinds`, and the editor's journal says `game refused the reload: maps/town.map.ron (placements[3].at[1]): map: 1:47: Expected float`. The tests are `crates/bevy_grimoire_core/tests/reload_verdicts.rs` (three bad files per kind), `crates/bevy_grimoire/tests/live_verdicts.rs` (the game keeps its entities and world, writes one verdict per edit) and `crates/bevy_grimoire_mapper/tests/the_games_per_file_verdicts_reach_grimoire_state.rs`.

The editor decides its own files the same way. The terrain view reads the open map's terrain, water field and carved volume, and keeps the last good parsed value of each in memory: a file that reads replaces its value, a refused one leaves it, so a refused edit leaves the entities drawn before it untouched and a bad file is never read again to redraw (when the origin moves, the drawing is rebuilt from the held values). A map that opens with a file refused has no earlier value and draws without it, and a file that is deleted (an undo) is no longer drawn or refused. Each edit is one decision, kept in `grimoire_state.reloads` (`{ok, seq, kinds}`, the same entries as `game.kinds`) and said once on the status line with the file and field, because the view only reads again when a file's modification time moves. `check` now also reads `water_plan`, `water_field`, `volume` and `data_registry`, with the type's own validation: water that does not fill its shape, an edit with no size, a registry from another version or naming an id twice are refused and say why. The test is `crates/bevy_grimoire_mapper/tests/a_refused_terrain_file_keeps_the_last_good_drawing.rs`.

Still to do in #200: a check for the 11 kinds `check` cannot read, the same per-file verdict from the editor's other watchers (notes, forest and tree views, family settings), the `needs_restart` outcome, and the rule that refuses a settings struct which adds a field with no default.

## What the audit found

These are the facts the later issues in the series start from.

- **The game's watcher sees every `.ron` under its directory** (`GrimoireLivePlugin`, the extension set `{ron}`) and runs the whole project load. It diffs placements, the library and masks. A change to a kind it does not read (notes, plans, references, forests) fires a reload that changes nothing, and a broken file of such a kind does not turn the verdict false, because nothing parses it. The class column names what each kind costs, not what the watcher notices.
- **The editor watches by name** (`watch.rs`): `kits.ron`, `vocab.ron`, `compositions.ron`, `rules.ron`, `standards.ron`, `repetition.ron`, each kit's `library.ron` and `project.ron`, and the open map. Terrain, water, volume, forest and tree files are polled by modification time in the views that draw them, and family settings by the family host every 0.1 s. Notes are polled by modification time every frame.
- **A file that does not read keeps the last good state in the project watcher, the game and the terrain view** (the editor says "kept what it had"). The forest and tree views, notes and family settings do not decide their files yet; making that uniform is the rest of #200.
- **Every write of a project file is atomic since #199.** The audit found `family`, `game_data`, `people`, `forest`, `scripts`, `terrain`, `trees`, `volume`, `water_plan`, `water_field`, `destructible_marks`, `destructible_bake`, `reference_images`, and `kits` and `vocab` at project creation written in place, so a reader could see half a file. All of them now go through `bevy_grimoire_core::atomic_file`: a uniquely named hidden temp file (`.<name>.<pid>-<n>.tmp`, never a `.ron` and skipped by the watcher) beside the target, flushed, renamed over it, then the directory flushed. A reader thread parses a map while it is replaced ten thousand times without the flushes and three hundred times with them, a writer process is killed at forty random points, four threads write one path, and `tests/atomic_writes_only.rs` fails on a new plain `fs::write`, `File::create`, `fs::copy` or `OpenOptions` in the non-test source of `bevy_grimoire_core` and `bevy_grimoire_mapper` (a write through another API is not seen by it). What is promised is no torn read and no half-written file after the writer dies; durability against power loss is not, because a test cannot show it. Caches (thumbnails, the animation bench cache) use `write_cache`, the same rename without the flushes.
- **`game_data` is a `Restart` kind** because `data_set` writes files the game owns and this repository ships no reader for them. Whether a game re-reads them is the game's. #202 moves behaviour into data and scripts so the list shrinks.

## What is not a kind

These are written but are not project data, so they have no class. None is read back as project data.

- `<grimoire>/.live/game.ron`, the game's verdict on its latest reload. The game writes it, and the watcher skips every name that starts with a dot so it cannot trigger the reload it reports.
- `slop/llm/vocab_proposals.ron`, a VLM's label proposals, `target/grimoire_thumbs/`, `target/anim_bench_cache.ron` and `target/loop/captures/snips/`: caches and captures.
