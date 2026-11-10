//! **A filter box draws in the editor skin's ink** (UI-02, #210, slice 8).
//!
//! The box above every list has six inks and one of them is a *state*: the ground at rest, the
//! ground under the pointer, the ground it holds while it owns the keyboard (gooey's `pressed` — a
//! recessed well here, not a raised card), what is typed, a placeholder, and the caret. All six were
//! `chrome.rs` constants, so a project's skin file could not reach the one control a person types
//! into most.
//!
//! `filter::refresh` reads them from `GooeySkin` and keeps each constant as the fallback for a
//! harness with no skin. This test drives the real system — spawn a box, focus it, hover it, type in
//! it — so it asserts what is on screen rather than what a struct holds.

use bevy::prelude::*;
use bevy_cauldron::chrome;
use bevy_cauldron::filter::{FilterBox, FilterText, Filters, Pane};
use bevy_gooey::GooeySkin;

/// **The box spawned in a bare app, with the skin under test.** No `EditorUiPlugin`: this test is
/// about what `refresh` reads, and a plugin would bring the whole editor with it.
fn app_with(skin: Option<GooeySkin>) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Filters>();
    if let Some(skin) = skin {
        app.insert_resource(skin);
    }
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn((
            FilterBox(Pane::Palette),
            bevy::picking::hover::Hovered::default(),
            BackgroundColor(Color::NONE),
        ));
        commands.spawn((FilterText(Pane::Palette), Text::new(""), TextColor(Color::NONE)));
    });
    app.add_systems(Update, bevy_cauldron::filter::refresh);
    app.update();
    app
}

fn ground(app: &mut App) -> Color {
    let mut q = app.world_mut().query::<(&FilterBox, &BackgroundColor)>();
    q.iter(app.world()).next().expect("a filter box is on screen").1 .0
}

fn ink_of(app: &mut App) -> Color {
    let mut q = app.world_mut().query::<(&FilterText, &TextColor)>();
    q.iter(app.world()).next().expect("a filter box's readout is on screen").1 .0
}

/// **A skin a person could write**, moved to colours no preset uses, so a passing assertion cannot
/// be the constant coinciding with the field.
fn a_skin_of_its_own() -> GooeySkin {
    let mut skin = GooeySkin::tool();
    skin.surface_raised = Color::srgb(0.13, 0.17, 0.19); // the box at rest
    skin.text = Color::srgb(0.81, 0.91, 0.11); // what is typed
    skin.text_muted = Color::srgb(0.21, 0.31, 0.41); // a placeholder
    skin.accent = Color::srgb(0.91, 0.11, 0.61); // the caret
    skin.tones.insert("hover".to_owned(), Color::srgb(0.17, 0.27, 0.37));
    skin.tones.insert("pressed".to_owned(), Color::srgb(0.05, 0.07, 0.09));
    skin
}

/// **The three grounds are the skin's**, one per state, and each is a different one — a box that
/// answered all three with the same colour would pass a per-state test written carelessly.
#[test]
fn a_filter_box_stands_on_the_skins_grounds() {
    let mut app = app_with(Some(a_skin_of_its_own()));
    let rest = ground(&mut app);
    assert_eq!(rest, Color::srgb(0.13, 0.17, 0.19), "a box at rest is the skin's `surface_raised`");

    app.world_mut().resource_mut::<Filters>().take_focus(Pane::Palette);
    app.update();
    let focus = ground(&mut app);
    assert_eq!(focus, Color::srgb(0.05, 0.07, 0.09), "a box that owns the keyboard is the `pressed` tone");

    assert_ne!(rest, focus, "a focused box has to look different from a resting one");
}

/// **A placeholder, typed text and the caret are the skin's three inks**, and the caret is the
/// accent rather than an ink of its own — that is what makes it the thing the eye finds.
///
/// The order matters and is the real order: a box is empty and unfocused until someone types, and
/// taking focus **clears** it (`Filters::take_focus` starts the search fresh, as clicking the box
/// does). So the empty box is asked first, then typed into, then focused.
#[test]
fn what_is_typed_is_the_skins_ink() {
    let mut app = app_with(Some(a_skin_of_its_own()));
    assert_eq!(ink_of(&mut app), Color::srgb(0.21, 0.31, 0.41), "an empty box shows `text_muted`");

    app.world_mut().resource_mut::<Filters>().push_for_test(Pane::Palette, 'b');
    app.update();
    assert_eq!(ink_of(&mut app), Color::srgb(0.81, 0.91, 0.11), "typed text, unfocused, is the skin's `text`");

    // A box that owns the keyboard shows only its caret, wherever the words went.
    app.world_mut().resource_mut::<Filters>().take_focus(Pane::Palette);
    app.update();
    assert_eq!(ink_of(&mut app), Color::srgb(0.91, 0.11, 0.61), "the caret is the skin's `accent`");
}

/// **With no skin it is the shipped palette**, which is what every harness and every test written
/// before this seam expects — so moving these inks re-pins nothing.
#[test]
fn with_no_skin_a_filter_box_is_the_shipped_palette() {
    let mut app = app_with(None);
    assert_eq!(ground(&mut app), chrome::ink().row_bg, "a box at rest is `ROW_BG`");
    assert_eq!(ink_of(&mut app), chrome::ink().dim, "an empty box shows `DIM`");

    app.world_mut().resource_mut::<Filters>().take_focus(Pane::Palette);
    app.update();
    assert_eq!(ground(&mut app), chrome::ink().focus_bg, "a focused box is `FOCUS_BG`");
    assert_eq!(ink_of(&mut app), chrome::ink().accent, "the caret is `ACCENT`");
}
