//! **Import every `.glb` under a directory into a library file, without a window.**
//!
//! The editor's Tiles tab imports one mesh at a time and scans the whole asset root; a game with a
//! flat directory of a hundred first-party pieces (Sanctuary's `assets/models/kit`) wants them all
//! in one step, then labelled in the editor. This is the same `import::scan` the editor calls, and
//! each candidate's proposal is appended exactly as the editor's accept key would append it: the
//! measurement and nothing else. Tags are a judgement and stay empty, for the labeller.
//!
//! Meshes the library already holds are skipped (that is what `scan` does), a mesh that cannot be
//! measured is reported and left out, and the library is validated before it is written.
//!
//! A piece too big for the editor's lattice is imported like any other: it can be placed and drawn,
//! and is only refused edge tokens (`Project::divisions_of` in the mapper).
//!
//! Run: `cargo run -p bevy_grimoire_core --example import_dir -- <asset-root> <dir> <library.ron>`
//! where `<dir>` is a directory under `<asset-root>`, for example
//! `-- /path/to/game/assets models/kit /path/to/game/assets/grimoire/kit/library.ron`.

use std::path::PathBuf;

use bevy_grimoire_core::import::{self, Severity};
use bevy_grimoire_core::library::Library;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (asset_root, dir, library_path) = match args.as_slice() {
        [a, d, l] => (a, d, l),
        _ => {
            return Err("usage: import_dir <asset-root> <dir under asset-root> <library.ron>".to_owned());
        }
    };
    let asset_root = PathBuf::from(asset_root);
    let dir = asset_root.join(dir);
    let library_path = PathBuf::from(library_path);

    let text = std::fs::read_to_string(&library_path)
        .map_err(|e| format!("{}: {e}", library_path.display()))?;
    let mut library = Library::parse(&text).map_err(|e| format!("{}: {e}", library_path.display()))?;

    let candidates = import::scan(&asset_root, &dir, &library)?;
    let mut added = 0usize;
    let mut warned = 0usize;
    for c in candidates {
        if c.blocked() {
            let why: Vec<&str> = c.findings.iter().map(|f| f.message.as_str()).collect();
            eprintln!("skipped {}: {}", c.mesh, why.join("; "));
            continue;
        }
        if c.worst() == Some(Severity::Warn) {
            warned += 1;
        }
        if library.get(&c.proposed.id).is_some() {
            eprintln!("skipped {}: id `{}` is already in the library", c.mesh, c.proposed.id);
            continue;
        }
        library.descriptors.push(c.proposed);
        added += 1;
    }

    library.validate().map_err(|e| format!("not written, the result is invalid: {e}"))?;
    let out = library.to_yaml()?;
    bevy_grimoire_core::ron_surgery::save_atomic(&library_path, &out)?;
    println!(
        "imported {added} mesh(es) from {} into {} ({warned} with warnings), {} descriptor(s) in all",
        dir.display(),
        library_path.display(),
        library.descriptors.len()
    );
    Ok(())
}
