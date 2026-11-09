//! **A machine that has the agent installed starts with it chosen** (#341).
//!
//! `EditorSettings::default().terminal_agent` is `Off`, and it stays `Off`: the built-in default is a
//! pure value, and `the_terminal_agent_is_a_setting` pins it. What a person meets on their own machine
//! is the *resolved* settings, and the resolution asks this machine whether the agent is installed: on
//! a machine where a login shell finds `omp` the terminal opens on the empty pane with an `Invoke
//! Agent` button, and on a machine where it does not, a plain shell exactly as it always has.
//!
//! **The question is put to the login shell, not to the editor's own `PATH`.** A macOS app opened from
//! Finder or the Dock inherits launchd's environment — `/usr/bin:/bin:/usr/sbin:/sbin` — so the owner's
//! `~/.local/bin/omp` is on no `PATH` the editor was given. It is on the login shell's, and the login
//! shell is what the `Invoke Agent` press types the command into (`terminal_agent.rs`), so the machine
//! is asked there: one answer, the same one in both places.
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
use bevy_gooey::ChoiceValue;
use bevy_gooey::testing::find_by_label;

use bevy_cauldron::setting_catalogue;
use bevy_cauldron::settings::{self, AGENT_PROGRAM, EditorSettings, MachineAgent, SettingsLayer, TerminalAgent, UserSettingsFile};
use bevy_cauldron::settings_screen::OpenSettings;

use crate::fixtures;

/// **`$SHELL` is one process's environment**, so the two tests that change it take turns: the settings
/// screen's editor sets it to `/bin/sh`, and the login-shell test points it at a stub.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

/// **A login shell that answers what a real one would**: the `command -v` line a stub is given, and
/// what it says. `sleep_s` makes it take that long first, so a patient wait and one that hangs are
/// both a stub away.
fn stub_shell(dir: &std::path::Path, tag: &str, command: &str, code: i32, sleep_s: u64) -> std::path::PathBuf {
    let path = dir.join(format!("{tag}.sh"));
    let text = if sleep_s == 0 {
        format!("#!/bin/sh\n{command}\nexit {code}\n")
    } else {
        format!("#!/bin/sh\nsleep {sleep_s}\n{command}\nexit {code}\n")
    };
    std::fs::write(&path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap_or_else(|e| panic!("{e}"));
    }
    path
}

/// **The folder test, off the environment**: a `PATH` holding the agent chooses it, and a `PATH`
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

/// **The machine is asked of its login shell, and answered there** (#341): what a Finder-launched app
/// needs, because the `PATH` it inherits is launchd's, not the person's.
///
/// A stub stands in for the shell, so no Dock and no Finder are needed: the answer is whatever the
/// stub says, and the process's own `PATH` — which on a developer's box *does* hold `omp` — is not
/// what decides. That is what the quiet stub proves: read off this process's `PATH` the machine would
/// answer `OhMyPi` on such a box, and a login shell that finds nothing must say `Off`.
#[test]
fn the_agent_is_asked_of_the_login_shell_and_answered_there() {
    let dir = scratch("machine_login_shell");
    let tool = dir.join("login_bin");
    std::fs::create_dir_all(&tool).unwrap_or_else(|e| panic!("{}: {e}", tool.display()));
    let found = program(&tool, AGENT_PROGRAM, true);
    let answer = format!("printf '%s\\n' \"{}\"", found.display());
    let answering = stub_shell(&dir, "answering", &answer, 0, 0);

    assert_eq!(
        MachineAgent::ask_the_login_shell(&answering.display().to_string(), 5),
        TerminalAgent::OhMyPi,
        "a login shell that finds the agent on its own PATH makes the terminal start with it, though the editor's PATH never named that folder"
    );

    // `command -v` says nothing when it finds nothing, so a stub that answers nothing is a machine
    // without the agent — whatever this process's `PATH` happens to hold.
    let quiet = stub_shell(&dir, "quiet", "exit 1", 1, 0);
    assert_eq!(
        MachineAgent::ask_the_login_shell(&quiet.display().to_string(), 5),
        TerminalAgent::Off,
        "a login shell that does not find the agent leaves the terminal its shell, even on a machine whose own PATH has one"
    );

    // **A shell that cannot say anything is not an answer.** A path printed by a shell that exits
    // non-zero is a shell that failed, so the machine is read as having no agent.
    let failing = stub_shell(&dir, "failing", &answer, 2, 0);
    assert_eq!(
        MachineAgent::ask_the_login_shell(&failing.display().to_string(), 5),
        TerminalAgent::Off,
        "a login shell that failed says nothing about the machine, so the terminal keeps its shell"
    );

    // **Bounded**: a login shell whose rc files hang must not hold the window open.
    let hanging = stub_shell(&dir, "hanging", &answer, 0, 30);
    let started = std::time::Instant::now();
    assert_eq!(
        MachineAgent::ask_the_login_shell(&hanging.display().to_string(), 1),
        TerminalAgent::Off,
        "a login shell that has not answered in its patience is a machine without an agent, not a window that waits"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "the wait is the patience and no longer: a login shell that hangs costs 1 s, not the window"
    );

    // A shell this machine does not have is a machine that answered nothing, never a panic.
    assert_eq!(
        MachineAgent::ask_the_login_shell("/nonexistent/login-shell", 1),
        TerminalAgent::Off,
        "a shell that cannot be started names no agent"
    );
}

/// **`ask()` puts the question to `$SHELL`**, which is the shell the `Invoke Agent` press types into:
/// the same shell, so a machine is not read one way and run another.
#[test]
fn ask_puts_the_question_to_the_shell_the_press_uses() {
    let _guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = scratch("machine_ask_shell");
    let tool = dir.join("login_bin");
    std::fs::create_dir_all(&tool).unwrap_or_else(|e| panic!("{}: {e}", tool.display()));
    let found = program(&tool, AGENT_PROGRAM, true);
    let answering = stub_shell(&dir, "answering", &format!("printf '%s\\n' \"{}\"", found.display()), 0, 0);
    let quiet = stub_shell(&dir, "quiet", "exit 1", 1, 0);
    let was = std::env::var("SHELL").ok();

    let asked = |shell: &std::path::Path| -> TerminalAgent {
        // **`$SHELL` is what the binary has too**: a person's shell, or none.
        unsafe { std::env::set_var("SHELL", shell) };
        MachineAgent::ask().0
    };
    let has_the_agent = asked(&answering);
    let has_none = asked(&quiet);

    match was {
        Some(shell) => unsafe { std::env::set_var("SHELL", shell) },
        None => unsafe { std::env::remove_var("SHELL") },
    }

    assert_eq!(
        has_the_agent,
        TerminalAgent::OhMyPi,
        "`SHELL` is asked the question, so the machine is answered in the shell the press will type into"
    );
    assert_eq!(has_none, TerminalAgent::Off, "and that shell's silence is a machine without the agent");
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

/// **Resolving on a machine where the agent was found chooses it**, and one where it was not does not.
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
    // someone whose agent is not on the login shell's PATH may name one they have.
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

/// **The Settings screen shows the same agent the pane is using** (#341): the row says the value the
/// terminal resolved, so a screen saying *Off* and a pane offering `Invoke Agent` cannot both be true.
///
/// Read the way a person reads it — the row's selected option, which is gooey's `ChoiceValue` on the
/// control the label belongs to.
#[test]
fn the_settings_screen_shows_the_agent_this_machine_starts_with() {
    // **The screen is opened three times in one process**, so it takes its turn with the tests that
    // change `$SHELL`: a pane's shell is started as the editor opens.
    let _guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let shown = |tag: &str, machine: Option<MachineAgent>| -> usize {
        let mut app = screen_with(tag, machine);
        let row = find_by_label(&mut app, "Terminal agent").unwrap_or_else(|e| panic!("the Terminal tab lists the terminal's agent: {e}"));
        let value = app
            .world()
            .get::<ChoiceValue>(row)
            .unwrap_or_else(|| panic!("`Terminal agent` is not a choice: the widget that says it has no ChoiceValue"));
        value.0
    };

    let agent = shown("settings_row_machine_has_the_agent", Some(MachineAgent(TerminalAgent::OhMyPi)));
    let shell = shown("settings_row_machine_has_no_agent", Some(MachineAgent(TerminalAgent::Off)));
    let never_asked = shown("settings_row_machine_never_asked", None);

    assert_eq!(
        agent,
        setting_catalogue::agent_index(TerminalAgent::OhMyPi),
        "with the machine answered OhMyPi and no file stating one, the row shows the agent the terminal is using"
    );
    assert_eq!(
        shell,
        setting_catalogue::agent_index(TerminalAgent::Off),
        "with the machine answered Off and no file stating one, the row shows the shell"
    );
    assert_eq!(
        never_asked,
        setting_catalogue::agent_index(TerminalAgent::Off),
        "an editor that never asked — every test's editor — shows the built-in default, as the terminal resolves it"
    );
}

/// **The Settings screen, opened**, on a project of its own, with `machine` asked before its first
/// frame — which is when the binary inserts the answer.
fn screen_with(tag: &str, machine: Option<MachineAgent>) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    if let Some(machine) = machine {
        app.insert_resource(machine);
    }
    let user = root.join(format!("user_config_{tag}_{}/settings.ron", std::process::id()));
    std::fs::create_dir_all(user.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(user));
    for _ in 0..3 {
        app.update();
    }
    app.world_mut().write_message(OpenSettings);
    for _ in 0..4 {
        app.update();
    }
    app
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
    use bevy_cauldron::terminal::Terminal;

    // **A shell this test does run is `/bin/sh`**, which every Unix has; the one an agent tab would
    // start is never started here. Held for the whole test, because the panes read it as they open.
    let _guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
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
