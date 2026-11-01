//! **What a project open spends its time on** — the measurement behind E-003's figures.
//!
//! ```text
//! cargo run -p bevy_cauldron --example open_cost -- <project-root>
//! ```
//!
//! The numbers in `Project::reload_kit`'s doc comment (a 27–315 ms open over six runs on the
//! shipped project, 90 descriptors and 2 kits, of which the triangle pass is 22–255 ms) come from
//! this: the open, then each bound kit's layering, then the triangle pass on its own. Kept rather
//! than thrown away because those three figures are the whole argument for the per-kit re-read,
//! and a later pass that doubts them should be able to re-take them in one command.

fn main() {
    let Some(root) = std::env::args().nth(1) else {
        eprintln!("usage: open_cost <project-root>");
        return;
    };
    let root = std::path::PathBuf::from(root);
    for round in 0..3 {
        let start = std::time::Instant::now();
        let project = match bevy_cauldron::project::Project::open(&root, None) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{e}");
                return;
            }
        };
        let open = start.elapsed();
        println!(
            "round {round}: open {:.1} ms ({} descriptors, {} kits)",
            open.as_secs_f64() * 1000.0,
            project.library.descriptors.len(),
            project.kits.len()
        );
        for kit in project.kits.iter() {
            let start = std::time::Instant::now();
            let laid = bevy_grimoire_core::policy::layered_library(&kit.dir).unwrap();
            println!(
                "  kit {} : layered {:.1} ms ({} rows)",
                kit.dir.file_name().unwrap().to_string_lossy(),
                start.elapsed().as_secs_f64() * 1000.0,
                laid.library.descriptors.len()
            );
        }
        let start = std::time::Instant::now();
        let mut p = project;
        p.remeasure_triangles();
        println!(
            "  triangles (whole library): {:.1} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}
