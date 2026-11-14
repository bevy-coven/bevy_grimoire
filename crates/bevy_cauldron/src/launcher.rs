//! **The Projects home: the first screen of a Cauldron started with no project** (UI-10, #216; #295).
//!
//! `bevy_cauldron` with no argument (and `--live`, which names no project) fills the window with a sidebar and a
//! page. The sidebar holds the splash, the three destinations (Projects, Learn, What's new), Settings, the version and the
//! updater. The Projects page lists the projects the editor has opened, most recent first, as cards (`recent.rs`, UI-11) with a
//! filter, and offers New project and Open folder…; on a first run, with nothing opened yet, it is a welcome whose one accent
//! button is **Open the example**. A project named on the command line skips this screen.
//!
//! # Opening a project happens in this process
//!
//! A project's meshes are named relative to the asset root, and the asset root is a reader that can move
//! (`project_assets.rs`, #294): choosing a project points it there ([`crate::project_assets::open_project`]),
//! with no second process and no second window. [`command_line`] is what a person pastes into a terminal to open the same
//! project the same way; it is the argv `open_project` parses.
//!
//! # The file chooser is the OS's own
//!
//! Folders and files are chosen with the system's chooser (`picker.rs`, #298): `rfd` with its `xdg-portal` backend on
//! Linux, which talks D-Bus to the desktop's portal and so pulls no GTK into the build (`rfd`'s other Linux backend
//! does, which is why a typed path stood here before). A system with no chooser service gets a toast saying so. A folder
//! that is not a project is a toast too, with the button that makes it one.
//!
//! # What is built from what is missing in `bevy_gooey`
//!
//! A card, the accent button and the vertical navigation are local until bevy_gooey#41, #40 and #42 land: a card is a gooey
//! button's skeleton with an image and two quiet lines, the accent button is [`crate::ui::submit_button`], and the navigation is
//! three buttons, the chosen one marked [`bevy::ui::Selected`].

use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Selected;
use bevy_gooey::{
    Activate, ContextMenu, ContextTarget, Role, SplitAxis, SplitFraction, SplitMin, TextHint, ToastAction, ValueChange, button, context_menu,
    fuzzy_score, heading, label, menu_item, observe, screen, separator, split, text_input, toast, toast_alert,
};

use crate::picker::{PickFor, PickRequest, Picked};
use crate::recent::{self, Listed, Recent, RecentProjects};
use crate::screen::Screen;
use crate::ui::submit_button;


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
    /// The line's `*For you: <sentence>*`: the change as a person who never reads the changelog would say it. The What's new page
    /// lists only the entries that have one (`tools/changelog/changelog.py add --user`).
    pub user: Option<String>,
    /// The still of the entry's clip, as the path the changelog gives (`docs/changelog/media/<name>.png`).
    pub still: Option<String>,
}

/// How wide the launcher's card is, in logical pixels: wide enough for a project's row, narrower than the window.
pub const CARD_WIDTH: f32 = 840.0;

/// **How wide a dialog's card is, in body text sizes**: the ceiling gooey's dialog skin gave a card under its scrim (36 text sizes, about 397 px at the editor's
/// text), which every dialog here was effectively drawn at while its own `width` asked for 520 to 840 px. `CardWidth::Text` states it, so a card is the width a
/// person saw before the card was built with `sized_panel` (which writes `width` and no ceiling): measured 396.7 px at the harness window both ways.
pub const DIALOG_CARD_TEXT_SIZES: f32 = 36.0;

/// The launcher's splash: pixel art by Retro Diffusion (`tools/ui/splash-jobs.json`), embedded like the icon sheet.
pub const SPLASH_PNG: &[u8] = include_bytes!("../assets/ui/splash.png");

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

/// The longest a title is shown; a changelog line carries its whole story and the launcher shows the start.
const TITLE_CHARS: usize = 140;

/// What starts a line's marked notes: the title ends at the first of these.
const TITLE_ENDS: [&str; 3] = [" *No clip", "<br>", " *For you: "];

/// Every entry of `text`, newest first as the file keeps them: a line `- **<id>** [`<sha>`] <title>`, the title
/// ending where the line's `*For you` sentence, its `*No clip` note or its clip's markup begins.
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
            let end = TITLE_ENDS.iter().filter_map(|m| rest.find(m)).min().unwrap_or(rest.len());
            let mut title = rest[..end].trim().to_owned();
            if title.chars().count() > TITLE_CHARS {
                title = title.chars().take(TITLE_CHARS - 1).collect::<String>() + "…";
            }
            let user = rest.split_once(TITLE_ENDS[2]).and_then(|(_, after)| after.split_once('*')).map(|(sentence, _)| sentence.trim().to_owned()).filter(|s| !s.is_empty());
            let still = rest.split_once("<img src=\"").and_then(|(_, after)| after.split_once('"')).map(|(src, _)| src.to_owned()).filter(|src| src.ends_with(".png"));
            Some(News { id: id.to_owned(), title, user, still })
        })
        .collect()
}

/// The entries that have a sentence for a person, newest first: what the What's new page lists.
pub fn for_you(text: &str) -> Vec<News> {
    news(text).into_iter().filter(|n| n.user.is_some()).collect()
}

/// The entries with a sentence for a person that are newer than `seen`, the id of the newest entry a person has been shown. Nothing
/// when none was stored (a first run has seen nothing to compare with) or when it is no longer in the file. This is the count on the
/// sidebar's What's new.
pub fn whats_new(text: &str, seen: Option<&str>) -> Vec<News> {
    let all = news(text);
    match seen.and_then(|id| all.iter().position(|n| n.id == id)) {
        Some(at) => all[..at].iter().filter(|n| n.user.is_some()).cloned().collect(),
        None => Vec::new(),
    }
}

/// A person chose a project to open: a card, the example, a folder from the chooser or a new project.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct OpenRequest(pub PathBuf);

/// A person chose to open a project **on a map**: the example's, from the welcome. Everything else about the request is
/// [`OpenRequest`]'s.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct OpenMapRequest(pub PathBuf, pub String);

/// The page the sidebar shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HomeTab {
    #[default]
    Projects,
    Learn,
    WhatsNew,
}

impl HomeTab {
    pub const ALL: [HomeTab; 3] = [HomeTab::Projects, HomeTab::Learn, HomeTab::WhatsNew];

    pub fn label(self) -> &'static str {
        match self {
            HomeTab::Projects => "Projects",
            HomeTab::Learn => "Learn",
            HomeTab::WhatsNew => "What's new",
        }
    }
}

/// **What the Projects home is showing.** Rebuilt every time the screen is entered.
#[derive(Resource, Default, Debug)]
pub struct Launcher {
    /// The recent projects as [`recent::Recent::listing`] orders them.
    pub rows: Vec<Listed>,
    /// What the filter box holds.
    pub filter: String,
    /// The page the sidebar chose.
    pub tab: HomeTab,
    /// A refusal to say: the launcher shows each new one as a toast. The updater and the new-project form write it.
    pub problem: Option<String>,
    /// The example project, when this build has one beside it.
    pub example: Option<PathBuf>,
    /// Every changelog entry that has a sentence for a person ([`for_you`]), newest first: the What's new page.
    pub news: Vec<News>,
    /// Those of them added since the last time a project was opened from here, newest first: the count on the sidebar's What's new.
    pub whats_new: Vec<News>,
    /// The id of the newest changelog entry, remembered when a project is opened.
    pub newest: Option<String>,
    /// "New project…" was pressed; `new_project.rs` opens its form and clears it.
    pub new_project_asked: bool,
    /// Create or Cancel was pressed on that form; `new_project.rs` takes it down and clears this.
    pub new_project_done: bool,
    /// The cards are out of date (a project was removed or located): the next frame draws them again.
    pub dirty: bool,
}

/// **Take the tour was pressed**: the example is being opened so the editor can offer its tour. The editor takes this on entering.
#[derive(Resource, Debug, Clone, Copy)]
pub struct TourRequested;

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
    if !root.join(crate::catalog::GRIMOIRE_DIR).is_dir() {
        return Err(format!(
            "{} is not a project: it has no {} folder",
            root.display(),
            crate::catalog::GRIMOIRE_DIR
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
pub(crate) fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
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

/// Where "Getting started guide" opens.
pub const GUIDE_URL: &str = "https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/guide/getting-started.md";

/// **The guide's own folder, as the editor publishes it**, with the trailing slash. [`GUIDE_URL`] is one page of it and
/// [`crate::help::url`] builds every other page's address from this, so the repository's location is written once.
pub fn guide_dir_url() -> &'static str {
    GUIDE_URL.strip_suffix(GUIDE_FILE).unwrap_or(GUIDE_URL)
}

/// The page file [`GUIDE_URL`] ends with, so [`guide_dir_url`] can cut it without a second copy of the folder.
const GUIDE_FILE: &str = "getting-started.md";

/// Where "See every change" opens.
pub const CHANGELOG_URL: &str = "https://github.com/bevy-coven/bevy_grimoire/blob/main/CHANGELOG.md";

/// The sidebar's width, in logical pixels.
const SIDEBAR_PX: f32 = 240.0;
/// **The sidebar's content width**: its width less the `chrome::PAD` it pads every side by.
///
/// The splash is drawn into the sidebar's padded content box, so a width taken from [`SIDEBAR_PX`]
/// alone ran `2 * PAD` wider than the box and pushed its right edge out of the sidebar — reported as
/// "This has no margin on the right side, but the rest of the sides do"
/// (`feedback-1793905798698`). One number for the content width, so the splash and everything else in
/// the sidebar that wants the full width agree.
const SIDEBAR_CONTENT_W: f32 = SIDEBAR_PX - 2.0 * crate::chrome::PAD;
/// The splash in the sidebar: the art is 256 x 80, drawn at the sidebar's content width.
const SIDEBAR_SPLASH: Vec2 = Vec2::new(SIDEBAR_CONTENT_W, SIDEBAR_CONTENT_W * 80.0 / 256.0);
/// The least the page beside the sidebar may be.
const PAGE_MIN_PX: f32 = 480.0;
/// The welcome card's splash is drawn at this many logical pixels per art pixel.
const WELCOME_SPLASH_SCALE: f32 = 2.0;
/// A project card's width, and the picture's size inside it.
const CARD_PX: f32 = 184.0;
const THUMB: Vec2 = Vec2::new(160.0, 90.0);
/// The longest a folder is shown before its middle is cut.
const FOLDER_CHARS: usize = 24;
/// The mark of a cut.
const FOLDER_ELLIPSIS: char = '…';

/// The splash decoded once, and its size in art pixels.
#[derive(Resource, Default)]
pub(crate) struct SplashArt(pub(crate) Option<(Handle<Image>, Vec2)>);

/// **One decoded picture per (path, modified time)**, so a card and a page share one texture and a
/// redraw does not decode the PNG again.
///
/// `project_picture` used to read, decode and `images.add` on **every** call: once per card on the
/// home, once for the project page's body, and again on every redraw. Each of those was a full PNG
/// decode, a fresh `Image` in `Assets`, and a fresh upload for the GPU, for a file that had not
/// changed. This keeps one handle per version of the file instead.
///
/// # What this is not
///
/// It is **not** the fix for the owner's "garbled flash" (`0.2.5`, `mac_air`, 2026-11-05). That was
/// measured and the guess was wrong: a picture added while the page is built is uploaded before the
/// frame is drawn, because `StateTransition` runs ahead of `Update` and asset extraction follows
/// `PostUpdate` (`bevy_app-0.20.0/src/main_schedule.rs`). Captured on big under Xvfb at 15 fps
/// against a 1 MiB portrait, the first frame of the page draws the picture correctly with no band —
/// both with and without this cache. The frames that settle that are on big at
/// `/tmp/pp-frames/big/`. What pins the switching is
/// `tests/the_project_page.rs::the_first_frame_of_the_page_is_whole`.
///
/// Keys on the file's modified time, so a freshly baked portrait is decoded again and the newest one
/// keeps changing what the cards show — the cache is a decode and upload saver, never a staleness
/// source.
#[derive(Resource, Default)]
pub(crate) struct ProjectPictures(std::collections::HashMap<(PathBuf, std::time::SystemTime), (Handle<Image>, Vec2)>);

/// The window's root node, which the context menu hangs under.
#[derive(Component)]
pub struct LauncherRoot;

/// The page beside the sidebar.
#[derive(Component)]
pub struct HomeContent;

#[derive(Component)]
pub struct SidebarRoot;

/// One page of the home; only the sidebar's choice is shown. `pub` so a test can find the page a capture
/// is about — the same reason `project_page::PageBody` is.
#[derive(Component)]
pub struct HomePage(pub HomeTab);

/// The body of the Projects page: the welcome or the grid, drawn again when the cards change.
#[derive(Component)]
struct ProjectsBody;

/// The scrolling box the cards stand in.
#[derive(Component)]
struct ProjectGrid;

/// A sidebar row.
#[derive(Component)]
pub struct NavTab(pub HomeTab);

/// The shared menu every card names.
#[derive(Component)]
struct CardMenu;

/// A project's card. `key` is what the filter reads: its name and its folder.
#[derive(Component)]
pub struct ProjectCard {
    pub path: PathBuf,
    key: String,
}

/// The card of a project whose folder is gone: it does not open, and offers Locate… and Remove.
#[derive(Component)]
pub struct MissingCard {
    pub path: PathBuf,
    key: String,
}


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
pub(crate) fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

pub struct LauncherPlugin;

impl Plugin for LauncherPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((crate::new_project::NewProjectPlugin, crate::picker::PickerPlugin, crate::project_page::ProjectPagePlugin))
            .add_message::<OpenRequest>()
            .add_message::<OpenMapRequest>()
            .init_resource::<SplashArt>()
            .init_resource::<ProjectPictures>()
            .init_resource::<Launcher>()
            .add_observer(on_toast_action)
            .add_systems(Update, (paint_the_quiet_lines, follow_the_updater))
            .add_systems(OnEnter(Screen::Launcher), (build_launcher, fill_the_home).chain())
            .add_systems(
                Update,
                (hide_the_divider, say_the_startup_notices, open_requested, on_picked, say_the_problem, redraw_the_cards, apply_filter, show_the_tab.before(bevy_gooey::GooeySystems::Style), mark_the_focus)
                    .chain()
                    .run_if(in_state(Screen::Launcher)),
            );
    }
}

/// The sidebar is not a thing to resize, so its divider is not drawn and not reachable.
pub(crate) fn hide_the_divider(mut dividers: Query<&mut Node, With<bevy_gooey::SplitDivider>>) {
    for mut node in &mut dividers {
        if node.display != Display::None {
            node.display = Display::None;
        }
    }
}

/// The names a card and the filter share: its folder's name, then the folder.
fn card_key(path: &Path) -> String {
    format!("{} {}", name_of(path), path.display())
}

/// `text` cut in the middle with an ellipsis when it is longer than `max` characters.
pub fn middle_ellipsis(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max || max < 3 {
        return text.to_owned();
    }
    let keep = max - 1;
    let tail = keep / 2;
    let head = keep - tail;
    chars[..head].iter().chain(std::iter::once(&FOLDER_ELLIPSIS)).chain(chars[chars.len() - tail..].iter()).collect()
}

/// A folder as a person reads it: the home folder is `~`, and a long one loses its middle.
///
/// `pub(crate)` because a saved feedback report (`report.rs`) says where it went in the same words a
/// project's card says where the project is: one way of naming a folder, not two.
pub(crate) fn shown_folder(path: &Path) -> String {
    let full = match dirs::home_dir().and_then(|home| path.strip_prefix(&home).ok().map(Path::to_path_buf)) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    };
    middle_ellipsis(&full, FOLDER_CHARS)
}

/// A quiet line under a name: the skin's muted ink ([`quiet_ink`], painted by [`paint_the_quiet_lines`]).
pub(crate) fn quiet(text: impl Into<String>) -> impl Bundle {
    (Text::new(text), crate::chrome::typed(crate::chrome::text::LABEL), TextColor(crate::chrome::ink().label), Quiet)
}

/// A line of muted words (a card's folder, a count).
#[derive(Component)]
pub struct Quiet;

/// The ink of a [`Quiet`] line: the skin's `text_muted`, or the shipped palette's own `Ink::label` in a harness with no skin.
pub fn quiet_ink(skin: Option<&bevy_gooey::GooeySkin>) -> Color {
    skin.map_or(crate::chrome::ink().label, |s| s.text_muted)
}

/// Keeps every [`Quiet`] line in the skin's ink, from the frame it appears and whenever the skin changes.
fn paint_the_quiet_lines(skin: Option<Res<bevy_gooey::GooeySkin>>, mut lines: Query<&mut TextColor, With<Quiet>>, fresh: Query<(), Added<Quiet>>) {
    if fresh.is_empty() && !skin.as_ref().is_some_and(|s| s.is_changed()) {
        return;
    }
    let ink = quiet_ink(skin.as_deref());
    for mut line in &mut lines {
        line.set_if_neq(TextColor(ink));
    }
}

/// A card that is a button (#260): gooey's button box in a column, a card wide. The padding is the button skin's (a card was always padded by it: the 12 px its raw node asked for
/// was overwritten, measured 9 x 3.6 px), and a button's role writes a gap along its row only, so a card's lines stand `STEP_TIGHT` apart by a bottom margin on each line above
/// the last ([`card_line`]).
pub(crate) fn card_button() -> impl Bundle {
    (
        bevy_gooey::button_box(),
        bevy_gooey::RowLayout { direction: Some(FlexDirection::Column), align: Some(AlignItems::Stretch), ..default() },
        bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Px(CARD_PX)), ..default() },
    )
}

/// The room a card's line leaves before the next one (what `row_gap` was when the card was a raw column).
pub(crate) fn card_line() -> bevy_gooey::Spacing {
    crate::chrome::spacing(crate::chrome::margin_bottom(crate::chrome::STEP_TIGHT))
}

/// A card's box on a card that is not a button (G-87): a sized column a card wide, `STEP_PAD` all round, its lines `GAP_TIGHT` px apart. A `Role::Card` writes only a colour, so the box is
/// gooey's `sized` and the caller `.insert(Role::Card)`s over its role (a bundle cannot carry two).
pub(crate) fn card_layout() -> impl Bundle {
    (
        bevy_gooey::sized(bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Px(CARD_PX)), ..default() }),
        bevy_gooey::RowLayout { direction: Some(FlexDirection::Column), ..default() },
        bevy_gooey::Spacing { padding: bevy_gooey::Sides::all(crate::chrome::STEP_PAD), ..default() },
        bevy_gooey::GapPx(crate::chrome::GAP_TIGHT),
    )
}

/// **The picture's box inside a card**: gooey's `image_box`, so it holds its room while the picture loads. A cropped picture is `.insert(picture)`ed over it:
/// `project_picture` chose the middle band that fits the box and put it on the `ImageNode`'s `rect`, and `image_box` alone would draw the whole image squeezed
/// into the box.
pub(crate) fn thumb_box(image: Handle<Image>) -> impl Bundle {
    bevy_gooey::image_box(image, THUMB)
}

/// Decodes the splash once, into `splash`.
pub(crate) fn ensure_splash(images: &mut Assets<Image>, splash: &mut SplashArt) {
    if splash.0.is_some() {
        return;
    }
    match splash_image() {
        Ok(image) => {
            let size = Vec2::new(image.width() as f32, image.height() as f32);
            splash.0 = Some((images.add(image), size));
        }
        // The PNG is compiled in, so a splash that will not decode is a build fault: the home shows none.
        Err(e) => error!("the home's splash does not decode: {e}"),
    }
}

fn build_launcher(
    mut commands: Commands,
    recents: Option<Res<RecentProjects>>,
    changelog: Option<Res<ChangelogText>>,
    before: Res<Launcher>,
    mut images: ResMut<Assets<Image>>,
    mut splash: ResMut<SplashArt>,
) {
    ensure_splash(&mut images, &mut splash);
    let (stored, problem) = match recents.as_deref().map(|r| Recent::load(&r.0)) {
        Some(Ok(r)) => (Some(r), None),
        Some(Err(e)) => (None, Some(e)),
        None => (None, None),
    };
    let rows = stored.as_ref().map(Recent::listing).unwrap_or_default();
    let text = changelog.as_deref().map_or(CHANGELOG, |c| c.0.as_str());
    let whats_new = whats_new(text, stored.as_ref().and_then(|r| r.changelog_seen.as_deref()));
    let newest = news(text).first().map(|n| n.id.clone());
    // The page the sidebar was on is kept: the project page's sidebar sends a person here to a page, and the editor's File → New
    // project… to the form on it.
    commands.insert_resource(Launcher { rows, problem, example: example_project(), news: for_you(text), whats_new, newest, tab: before.tab, new_project_asked: before.new_project_asked, ..default() });
    spawn_shell(&mut commands, Screen::Launcher);
}

/// **The window the home and the project page share**: a split of a fixed sidebar and a page, with the terminal docked across the
/// bottom (#314). `SplitMin` holds the sidebar at its width and the divider is not drawn ([`hide_the_divider`]): the sidebar is not a
/// thing to resize. Everything on `screen` is despawned when it is left; the terminal's shells are not (`terminal::Terminal`).
pub(crate) fn spawn_shell(commands: &mut Commands, screen_of: Screen) -> Entity {
    commands
        .spawn((screen(), DespawnOnExit(screen_of), LauncherRoot, Name::new("projects home")))
        .insert(Pickable::default())
        .with_children(|root| {
            root.spawn((
                split(
                    SplitAxis::Horizontal,
                    (page_surface(1), SidebarRoot),
                    (page_surface(0), bevy_gooey::Sizing { min_height: Some(bevy_gooey::Extent::Px(0.0)), ..default() }, HomeContent),
                ),
                SplitFraction(0.0),
                SplitMin::new(SIDEBAR_PX, PAGE_MIN_PX),
            ))
            // **The split takes what the terminal leaves**, not the whole window: gooey's split is 100% tall, and beside a docked pane that would push the pane off the
            // bottom. A filling split (G-88): the window wide, as tall as its content asks and growing into what the pane leaves, with no least height.
            .insert((
                bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), height: Some(bevy_gooey::Extent::Auto), min_height: Some(bevy_gooey::Extent::Px(0.0)), ..default() },
                bevy_gooey::TextFlow { grow: Some(1.0), ..default() },
            ));
        })
        .insert(crate::terminal_pane::TerminalDock)
        .id()
}

/// **The sidebar**: the splash, the three pages, Settings, the version and the updater. On the project page a press on a page goes
/// back to the home on that page.
pub(crate) fn fill_sidebar(commands: &mut Commands, sidebar: Entity, launcher: &Launcher, splash: &SplashArt, updater: Option<&crate::update::Updater>) {
    commands.entity(sidebar).with_children(|side| {
        if let Some((handle, _)) = splash.0.as_ref() {
            // **The box is the sidebar's content width**, not its outer width, and the height follows
            // the art's own 256 x 80 so the picture is never stretched (`feedback-1793905798698`).
            side.spawn((
                bevy_gooey::image_box(handle.clone(), SIDEBAR_SPLASH),
                Name::new("sidebar splash"),
            ));
        }
        for tab in HomeTab::ALL {
            let badge = if tab == HomeTab::WhatsNew && !launcher.whats_new.is_empty() { format!(" ({})", launcher.whats_new.len()) } else { String::new() };
            let mut nav = side.spawn((
                button(format!("{}{badge}", tab.label())),
                NavTab(tab),
                observe(move |_: On<Activate>, mut launcher: ResMut<Launcher>, screen: Res<State<Screen>>, mut next: ResMut<NextState<Screen>>| {
                    launcher.tab = tab;
                    if *screen.get() != Screen::Launcher {
                        next.set(Screen::Launcher);
                    }
                }),
            ));
            if tab == launcher.tab {
                // **Selection is a shape, not a shade.** `ROW_SELECTED` (L* 25.0) against `ROW_BG`
                // (15.9) is a few ramp steps, and in the shipped screenshot the selected tab was not
                // tellable from its neighbours. `ui::paint_marked_buttons` gives a `Selected` button
                // the accent rail down its leading edge (`crate::chrome::SELECT_RAIL_W`), the same vocabulary
                // the kit's rows use — so the tab strip and a kit's list say "this one" the same way.
                nav.insert(Selected);
            }
        }
        side.spawn(bevy_gooey::spacer());
        side.spawn((
            button("Settings"),
            observe(|_: On<Activate>, mut out: MessageWriter<crate::settings_screen::OpenSettings>| {
                out.write(crate::settings_screen::OpenSettings);
            }),
        ));
        // **One version line, and a button only when there is something to do** (design review,
        // 2026-11-05). It used to be `Version 0.2.4` *and* an always-present `Up to date — v0.2.4`
        // button: the same fact twice, the second dressed in a page tab's skin and inviting a press
        // that does nothing. The line is drawn here; what it says, and the button under it, are
        // [`follow_the_updater`]'s, because the launch check answers after this sidebar is built.
        side.spawn((quiet(version_line(updater)), VersionLine));
    });
}

/// The sidebar's version line, which [`follow_the_updater`] keeps saying what the updater knows.
#[derive(Component)]
pub struct VersionLine;

/// **What the version line says**: the running version, and `up to date` only once a check has found
/// nothing newer. A check still running, one that failed and a blocked updater have not found that.
fn version_line(updater: Option<&crate::update::Updater>) -> String {
    let version = env!("CARGO_PKG_VERSION");
    match updater.map(|u| &u.state) {
        Some(crate::update::UpdateState::UpToDate) => format!("v{version} · up to date"),
        _ => format!("v{version}"),
    }
}

/// **The sidebar says what the updater knows now, not what it knew when the sidebar was built.**
///
/// The launch check answers from a thread about half a second after the window opens
/// (`update::start_the_first_check`), and the home's sidebar is built in `OnEnter(Screen::Launcher)`,
/// before that. The sidebar used to be drawn once from the updater as it stood then: `Idle`, printed
/// as `v0.3.4 · up to date` with no button. The answer that 0.3.5 and 0.3.6 were out never reached
/// it, so a 0.3.4 home offered nothing (reported at the keyboard on 2026-11-06). The editor's chrome
/// bar already follows the updater (`update::place_in_the_chrome_bar`); this is the same rule for the
/// sidebar on both screens that draw one: one [`crate::update::UpdateButton`] after the line while
/// [`crate::update::Updater::actionable`], none otherwise. `update::relabel` keeps its words.
fn follow_the_updater(
    mut commands: Commands,
    updater: Option<Res<crate::update::Updater>>,
    mut lines: Query<(&ChildOf, &mut Text, Ref<VersionLine>)>,
    buttons: Query<(Entity, &ChildOf), With<crate::update::UpdateButton>>,
    checks: Query<(Entity, &ChildOf), With<crate::update::CheckButton>>,
    children: Query<&Children>,
) {
    let fresh = lines.iter().any(|(_, _, line)| line.is_added());
    if !fresh && !updater.as_ref().is_some_and(|u| u.is_changed()) {
        return;
    }
    let words = version_line(updater.as_deref());
    let offer = updater.as_deref().filter(|u| u.actionable());
    for (sidebar, mut text, _) in &mut lines {
        if text.0 != words {
            text.0.clone_from(&words);
        }
        let placed: Vec<Entity> = buttons.iter().filter(|(_, p)| p.parent() == sidebar.parent()).map(|(e, _)| e).collect();
        let check_placed: Vec<Entity> = checks.iter().filter(|(_, p)| p.parent() == sidebar.parent()).map(|(e, _)| e).collect();
        match offer {
            Some(updater) if placed.is_empty() => {
                // Above the quiet control when it is already there: the offer is the primary thing on the home.
                let new = commands.spawn(crate::update::update_button(updater.label())).id();
                let at = children.get(sidebar.parent()).ok().and_then(|kids| kids.iter().position(|kid| check_placed.contains(&kid)));
                match at {
                    Some(at) => commands.entity(sidebar.parent()).insert_children(at, &[new]),
                    None => commands.entity(sidebar.parent()).add_child(new),
                };
            }
            Some(_) => {}
            None => {
                for button in placed {
                    commands.entity(button).despawn();
                }
            }
        }
        // **The quiet control is the sidebar's last row**, under the button that takes an update when there is one: the
        // offer is the primary thing on the home and the way to look again is not.
        match updater.as_deref().filter(|u| u.offers_a_check()) {
            Some(updater) if check_placed.is_empty() => {
                commands.entity(sidebar.parent()).with_child(crate::update::check_button(updater.check_label()));
            }
            Some(_) => {}
            None => {
                for button in check_placed {
                    commands.entity(button).despawn();
                }
            }
        }
    }
}

/// What a card or a page needs to be drawn, bundled so the builders stay within the argument budget.
struct Art<'a> {
    splash: Option<(Handle<Image>, Vec2)>,
    images: &'a mut Assets<Image>,
    /// The decoded portraits, so a redraw reuses a texture the GPU already has ([`ProjectPictures`]).
    pictures: &'a mut ProjectPictures,
    now: u64,
    menu: Entity,
}

#[allow(clippy::too_many_arguments)]
fn fill_the_home(
    mut commands: Commands,
    launcher: Res<Launcher>,
    splash: Res<SplashArt>,
    updater: Option<Res<crate::update::Updater>>,
    mut images: ResMut<Assets<Image>>,
    mut pictures: ResMut<ProjectPictures>,
    root: Query<Entity, With<LauncherRoot>>,
    sidebar: Query<Entity, With<SidebarRoot>>,
    content: Query<Entity, With<HomeContent>>,
) {
    let (Ok(root), Ok(sidebar), Ok(content)) = (root.single(), sidebar.single(), content.single()) else { return };
    let menu = commands.spawn((context_menu(()), CardMenu, ChildOf(root))).id();
    spawn_card_menu(&mut commands, menu);

    fill_sidebar(&mut commands, sidebar, &launcher, &splash, updater.as_deref());

    let mut art = Art { splash: splash.0.clone(), images: &mut images, pictures: &mut pictures, now: recent::now(), menu };
    commands.entity(content).with_children(|content| {
        content.spawn(bevy_gooey::fill_region()).with_children(|pages| {
            pages
                .spawn((bevy_gooey::fill_region_with(crate::chrome::GAP_GROUP_STEPS), bevy_gooey::Shown(true), HomePage(HomeTab::Projects)))
                .with_children(|page| {
                    page.spawn((bevy_gooey::fill_region_with(crate::chrome::GAP_GROUP_STEPS), ProjectsBody))
                        .with_children(|body| projects_body(body, &launcher, &mut art));
                });
            pages
                .spawn((bevy_gooey::column(crate::chrome::GAP_GROUP_STEPS), bevy_gooey::Shown(false), HomePage(HomeTab::Learn)))
                .with_children(|page| learn_page(page, &launcher));
            pages
                .spawn((bevy_gooey::fill_region_with(crate::chrome::GAP_ROW_STEPS), bevy_gooey::Shown(false), HomePage(HomeTab::WhatsNew)))
                .with_children(|page| {
                    // FOLLOW-OK: the entries are focusable buttons in a column, not a list with a
                    // highlight of its own — gooey's focus navigation scrolls a focused entry into
                    // view and the wheel scrolls the rest, which is what `ProjectGrid` beside this
                    // page relies on for the same reason. There is no selection here for anything to
                    // follow: the page exists so its rows can be read to the end, which is the
                    // capture's complaint (`feedback-1793905912313`).
                    //
                    // **The page scrolls, the way the Projects grid does.** Ten entries with their
                    // pictures are taller than the box a 1280 x 703 window leaves, and before this
                    // the rows below the third were drawn past the window with nothing to bring them
                    // in. `scroll_list` is gooey's viewport plus its bar, and the bar shows itself
                    // only when there is somewhere to go.
                    crate::chrome::scroll_list(page, NewsScroll).with_children(|scroll| {
                        scroll.spawn(bevy_gooey::column(crate::chrome::GAP_ROW_STEPS))
                            .with_children(|body| news_page(body, &launcher, &mut art));
                    });
                });
        });
    });
}

/// The Projects page: the welcome on a first run, else the header, the filter and the cards.
fn projects_body(body: &mut ChildSpawnerCommands, launcher: &Launcher, art: &mut Art) {
    if launcher.rows.is_empty() {
        welcome(body, launcher, art);
        return;
    }
    crate::chrome::gapped_row(body, crate::chrome::GAP_GROUP_STEPS, crate::chrome::NO_MARGIN, AlignItems::Center).with_children(|header| {
        header.spawn(heading("Projects"));
        header.spawn(bevy_gooey::spacer());
        // The one accent button of the page: New project, or, with nothing else to start from, nothing.
        header.spawn((submit_button("New project"), observe(|_: On<Activate>, mut launcher: ResMut<Launcher>| launcher.new_project_asked = true)));
        header.spawn((button("Open folder…"), observe(|_: On<Activate>, mut out: MessageWriter<PickRequest>| { out.write(PickRequest(PickFor::OpenFolder)); })));
    });
    body.spawn((
        text_input("", launcher.filter.clone()),
        TextHint("Filter projects".into()),
        observe(|change: On<ValueChange<String>>, mut launcher: ResMut<Launcher>| launcher.filter = change.value.clone()),
    ));
    // FOLLOW-OK: the cards are a grid of focusable buttons, not a list with a highlight of its own: gooey's focus navigation
    // scrolls a focused card into view, and the wheel scrolls the rest.
    // **Type-select (UI-R04, #334):** the cards are a group, so typing a project's first letters moves to it, as it does in a list.
    crate::chrome::scroll_list(body, (ProjectGrid, bevy_gooey::TypeSelectGroup)).with_children(|scroll| {
        crate::chrome::wrapping_row(scroll, crate::chrome::GAP_GROUP_STEPS, crate::chrome::NO_MARGIN, AlignItems::Stretch).with_children(|grid| {
            for row in &launcher.rows {
                if row.missing {
                    missing_card(grid, row);
                } else {
                    project_card(grid, row, art);
                }
            }
        });
    });
}

/// The first run: what this is, what it does, the one accent action, and how it works in three steps.
///
/// # Why it is shaped this way (design review, 2026-11-05)
///
/// The shipped 0.2.4 screen was a centred block whose loudest element was the splash and whose title was
/// **the same 10 px as the sentence under it** (`chrome::text::HEADING` and `chrome::text::LABEL` are both 10), while the
/// sidebar's own nav buttons were 13. FiraMono ships one weight, so size is the only lever a title has — and
/// it was not being used. Four changes, each a finding:
///
/// - **The type was already sane, and the review's first pass said otherwise.** A whole-line band made the
///   title and the prose look identical; a single glyph against a known baseline (`editor_skin.yaml`'s
///   `text_size: 11.0`) measures them at 15 and 11, and the sidebar at 11. So the title and prose stay on
///   gooey's own `heading`/`label` builders, and the claim is withdrawn rather than "fixed" by a no-op.
/// - **The copy says what Cauldron is for.** The old sentence named a grimoire without defining it, and the
///   screen never said *Bevy* — the one fact that tells a person whether their engine is supported.
/// - **The column is a column**: 560 px, top-aligned rather than centred in 1160, so the space above the
///   splash is a margin rather than 102 logical px of void, and the prose has a measure instead of one
///   508-px line.
/// - **The three steps answer "then what?"** in the space the centring left empty, and **Take the tour** —
///   the actual onboarding, which was two clicks away under Learn — is on the screen a first-timer lands on.
fn welcome(body: &mut ChildSpawnerCommands, launcher: &Launcher, art: &Art) {
    body.spawn((
        bevy_gooey::column_with(crate::chrome::GAP_GROUP_STEPS, bevy_gooey::RowLayout { align: Some(AlignItems::FlexStart), ..default() }),
        bevy_gooey::Sizing { max_width: Some(bevy_gooey::Extent::Px(WELCOME_COLUMN_PX)), ..default() },
    ))
    .with_children(|card| {
        if let Some((handle, size)) = art.splash.as_ref() {
            card.spawn((
                bevy_gooey::aspect_image(handle.clone(), size.x * WELCOME_SPLASH_SCALE, size.x / size.y),
                Name::new("welcome splash"),
            ));
        }
        card.spawn(heading("Welcome to Cauldron"));
        card.spawn((label(WELCOME_PITCH), TextLayout::new(Justify::Left, LineBreak::WordBoundary)));
        crate::chrome::gapped_row(card, crate::chrome::GAP_GROUP_STEPS, crate::chrome::NO_MARGIN, AlignItems::Stretch).with_children(|actions| {
            let example = launcher.example.is_some();
            if example {
                actions.spawn((submit_button("Open the example"), observe(open_the_example)));
            }
            let new = observe(|_: On<Activate>, mut launcher: ResMut<Launcher>| launcher.new_project_asked = true);
            if example {
                actions.spawn((button("New project"), new));
            } else {
                actions.spawn((submit_button("New project"), new));
            }
            actions.spawn((button("Open folder…"), observe(|_: On<Activate>, mut out: MessageWriter<PickRequest>| { out.write(PickRequest(PickFor::OpenFolder)); })));
        });
        card.spawn(bevy_gooey::column(crate::chrome::GAP_ROW_STEPS)).with_children(|steps| {
            for (number, step) in WELCOME_STEPS.iter().enumerate() {
                steps.spawn((
                    Text::new(format!("{}. {step}", number + 1)),
                    crate::chrome::typed(crate::chrome::text::LABEL),
                    TextColor(quiet_ink(None)),
                    TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                ));
            }
        });
        if launcher.example.is_some() {
            card.spawn((button("Take the tour"), observe(|activate: On<Activate>, launcher: ResMut<Launcher>, out: MessageWriter<OpenMapRequest>, plain: MessageWriter<OpenRequest>, mut commands: Commands| {
                commands.insert_resource(TourRequested);
                open_the_example(activate, launcher, out, plain);
            })));
        }
        card.spawn(quiet("Open folder… reads an existing project. Nothing on disk changes until you save."));
    });
}

/// **What Cauldron is, in the sentence a first run has room for.**
///
/// Named: the engine and its version, the two nouns this editor actually works in (pieces, maps), the
/// engine, and the one capability that is not in Unity, Tiled or an art tool — an agent that edits the same
/// map, through the same operations, beside the person. The old sentence named a *grimoire* and defined
/// nothing, and the screen never said *Bevy*.
///
/// **The folder's path is deliberately not here.** The first draft of this sentence named `assets/grimoire`
/// and `no_internal_words_on_screen` refused it: a path under `assets/` is a word only the editor's makers
/// know, and a person never types it. What they need to know is that there is **no export step**, which is
/// the property; where the file lands is the editor's business.
const WELCOME_PITCH: &str = "A level editor for Bevy 0.20. Put your own meshes on a map and Cauldron \
saves it in a form your game reads directly — no export step. An AI agent can edit the same map beside you.";

/// **What happens after the first click**, which the old screen never said. Three steps, in order.
const WELCOME_STEPS: [&str; 3] = [
    "Open the example — it is copied to your own folder, so nothing you do is lost.",
    "Place pieces from the library, or add your own .glb meshes.",
    "Save the map; your game reads it directly, with nothing to export.",
];

/// The welcome column's measure, in logical px. Wide enough for the pitch on two comfortable lines,
/// narrow enough that it reads as a column rather than as a page-wide bar — the shipped screen set one
/// 508 px line in a 1160 px page.
const WELCOME_COLUMN_PX: f32 = 560.0;

/// Opens the example project on its map: the one beside the source, the person's own copy, or the app's copy made theirs first.
fn open_the_example(_: On<Activate>, mut launcher: ResMut<Launcher>, mut out: MessageWriter<OpenMapRequest>, mut plain: MessageWriter<OpenRequest>) {
    match launcher.example.clone().map(|example| example_to_open(&example)) {
        None => {}
        Some(Ok(root)) => match example_map(&root) {
            Some(map) => {
                out.write(OpenMapRequest(root, map));
            }
            None => {
                plain.write(OpenRequest(root));
            }
        },
        Some(Err(e)) => launcher.problem = Some(e),
    }
}

/// The map the example opens on: `bedroom` when it has one, else its first.
const EXAMPLE_MAP: &str = "bedroom";

fn example_map(root: &Path) -> Option<String> {
    let maps = crate::catalog::Catalog::scan(root).ok()?.maps;
    maps.iter().find(|m| m.name == EXAMPLE_MAP).or(maps.first()).map(|m| m.name.clone())
}

/// The picture on a card: the newest portrait of a piece the project has baked (`thumbs.rs` keeps them in
/// `target/grimoire_thumbs`), cropped to the card's shape; else the splash, cropped the same way.
///
/// **`pictures` is what keeps a node's texture already on the GPU** — see [`ProjectPictures`]. Every
/// caller passes it; there is no second way to get a picture.
pub(crate) fn project_picture(root: &Path, images: &mut Assets<Image>, splash: Option<(Handle<Image>, Vec2)>, pictures: &mut ProjectPictures) -> Option<ImageNode> {
    let newest = std::fs::read_dir(root.join("target/grimoire_thumbs"))
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max();
    // **The same file, unchanged, is the same handle.** `modified` is the key's other half because a
    // portrait baked again under the same name is a different picture.
    let (handle, size) = match newest {
        Some((modified, path)) => {
            let key = (path.clone(), modified);
            match pictures.0.get(&key) {
                Some(cached) => cached.clone(),
                None => {
                    let bytes = std::fs::read(&path).ok()?;
                    let image = Image::from_buffer(&bytes, ImageType::Extension("png"), CompressedImageFormats::NONE, true, ImageSampler::Default, RenderAssetUsages::default()).ok()?;
                    let size = Vec2::new(image.width() as f32, image.height() as f32);
                    let made = (images.add(image), size);
                    pictures.0.insert(key, made.clone());
                    made
                }
            }
        }
        // The splash is one handle `SplashArt` keeps for the life of the process: it needs no cache.
        None => splash?,
    };
    // The middle band of the picture that is as wide as the card's picture is for its height.
    let (w, h) = (size.x, size.y);
    let band = (w / THUMB.x * THUMB.y).min(h);
    let wide = (band / THUMB.y * THUMB.x).min(w);
    let rect = Rect::from_center_size(Vec2::new(w / 2.0, h / 2.0), Vec2::new(wide, band));
    Some(ImageNode::new(handle).with_rect(rect))
}

/// The scale a project card is drawn at while it is pressed: gooey's `PRESS_SCALE`, named here because the card asks for it.
const CARD_PRESS_SCALE: f32 = 0.97;

/// A page's ground (G-87): a surface step with `STEP_PAD` all round and its children `GAP_GROUP` px apart, in a column (a surface writes only a colour).
fn page_surface(level: u8) -> impl Bundle {
    (
        bevy_gooey::surface(level),
        bevy_gooey::Spacing { padding: bevy_gooey::Sides::all(crate::chrome::STEP_PAD), ..default() },
        bevy_gooey::GapPx(crate::chrome::GAP_GROUP),
    )
}

fn project_card(grid: &mut ChildSpawnerCommands, row: &Listed, art: &mut Art) {
    let path = row.project.path.clone();
    let picture = project_picture(&path, art.images, art.splash.clone(), art.pictures);
    let when = format!("Opened {}", ago(row.project.last_opened, art.now));
    let key = card_key(&path);
    grid.spawn((
        // CHROME-OK: a card is none of chrome's three clickables, it is a gooey button's box holding a picture (bevy_gooey#41)
        // FOCUS-DECISION-REOPENED: a card is a button like gooey's own, reached by Tab and the arrows (`button_box` is a tab stop) on a screen no editor chord routes
        card_button(),
        ContextMenu(art.menu),
        ProjectCard { path: path.clone(), key },
        // **A card is drawn a little smaller under the press** (UI-R04, #334), as gooey's own card cells are: a card here is a button
        // holding a picture, not gooey's `CardCell`, so it asks for the scale itself.
        bevy_gooey::PressScale(CARD_PRESS_SCALE),
        observe(on_card),
    ))
    .with_children(|card| {
        if let Some(picture) = picture {
            card.spawn(thumb_box(picture.image.clone())).insert((picture, card_line()));
        }
        card.spawn((Text::new(name_of(&path)), Role::ButtonText, card_line(), Pickable::IGNORE));
        card.spawn((quiet(shown_folder(&path)), Role::Sized, card_line(), Pickable::IGNORE));
        card.spawn((quiet(when), Pickable::IGNORE));
    });
}

fn missing_card(grid: &mut ChildSpawnerCommands, row: &Listed) {
    let path = row.project.path.clone();
    let key = card_key(&path);
    grid.spawn((card_layout(), MissingCard { path: path.clone(), key }))
    .insert(Role::Card)
    .with_children(|card| {
        card.spawn(quiet(name_of(&path)));
        card.spawn(label("Folder not found"));
        card.spawn(quiet(shown_folder(&path)));
        crate::chrome::gapped_row(card, crate::chrome::GAP_ROW_STEPS, crate::chrome::NO_MARGIN, AlignItems::Stretch).with_children(|actions| {
            let locate = path.clone();
            actions.spawn((button("Locate…"), observe(move |_: On<Activate>, mut out: MessageWriter<PickRequest>| { out.write(PickRequest(PickFor::Locate(locate.clone()))); })));
            let gone = path.clone();
            actions.spawn((
                button("Remove"),
                observe(move |_: On<Activate>, recents: Option<Res<RecentProjects>>, mut launcher: ResMut<Launcher>| remove_from_list(recents.as_deref(), &mut launcher, &gone)),
            ));
        });
    });
}

/// A card was activated: **it opens that folder** (feedback-1794103491165, #344).
///
/// It used to take two presses — the first choosing the card and only the rail saying so, the second
/// opening it — and a person read the second press as the app having ignored the first. Every other
/// control on the home (`New project`, `Open folder…`, `Open the example`, `Take the tour`) acts on one
/// press, and so did a card pressed from the keyboard, which is what says the two-press step was never
/// a design: it was an invisible mode. Opening is not destructive and the card already shrinks under
/// the press, so one press is enough.
///
/// **The rail is the keyboard's** ([`mark_the_focus`]): it marks the card with input focus or the
/// pointer, which is a fact about the screen rather than a place a click is remembered.
fn on_card(activate: On<Activate>, cards: Query<&ProjectCard>, mut out: MessageWriter<OpenRequest>) {
    let Ok(card) = cards.get(activate.entity) else { return };
    out.write(OpenRequest(card.path.clone()));
}

/// Takes `path` out of the recent list (the folder is untouched) and draws the cards again.
fn remove_from_list(recents: Option<&RecentProjects>, launcher: &mut Launcher, path: &Path) {
    match edit_recent(recents, |r| {
        r.remove(path);
        Ok(())
    }) {
        Ok(rows) => {
            launcher.rows = rows;
            launcher.dirty = true;
        }
        Err(e) => launcher.problem = Some(e),
    }
}

/// Reads the recent list, applies `edit`, writes it back, and returns the list as shown.
fn edit_recent(recents: Option<&RecentProjects>, edit: impl FnOnce(&mut Recent) -> Result<(), String>) -> Result<Vec<Listed>, String> {
    let Some(file) = recents.map(|r| &r.0) else { return Err("this run keeps no list of projects".to_owned()) };
    let mut list = Recent::load(file)?;
    edit(&mut list)?;
    list.save(file)?;
    Ok(list.listing())
}

/// What a menu item of a card does.
#[derive(Clone, Copy)]
enum Verb {
    Open,
    Show,
    CopyLine,
    Remove,
}

/// "Show in Finder" on a Mac, "Show in Files" elsewhere.
const SHOW: &str = if cfg!(target_os = "macos") { "Show in Finder" } else { "Show in Files" };

fn spawn_card_menu(commands: &mut Commands, menu: Entity) {
    for (label, verb) in [("Open", Verb::Open), (SHOW, Verb::Show), ("Copy command line", Verb::CopyLine), ("Remove from list", Verb::Remove)] {
        if matches!(verb, Verb::Remove) {
            commands.spawn((separator(), ChildOf(menu)));
        }
        commands.spawn((menu_item(label), ChildOf(menu))).observe(on_card_verb(verb));
    }
}

#[allow(clippy::type_complexity)]
fn on_card_verb(
    verb: Verb,
) -> impl FnMut(On<Activate>, Query<&ChildOf>, Query<&ContextTarget>, Query<&ProjectCard>, Option<Res<RecentProjects>>, Option<Res<ClipboardOff>>, ResMut<Launcher>, MessageWriter<OpenRequest>, Commands) {
    move |activate, parents, targets, cards, recents, off, mut launcher, mut out, mut commands| {
        let Some(card) = ContextTarget::of(activate.entity, &parents, &targets).and_then(|e| cards.get(e).ok()) else { return };
        match verb {
            Verb::Open => {
                out.write(OpenRequest(card.path.clone()));
            }
            Verb::Show => {
                if let Err(e) = open_in_os(&card.path.to_string_lossy()) {
                    launcher.problem = Some(e);
                }
            }
            Verb::CopyLine => {
                let line = command_line(&card.path, None, None);
                let copied = if off.is_some() { Err("no clipboard in this run".to_owned()) } else { copy_to_clipboard(&line) };
                match copied {
                    Ok(()) => commands.spawn(toast(format!("Copied: {line}"))),
                    Err(e) => commands.spawn(toast_alert(format!("The clipboard did not take it ({e}). The line is: {line}")).sticky()),
                };
            }
            Verb::Remove => remove_from_list(recents.as_deref(), &mut launcher, &card.path),
        }
    }
}

/// Opens a folder or an address with the system's own opener, without waiting for it.
pub fn open_in_os(target: &str) -> Result<(), String> {
    let (program, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("open", &[])
    } else if cfg!(target_os = "windows") {
        ("explorer", &[])
    } else {
        ("xdg-open", &[])
    };
    let mut child = std::process::Command::new(program).args(args).arg(target).spawn().map_err(|e| format!("could not run {program}: {e}"))?;
    // Reaped on its own thread, so the opener never lingers as a zombie of this process.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Puts `text` on the OS clipboard, as `notice.rs`'s Cmd+C does.
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    arboard::Clipboard::new().and_then(|mut c| c.set_text(text)).map_err(|e| e.to_string())
}

/// The Learn page: the tour, the example, the written guide and the keys.
fn learn_page(page: &mut ChildSpawnerCommands, launcher: &Launcher) {
    page.spawn(heading("Learn"));
    let example = launcher.example.is_some();
    page.spawn(label("Start with a two-minute tour of the example, or read the guide."));
    if example {
        page.spawn((
            button("Take the tour"),
            observe(|activate: On<Activate>, mut commands: Commands, launcher: ResMut<Launcher>, out: MessageWriter<OpenMapRequest>, plain: MessageWriter<OpenRequest>| {
                commands.insert_resource(TourRequested);
                open_the_example(activate, launcher, out, plain);
            }),
        ));
        page.spawn((button("Open the example"), observe(open_the_example)));
    }
    page.spawn((button("Getting started guide"), observe(|_: On<Activate>, mut launcher: ResMut<Launcher>| {
        if let Err(e) = open_in_os(GUIDE_URL) {
            launcher.problem = Some(e);
        }
    })));
    page.spawn((button("Keyboard shortcuts"), observe(|_: On<Activate>, mut out: MessageWriter<crate::keymap_screen::OpenKeymap>| { out.write(crate::keymap_screen::OpenKeymap); })));
}

/// How many entries the What's new page lists; the rest are one press away on "See every change".
pub const NEWS_SHOWN: usize = 10;

/// **How wide an entry's prose column is**, in logical pixels: a readable measure, not the window's
/// width.
///
/// At the capture's own window (1280 logical) the page's rows ran 778 px of prose — about **118
/// characters** at this face's advance — and the note is *"These boxes like this are hard to read"*
/// (`feedback-1793905698793`). Bringhurst's comfortable measure for one column is 45–75 characters,
/// and past that the eye loses its place between one line and the next. At ~6.6 px a character
/// (FiraMono at 11 px), `MEASURE_CHARS` is 66 characters — inside that range, and fixed, so a wider
/// window buys page margin rather than a longer line.
pub const NEWS_MEASURE_PX: f32 = 440.0;

/// The still of an entry's clip, decoded, or `None` when it cannot be had: the changelog's stills are stored in Git LFS and are
/// beside the source a build came from or in the app's `Resources`, so an install without them lists the entry without its picture.
fn still_image(rel: &str) -> Option<Image> {
    let beside_source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
    let in_the_app = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join("../Resources").join(rel)));
    let bytes = std::iter::once(beside_source).chain(in_the_app).find_map(|p| std::fs::read(p).ok())?;
    // An LFS pointer or a damaged file does not decode, which is the same as no still.
    Image::from_buffer(&bytes, ImageType::Extension("png"), CompressedImageFormats::NONE, true, ImageSampler::Default, RenderAssetUsages::default()).ok()
}

/// The What's new page: every change worth telling a person about, in a sentence and with the picture of its clip. A press opens the
/// changelog on GitHub. The ones added since a project was last opened here are marked, and counted on the sidebar.
fn news_page(page: &mut ChildSpawnerCommands, launcher: &Launcher, art: &mut Art) {
    page.spawn(heading("What's new"));
    if launcher.news.is_empty() {
        page.spawn(label("Nothing to tell you about yet."));
    }
    for item in launcher.news.iter().take(NEWS_SHOWN) {
        let Some(sentence) = item.user.clone() else { continue };
        let fresh = launcher.whats_new.iter().any(|n| n.id == item.id);
        let picture = item.still.as_deref().and_then(still_image).map(|image| art.images.add(image));
        page.spawn((
            // CHROME-OK: an entry is none of chrome's three clickables, it is a gooey button's box holding a picture (bevy_gooey#41): the button's skin pads it and puts its half gap between
            // its parts, as it did the raw node (the 16 px gap and 12 px padding the node asked for were overwritten).
            bevy_gooey::button_box(),
            bevy_gooey::RowLayout { align: Some(AlignItems::Center), ..default() },
            NewsEntry(item.id.clone()),
            observe(|_: On<Activate>, mut launcher: ResMut<Launcher>| {
                if let Err(e) = open_in_os(CHANGELOG_URL) {
                    launcher.problem = Some(e);
                }
            }),
        ))
        .with_children(|entry| {
            if let Some(handle) = picture {
                entry.spawn(thumb_box(handle));
            }
            // **The prose keeps a measure, and it has the room to.** `flex_shrink` alone let the column
            // take whatever the window gave it — 118 characters a line at the capture's own width —
            // so it is bounded to `NEWS_MEASURE_PX` and the row's leftover is margin
            // (`feedback-1793905698793`).
            entry.spawn((
                bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(1.0), ..default() }),
                bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Px(NEWS_MEASURE_PX)), max_width: Some(bevy_gooey::Extent::Px(NEWS_MEASURE_PX)), ..default() },
            ))
            .with_children(|words| {
                if fresh {
                    words.spawn((Text::new("New"), Role::Label, Pickable::IGNORE));
                }
                words.spawn((Text::new(sentence), Role::ButtonText, Pickable::IGNORE));
            });
        });
    }
    page.spawn((button("See every change"), observe(|_: On<Activate>, mut launcher: ResMut<Launcher>| {
        if let Err(e) = open_in_os(CHANGELOG_URL) {
            launcher.problem = Some(e);
        }
    })));
}

/// A What's new entry, by the changelog id it came from.
#[derive(Component)]
pub struct NewsEntry(pub String);

/// **The What's new page's scroll area** (`feedback-1793905912313`): the viewport `chrome::scroll_list`
/// makes for the page's entries, so a test can find the box the page scrolls in.
#[derive(Component)]
pub struct NewsScroll;

/// The ways a request to open a project ends: a toast that says why not, or the project opened in this process
/// ([`crate::project_assets::open_project`]), on a map when the request names one.
fn open_requested(
    mut commands: Commands,
    mut requests: MessageReader<OpenRequest>,
    mut on_a_map: MessageReader<OpenMapRequest>,
    mut launcher: ResMut<Launcher>,
    recents: Option<Res<RecentProjects>>,
) {
    let wanted = on_a_map.read().last().map(|OpenMapRequest(root, map)| (root.clone(), Some(map.clone()))).or_else(|| requests.read().last().map(|OpenRequest(root)| (root.clone(), None)));
    let Some((root, map)) = wanted else { return };
    match check_project(&root) {
        Ok(root) => {
            launcher.problem = None;
            if let Some(file) = recents.as_deref().map(|r| &r.0)
                && let Err(e) = recent::record_seen(file, &root, launcher.newest.as_deref())
            {
                warn!("recent projects not updated: {e}");
            }
            commands.queue(move |world: &mut World| {
                if let Err(e) = crate::project_assets::open_project(world, &root, map.as_deref()) {
                    world.spawn(toast_alert(e).sticky());
                }
            });
        }
        // A folder that is not a project can become one in place; anything else is only said.
        Err(_) if root.is_dir() => {
            commands.spawn(toast_alert("That folder is not a Cauldron project. Make it one?").action("Make it a project", format!("{MAKE_PROJECT}{}", root.display())).sticky());
        }
        Err(e) => {
            commands.spawn(toast_alert(e).sticky());
        }
    }
}

/// The id of the toast button that makes the folder after it a project.
const MAKE_PROJECT: &str = "make-project:";

/// "Make it a project" was pressed: the folder gets what a project needs, keeps everything it held, and opens.
fn on_toast_action(action: On<ToastAction>, mut commands: Commands, mut out: MessageWriter<OpenRequest>) {
    let Some(folder) = action.id.strip_prefix(MAKE_PROJECT).map(PathBuf::from) else { return };
    match crate::catalog::make_project_here(&folder, crate::new_project::VOCAB) {
        Ok(()) => {
            out.write(OpenRequest(folder));
        }
        Err(e) => {
            commands.spawn(toast_alert(e).sticky());
        }
    }
}

/// An answer from the file chooser: Open folder… opens, Locate… repoints the card.
fn on_picked(mut picked: MessageReader<Picked>, recents: Option<Res<RecentProjects>>, mut launcher: ResMut<Launcher>, mut out: MessageWriter<OpenRequest>) {
    for Picked { for_what, paths } in picked.read() {
        let Some(path) = paths.first() else { continue };
        match for_what {
            PickFor::OpenFolder => {
                out.write(OpenRequest(path.clone()));
            }
            PickFor::Locate(old) => match edit_recent(recents.as_deref(), |r| r.locate(old, path)) {
                Ok(rows) => {
                    launcher.rows = rows;
                    launcher.dirty = true;
                }
                Err(e) => launcher.problem = Some(e),
            },
            // The new-project form's Location and the editor's Add meshes… take their own answers.
            PickFor::Location | PickFor::AddMeshes => {}
        }
    }
}

/// **Says what the one-time upgrade of a person's own files did**, once, on the first screen they see.
///
/// `legacy::upgrade_user_files` converts `settings.ron`, `recent.ron` and `trusted.ron` at startup and answers a sentence per
/// file. It also goes to the log, but a bundle launch has no terminal — and a person whose keymap was just carried over should
/// be told rather than discover it. The resource is taken as it is read, so the notices are said once and not every entry.
fn say_the_startup_notices(mut commands: Commands, mut notices: Option<ResMut<crate::legacy::StartupNotices>>) {
    let Some(notices) = notices.as_mut() else { return };
    // **`toast_alert`, because every sentence is one a person has to act on**: `settings.ron` was converted (their keymap is
    // there), or it could not be and this build ignores it. It stands until dismissed, which is what a message about a file on
    // their disk deserves.
    for notice in notices.0.drain(..) {
        commands.spawn(toast_alert(notice).sticky());
    }
}

/// Says each new refusal in a toast. The text stays in [`Launcher::problem`] until the next choice clears it.
fn say_the_problem(mut commands: Commands, launcher: Res<Launcher>, mut said: Local<Option<String>>) {
    if !launcher.is_changed() {
        return;
    }
    if launcher.problem != *said {
        if let Some(problem) = launcher.problem.clone() {
            commands.spawn(toast_alert(problem).sticky());
        }
        said.clone_from(&launcher.problem);
    }
}

/// Draws the Projects body again after the cards changed.
fn redraw_the_cards(
    mut commands: Commands,
    mut launcher: ResMut<Launcher>,
    splash: Res<SplashArt>,
    mut images: ResMut<Assets<Image>>,
    mut pictures: ResMut<ProjectPictures>,
    body: Query<Entity, With<ProjectsBody>>,
    menu: Query<Entity, With<CardMenu>>,
) {
    if !launcher.dirty {
        return;
    }
    launcher.dirty = false;
    let (Ok(body), Ok(menu)) = (body.single(), menu.single()) else { return };
    let mut art = Art { splash: splash.0.clone(), images: &mut images, pictures: &mut pictures, now: recent::now(), menu };
    commands.entity(body).despawn_children();
    commands.entity(body).with_children(|body| projects_body(body, &launcher, &mut art));
}

/// Hides the cards the filter does not match, by [`fuzzy_score`] over each one's name and folder.
/// A card of either kind, with the words the filter reads.
type Filterable = (&'static mut Node, Option<&'static ProjectCard>, Option<&'static MissingCard>);

#[allow(clippy::type_complexity)]
fn apply_filter(launcher: Res<Launcher>, mut cards: Query<Filterable, Or<(With<ProjectCard>, With<MissingCard>)>>) {
    let query = launcher.filter.trim();
    for (mut node, project, missing) in &mut cards {
        let key = project.map(|c| c.key.as_str()).or(missing.map(|c| c.key.as_str())).unwrap_or_default();
        let show = query.is_empty() || fuzzy_score(query, key).is_some();
        let display = if show { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
}

/// Shows the page the sidebar chose, and marks its row.
fn show_the_tab(launcher: Res<Launcher>, mut pages: Query<(&HomePage, &mut bevy_gooey::Shown)>, nav: Query<(Entity, &NavTab, Has<Selected>)>, mut commands: Commands) {
    for (page, mut shown) in &mut pages {
        let on = page.0 == launcher.tab;
        if shown.0 != on {
            shown.0 = on;
        }
    }
    for (entity, tab, selected) in &nav {
        match (tab.0 == launcher.tab, selected) {
            (true, false) => {
                commands.entity(entity).insert(Selected);
            }
            (false, true) => {
                commands.entity(entity).remove::<Selected>();
            }
            _ => {}
        }
    }
}

/// **Marks the card the keyboard or the pointer is on**, so `Selected`'s rail keeps saying *this one*
/// without a press having to be remembered (feedback-1794103491165, #344).
///
/// It replaces `mark_the_selection`, which compared each card against `Launcher::selected` — a field
/// that existed for no other reason than the choose-then-open step, and that went with it. The two
/// facts a card can honestly be marked for are the ones the screen already holds: `InputFocus` (Tab or
/// an arrow put it there) and `Hovered` (the pointer is on it). A card that has lost both loses the
/// rail, so nothing is left behind to make the next press look like a second one.
fn mark_the_focus(
    focus: Option<Res<bevy::input_focus::InputFocus>>,
    cards: Query<(Entity, &Hovered, Has<Selected>), With<ProjectCard>>,
    mut commands: Commands,
) {
    let focused = focus.as_deref().and_then(|focus| focus.get());
    for (entity, hovered, selected) in &cards {
        let want = hovered.get() || focused == Some(entity);
        match (want, selected) {
            (true, false) => {
                commands.entity(entity).insert(Selected);
            }
            (false, true) => {
                commands.entity(entity).remove::<Selected>();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The app carries the stills the What's new page lists**: `build_app.sh` copies the stills of the newest entries with a For you
    /// sentence into the app's Resources, and how many is [`NEWS_SHOWN`] written in the script. When the two differ the page lists
    /// entries whose pictures the app does not hold.
    #[test]
    fn the_release_script_copies_as_many_stills_as_the_page_lists() {
        let script = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/release/macos/build_app.sh")).unwrap_or_else(|e| panic!("{e}"));
        let line = format!("NEWS_SHOWN={NEWS_SHOWN} ");
        assert!(script.contains(&line), "build_app.sh must say `{}` so the app holds what the page shows", line.trim());
    }
}
