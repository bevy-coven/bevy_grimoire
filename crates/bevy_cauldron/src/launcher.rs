//! **The launcher: the first screen of a Cauldron started with no project** (UI-10, #216).
//!
//! `bevy_cauldron` with no argument (and `--live`, which names no project) lists the projects it has opened, most recent first, with each one's
//! name, folder and when it was last opened (`recent.rs`, UI-11), and offers to open a folder a person
//! types, or the example project that ships with the source. A project named on the command line skips
//! this screen: a bare `ROOT` goes to the chooser, and a door or a map goes through the loading screen (`loading.rs`) into the editor.
//!
//! # Opening a project happens in this process
//!
//! A project's meshes are named relative to the asset root, and the asset root is a reader that can move
//! (`project_assets.rs`, #294): choosing a project points it there ([`crate::project_assets::open_project`])
//! and the window goes on to the chooser, with no second process and no second window. [`command_line`] is
//! what a person pastes into a terminal to open the same project the same way; it is the argv `open_project` parses.
//!
//! The chooser (`chooser.rs`) is still where a kit and a map are picked, until the workbench (UI-30) takes that
//! over; this screen replaces only the way into a project.
//!
//! # What is here, and what is not yet
//!
//! A typed folder path, not the OS folder dialog: no family crate opens one and `rfd`'s Linux backends
//! pull a D-Bus stack or GTK into every build, which wants its own reviewed step. The thumbnail on each
//! row waits for the thumbnail cache; the Settings button opens the settings screen (`settings_screen.rs`, UI-71).

use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use bevy_gooey::{TextHint, ValueChange, button, heading, label, list, menu, observe, panel, screen, text_input};

use crate::recent::{self, Listed, RecentProjects};
use crate::screen::Screen;

/// The binary's name, as the copied command line spells it.
pub const PROGRAM: &str = "bevy_cauldron";

/// The repository's `CHANGELOG.md` as of this build, which is what "new since you last opened" reads.
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// A changelog to read in place of the one built in, for a test to set.
#[derive(Resource)]
pub struct ChangelogText(pub String);

/// One line of the changelog: its id and what it says it added.
#[derive(Clone, Debug, PartialEq)]
pub struct News {
    pub id: String,
    pub title: String,
}

/// How wide the launcher's card is, in logical pixels: wide enough for a project's row, narrower than the window.
pub const CARD_WIDTH: f32 = 840.0;

/// The launcher's splash: pixel art by Retro Diffusion (`tools/ui/splash-jobs.json`), embedded like the icon sheet.
pub const SPLASH_PNG: &[u8] = include_bytes!("../assets/ui/splash.png");

/// The splash is drawn at this many logical pixels per art pixel: a whole number keeps the pixel grid crisp.
const SPLASH_SCALE: f32 = 2.0;

/// The splash decoded with a nearest sampler, because linear filtering blurs pixel art.
fn splash_image() -> Result<Image, String> {
    Image::from_buffer(
        SPLASH_PNG,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::default(),
    )
    .map_err(|e| e.to_string())
}

/// How many new entries the launcher lists before it says how many more there are: three, so the list, the splash
/// and a full recent list still fit a 720 px window (owner, 2026-11-04; six squeezed the splash to a sliver).
const NEWS_ROWS: usize = 3;

/// The longest a title is shown; a changelog line carries its whole story and the launcher shows the start.
const TITLE_CHARS: usize = 140;

/// Every entry of `text`, newest first as the file keeps them: a line `- **<id>** [`<sha>`] <title>`, the title
/// ending where the line's `*No clip` note or its clip's markup begins.
pub fn news(text: &str) -> Vec<News> {
    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("- **")?;
            let (id, rest) = rest.split_once("**")?;
            let rest = rest.trim_start();
            let rest = match rest.strip_prefix('`') {
                Some(after) => after.split_once('`').map_or(rest, |(_, t)| t.trim_start()),
                None => rest,
            };
            let end = [" *No clip", "<br>"].iter().filter_map(|m| rest.find(m)).min().unwrap_or(rest.len());
            let mut title = rest[..end].trim().to_owned();
            if title.chars().count() > TITLE_CHARS {
                title = title.chars().take(TITLE_CHARS - 1).collect::<String>() + "…";
            }
            Some(News { id: id.to_owned(), title })
        })
        .collect()
}

/// The entries newer than `seen`, the id of the newest one a person has been shown. Nothing when none was
/// stored (a first run has seen nothing to compare with) or when it is no longer in the file.
pub fn whats_new(text: &str, seen: Option<&str>) -> Vec<News> {
    let all = news(text);
    match seen.and_then(|id| all.iter().position(|n| n.id == id)) {
        Some(at) => all[..at].to_vec(),
        None => Vec::new(),
    }
}

/// A person chose a project to open: a row of the list, the example, or a typed folder.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct OpenRequest(pub PathBuf);

/// **What the launcher is showing.** Rebuilt every time the screen is entered.
#[derive(Resource, Default, Debug)]
pub struct Launcher {
    /// The recent projects as [`recent::Recent::listing`] orders them.
    pub rows: Vec<Listed>,
    /// A refusal, shown on the screen until the next choice.
    pub problem: Option<String>,
    /// The example project, when this build has one beside it.
    pub example: Option<PathBuf>,
    /// What the changelog added since the last time a project was opened from here, newest first.
    pub whats_new: Vec<News>,
    /// The id of the newest changelog entry, remembered when a project is opened.
    pub newest: Option<String>,
    /// "New project…" was pressed; `new_project.rs` opens its form and clears it.
    pub new_project_asked: bool,
    /// Create or Cancel was pressed on that form; `new_project.rs` takes it down and clears this.
    pub new_project_done: bool,
    /// The command line last asked for with "Copy command line", shown whether or not the clipboard took it.
    pub line: Option<String>,
}

/// **A run with no clipboard to reach**: "Copy command line" says so instead of touching the OS clipboard. A
/// test inserts it, so a headless run never overwrites what a person has copied.
#[derive(Resource)]
pub struct ClipboardOff;

/// The argv the next process is started with: `<root>`, then `--kit` and `--door` when chosen. The same
/// words `bevy_cauldron` itself parses ([`crate::args::split`]), so a project opens the same way from the
/// launcher, from the command line and from a pasted line.
pub fn argv_for(root: &Path, kit: Option<&str>, door: Option<&str>) -> Vec<String> {
    let mut argv = vec![root.to_string_lossy().into_owned()];
    if let Some(door) = door {
        argv.push("--door".to_owned());
        argv.push(door.to_owned());
    }
    if let Some(kit) = kit {
        argv.push("--kit".to_owned());
        argv.push(kit.to_owned());
    }
    argv
}

/// One word as a POSIX shell reads it back: left alone when it is plain, else single-quoted.
fn shell_word(word: &str) -> String {
    let plain = !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || "/._-:=+,@%~".contains(c));
    if plain { word.to_owned() } else { format!("'{}'", word.replace('\'', r"'\''")) }
}

/// The line a person pastes into a terminal to open `root` (with `kit` and `door` when chosen) the way the
/// launcher would.
pub fn command_line(root: &Path, kit: Option<&str>, door: Option<&str>) -> String {
    let words: Vec<String> = argv_for(root, kit, door).iter().map(|w| shell_word(w)).collect();
    format!("{PROGRAM} {}", words.join(" "))
}

/// Whether `root` is a project the Cauldron can open: a folder holding `assets/grimoire`.
pub fn check_project(root: &Path) -> Result<PathBuf, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a folder", root.display()));
    }
    if !root.join(crate::chooser::GRIMOIRE_DIR).is_dir() {
        return Err(format!(
            "{} is not a project: it has no {} folder",
            root.display(),
            crate::chooser::GRIMOIRE_DIR
        ));
    }
    Ok(std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf()))
}

/// The example project to offer, or `None` when there is none: the folder `GRIMOIRE_EXAMPLE` names, the
/// one beside the source this build came from, the copy a person already has ([`user_example`]), or the
/// one inside the installed app ([`bundled_example`]), which is copied out before it is opened.
pub fn example_project() -> Option<PathBuf> {
    let from_env = std::env::var_os("GRIMOIRE_EXAMPLE").map(PathBuf::from);
    let beside_source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from_env
        .into_iter()
        .chain(std::iter::once(beside_source))
        .chain(user_example())
        .find_map(|p| check_project(&p).ok())
        .or_else(bundled_example)
}

/// The example inside the macOS app: `Contents/Resources/example`, beside `Contents/MacOS/<exe>`.
pub fn bundled_example() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    check_project(&exe.parent()?.join("../Resources/example")).ok()
}

/// Where the bundled example is copied to be opened, so saves never write inside the signed app.
pub fn user_example() -> Option<PathBuf> {
    Some(dirs::document_dir()?.join("Cauldron Example"))
}

/// Copies the folder `from` to `to`: files and folders, never a symlink.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            bevy_grimoire_core::atomic_file::write_new(&target, std::fs::read(entry.path())?)?;
        }
    }
    Ok(())
}

/// The folder "Open example" opens: `example` itself, or for the app's bundled copy the person's own copy
/// of it, made on the first open.
fn example_to_open(example: &Path) -> Result<PathBuf, String> {
    if bundled_example().as_deref() != Some(example) {
        return Ok(example.to_path_buf());
    }
    let Some(mine) = user_example() else { return Err("this user has no Documents folder to copy the example into".to_owned()) };
    if !mine.exists() {
        copy_tree(example, &mine).map_err(|e| format!("the example was not copied to {}: {e}", mine.display()))?;
    }
    check_project(&mine).map_err(|_| {
        format!("{} exists and is not a project: rename it, then open the example again", mine.display())
    })
}

/// A typed path as a folder: `~` is the home folder.
fn typed_path(text: &str) -> PathBuf {
    let text = text.trim();
    match text.strip_prefix("~/").and_then(|rest| dirs::home_dir().map(|h| h.join(rest))) {
        Some(p) => p,
        None => PathBuf::from(text),
    }
}

/// A recent project's row: its folder, and whether that folder is gone.
#[derive(Component)]
struct ProjectRow(PathBuf);

/// The "Copy command line" button of a recent project's row.
#[derive(Component)]
struct CopyLine(PathBuf);

/// The launcher's own root node, which the new-project form is added to.
#[derive(Component)]
pub struct LauncherRoot;

/// The launcher's list card, which the new-project form replaces while it is open.
#[derive(Component)]
pub struct LauncherCard;

/// The line that shows the refusal.
#[derive(Component)]
pub(crate) struct ProblemLine;

/// The line that shows the command line last asked for.
#[derive(Component)]
struct CommandLine;

/// "3 days ago", for a time in seconds since the epoch.
pub fn ago(then: u64, now: u64) -> String {
    let secs = now.saturating_sub(then);
    let (n, unit) = match secs {
        0..=59 => return "just now".to_owned(),
        60..=3599 => (secs / 60, "minute"),
        3600..=86_399 => (secs / 3600, "hour"),
        _ => (secs / 86_400, "day"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

/// A project's name: its folder's last word.
fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

pub struct LauncherPlugin;

impl Plugin for LauncherPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::new_project::NewProjectPlugin).add_message::<OpenRequest>()
            .add_systems(OnEnter(Screen::Launcher), build_launcher)
            .add_systems(
                Update,
                (open_requested, show_problem, show_line).chain().run_if(in_state(Screen::Launcher)),
            );
    }
}

fn build_launcher(
    mut commands: Commands,
    recents: Option<Res<RecentProjects>>,
    changelog: Option<Res<ChangelogText>>,
    updater: Option<Res<crate::update::Updater>>,
    mut images: ResMut<Assets<Image>>,
    // The handle and the art's size in pixels, kept so re-entering the launcher (a cancelled load) reuses one image.
    mut splash: Local<Option<(Handle<Image>, Vec2)>>,
) {
    if splash.is_none() {
        match splash_image() {
            Ok(image) => {
                let size = Vec2::new(image.width() as f32, image.height() as f32);
                *splash = Some((images.add(image), size));
            }
            // The PNG is compiled in, so a splash that will not decode is a build fault: the launcher shows none.
            Err(e) => error!("the launcher's splash does not decode: {e}"),
        }
    }
    let stored = recents.as_deref().and_then(|r| recent::Recent::load(&r.0).ok());
    let rows = stored.as_ref().map(recent::Recent::listing).unwrap_or_default();
    let text = changelog.as_deref().map_or(CHANGELOG, |c| c.0.as_str());
    let whats_new = whats_new(text, stored.as_ref().and_then(|r| r.changelog_seen.as_deref()));
    let newest = news(text).first().map(|n| n.id.clone());
    let problem = recents.as_deref().and_then(|r| recent::Recent::load(&r.0).err());
    let example = example_project();
    let now = recent::now();
    let launcher = Launcher { rows, problem, example, whats_new, newest, ..default() };

    commands
        .spawn((screen(), DespawnOnExit(Screen::Launcher), LauncherRoot, Name::new("launcher")))
        .insert(Pickable::default())
        .with_children(|root| {
            root.spawn((panel(), LauncherCard))
                // Bounded by the viewport, not by `screen()`, whose root grows with its content; `min_height: 0` lets the bound
                // win over the card's content height, so its children give way instead of the window's edge.
                .insert(Node { flex_direction: FlexDirection::Column, width: Val::Px(CARD_WIDTH), max_width: Val::Percent(100.0), max_height: Val::Vh(100.0), min_height: Val::Px(0.0), ..default() })
                .with_children(|card| {
                if let Some((handle, size)) = splash.as_ref() {
                    card.spawn((
                        ImageNode::new(handle.clone()),
                        // The height is what gives way when the card would outgrow the window (six new lines and a full recent
                        // list): the splash shrinks, its width following through the aspect ratio, and every button stays on screen.
                        Node {
                            height: Val::Px(size.y * SPLASH_SCALE),
                            min_height: Val::Px(0.0),
                            max_width: Val::Percent(100.0),
                            aspect_ratio: Some(size.x / size.y),
                            flex_shrink: 1.0,
                            align_self: AlignSelf::Center,
                            ..default()
                        },
                        Name::new("launcher splash"),
                    ));
                }
                card.spawn(heading("Cauldron"));
                card.spawn(label(format!("Version {}", env!("CARGO_PKG_VERSION"))));
                card.spawn(label("The editor that writes a grimoire, the book your game loads."));
                if !launcher.whats_new.is_empty() {
                    card.spawn(heading("New since you last opened"));
                    for item in launcher.whats_new.iter().take(NEWS_ROWS) {
                        card.spawn(label(format!("{}: {}", item.id, item.title)));
                    }
                    if launcher.whats_new.len() > NEWS_ROWS {
                        card.spawn(label(format!("and {} more in CHANGELOG.md", launcher.whats_new.len() - NEWS_ROWS)));
                    }
                }
                card.spawn(heading("Recent projects"));
                if launcher.rows.is_empty() {
                    card.spawn(label("No project has been opened yet. Type a folder below, or open the example."));
                } else {
                    let rows: Vec<_> = launcher.rows.iter().map(|r| project_row(r, now)).collect();
                    card.spawn(list(5.0, Children::spawn(SpawnIter(rows.into_iter()))));
                }
                card.spawn((
                    text_input("Open a folder", ""),
                    TextHint("a folder, then Enter".into()),
                    observe(|change: On<ValueChange<String>>, mut out: MessageWriter<OpenRequest>| {
                        if change.is_final && !change.value.trim().is_empty() {
                            out.write(OpenRequest(typed_path(&change.value)));
                        }
                    }),
                ));
                card.spawn(menu()).with_children(|actions| {
                    actions.spawn((
                        button("New project…"),
                        observe(|_: On<bevy_gooey::Activate>, mut launcher: ResMut<Launcher>| launcher.new_project_asked = true),
                    ));
                    actions.spawn((
                        button("Settings"),
                        observe(|_: On<bevy_gooey::Activate>, mut out: MessageWriter<crate::settings_screen::OpenSettings>| {
                            out.write(crate::settings_screen::OpenSettings);
                        }),
                    ));
                    if let Some(updater) = updater.as_deref() {
                        actions.spawn(crate::update::update_button(updater.label()));
                    }
                    if let Some(example) = launcher.example.clone() {
                        actions.spawn((
                            button("Open example"),
                            observe(move |_: On<bevy_gooey::Activate>, mut out: MessageWriter<OpenRequest>, mut launcher: ResMut<Launcher>| {
                                match example_to_open(&example) {
                                    Ok(root) => {
                                        out.write(OpenRequest(root));
                                    }
                                    Err(e) => launcher.problem = Some(e),
                                }
                            }),
                        ));
                    }
                });
                card.spawn((label(launcher.problem.clone().unwrap_or_default()), ProblemLine));
                card.spawn((label(""), CommandLine));
            });
        });
    commands.insert_resource(launcher);
}

/// One recent project: its name as a button that opens it, its folder, when it was opened, and the button
/// that copies the line that opens it from a terminal.
fn project_row(row: &Listed, now: u64) -> impl Bundle {
    let path = row.project.path.clone();
    let when = if row.missing {
        format!("missing: {} is not there now", path.display())
    } else {
        format!("{}  ·  opened {}", path.display(), ago(row.project.last_opened, now))
    };
    let open = path.clone();
    let copy = path.clone();
    (
        Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, column_gap: Val::Px(crate::chrome::GAP_GROUP), ..default() },
        children![
            (
                button(name_of(&path)),
                ProjectRow(open),
                observe(|activate: On<bevy_gooey::Activate>, rows: Query<&ProjectRow>, mut out: MessageWriter<OpenRequest>| {
                    if let Ok(row) = rows.get(activate.entity) {
                        out.write(OpenRequest(row.0.clone()));
                    }
                }),
            ),
            label(when),
            (
                button("Copy command line"),
                CopyLine(copy),
                observe(
                    |activate: On<bevy_gooey::Activate>, rows: Query<&CopyLine>, off: Option<Res<ClipboardOff>>, mut launcher: ResMut<Launcher>| {
                        let Ok(row) = rows.get(activate.entity) else { return };
                        let line = command_line(&row.0, None, None);
                        let copied = if off.is_some() { Err("no clipboard in this run".to_owned()) } else { copy_to_clipboard(&line) };
                        launcher.problem = match copied {
                            Ok(()) => None,
                            Err(e) => Some(format!("the clipboard did not take it ({e}); the line is below")),
                        };
                        launcher.line = Some(line);
                    },
                ),
            ),
        ],
    )
}

/// Puts `text` on the OS clipboard, as `notice.rs`'s Cmd+C does.
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    arboard::Clipboard::new().and_then(|mut c| c.set_text(text)).map_err(|e| e.to_string())
}

/// The ways a request to open a project ends: refused on the screen, or opened in this process
/// ([`crate::project_assets::open_project`]).
fn open_requested(
    mut commands: Commands,
    mut requests: MessageReader<OpenRequest>,
    mut launcher: ResMut<Launcher>,
    recents: Option<Res<RecentProjects>>,
) {
    let Some(OpenRequest(root)) = requests.read().last().cloned() else { return };
    match check_project(&root) {
        Ok(root) => {
            launcher.problem = None;
            if let Some(file) = recents.as_deref().map(|r| &r.0)
                && let Err(e) = recent::record_seen(file, &root, launcher.newest.as_deref())
            {
                warn!("recent projects not updated: {e}");
            }
            commands.queue(move |world: &mut World| {
                if let Err(e) = crate::project_assets::open_project(world, &root, None)
                    && let Some(mut launcher) = world.get_resource_mut::<Launcher>()
                {
                    launcher.problem = Some(e);
                }
            });
        }
        Err(e) => launcher.problem = Some(e),
    }
}

fn show_problem(launcher: Res<Launcher>, mut lines: Query<&mut Text, With<ProblemLine>>) {
    if !launcher.is_changed() {
        return;
    }
    let want = launcher.problem.clone().unwrap_or_default();
    for mut text in &mut lines {
        if text.0 != want {
            text.0 = want.clone();
        }
    }
}

fn show_line(launcher: Res<Launcher>, mut lines: Query<&mut Text, With<CommandLine>>) {
    if !launcher.is_changed() {
        return;
    }
    let want = launcher.line.clone().unwrap_or_default();
    for mut text in &mut lines {
        if text.0 != want {
            text.0 = want.clone();
        }
    }
}

