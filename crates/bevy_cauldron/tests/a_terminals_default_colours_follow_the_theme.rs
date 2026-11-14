//! **A terminal's default colours follow the theme the frame it changes, and what a program chose stays what it chose** (#428 x #421).
//!
//! The emulator keeps a cell's colour as the program wrote it (`default foreground`, ANSI slot 1, a 24-bit value) and the screen
//! resolves it into the rows it hands the pane. Those rows were rebuilt only when the emulator was dirty, so after a theme switch
//! every row already drawn kept the old theme's text and ground until the program redrew it: a prompt in Tool dark's light text on
//! Parchment's cream. A screen now notices that the live ink is not the ink it last built with, and builds again.
//!
//! What a program chose is not the theme's to repaint: a 24-bit colour stays exactly as it was written, and an ANSI slot is the
//! editor's own word for it (slot 1 is the ink's danger), so it is the running theme's, and never the default text colour.

use bevy::prelude::*;
use bevy_cauldron::chrome::Ink;
use bevy_cauldron::settings::{Theme, UserSettingsFile};
use bevy_cauldron::terminal::TerminalScreen;
use bevy_gooey::GooeySkin;

use crate::fixtures;

/// Seconds the file watcher's wall-clock poll is given to see a settings file change.
const WATCH_PATIENCE_S: u64 = 10;
/// How far two sRGB channels may differ and still be the same colour: under one 8-bit step.
const SAME: f32 = 1.0e-3;

fn named(theme: Theme) -> &'static str {
    match theme {
        Theme::ToolDark => "ToolDark",
        Theme::Parchment => "Parchment",
        Theme::HighContrast => "HighContrast",
    }
}

fn same(a: Color, b: Color) -> bool {
    let (a, b) = (Srgba::from(a), Srgba::from(b));
    (a.red - b.red).abs() <= SAME && (a.green - b.green).abs() <= SAME && (a.blue - b.blue).abs() <= SAME
}

fn ink_of(theme: Theme) -> Ink {
    Ink::of(&bevy_cauldron::ui::skin_for(theme))
}

fn editor(tag: &str, theme: Theme) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let file = root.join(format!("user_config_{tag}_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(file.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(&file, format!("{{version: 1, theme: {}}}", named(theme))).expect("a settings file");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(file.clone()));
    app.insert_resource(bevy_gooey::MotionSettings { duration_scale: 0.0, reduced: false });
    for _ in 0..12 {
        app.update();
    }
    (app, file)
}

fn choose(app: &mut App, file: &std::path::Path, theme: Theme) {
    std::fs::write(file, format!("{{version: 1, theme: {}}}", named(theme))).expect("a settings file");
    let want = bevy_cauldron::ui::skin_for(theme).surface;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(WATCH_PATIENCE_S);
    while app.world().resource::<GooeySkin>().surface != want && std::time::Instant::now() < deadline {
        app.update();
    }
    for _ in 0..6 {
        app.update();
    }
}

/// What the screen's first row holds: a plain word, a word in ANSI red, and a word in a 24-bit colour.
const PROGRAM: &str = "plain \x1b[31mred\x1b[0m \x1b[38;2;10;20;30mtrue\x1b[0m";

/// The colours of the word that starts at `column` of the screen's first row: `(ink, ground)`.
fn at(screen: &mut TerminalScreen, column: usize) -> (Color, Color) {
    let cell = screen.lines()[0].cells[column];
    (cell.fg, cell.bg)
}

#[test]
fn default_cells_take_the_new_themes_colours_and_chosen_ones_keep_theirs() {
    let (mut app, file) = editor("terminal_theme", Theme::ToolDark);
    let mut screen = TerminalScreen::new(40, 4);
    screen.feed(PROGRAM.as_bytes());

    let before = ink_of(Theme::ToolDark);
    let (plain_fg, plain_bg) = at(&mut screen, 0);
    assert!(same(plain_fg, before.text) && same(plain_bg, before.panel_bg), "Tool dark: the default cells are its text and ground");
    let (red_fg, _) = at(&mut screen, 6);
    assert!(same(red_fg, before.danger), "Tool dark: ANSI red is the ink's danger");
    let (true_fg, _) = at(&mut screen, 10);
    let written = Color::srgb_u8(10, 20, 30);
    assert!(same(true_fg, written), "the 24-bit colour is as written");
    assert!(!screen.take_dirty(), "a settled screen is not dirty");

    for theme in [Theme::Parchment, Theme::HighContrast, Theme::ToolDark] {
        choose(&mut app, &file, theme);
        let ink = ink_of(theme);
        // The pane asks `lines` every frame, and the screen is not fed: only the theme changed.
        let (plain_fg, plain_bg) = at(&mut screen, 0);
        assert!(same(plain_fg, ink.text), "{theme:?}: a default cell's text is the theme's text, not another theme's: {plain_fg:?}");
        assert!(same(plain_bg, ink.panel_bg), "{theme:?}: and its ground is the theme's panel: {plain_bg:?}");
        let (red_fg, _) = at(&mut screen, 6);
        assert!(same(red_fg, ink.danger), "{theme:?}: ANSI red stays the red slot, and is not the default text: {red_fg:?}");
        assert!(!same(red_fg, ink.text), "{theme:?}: red is not the text colour");
        let (true_fg, _) = at(&mut screen, 10);
        assert!(same(true_fg, written), "{theme:?}: the 24-bit colour a program wrote is untouched: {true_fg:?}");
        assert!(!screen.take_dirty(), "{theme:?}: and once rebuilt the screen is settled again");
    }
}
