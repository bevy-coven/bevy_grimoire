//! **What a project open spends its time on** — the measurement behind E-003's figures.
//!
//! ```text
//! cargo run -p bevy_cauldron --example open_cost -- <project-root> [map-name]
//! ```
//!
//! The numbers in `Project::reload_kit`'s doc comment (a 27–315 ms open over six runs on the
//! shipped project, 90 descriptors and 2 kits, of which the triangle pass is 22–255 ms) come from
//! this: the open, then each bound kit's layering, then the triangle pass on its own. Kept rather
//! than thrown away because those three figures are the whole argument for the per-kit re-read,
//! and a later pass that doubts them should be able to re-take them in one command.

/// How many editor frames are stepped after the open: far past the first-frame work.
const FRAMES: usize = 20;

fn main() {
    let Some(root) = std::env::args().nth(1) else {
        eprintln!("usage: open_cost <project-root> [map-name]");
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

    // **The editor's own first frames after the open** (#430): what the window is held for once the loading screen has
    // handed over. The headless editor is the plugin graph the binary ships, with no window and no GPU, so a real
    // window is slower, never faster.
    let map = std::env::args().nth(2).unwrap_or_else(|| "untitled_map".to_owned());
    let start = std::time::Instant::now();
    let mut app = match bevy_cauldron::harness::build_headless(&root, &map, None) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };
    println!("editor built (open + plugins): {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
    let frames: Vec<String> = (0..FRAMES)
        .map(|_| {
            let start = std::time::Instant::now();
            app.update();
            format!("{:.0}", start.elapsed().as_secs_f64() * 1000.0)
        })
        .collect();
    println!("editor frames (ms): {}", frames.join(" "));
}
