//! **A settings edit made while a capture sequence runs reaches the terminal's text size** (#327).
//!
//! Found recording the terminal-settings clip: `tools/demo/record.py` saved a new `terminal_text_scale`
//! into the project's `editor.yaml` while `grimoire_capture_sequence` ran, and after the capture the
//! terminal had more columns than before (213, from 150) where the same edit without a capture took the
//! count down (161 to 113). The edit is the same file and the same reader, so a capture must not change
//! what the terminal does with it.

use std::time::Duration;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::sequencer::{CanRender, SequenceParams, Sequencer};
use bevy_cauldron::surface::Surface;
use bevy_cauldron::terminal_pane::TerminalPane;
use bevy_cauldron::terminal_view::CellMetrics;

use crate::fixtures;

/// How many rounds, 50 ms apart, the test waits for an edit to land.
const TRIES: usize = 60;

/// The text size the edit asks for. Bigger text is bigger cells, so fewer columns.
const SCALE: f32 = 1.4;

/// **`SHELL` is process-wide**, so the tests of this file take turns.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn columns(app: &App) -> u16 {
    app.world().resource::<TerminalPane>().cells().0
}

#[test]
fn a_text_size_saved_while_a_capture_runs_makes_the_terminal_narrower_in_cells() {
    let tag = format!("capture_settings_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    let file = root.join("assets/grimoire/editor.yaml");
    std::fs::write(&file, "{version: 1}").unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.insert_non_send(PosixShell(guard));
    app.world_mut().spawn((
        Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() },
        PrimaryWindow,
    ));
    // **An editor that behaves as if it can render**, like `a_capture_sequence_takes_one_frame_at_a_time`.
    app.insert_resource(CanRender(true));
    app.insert_resource(Surface { image: Handle::default() });
    for _ in 0..10 {
        app.update();
    }
    let before = columns(&app);
    assert!(
        app.world().resource::<CellMetrics>().is_measured() && before > 2,
        "the terminal measured its cell and has columns before the capture ({before})"
    );

    let out_dir = std::env::temp_dir().join(format!("grimoire-327-{}", std::process::id()));
    let id = app
        .world_mut()
        .resource_mut::<Sequencer>()
        .start(
            &SequenceParams {
                label: "settings_during_capture".to_owned(),
                out_dir: out_dir.to_string_lossy().into_owned(),
                seconds: 4.0,
                fps: 4,
                operation: None,
            },
            0.0,
        )
        .expect("the sequence starts");
    app.update();
    let running = |app: &App| app.world().resource::<Sequencer>().status(id)["status"] == "running";
    assert!(running(&app), "the capture is running when the file is saved");

    std::fs::write(&file, format!("{{version: 1, terminal_text_scale: {SCALE}}}"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut after = before;
    for _ in 0..TRIES {
        for _ in 0..2 {
            app.update();
        }
        after = columns(&app);
        if after < before {
            break;
        }
        assert!(running(&app), "the capture ended before the edit landed, so the test proved nothing");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        after < before,
        "a {SCALE}x terminal text size saved during a capture left {after} columns, from {before}: \
         bigger text is fewer columns"
    );
    let _ = std::fs::remove_dir_all(&out_dir);
}
