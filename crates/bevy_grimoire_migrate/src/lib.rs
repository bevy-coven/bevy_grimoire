//! **Moving a project's files from RON to YAML, losslessly** (GD-03, #365).
//!
//! A project used to be RON. This crate is what turns one into YAML: [`migrate`] converts a folder,
//! [`report`]-shaped [`Report`] says what it did, and [`leftover`] is the refusal an editor's open raises
//! when a project has not been converted.
//!
//! # Why this is a crate of its own
//!
//! **It is the only thing in the workspace that still reads RON.** The cutover's acceptance is that
//! `ron` is not a dependency of `bevy_grimoire_core` — no project file is read in the old spelling any
//! more — while a person's old project still has to be convertible. One crate that reads RON, called by
//! the editor's `migrate` subcommand and by nothing else, is the smallest arrangement that satisfies both.
//!
//! # What "lossless" means here, and why the check is typed
//!
//! Each file is read with the type's own RON reader, written with the one YAML writer, its comments
//! placed in that text, and the commented text read back with that same type's YAML reader. A read that
//! fails, or a value that writes differently than it read, stops the whole migration, names the file,
//! and **writes nothing at all**.
//!
//! Comparing the two texts would report a difference for every file holding an `Option` or an enum: RON
//! spells `Some("x")` as a constructor and an enum variant as `Near(to: "desk@1")`, where YAML has a plain
//! scalar and a tag. The value, read by the type, is the only thing that can be compared.

#![deny(missing_docs)]

pub mod migrate;
pub mod ron_tree;
pub mod user_files;

pub use migrate::{ARCHIVE_DIR, COMMAND, Converted, GRIMOIRE_DIR, RON_EXT, Report, Unplaced, YAML_EXT, leftover, migrate as convert_project, migrate_as, convert_value, plan, read_ron, reads_as_ron, written_as_yaml, yaml_path};

pub use user_files::{USER_ARCHIVE_DIR, upgrade_user_files};
