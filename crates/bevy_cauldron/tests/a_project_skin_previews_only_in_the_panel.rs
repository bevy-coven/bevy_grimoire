//! **A project's game skin previews only inside the Game UI preview panel** (UI-03, #211).
//!
//! A game ships `family/bevy_gooey.yaml` so its own UI matches its art. Until this issue, that file
//! *was* the editor's running `GooeySkin`, so a game's palette repainted the Cauldron's own chrome —
//! and a light game skin left every editor label at about 1.1:1. The issue's What is one sentence:
//! "Build a `Game UI preview` panel under a `SkinScope` loaded from `family/bevy_gooey.yaml`. The editor
//! chrome never takes it."
//!
//! # The two acceptance items, as tests
//!
//! - **Editing the project skin's `accent` changes the preview's buttons and no chrome button.** The
//!   preview's primary button is filled from the skin it stands under, so a new `accent` in the file
//!   moves its fill and its label's ink; the editor's own button roles are drawn from the running
//!   `GooeySkin`, which is the editor's own, so `a_project_skin_never_reaches_a_chrome_button` reads
//!   the running resource and the ink the chrome paints with.
//! - **The clip** is owed with the changelog line and is not assertable here.
//!
//! # Why the scope is read off the panel and not off the resource
//!
//! `bevy_gooey` styles a widget from its nearest `SkinScope` at or above it, else the resource. The
//! assertion that means "the preview takes the project's skin and the chrome does not" is therefore
//! about **which of the two answers a widget gets**: the sample button has a scope above it and the
//! chrome bar does not. Both are asserted, so a change that put the file back on the resource fails the
//! second and a change that stopped reading the file fails the first.

use bevy::prelude::*;
use bevy_cauldron::chrome;
use bevy_cauldron::game_ui_preview::{
    GameUiPreview, GameUiPreviewItem, PreviewPlain, PreviewPrimary,
};
use bevy_cauldron::ui_skin::GameSkin;
use bevy_cauldron::ui::{editor_skin, skin_for};
use bevy_cauldron::settings::Theme;
use bevy_gooey::{GooeySkin, SkinScope, ValueChange};

use crate::fixtures;

/// The project skill this file writes: a skin that shares no colour with the editor's own.
const GAME_SKIN: &str = "{accent: !Srgba {red: 0.20, green: 0.60, blue: 0.90, alpha: 1.0}, surface: !Srgba {red: 0.10, green: 0.14, blue: 0.20, alpha: 1.0}}";

/// A blue accent unlike the editor's amber, so "the project's skin reached this" is visible.
fn game_accent() -> Color {
    Color::srgba(0.20, 0.60, 0.90, 1.0)
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// An editor on a project that ships the game skin above, with the preview panel open.
fn editor_with_a_game_skin(tag: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    std::fs::create_dir_all(root.join("family")).expect("a family folder");
    std::fs::write(root.join("family.yaml"), "{version: 1, crates: [\"bevy_gooey\"]}").expect("a family.yaml");
    std::fs::write(root.join("family/bevy_gooey.yaml"), GAME_SKIN).expect("a project skin");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 12);
    show_the_preview(&mut app, true);
    (root, app)
}

/// Press the View ▸ Game UI preview row, as a person does: the `ValueChange<bool>` the check item sends.
fn show_the_preview(app: &mut App, on: bool) {
    let row = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<GameUiPreviewItem>>()
            .single(world)
            .expect("the View menu carries one Game UI preview row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: on, is_final: true });
    updates(app, 6);
}

/// **The panel takes the project's skin, and its own buttons are drawn from it.**
///
/// Read as the nearest `SkinScope` at or above the primary button, which is the answer `bevy_gooey`'s
/// `apply_skin` gives a widget — not a resource that happens to hold the same value.
#[test]
fn the_preview_panel_takes_the_projects_skin() {
    let (_root, mut app) = editor_with_a_game_skin("preview_scope");
    let panel = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<GameUiPreview>>()
            .single(world)
            .expect("the panel is open")
    };
    let scope = app
        .world()
        .get::<SkinScope>(panel)
        .expect("the panel carries the project's skin as a SkinScope");
    assert_eq!(scope.0.accent, game_accent(), "the scope is the file's accent, not the editor's");
    // And the panel is inside the editor's own pane, so it is a subtree of the window rather than a
    // second screen: the chrome above it is drawn from the resource.
    let button = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<PreviewPrimary>>()
            .single(world)
            .expect("the sample screen has its one main action")
    };
    // **Inside the panel**, through the sample screen's column: a widget is styled from the nearest
    // `SkinScope` at or above it, so "is it in the panel" is asked by walking up.
    let mut at = app.world().get::<ChildOf>(button).map(ChildOf::parent).expect("a button has a parent");
    let mut on_panel = at == panel;
    for _ in 0..8 {
        if on_panel {
            break;
        }
        let Some(above) = app.world().get::<ChildOf>(at).map(ChildOf::parent) else { break };
        at = above;
        on_panel = at == panel;
    }
    assert!(on_panel, "the primary button stands under the panel that carries the skin");
}

/// **Editing the project skin's `accent` changes the preview's buttons.** The load-bearing half: the
/// panel's widgets read the file rather than a copy, so a new accent in the file is a new fill.
#[test]
fn editing_the_projects_accent_moves_the_previews_buttons() {
    let (root, mut app) = editor_with_a_game_skin("preview_accent");
    let panel = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<GameUiPreview>>().single(world).expect("the panel is open")
    };
    assert_eq!(app.world().get::<SkinScope>(panel).expect("a scope").0.accent, game_accent());

    // **A new accent, written the way a person writes it**: the file, and the frames that read it.
    let moved = Color::srgba(0.95, 0.30, 0.10, 1.0);
    std::fs::write(
        root.join("family/bevy_gooey.yaml"),
        "{accent: !Srgba {red: 0.95, green: 0.30, blue: 0.10, alpha: 1.0}}",
    )
    .expect("a moved accent");
    let mut seen = None;
    for _ in 0..80 {
        app.update();
        let scope = app.world().get::<SkinScope>(panel).map(|s| s.0.accent);
        if scope == Some(moved) {
            seen = scope;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(seen, Some(moved), "the preview's scope follows the file's accent");

    // **And the fill the button is actually drawn with is that accent**, read off its own
    // `BackgroundColor` once gooey has styled it (review, 2026-11-10): `variant_fill` is the library's
    // answer for a skin, and the question here is what reached the widget on screen.
    let primary = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<PreviewPrimary>>().single(world).expect("the sample screen's main action")
    };
    let drawn = app
        .world()
        .get::<BackgroundColor>(primary)
        .map(|bg| bg.0)
        .expect("a gooey button is filled by `apply_skin`, so it has a BackgroundColor");
    assert_eq!(drawn, moved, "the preview's primary button is painted the file's accent");
    // **The ordinary button beside it is a different fill**, so "the accent reached it" is not "every
    // button is the accent".
    let plain = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<PreviewPlain>>().single(world).expect("a plain sample button")
    };
    let plain_fill = app.world().get::<BackgroundColor>(plain).map(|bg| bg.0);
    assert_ne!(plain_fill, Some(moved), "the ordinary button is not also the accent");
}

/// **The scope is on the preview panel and nowhere else.**
///
/// #211's acceptance is that the editor chrome never takes the project's skin, and `bevy_gooey` styles
/// a widget from the nearest `SkinScope` at or above it — so one scope landing on the window root, a
/// door, or any panel that is not the preview would restyle the chrome without changing the resource.
/// This walks every scope in the world and holds each to the preview panel, which is the only check
/// that can catch that.
#[test]
fn the_projects_skin_scope_is_on_the_preview_panel_and_nowhere_else() {
    let (_root, mut app) = editor_with_a_game_skin("preview_scope_only");
    let panel = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<GameUiPreview>>().single(world).expect("the panel is open")
    };
    let scopes: Vec<Entity> = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<SkinScope>>().iter(world).collect()
    };
    assert_eq!(scopes, vec![panel], "one scope, on the preview panel: any other would restyle the chrome");
}

/// **No chrome button takes the project's skin.** The editor's running skin is the editor's own and the
/// ink its chrome paints with is unchanged, whatever the project ships — the half #210's `Ink` and
/// #211's scope together make true.
#[test]
fn a_project_skin_never_reaches_a_chrome_button() {
    let (_root, app) = editor_with_a_game_skin("preview_chrome");
    let running = app.world().resource::<GooeySkin>().clone();
    assert_eq!(
        running.accent,
        editor_skin().accent,
        "the editor's own accent is the running one, not the project's"
    );
    assert_ne!(running.accent, game_accent(), "and the project's accent is not it");
    assert_eq!(
        chrome::ink().accent,
        editor_skin().accent,
        "the ink every chrome label and rail is drawn with is the editor's own"
    );
    // The theme a person chose is what the chrome is drawn from, and a project's file cannot move it.
    assert_eq!(running, skin_for(Theme::default()), "a project's file changes no editor colour");
}

/// **The project's skin is not the running skin even while its own file is the one on screen** — the
/// defect the issue is about, stated as a negative: the resource `apply_skin` styles unscoped widgets
/// from never holds the game's palette.
#[test]
fn the_projects_skin_lives_outside_the_running_one() {
    let (_root, app) = editor_with_a_game_skin("preview_resource");
    let game = app.world().resource::<GameSkin>();
    assert_eq!(
        game.0.as_ref().map(|s| s.accent),
        Some(game_accent()),
        "the file is read into the preview's own resource"
    );
    assert_eq!(
        app.world().resource::<GooeySkin>().accent,
        editor_skin().accent,
        "and never into the running one"
    );
}

/// **With no file, the preview shows the editor's own palette and says so.** A project that ships no
/// game UI is the ordinary case, and the panel must not be an empty box: it draws the editor's skin and
/// its line says which one a person is looking at.
#[test]
fn a_project_with_no_skin_previews_the_editors_own_and_says_so() {
    let root = fixtures::Fixture::new(&format!("preview_noskin_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 12);
    show_the_preview(&mut app, true);
    assert_eq!(app.world().resource::<GameSkin>().0, None, "no file, no game skin");
    let panel = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<GameUiPreview>>().single(world).expect("the panel is open")
    };
    assert!(
        app.world().get::<SkinScope>(panel).is_none(),
        "a project with no skin gets no scope: the resource is already the editor's"
    );
}

/// **Asking again takes the panel away**, and the tick on the row follows it — the pattern the outliner
/// and the library browser use, so a person can close what they opened.
#[test]
fn the_panel_goes_when_it_is_asked_again() {
    let (_root, mut app) = editor_with_a_game_skin("preview_toggle");
    assert!(app.world_mut().query_filtered::<Entity, With<GameUiPreview>>().single(app.world()).is_ok());
    show_the_preview(&mut app, false);
    assert!(
        app.world_mut().query_filtered::<Entity, With<GameUiPreview>>().single(app.world()).is_err(),
        "View ▸ Game UI preview off takes the panel away"
    );
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<GameUiPreviewItem>>().single(world).expect("a row")
    };
    assert!(app.world().get::<bevy_gooey::Checked>(row).is_none(), "and the menu row's tick is off");
}
