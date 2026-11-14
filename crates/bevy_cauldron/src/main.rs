//! **bevy_cauldron** — the binary. Argument parsing and the app; everything else is the
//! library beside this, so `tests/` can drive the editor without a window. See `lib.rs`.

use bevy_cauldron::{args, harness, project_assets, screen, screen::Screen};

use bevy::prelude::*;
// `DefaultPlugins::disable` is `PluginGroup`'s method, and the prelude does not carry the trait.
use bevy::app::PluginGroup;

/// **One application, two screens.**
///
/// This was three processes: a supervisor that never opened a window, a menu process per lap, and an
/// editor process per door. The shape existed because **winit builds at most one event loop per
/// process** — a second `App::run` dies on `RecreationAttempt` — so a menu you could return to had
/// to be a fresh process, and a supervisor had to spawn it.
///
/// Asked for at the keyboard, 2026-08-16: *"can we not open a whole another editing window? I'd like
/// to keep the same bevy application running across whether it's the UI or the editor."*
///
/// The winit limit was never the real obstacle: one process can show a menu and an editor in one
/// window as long as they are two *states* rather than two `App`s. The real obstacle was that in
/// Bevy 0.19 a **missing `Res<T>` panics its system**, and about a hundred parameter positions take
/// `Res<Project>` — so a menu inside the editor's `App` meant gating every one of them. That is what
/// `screen.rs` now does, and this file is what a binary should be again: parse arguments, build one
/// app, run it.
fn main() -> AppExit {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if args::asks_for_help(&argv) {
        println!("{}", args::USAGE);
        return AppExit::Success;
    }
    // **`--print-operations` prints the registry and exits**, so a benchmark asks the binary it is about to
    // run which operations exist rather than trusting a list baked into the harness (`cauldron-bench`,
    // BM-02). It opens nothing: no window, no project, no device.
    if args::asks_for_operations(&argv) {
        for op in bevy_cauldron::ops::registry() {
            println!("{}", op.name());
        }
        return AppExit::Success;
    }
    // **`migrate` runs and exits** (GD-03, #365): a project's files are moved from RON to YAML, and the
    // editor is never started, so the command works with no window, on a machine with no GPU, over ssh.
    // It is a *subcommand* rather than a flag because it changes a project on disk and never opens one —
    // the same reason this subcommand is what every refusal names (`migrate::COMMAND`).
    if args::subcommand(&argv) == Some("migrate") {
        return migrate_and_exit(&argv, args::PROGRAM);
    }
    // **`bake` runs and exits** (GD-08, #370), for the same reason `migrate` does: it reads a project and writes a folder.
    if args::subcommand(&argv) == Some("bake") {
        return bake_and_exit(&argv, args::PROGRAM);
    }

    // **`bevy_audio` is opened only when something plays a sound.** The editor links
    // `bevy_audio` through `DefaultPlugins`, and an audio device that fails is what loops
    // (`crate::log_filter` has the measurement: 13.5 GB in five minutes). The only thing in this
    // binary that plays anything is `bevy_siren`'s audition, behind the feature of that name, so a
    // build without it — the editor on a headless box, a storyboard capture, CI — asks for no device
    // at all rather than opening one to log its failure.
    #[cfg(not(feature = "bevy_siren"))]
    let plugins = DefaultPlugins.build().disable::<bevy::audio::AudioPlugin>();
    #[cfg(feature = "bevy_siren")]
    let plugins = DefaultPlugins.build();

    let mut app = App::new();
    // **The log goes to a file as well as to stderr** (`bevy_cauldron::log_file`, #306). Measured 2026-11-04 on
    // `mac_air`: a release launched from Finder drew a black window and nothing said why — a bundle launch has no
    // terminal for stdout, and macOS wrote no crash report. Opened *before* `DefaultPlugins` and before anything
    // can panic, so the first line of a run that dies while the window is made still reaches the file.
    let log = bevy_cauldron::log_file::LogFile::open();
    if let Some(log) = &log {
        log.line(&bevy_cauldron::log_file::starting_line(&argv));
        bevy_cauldron::log_file::install_panic_hook(log.clone());
    }
    // **A person's old `.ron` files are converted once, here** (M3 of the review): settings, recent projects and
    // trusted guides would otherwise reset on upgrade. After the log is open, because this is the one sentence a
    // person needs after a Finder launch and a bundle has no terminal for stderr; and carried into the launcher as
    // well (`bevy_cauldron::legacy::StartupNotices`), so it is on screen and not only in a file. A file that cannot
    // be converted is said out loud, never skipped.
    let upgrading = bevy_cauldron::legacy::upgrade_user_files(bevy_grimoire_core::user_dirs::config_dir(), bevy_grimoire_core::user_dirs::data_dir());
    for notice in &upgrading {
        eprintln!("{notice}");
        if let Some(log) = &log {
            log.line(notice);
        }
    }
    // **The project is the asset root, and which project is decided later** (`project_assets`): a project named
    // on the command line is pointed at below, and one chosen on the launcher is pointed at when it is chosen.
    // Registered before `DefaultPlugins`, which builds the asset sources.
    project_assets::install(&mut app, None);
    // In the world before `DefaultPlugins` builds its subscriber, which is where `plugin_layer` looks.
    if let Some(log) = log.clone() {
        app.insert_resource(log);
    }
    app.insert_resource(bevy_cauldron::legacy::StartupNotices(upgrading));
    app.add_plugins(
        plugins
            // **One repeated log line filled a 16 GB RAM disk** (`bevy_cauldron::log_filter`): an
            // editor with no audio device wrote 13.5 GB in five minutes, all of it `rodio::stream`'s
            // ALSA error. `LogPlugin` puts its `level` first and `RUST_LOG` last, and a directive with
            // a longer target beats both, so this silences that one target and nothing else.
            .set(bevy::log::LogPlugin {
                filter: bevy_cauldron::log_filter::LOG_SILENCES.to_owned(),
                // The file layer comes *after* the filter, so both sinks carry the same events: a log that
                // said more or less than the terminal would be a second answer to the same question.
                custom_layer: bevy_cauldron::log_file::plugin_layer,
                ..default()
            })
            // Meshes are named relative to the project, so the project IS the asset root: the default asset
            // source is `project_assets::ProjectAssetReader`, installed above, and `AssetPlugin` keeps no path.
            .set(WindowPlugin {
                primary_window: Some(Window {
                    // **One window for both screens**, so the title is state rather than a build-time
                    // decision — see [`name_the_window`].
                    title: "Cauldron".to_owned(),
                    // **Never smaller than the layout is built for**; where it opens, and where it goes back
                    // to, is `window_frame`'s decision (#292).
                    resize_constraints: bevy::window::WindowResizeConstraints {
                        min_width: bevy_cauldron::window_frame::MIN_SIZE.x,
                        min_height: bevy_cauldron::window_frame::MIN_SIZE.y,
                        ..default()
                    },
                    // **Fullscreen for automated checks.** A virtual pointer is ABSOLUTE over the
                    // whole output, so a screen fraction only means a window fraction when the
                    // window IS the screen.
                    mode: if std::env::var("GRIMOIRE_FULLSCREEN").as_deref() == Ok("1") {
                        bevy::window::WindowMode::BorderlessFullscreen(
                            bevy::window::MonitorSelection::Primary,
                        )
                    } else {
                        bevy::window::WindowMode::Windowed
                    },
                    ..default()
                }),
                ..default()
            }),
    )
    .insert_resource(ClearColor(bevy_cauldron::chrome::ink().void))
    // **One knob for the whole interface.** `UiScale` multiplies every `Val::Px` and every font
    // size, so the panels grow together and nothing has to be re-tuned relative to anything else
    // — the alternative is forty constants that drift apart the first time one is missed.
    .insert_resource(UiScale(bevy_cauldron::chrome::EDITOR_UI_SCALE));

    // **The preview uses the game's own sky** (#98): `bevy_elements`' plugins, always added, dress
    // the editor camera only in a project that has an environment file (`environment::dress_camera`).
    #[cfg(feature = "bevy_elements")]
    {
        use bevy::prelude::*;
        app.add_plugins(bevy_elements::ElementsPlugins)
            .add_systems(Update, (bevy_cauldron::environment::dress_camera, bevy_cauldron::environment::key_light_follows_dressing));
    }

    // **A placed `bevy_lights` `Fire` draws its flame and its pooled light** (#103): the crate's own plugins, which need
    // the render app a window brings, so the headless harness leaves them out.
    #[cfg(feature = "bevy_lights")]
    app.add_plugins(bevy_cauldron::light_view::FirePreviewPlugin);

    // **A bare `bevy_cauldron` opens the launcher** (`launcher.rs`, UI-10); a project named on the command
    // line skips it. `insert_state` before `ScreenPlugin`, whose `init_state` leaves a state that exists as it is.
    let first = if args::opens_the_launcher(&argv) { Screen::Launcher } else { project_assets::screen_for(&argv) };
    app.insert_state(first);
    // The state both halves are gated by, and the two transitions that load and unload a door.
    app.add_plugins(screen::ScreenPlugin);
    // **What drawing works, into the same log** (`startup_probe`): the surface, the cameras, a self-check of what the
    // window itself shows, then a pulse of frames, so a black window says which part of it failed (#306, #307).
    app.add_plugins(bevy_cauldron::startup_probe::StartupProbePlugin);
    app.add_plugins((bevy_cauldron::launcher::LauncherPlugin, bevy_cauldron::loading::LoadingPlugin, bevy_cauldron::window_frame::WindowFramePlugin));
    // **Every opening is recorded in the user's recent-projects list** (`recent.rs`, UI-11). Only the
    // binary names the file: a test's editor has none, so no test writes to the real data directory.
    if let Some(file) = bevy_cauldron::recent::user_file() {
        app.insert_resource(bevy_cauldron::recent::RecentProjects(file));
    }
    // **The user's settings layer** (`settings.rs`, UI-70): only the binary names the file, as with the recent list.
    if let Some(file) = bevy_cauldron::settings::user_file() {
        app.insert_resource(bevy_cauldron::settings::UserSettingsFile(file));
    }
    // **The open tabs are the person's, not the project's** (`session.rs`, NV-04): they are kept under the
    // user's data directory, and only the binary names it, as with the recent list.
    if let Some(dir) = bevy_grimoire_core::user_dirs::data_dir() {
        app.insert_resource(bevy_cauldron::session::SessionStore(dir));
    }
    // **What this machine has** (#341): the terminal starts with the agent chosen on a machine whose
    // login shell finds `omp`, and with a plain shell on one whose does not — the shell the `Invoke
    // Agent` press types into, not this process's `PATH`, which a Finder-launched app is not given. A
    // file either layer states still wins. Only the binary asks, so a test's editor resolves from the
    // built-in default alone.
    app.insert_resource(bevy_cauldron::settings::MachineAgent::ask());
    // **The person's own agent guide** (`agent_guide.rs`, AG-02): only the binary names the file, for the
    // same reason, so no test reads or writes the real one.
    if let Some(file) = bevy_cauldron::agent_guide::user_file() {
        app.insert_resource(bevy_cauldron::agent_guide::UserGuideFile(file));
    }
    // **What an agent tab needs** (`terminal_agent.rs`): the remote protocol's port, the MCP server beside
    // this binary, and a folder for the configs a launch writes. Only the binary names them, as with the
    // settings file, so no test hands an agent the real data directory.
    bevy_cauldron::terminal_agent::insert_for_the_running_binary(&mut app);
    // **The installed app's own update** (`update.rs`): only inside a `.app`, so `cargo run` never replaces itself.
    bevy_cauldron::update::insert_for_the_running_app(&mut app);
    // **A quit that does not finish is finished for it** (`exit_watchdog.rs`, #337): only the binary adds it, because
    // a test editor writes `AppExit` and goes on.
    app.add_plugins(bevy_cauldron::exit_watchdog::ExitWatchdogPlugin::ends_the_process());
    // **And a second press of the quit key ends a quit that is stuck** (#427): read from the watchdog's own thread, because a quit
    // that is stuck is stuck on the main thread, where no key event arrives.
    app.insert_resource(bevy_cauldron::exit_watchdog::SecondPress::this_machine());

    // **Both screens' plugins, always added.** Which one *runs* is the state's business, not the
    // plugin list's — a list that changed with the screen would be two plugin graphs, and only one
    // of them would ever be the one under test.
    harness::add_editor_plugins(&mut app);
    // **`--live`: every committed change goes to disk at once**, so a running game reloads it.
    if args::split(&argv).live {
        app.insert_resource(bevy_cauldron::live::LiveWrite);
    }
    // Behind the feature, so an ordinary build has no `bevy_remote` in its dependency graph at all.
    #[cfg(feature = "debugger")]
    harness::add_debugger_plugins(&mut app);

    app.add_systems(Update, name_the_window);

    // **A project named on the command line is opened the way one chosen on the launcher is**
    // (`project_assets::point_at`): the project page's output IS this argv (`args::open`), so
    // `bevy_cauldron . --door kit --kit ozea` and pressing Enter on the same row take the identical path in.
    // The state it names is the one inserted above, so the window starts on it.
    //
    // **A door or a map is opened on the loading screen** (`loading.rs`, UI-20): the open runs as named stages
    // off the main thread, so a slow project shows a spinner and then a progress bar instead of a window that
    // does not answer, and a project that will not open is said on that screen (and on the terminal the flag
    // was typed into) with a way back to the launcher. `GRIMOIRE_LOAD_STAGE_DELAY_MS` pauses as each stage
    // starts, to see the screen on a project that opens too fast to.
    if first == Screen::Launcher {
        // No project: the face compiled in, until one is chosen.
        if let Err(e) = harness::install_font_in(app.world_mut(), None) {
            eprintln!("bevy_cauldron: {e}");
            std::process::exit(1);
        }
    } else {
        if let Err(e) = project_assets::point_at(app.world_mut(), &argv) {
            eprintln!("{e}");
            return AppExit::error();
        }
        if first == Screen::Loading
            && let Some(ms) = std::env::var("GRIMOIRE_LOAD_STAGE_DELAY_MS").ok().and_then(|v| v.parse::<u64>().ok())
        {
            app.insert_resource(bevy_cauldron::loading::StageDelay(std::time::Duration::from_millis(ms)));
        }
    }

    // **How far startup got, into the log** (`cauldron::startup`): "black window" could be the device, the surface,
    // the first frame or the state machine; this line and `startup_probe`'s say which one it got to. `LogPlugin`
    // installed the subscriber while `add_plugins` ran, so this reaches the file.
    bevy::log::info!(target: "cauldron::startup", "plugins added; first screen {first:?}");
    app.run()
}

/// **The window says which screen it is showing**, and follows a door change.
///
/// It was set once when the window was built, which was fine while a door was a process and its
/// title could be decided before the window existed. One window across both screens makes the title
/// state — and a stale title is exactly how an author ends up in the Map door believing they are in
/// the Kit door, which is where an evening went on 2026-08-16 before the log said `— map — test`.
fn name_the_window(
    screen: Res<State<Screen>>,
    door: Option<Res<bevy_cauldron::workspace::Workspace>>,
    open_map: Option<Res<bevy_cauldron::project::OpenMap>>,
    project: Option<Res<bevy_cauldron::project::Project>>,
    mut windows: Query<&mut Window>,
) {
    let want = match (screen.get(), door.as_deref()) {
        (Screen::Launcher, _) => "Cauldron".to_owned(),
        (Screen::Loading, _) => "Cauldron — opening a project".to_owned(),
        (Screen::Project, _) => "Cauldron — a project".to_owned(),
        (Screen::Editor, Some(d)) => {
            let door = d.label().to_lowercase();
            // **Which name to show is `chrome::subject`'s decision, not a second copy of it.** The
            // chrome bar states the same fact where the eye actually goes, and a title that went on
            // saying the kit while the bar said the map is the drift this shares one function to
            // avoid.
            match bevy_cauldron::chrome::subject(open_map.as_deref(), project.as_deref()) {
                Some(name) => format!("Cauldron — {door} — {name}"),
                None => format!("Cauldron — {door}"),
            }
        }
        (Screen::Editor, None) => "Cauldron".to_owned(),
    };
    for mut w in &mut windows {
        if w.title != want {
            w.title = want.clone();
        }
    }
}


/// **`bevy_cauldron bake <project folder> --out <folder>`**: turn every YAML file the game loads into postcard, prove each
/// against its YAML, and say what was written (GD-08, #370). A file that does not read, or does not survive the bake, fails
/// the command naming it and writes nothing.
fn bake_and_exit(argv: &[String], program: &str) -> AppExit {
    let command = format!("{program} bake <project folder> --out <folder>");
    let rest = &argv[1..];
    let out_at = rest.iter().position(|a| a == "--out");
    let out = out_at.and_then(|at| rest.get(at + 1));
    let root = rest.iter().enumerate().find(|(at, a)| !a.starts_with('-') && out_at.is_none_or(|o| *at != o + 1)).map(|(_, a)| a);
    let (Some(root), Some(out)) = (root, out) else {
        eprintln!("usage: {command}");
        return AppExit::error();
    };
    // **An unknown flag is refused, never dropped**, as `migrate` refuses one: a run that silently ignored `--check` would write a
    // bake the flag was written to preview.
    if let Some(flag) = rest.iter().find(|a| a.starts_with('-') && a.as_str() != "--out") {
        eprintln!("{program} bake: `{flag}` is not a flag this command takes. usage: {command}");
        return AppExit::error();
    }
    match bevy_grimoire_core::bake::bake(std::path::Path::new(root), std::path::Path::new(out)) {
        Ok(report) => {
            println!("baked {} file(s), {} bytes, into {} (manifest: {})", report.files, report.bytes, out, report.manifest.display());
            AppExit::Success
        }
        Err(problem) => {
            eprintln!("{}", problem.line());
            AppExit::error()
        }
    }
}

/// **`bevy_cauldron migrate <project folder>`**: move a project's files from RON to YAML, and say what
/// happened (GD-03, #365).
///
/// The work is the migration crate's (`bevy_grimoire_migrate`), which computes every conversion before it
/// writes one — so a project this refuses is byte for byte what it was. This function only prints:
///
/// - one line per converted file, then every comment that had no place, then where the old files went,
///   or one line saying there was nothing to convert;
/// - on a refusal, the `Problem`'s own line, which names the file and the fix, and a non-zero exit.
///
/// **It converts every kind a project names**: a kind whose type the migration crate has as that type, any other (the
/// editor's, the checks', a family crate's, a game's own data) by value, each read back and compared before anything is written.
///
/// **The folder is required and is the first positional.** A missing one is a refusal rather than a guess
/// at the current folder: `migrate` moves a person's files, and running it in the wrong directory is a
/// run they did not mean.
fn migrate_and_exit(argv: &[String], program: &str) -> AppExit {
    let command = format!("{program} migrate <project folder>");
    let Some(root) = args::subcommand_argument(argv) else {
        eprintln!(
            "usage: {command}\n\n\
             Moves a project's files from RON to YAML. Every file is converted in memory and read back \
             before anything is written, so a project this refuses is left exactly as it was, and the \
             old files are moved to .dark_factory/migrated-ron/ rather than deleted."
        );
        return AppExit::error();
    };
    // **An unknown flag is refused, never dropped.** This command moves a person's files: a run that
    // silently ignored `--dry-run` would do exactly the thing the flag was written to prevent.
    if let Some(flag) = argv.iter().skip(1).find(|a| a.starts_with('-')) {
        eprintln!("{program} migrate: `{flag}` is not a flag this command takes. usage: {command}");
        return AppExit::error();
    }
    let root = std::path::Path::new(root);
    match bevy_grimoire_migrate::migrate_as(root, &command) {
        Ok(report) => {
            let lines = report.lines();
            if lines.is_empty() {
                println!("nothing to migrate: this project holds no `.ron` project file, and no `data.yaml` names one");
            }
            for line in lines {
                println!("{line}");
            }
            AppExit::Success
        }
        Err(problem) => {
            eprintln!("{}", problem.line());
            AppExit::error()
        }
    }
}
