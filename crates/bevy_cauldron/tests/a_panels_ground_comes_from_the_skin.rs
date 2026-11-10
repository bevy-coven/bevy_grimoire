//! **A panel's fill comes from the running skin, not from a constant** (UI-02, #210, slice 4b).
//!
//! `panel_root` spawns every panel in the editor with `BackgroundColor(PANEL_BG)` and
//! `Ground(PANEL_BG)` — the *constant* — at the moment the panel is built. The skin is a resource
//! (`editor_skin.yaml`, `EditorUiPlugin`), and a project may replace it at run time. So until now a
//! theme edit reached the list rows (`RowFills`, slice 3) and every other piece of ink that had been
//! migrated, but **not the largest surface in the editor**: every panel's own ground was fixed at
//! spawn and never asked the skin again.
//!
//! That is the defect this file pins, and the reason it is not cosmetic: `#210`'s acceptance is that
//! `chrome.rs` declares no colour constants, and a constant read once at spawn is the hardest kind to
//! retire — the call site looks migrated, and the panel silently ignores the theme.
//!
//! **What is asserted here is the *expression*, not a pixel.** `PanelFills::of` is the one function
//! both the spawn and the repaint call, so an assertion on it is an assertion about what the panel
//! paints, and it runs with no window and no pointer — the same reasoning `RowFills` records in
//! `the_lists_words_come_from_the_skin.rs`.

use bevy::prelude::*;
use bevy_cauldron::chrome;

#[test]
fn a_panels_ground_is_the_skins_surface() {
    let skin = bevy_cauldron::ui::editor_skin();
    let fills = chrome::PanelFills::of(Some(&skin));
    assert_eq!(
        fills.panel, skin.surface,
        "the panel's fill must be the skin's `surface`, so a theme edit reaches every panel"
    );
    assert_eq!(
        fills.panel, chrome::PANEL_BG,
        "and the shipped skin's surface is the constant this panel used to be built from — the edit \
         changes where the colour comes from, not which colour the editor looks like today"
    );
}

/// **With no skin the constants still paint**, which is what a headless harness that never ran
/// `EditorUiPlugin` gets. Same rule as `RowFills::of`: the fallback is the shipped table, never a
/// second palette.
#[test]
fn with_no_skin_the_constant_still_paints() {
    let fills = chrome::PanelFills::of(None);
    assert_eq!(fills.panel, chrome::PANEL_BG);
    assert_eq!(fills.band, chrome::BAR_BG);
}

/// **A theme edit changes the expression**, and this is the half that would have failed before: the
/// value has to come *from the skin*, so a skin whose `surface` is not `PANEL_BG` must paint its own.
#[test]
fn a_skin_that_is_not_the_editors_paints_its_own_surface() {
    let mut skin = bevy_cauldron::ui::editor_skin();
    skin.surface = Color::srgb(0.2, 0.6, 0.95);
    let fills = chrome::PanelFills::of(Some(&skin));
    assert_eq!(fills.panel, Color::srgb(0.2, 0.6, 0.95));
    assert_ne!(fills.panel, chrome::PANEL_BG);
}

/// **The repaint reaches every panel, including one built before the edit** — which is the half the
/// expression test cannot prove, and the half that was broken.
///
/// A panel is spawned while the shipped skin is running, then the skin is replaced with one whose
/// `surface` is blue, then the system runs. A panel that read its fill only at spawn stays
/// `PANEL_BG`; a panel whose ground is the skin's turns blue. That is exactly the difference between
/// "the constant still exists" and "a theme edit reaches the UI".
#[test]
fn a_theme_edit_reaches_a_panel_that_was_already_built() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.insert_resource(bevy_cauldron::keys::Live::default());
    app.insert_resource(bevy_cauldron::ui::editor_skin());
    app.add_systems(Update, chrome::dim_the_ground);

    let panel = app
        .world_mut()
        .spawn((
            chrome::Ground(chrome::PANEL_BG),
            chrome::PanelGround,
            BackgroundColor(chrome::PANEL_BG),
        ))
        .id();
    // A band is marked as a band, and follows the skin's `"bar_bg"` tone through the same system.
    let band = app
        .world_mut()
        .spawn((
            chrome::Ground(chrome::BAR_BG),
            chrome::BandGround,
            BackgroundColor(chrome::BAR_BG),
        ))
        .id();
    // A ground with neither marker — a row's, a field's — keeps its own colour.
    let plain = app
        .world_mut()
        .spawn((chrome::Ground(chrome::SLOT_BG), BackgroundColor(chrome::SLOT_BG)))
        .id();
    app.update();
    assert_eq!(app.world().get::<BackgroundColor>(panel).unwrap().0, chrome::PANEL_BG);

    // The edit: a project writes a skin with a blue surface while the panel already exists.
    let mut edited = bevy_cauldron::ui::editor_skin();
    edited.surface = Color::srgb(0.2, 0.6, 0.95);
    app.insert_resource(edited);
    app.update();

    assert_eq!(
        app.world().get::<BackgroundColor>(panel).unwrap().0,
        Color::srgb(0.2, 0.6, 0.95),
        "a panel built before the edit must take the new skin's surface"
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(band).unwrap().0,
        chrome::BAR_BG,
        "the shipped skin carries the band's colour as its `bar_bg` tone, so a band is unchanged today"
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(plain).unwrap().0,
        chrome::SLOT_BG,
        "a ground with no marker keeps its own colour through the same system"
    );
}

/// **A project's skin can restyle the bands, and the editor's own cannot move** — which is the whole
/// reason the band reads a `tone` and falls back rather than shipping one.
///
/// `the_skin_carries_the_editors_other_colours.rs` pins the editor skin's tone set *exactly*, so a tone
/// added to `editor_skin.yaml` re-pins a protected assertion and is not this pass's to make. A tone a
/// **project** writes is a different thing: it is in the project's own `family/bevy_gooey.yaml`, no
/// protected set describes it, and it must reach the status bar and the chrome bar.
#[test]
fn a_projects_bar_tone_repaints_the_bands() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.insert_resource(bevy_cauldron::keys::Live::default());
    app.add_systems(Update, chrome::dim_the_ground);

    let band = app
        .world_mut()
        .spawn((
            chrome::Ground(chrome::BAR_BG),
            chrome::BandGround,
            BackgroundColor(chrome::BAR_BG),
        ))
        .id();

    // A project's skin: the editor's own, plus one tone it wrote for its bands.
    let mut project = bevy_cauldron::ui::editor_skin();
    project.tones.insert("bar_bg".to_owned(), Color::srgb(0.5, 0.1, 0.6));
    app.insert_resource(project);
    app.update();

    assert_eq!(
        app.world().get::<BackgroundColor>(band).unwrap().0,
        Color::srgb(0.5, 0.1, 0.6),
        "a project's `bar_bg` tone must reach the band"
    );
}
