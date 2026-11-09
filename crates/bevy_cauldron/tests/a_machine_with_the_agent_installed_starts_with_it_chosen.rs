//! **A machine that has the agent installed starts with it chosen** (#341).
//!
//! `EditorSettings::default().terminal_agent` is `Off`, and it stays `Off`: the built-in default is a
//! pure value, and `the_terminal_agent_is_a_setting` pins it. What a person meets on their own machine
//! is the *resolved* settings, and the resolution asks this machine whether the agent is installed: on
//! a machine with `omp` on its `PATH` the terminal opens on the empty pane with an `Invoke Agent`
//! button, and on a machine without one it opens a plain shell exactly as it always has.
//!
//! Nothing starts by itself either way — a session costs tokens to warm up — so the only thing the
//! resolution decides is which of the two empty panes a person is shown.
//!
//! **A file still wins.** The resolution fills the built-in default, so a `terminal_agent` in the
//! user's or the project's settings is never overridden by what happens to be installed.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::settings::{self, AGENT_PROGRAM, EditorSettings, MachineAgent, SettingsLayer, TerminalAgent};

use crate::fixtures;

/// A scratch folder, unique to this test and this process.
fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// A file under `dir` named `name`, made executable when `runnable`.
fn program(dir: &std::path::Path, name: &str, runnable: bool) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, "#!/bin/sh\n").unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if runnable { 0o755 } else { 0o644 };
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap_or_else(|e| panic!("{e}"));
    }
    // **No mode to set off Unix**, so the flag is read and means nothing there — the editor's terminal
    // is Unix-only, and this test is about the machine's `PATH`.
    #[cfg(not(unix))]
    let _ = runnable;
    path
}

/// **The machine's answer, off the environment**: a `PATH` holding the agent chooses it, and a `PATH`
/// without it — or no `PATH` at all — leaves the terminal a shell.
#[test]
fn a_path_holding_the_agent_chooses_it_and_a_path_without_it_does_not() {
    let with = scratch("machine_with_agent");
    program(&with, AGENT_PROGRAM, true);
    let path_with = with.display().to_string();
    assert_eq!(
        settings::agent_in_path(Some(path_with.as_str())),
        TerminalAgent::OhMyPi,
        "a machine with {AGENT_PROGRAM} on its PATH starts with the agent chosen"
    );

    // The agent is installed, but not where a person can run it: a file without the executable bit is
    // not a program, and a machine with one has no agent.
    let still = scratch("machine_with_a_dead_agent");
    program(&still, AGENT_PROGRAM, false);
    let path_still = still.display().to_string();
    assert_eq!(
        settings::agent_in_path(Some(path_still.as_str())),
        TerminalAgent::Off,
        "a file that cannot be run is not an installed agent"
    );

    // No folder holds it: the terminal opens a shell, as it did before this resolved anything.
    let without = scratch("machine_without_agent");
    let path_without = without.display().to_string();
    assert_eq!(
        settings::agent_in_path(Some(path_without.as_str())),
        TerminalAgent::Off,
        "a machine without {AGENT_PROGRAM} keeps the shell"
    );
    assert_eq!(
        settings::agent_in_path(None),
        TerminalAgent::Off,
        "no PATH at all names no agent"
    );

    // **The whole list is searched**, not just its first folder: a person's `omp` lives in a
    // `.local/bin` further along, and a machine that stopped at the first entry would say it had none.
    // The list's own separator, which is `:` on a mac and a Linux box and `;` on Windows — not the
    // path separator, which is what a folder's parts are joined by.
    let list_separator = if cfg!(windows) { ';' } else { ':' };
    let path_two = format!("{}{list_separator}{path_with}", without.display());
    assert_eq!(
        settings::agent_in_path(Some(path_two.as_str())),
        TerminalAgent::OhMyPi,
        "the agent is found in the second folder of the PATH"
    );
}

/// **The built-in default is untouched** — the resolution, not the default, is where the machine is
/// asked — so the terminal of a machine with no settings file and no agent is the shell it was.
#[test]
fn the_built_in_default_is_still_the_shell() {
    assert_eq!(
        EditorSettings::default().terminal_agent,
        TerminalAgent::Off,
        "the default is a pure value; the machine's answer is the resolution's"
    );
}

/// **Resolving on a machine with the agent chooses it**, and one without does not.
#[test]
fn resolving_on_this_machine_uses_what_is_installed_unless_a_file_says() {
    let agent = EditorSettings::resolve_on_this_machine(None, None, TerminalAgent::OhMyPi);
    assert_eq!(
        agent.terminal_agent,
        TerminalAgent::OhMyPi,
        "a machine with the agent installed resolves to it, so the pane offers Invoke Agent"
    );
    let shell = EditorSettings::resolve_on_this_machine(None, None, TerminalAgent::Off);
    assert_eq!(shell.terminal_agent, TerminalAgent::Off, "a machine without it resolves to the shell");

    // **A file wins, both ways**: someone on a machine with `omp` may still want a plain shell, and
    // someone whose agent is not on the PATH may name one they have.
    let off = SettingsLayer::read("editor.ron", "(version: 1, terminal_agent: Some(Off))")
        .unwrap_or_else(|e| panic!("{}", e.reason));
    assert_eq!(
        EditorSettings::resolve_on_this_machine(None, Some(&off), TerminalAgent::OhMyPi).terminal_agent,
        TerminalAgent::Off,
        "a file that names Off beats the machine's installed agent"
    );
    let pi = SettingsLayer::read("editor.ron", "(version: 1, terminal_agent: Some(OhMyPi))")
        .unwrap_or_else(|e| panic!("{}", e.reason));
    assert_eq!(
        EditorSettings::resolve_on_this_machine(None, Some(&pi), TerminalAgent::Off).terminal_agent,
        TerminalAgent::OhMyPi,
        "a file that names the agent beats a machine that has none"
    );

    // **The project's layer is the later one**, exactly as every other key resolves.
    let project = SettingsLayer::read("editor.ron", "(version: 1, terminal_agent: Some(Off))")
        .unwrap_or_else(|e| panic!("{}", e.reason));
    assert_eq!(
        EditorSettings::resolve_on_this_machine(Some(&pi), Some(&project), TerminalAgent::Off).terminal_agent,
        TerminalAgent::Off,
        "the project's layer wins over the user's, as a later layer does for every key"
    );
}

/// **The machine's answer is asked once**, and carried as a resource: a `PATH` that gains an agent
/// while the editor runs does not change the editor under the person using it.
#[test]
fn the_machine_is_asked_once_for_the_whole_session() {
    let asked = MachineAgent::ask();
    assert_eq!(asked.0, settings::agent_in_path(std::env::var("PATH").ok().as_deref()), "the resource holds the answer the PATH gives");
    assert_eq!(MachineAgent::ask().0, asked.0, "a second ask answers the same, so a session's agent does not change under it");
}

/// **The editor with a window and its terminal raised**, on a project of its own.
///
/// Not `the_terminal_paints_grounds_and_the_selection::editor_with_terminal`, which holds the
/// process-wide `SHELL_ENV` mutex for as long as its `App` lives: this test opens three editors, and
/// the third lock would wait on the first two. Nothing here types into a shell, so nothing needs the
/// shell to be settled.
///
/// **The machine's answer is given before the editor's first frame**, because that is when the binary
/// inserts it (settings are read and the pane is built on the frames that follow) — a resource added
/// after the pane has opened a shell would be a change too late for a setting whose decision was already
/// made.
fn editor_with_its_terminal_raised(tag: &str, machine: Option<MachineAgent>) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // **Before the first frame**, which is what the binary does: settings are read and the pane is
    // built on the frames that follow, and a resource added after the pane has opened a shell would be
    // a change too late for a decision that was already made.
    if let Some(machine) = machine {
        app.insert_resource(machine);
    }
    // A shell this test does run is `/bin/sh`, which every Unix has; the one an agent tab would start
    // is never started here.
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let window = app
        .world_mut()
        .spawn((
            Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() },
            PrimaryWindow,
        ))
        .id();
    for _ in 0..10 {
        app.update();
    }
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in [KeyCode::ControlLeft, KeyCode::Backquote] {
            app.world_mut().write_message(KeyboardInput {
                key_code: key,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window,
            });
        }
        app.update();
    }
    app
}

/// **The pane is what a person meets**, so the editor is opened three times and the terminal's pane is
/// read: with the machine answered, the empty pane is the one that offers the agent; without it, a shell.
///
/// Nothing starts on either — an agent session costs tokens — so what is asserted is which of the two
/// empty states the person is shown.
#[test]
fn the_pane_offers_the_agent_on_a_machine_that_has_it_and_a_shell_on_one_that_does_not() {
    use bevy_cauldron::settings::MachineAgent;
    use bevy_cauldron::terminal::Terminal;
    use bevy_cauldron::terminal_pane::TerminalInvokeButton;

    let offers = |tag: &str, machine: Option<MachineAgent>| -> bool {
        let mut app = editor_with_its_terminal_raised(tag, machine);
        for _ in 0..40 {
            app.update();
        }
        let inviting = app.world_mut().query_filtered::<Entity, With<TerminalInvokeButton>>().iter(app.world()).next().is_some();
        let shell_open = !app.world().resource::<Terminal>().is_empty();
        assert!(
            inviting != shell_open,
            "the pane is one of the two empty states, never both and never neither: Invoke Agent {inviting}, a shell {shell_open}"
        );
        inviting
    };

    assert!(
        offers("machine_has_the_agent", Some(MachineAgent(TerminalAgent::OhMyPi))),
        "a machine that has the agent opens on the pane that offers it"
    );
    assert!(
        !offers("machine_has_no_agent", Some(MachineAgent(TerminalAgent::Off))),
        "a machine without it opens a shell, as it did before this resolved anything"
    );
    assert!(
        !offers("machine_never_asked", None),
        "an app that never asked — every test editor — opens a shell"
    );
}
