//! **bevy_grimoire_mapper** — a standalone world-building editor.
//!
//! It opens a *project directory*, not a game: `grimoire/vocab.ron` says what tokens exist,
//! `grimoire/library.ron` says what can be placed, meshes resolve under the same root, and the map it
//! writes is `bevy_grimoire_core::map::Map` — which any engine can read, because the schema crate has no
//! engine in it. `crates/bevy_grimoire_core/tests/engine_free.rs` fails the build if that stops being true.
//!
//! ```text
//! bevy_grimoire_mapper [project-dir] [map-name] [--kit <name>]
//! ```
//!
//! The second argument is a **name**, not a filename: `bevy_grimoire_mapper . site_67` opens (or starts)
//! `assets/grimoire/site_67.map.ron`. Names are snake_case and are *forced* into it rather than
//! checked, so there is no path through this program on which a map is called something the
//! filesystem and the schema disagree about. Both arguments default, so a bare
//! `cargo run -p bevy_grimoire_mapper` from the workspace root opens the shipped library.
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
pub mod chooser;
pub mod checks;
pub mod chrome;
pub mod command_palette;
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
pub mod family_host;
#[cfg(feature = "bevy_elements")]
pub mod environment;
pub mod fill;
pub mod filter;
#[cfg(feature = "debugger")]
pub mod guided;
pub mod harness;
pub mod keys;
pub mod label_booth;
pub mod labels;
pub mod notice;
pub mod live;
pub mod present;
pub mod ops;
#[cfg(feature = "debugger")]
pub mod ops_brp;
pub mod ops_compose;
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
#[cfg(feature = "bevy_speech_bubbles")]
pub mod ops_dialogue;
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
pub mod palette_menu;
pub mod piece_menu;
pub mod ui;
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
pub mod oplog_view;
pub mod data_view;
pub mod reference_view;
pub mod reload_files;
pub mod ops_play;
pub mod ops_notes;
pub mod ops_references;
pub mod ops_people;
pub mod ops_scripts;
pub mod ops_runs;
pub mod project;
pub mod recent;
pub mod screen;
pub mod settings;
pub mod sequencer;
pub mod snip;
pub mod after_edit;
pub mod steps;
pub mod stages;
pub mod surface;
pub mod thumbs;
pub mod tiles;
pub mod token_prompt;
pub mod view;
pub mod vlm;
pub mod watch;
