//! **bevy_cauldron** — a standalone world-building editor.
//!
//! It opens a *project directory*, not a game: `grimoire/vocab.yaml` says what tokens exist,
//! `grimoire/library.yaml` says what can be placed, meshes resolve under the same root, and the map it
//! writes is `bevy_grimoire_core::map::Map` — which any engine can read, because the schema crate has no
//! engine in it. `crates/bevy_grimoire_core/tests/engine_free.rs` fails the build if that stops being true.
//!
//! ```text
//! bevy_cauldron [project-dir] [map-name] [--kit <name>]
//! ```
//!
//! The second argument is a **name**, not a filename: `bevy_cauldron . site_67` opens (or starts)
//! `assets/grimoire/site_67.map.yaml`. Names are snake_case and are *forced* into it rather than
//! checked, so there is no path through this program on which a map is called something the
//! filesystem and the schema disagree about. Both arguments default, so a bare
//! `cargo run -p bevy_cauldron` from the workspace root opens the shipped library.
//!
//! # Why it is a separate binary and not a mode of the game
//!
//! `sim_harness.rs` is the precedent: the same plugin graph, a different entry point, *"not a second
//! code path"*. An editor welded into the game inherits the game's title screen, its save system, its
//! camera rules and its notion of what a level is — which is exactly how the F7 Site editor ended up
//! only able to edit one hub. The reusable part is the schema and the solvers, and those live in
//! `bevy_grimoire_core` where a second game can have them.
//!
//! # Before writing any Bevy here, read the vendored 0.19 source
//!
//! Not bevy.org, which tracks `main`. `CLAUDE.md` lists the traps this project has already paid for;
//! the two that bite an editor hardest are that **a `Single<.., With<Camera3d>>` silently skips its
//! system** when a second 3D camera exists, and that **every run condition is evaluated** — no
//! short-circuit — so a bare `Res<T>` in a `.run_if` panics whenever that resource is absent.

//! # A library as well as a binary
//!
//! **So the editor can be tested without a screen.** Every system here reads and writes resources,
//! which is exactly what an `App` in a test can drive — but a bin-only crate has nothing an
//! integration test can link against, so the only way to check a system was wired at all was to run
//! the editor and look at it. That meant taking over the machine's keyboard and display to answer
//! questions like "is this resource registered", which is both slow and rude.
//!
//! The game crate made the same split for the same reason (`TESTING.md`). `main.rs` keeps only
//! argument parsing and the app; everything else lives here, where `tests/` can reach it.

pub mod activity_bar;
pub mod agent_guide;
pub mod anim_cache;
pub mod anim_plots;
pub mod anim_stage;
pub mod anim_tab;
pub mod anim_watch;
pub mod args;
#[cfg(feature = "asset_spike")]
pub mod asset_spike;
pub mod badges;
pub mod build;
pub mod build_view;
pub mod catalog;
pub mod checks;
pub mod chrome;
pub mod command_palette;
pub mod crosslinks;
pub mod nav;
pub mod goto;
pub mod command_run;
pub mod commands;
pub mod compass;
pub mod compose;
pub mod compose_labels;
pub mod confirm;
#[cfg(all(feature = "bevy_orca", feature = "bevy_stigmergy"))]
pub mod crowd_preview;
pub mod decide_labels;
pub mod devshot;
pub mod editor;
pub mod file_outline;
pub mod files;
pub mod files_lang;
pub mod files_panel;
pub mod files_search;
pub mod ops_files;
pub mod inspector_view;
pub mod familiar;
pub mod outliner;
pub mod library_browser;
pub mod game_ui_preview;
pub mod game_view;
pub mod agent_view;
// **A hand-back the person kept becomes a benchmark task draft** (BM-07, #417).
pub mod bench_draft;
pub mod shell_view;
pub mod app_verbs;
pub mod menu_bar;
pub mod empty_state;
pub mod family_host;
#[cfg(feature = "bevy_elements")]
pub mod family_panel;
pub mod log_filter;
pub mod log_file;
pub mod startup_probe;
#[cfg(feature = "bevy_elements")]
pub mod environment;
pub mod fill;
pub mod follow;
pub mod reveal;
pub mod filter;
pub mod exit_watchdog;
#[cfg(feature = "debugger")]
pub mod guided;
pub mod guide_template;
#[cfg(feature = "debugger")]
pub mod tour;
pub mod harness;
pub mod help;
pub mod icons;
pub mod keys;
pub mod launcher;
pub mod legacy;
pub mod label_booth;
pub mod label_suggestion;
pub mod labels;
pub mod new_project;
pub mod add_meshes;
pub mod picker;
pub mod picking;
pub mod notice;
pub mod jobs;
pub mod live;
pub mod loading;
pub mod present;
pub mod style_proposals;
pub mod style_followed;
pub mod ops;
pub mod ops_view;
#[cfg(feature = "debugger")]
pub mod ops_brp;
pub mod ops_compose;
pub mod ops_kit;
pub mod ops_piece;
pub mod ops_stamp;
pub mod ops_plan;
pub mod ops_relations;
pub mod ops_repetition;
#[cfg(all(feature = "bevy_orca", feature = "bevy_stigmergy"))]
pub mod ops_crowd;
#[cfg(feature = "bevy_carnage")]
pub mod ops_destructible;
pub mod ops_data;
pub mod ops_label_vlm;
pub mod ops_labeller;
pub mod ops_tile_page;
#[cfg(feature = "bevy_speech_bubbles")]
pub mod ops_dialogue;
#[cfg(feature = "bevy_siren")]
pub mod audio_panel;
pub mod balloon;
pub mod ops_family;
#[cfg(feature = "bevy_light_grid")]
pub mod light_overlay;
#[cfg(feature = "bevy_light_grid")]
pub mod ops_light;
#[cfg(feature = "bevy_lights")]
pub mod ops_pool;
#[cfg(feature = "bevy_lights")]
pub mod light_view;
#[cfg(feature = "bevy_lights")]
pub mod fixture;
pub mod palette_menu;
pub mod piece_menu;
pub mod theme_ink;
pub mod ui;
pub mod ui_budget;
pub mod ui_skin;
pub mod ops_terrain;
pub mod ops_volume;
#[cfg(feature = "isomesh")]
pub mod volume_mesh;
pub mod map_files;
pub mod ops_forest;
#[cfg(all(feature = "bevy_flora", feature = "bevy_elements"))]
pub mod forest_season;
#[cfg(feature = "bevy_flora")]
pub mod forest_view;
pub mod ops_tree;
pub mod ops_water;
pub mod terrain_view;
pub mod notes_view;
pub mod checks_view;
pub mod problems_view;
pub mod oplog_view;
pub mod data_view;
pub mod reference_view;
pub mod reload_files;
pub mod report;
pub mod ops_play;
pub mod ops_notes;
pub mod ops_board;
pub mod ops_design;
pub mod design_areas;
pub mod design_templates;
pub mod design_view;
pub mod ops_people;
pub mod ops_scripts;
pub mod ops_runs;
pub mod project;
pub mod project_assets;
pub mod project_page;
pub mod recent;
pub mod trusted;
pub mod screen;
pub mod selection;
pub mod keymap_screen;
pub mod project_config;
pub mod setting_catalogue;
pub mod settings;
pub mod settings_screen;
pub mod sequencer;
pub mod session;
pub mod snip;
pub mod stage_panels;
pub mod after_edit;
pub mod steps;
pub mod stages;
pub mod surface;
// **A shell in the editor's own window** (`Ctrl+\``): a PTY, a VTE parser and a compact dock.
// `terminal/` is the engine (no Bevy in `pty.rs`/`grid.rs` beyond a `Color`), `terminal_pane.rs` is the
// dock and `terminal_view.rs` is what draws it and swallows its keys; `terminal_agent.rs` is the agent a
// tab starts and its self test, and `agent_resume.rs` is the record of which session a project had running.
// See `terminal/mod.rs`.
pub mod agent_resume;
pub mod terminal;
pub mod terminal_agent;
pub mod terminal_pane;
pub mod terminal_view;
pub mod thumbs;
pub mod tiles;
pub mod toolbar;
pub mod token_prompt;
pub mod update;
pub mod view;
pub mod workspace;
pub mod watch;
pub mod window_frame;
pub mod wish_view;
pub mod workspace_panels;
pub mod world_lists;
