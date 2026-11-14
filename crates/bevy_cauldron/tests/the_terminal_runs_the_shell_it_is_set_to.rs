//! **A new terminal tab runs the shell the settings name, with the arguments they give** (#324).
//!
//! VS Code's default terminal profile: a program and its `args`. Until now a tab ran `$SHELL`, whatever
//! the person's settings said. `terminal_shell` and `terminal_shell_args` are keys of the editor's
//! settings files; no settings screen row, because it has no text control, as `terminal_agent_command`.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::settings::{EditorSettings, SettingsLayer};
use bevy_cauldron::terminal::Terminal;

use crate::fixtures;

/// How many 50 ms checks the test waits for the shell.
const TRIES: usize = 80;

/// **`SHELL` is process-wide**, so the tests of this file take turns.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn screen_lines(app: &App) -> Vec<String> {
    let Some(tab) = app.world().resource::<Terminal>().tab() else {
        return Vec::new();
    };
    tab.screen()
        .built_lines()
        .iter()
        .map(|line| line.cells.iter().map(|c| c.ch).collect::<String>().trim_end().to_owned())
        .collect()
}

#[test]
fn a_tab_runs_the_shell_the_settings_name_with_its_arguments() {
    let tag = format!("terminal_shell_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    // **A shell that says what it was started as**, then stays: it prints its two arguments.
    let script = root.join("my_shell.sh");
    std::fs::write(&script, "#!/bin/sh\nprintf 'custom shell [%s] [%s]\\n' \"$1\" \"$2\"\nexec sleep 120\n")
        .unwrap_or_else(|e| panic!("{}: {e}", script.display()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap_or_else(|e| panic!("{e}"));
    }
    let file = root.join("assets/grimoire/editor.yaml");
    std::fs::write(
        &file,
        format!("{{version: 1, terminal_shell: {:?}, terminal_shell_args: [\"--login\", \"two words\"]}}", script.display().to_string()),
    )
    .unwrap_or_else(|e| panic!("{}: {e}", file.display()));

    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // The environment names another shell, which the setting must beat.
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.insert_non_send(PosixShell(guard));
    app.world_mut().spawn((
        Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() },
        PrimaryWindow,
    ));
    let want = "custom shell [--login] [two words]";
    for _ in 0..TRIES {
        for _ in 0..2 {
            app.update();
        }
        if screen_lines(&app).iter().any(|line| line == want) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("the tab never ran the shell the settings name and printed {want:?}. The pane shows: {:?}", screen_lines(&app));
}

#[test]
fn a_shell_that_names_nothing_to_run_is_refused_with_its_key() {
    let empty = SettingsLayer::read("editor.yaml", "{version: 1, terminal_shell: \"  \"}")
        .expect_err("a blank shell runs nothing");
    assert_eq!(empty.field.as_deref(), Some("terminal_shell"), "the refusal names the key: {}", empty.reason);
    let orphan = SettingsLayer::read("editor.yaml", "{version: 1, terminal_shell_args: [\"-l\"]}")
        .expect_err("arguments with no shell are for nothing");
    assert_eq!(orphan.field.as_deref(), Some("terminal_shell_args"), "the refusal names the key: {}", orphan.reason);
}

#[test]
fn a_layer_that_names_a_shell_brings_its_own_arguments_and_not_the_one_beneath() {
    let user = SettingsLayer::read("settings.yaml", "{version: 1, terminal_shell: \"/bin/zsh\", terminal_shell_args: [\"-l\"]}")
        .unwrap_or_else(|e| panic!("{}", e.reason));
    let project = SettingsLayer::read("editor.yaml", "{version: 1, terminal_shell: \"/bin/fish\"}")
        .unwrap_or_else(|e| panic!("{}", e.reason));
    let settings = EditorSettings::resolve(Some(&user), Some(&project));
    assert_eq!(settings.terminal_shell.as_deref(), Some("/bin/fish"), "the project's shell beats the person's");
    assert!(settings.terminal_shell_args.is_empty(), "zsh's `-l` is not fish's: {:?}", settings.terminal_shell_args);
    let only_user = EditorSettings::resolve(Some(&user), None);
    assert_eq!(only_user.terminal_shell_args, vec!["-l".to_owned()], "a layer's own arguments are kept");
}
