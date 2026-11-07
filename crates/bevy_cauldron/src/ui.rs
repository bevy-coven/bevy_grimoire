//! **The one plugin that owns the editor's UI library** (UI-01, #209).
//!
//! The editor's UI is `bevy_gooey`, in every build. [`EditorUiPlugin`] is the only place that adds
//! gooey's plugins, so a later step of the UI series (a theme, a command palette, a menu bar) adds
//! its gooey plugin here and nowhere else.

use bevy::prelude::*;
use bevy_gooey::{GooeyNavPlugin, GooeySkin, GooeyStylePlugin};

/// The editor's own skin, as data: `src/editor_skin.ron`.
const EDITOR_SKIN: &str = include_str!("editor_skin.ron");

/// **The editor's look as `bevy_gooey`'s own type** (UI-02, #210): the colours and measures
/// `chrome.rs` draws with, the elevation ladder in `surfaces` and the sizes a compact tool wants.
/// `GooeySkin::problems()` is the check that the ladder is a ladder and the ink is readable.
pub fn editor_skin() -> GooeySkin {
    crate::ui_skin::from_ron(EDITOR_SKIN).unwrap_or_else(|e| panic!("src/editor_skin.ron does not read: {e}"))
}

/// Adds gooey's style plugin and its navigation plugin, puts the editor's skin in as the running one and adds the system that applies the project's skin file ([`crate::ui_skin::apply_ui_skin`]).
pub struct EditorUiPlugin;

/// **The wheel scrolls a panel** (#283): `bevy_ui_widgets::ScrollAreaPlugin`, which is the machinery
/// that turns a wheel event over a `ScrollArea` into a `ScrollPosition`.
///
/// `chrome::scroll_list` has put a `ScrollArea` and `overflow: scroll_y()` on every scrollable box in
/// this editor since it was written, and drew a scrollbar beside it — but `DefaultPlugins` does not
/// include `UiWidgetsPlugins`, and nothing here added this one, so **no panel answered the wheel at
/// all**. Measured on big 2026-11-03 at 1280x720: the INSPECTOR's `ENVIRONMENT` heading sits at
/// y = 898 in a 720 px window, and ten wheel notches over the panel left it at 898.5. Every section
/// below `EDGES` was unreachable, to a person as much as to a storyboard.
///
/// Its own plugin rather than a line inside [`EditorUiPlugin`] so a test can add exactly this and ask
/// whether it arrived, which is what `tests/the_wheel_scrolls_a_panel.rs` does.
pub struct ScrollWheelPlugin;

impl Plugin for ScrollWheelPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy::ui_widgets::ScrollAreaPlugin>() {
            app.add_plugins(bevy::ui_widgets::ScrollAreaPlugin);
        }
    }
}

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<GooeyStylePlugin>() {
            app.add_plugins(GooeyStylePlugin);
        }
        app.add_plugins(ScrollWheelPlugin);
        // **Menus need it** (UI-36, #230): gooey registers the context-menu click, the popovers' anchoring
        // and their closing in the navigation plugin. The editor's arrow keys stay its own because gooey's
        // only act on widgets that opt in (`AutoDirectionalNavigation`), and the viewport is not one.
        if !app.is_plugin_added::<GooeyNavPlugin>() {
            app.add_plugins(GooeyNavPlugin);
        }
        // **The settings screen** (UI-71, #249): Cmd/Ctrl+, over any screen.
        app.add_plugins((crate::settings_screen::SettingsScreenPlugin, crate::project_config::ProjectConfigPlugin, crate::keymap_screen::KeymapScreenPlugin));
        app.insert_resource(editor_skin());
        // **A person's arrangement, kept and restored** (UI-33, #227). The observer saves each change
        // gooey reports; the system is what a file edit or Reset reaches the dock through.
        app.add_observer(save_the_layout);
        app.add_systems(
            Update,
            follow_the_saved_layout.after(crate::settings::sync_settings).before(bevy_gooey::GooeySystems::Style),
        );
        // **The shell's regions as gooey panels** (UI-30, #224). A `Dock` shows only what a
        // `PanelRegistry` names, so the regions the editor draws are registered here — the one place
        // that owns the UI library — and the dock that arranges them is spawned in
        // `chrome::frame`, where the outer layout already lives.
        //
        // The contents keep the editor's own colours for now; UI-02 (#210) moves those onto the skin.
        // What changes here is the *arrangement*: a region can be dragged and resized by gooey rather
        // than by a hand-built flex row nothing can move.
        app.add_systems(PreStartup, register_the_shell_panels);
        // **`K` is the reveal, and gooey owns the rule** (UI-41, #235; bevy-coven/bevy_gooey#36).
        //
        // `ShortcutBadges` is ExposeHK's overlay, and until G-32 it could only be shown by a
        // *modifier* held exactly — `Modifiers::held(keys) == hold`. This editor's overlay key is
        // `K`, a plain key (`keys::Action::Shortcuts`, and `a_key_still_fires_while_k_is_held` pins
        // that `K`+verb still fires), so its own `keys::pressed` poll was the only way to reveal it.
        // `Reveal::Key` says the same thing in the crate, and `BadgesShown` reports it back.
        //
        // **Zero delay**: `K` is not a chord prefix but the overlay's own key, and the 0.3 s default
        // exists to stop a modifier flashing the screen on the way to `Ctrl+S`. Nothing types `K`
        // to reach a chord, so the delay would only make the overlay feel slow.
        app.insert_resource(bevy_gooey::ShortcutBadges {
            hold: bevy_gooey::Reveal::Key(bevy::input::keyboard::KeyCode::KeyK),
            delay_seconds: 0.0,
        });
        // **Refusals stand at the top of the viewport** (UI-39, #233), where the card they replace stood.
        app.insert_resource(bevy_gooey::ToastSettings { anchor: bevy_gooey::ToastAnchor::TopCenter, ..default() });
        app.add_systems(
            Update,
            crate::ui_skin::apply_ui_skin
                .after(crate::family_host::Reloaded)
                .before(bevy_gooey::GooeySystems::Style),
        );
        app.add_systems(Update, (paint_marked_buttons, clear_stale_rails).chain().after(bevy_gooey::GooeySystems::Style));
    }
}

/// **The one primary button of a screen**, filled with the skin's accent (#295). `bevy_gooey` has only one kind of button, so
/// this marks a [`bevy_gooey::button`] and [`paint_marked_buttons`] repaints it after gooey's own restyle
/// (bevy-coven/bevy_gooey#40 asks for the widget itself). A screen has at most one.
#[derive(Component)]
pub struct AccentButton;

/// A [`bevy_gooey::button`] marked as the screen's primary action: [`AccentButton`].
pub fn accent_button(text: impl Into<String>) -> impl Bundle {
    (bevy_gooey::button(text), AccentButton)
}

/// How far the accent moves toward white while hovered, and toward black while pressed.
const ACCENT_HOVER_MIX: f32 = 0.18;
const ACCENT_PRESS_MIX: f32 = 0.2;

/// Repaints an [`AccentButton`] (accent fill, accent text) and a [`bevy::ui::Selected`] button (a card or a navigation row that
/// is the chosen one: the skin's hovered fill, plus a [`SelectedRail`]). It runs after gooey's restyle, which does not restyle on
/// a colour change, so the two never fight; a pressed selected button keeps gooey's own pressed fill.
///
/// **The rail is why this exists at all now.** A fill a few ramp steps lighter is not a selected state a person can see:
/// measured in the shipped 0.2.4 screenshot on 2026-11-05, `Projects` — the selected tab — was not tellable from `Learn`. A
/// shape can be, and the palette already has one (`crate::chrome::SELECT_RAIL_W`), so a selected button gains a strip down its
/// leading edge. It is inserted by this system rather than by each caller: every `Selected` button already means "this one",
/// so every one of them should say it the same way.
#[derive(Component)]
struct SelectedRail;

#[allow(clippy::type_complexity)]
fn paint_marked_buttons(
    skin: Res<bevy_gooey::GooeySkin>,
    mut buttons: Query<
        (Entity, &mut BackgroundColor, Has<AccentButton>, Has<bevy::ui::Selected>, Option<&bevy::picking::hover::Hovered>, Has<bevy::ui::Pressed>, Has<bevy::ui::InteractionDisabled>, Option<&Children>),
        (With<bevy_gooey::Role>, Or<(With<AccentButton>, With<bevy::ui::Selected>)>),
    >,
    rails: Query<(), With<SelectedRail>>,
    mut texts: Query<(&bevy_gooey::Role, &mut TextColor, Option<&SelectedRail>)>,
    mut commands: Commands,
) {
    for (entity, mut fill, accent, selected, hovered, pressed, disabled, children) in &mut buttons {
        let want = if disabled {
            skin.surface
        } else if accent {
            let base = skin.accent;
            if pressed {
                bevy::color::Mix::mix(&base, &Color::BLACK, ACCENT_PRESS_MIX)
            } else if hovered.is_some_and(|h| h.0) {
                bevy::color::Mix::mix(&base, &Color::WHITE, ACCENT_HOVER_MIX)
            } else {
                base
            }
        } else if pressed {
            continue;
        } else {
            skin.hovered()
        };
        if fill.0 != want {
            fill.0 = want;
        }
        if accent && !disabled {
            for child in children.into_iter().flat_map(|c| c.iter()) {
                if let Ok((role, mut ink, _)) = texts.get_mut(child)
                    && *role == bevy_gooey::Role::ButtonText
                    && ink.0 != skin.accent_text
                {
                    ink.0 = skin.accent_text;
                }
            }
        }
        // **The rail, for a button that is the chosen one and is not already the accent colour.**
        let wants = selected && !accent && !disabled;
        let has = children.is_some_and(|c| c.iter().any(|child| rails.get(child).is_ok()));
        match (wants, has) {
            (true, false) => {
                commands.entity(entity).with_child((
                    SelectedRail,
                    Node {
                        // PLACES-ITSELF-OK: a rail pinned to its own button's leading edge, the way
                        // `crate::chrome::SELECT_RAIL_W` is drawn on a kit's row — it is not a panel.
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(crate::chrome::SELECT_RAIL_W),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(skin.accent),
                    bevy::picking::Pickable::IGNORE,
                ));
            }
            (false, true) => {
                for child in children.into_iter().flat_map(|c| c.iter()) {
                    if rails.get(child).is_ok() {
                        commands.entity(child).despawn();
                    }
                }
            }
            _ => {}
        }
    }
}

/// **Takes the rail off a button that is no longer `Selected`.** [`paint_marked_buttons`] only visits buttons that still carry
/// `Selected`, so the rail of a button that has just lost it was never removed: choosing the next sidebar tab left the old one
/// railed too (`feedback-1793927950267`).
fn clear_stale_rails(rails: Query<(Entity, &ChildOf), With<SelectedRail>>, selected: Query<(), With<bevy::ui::Selected>>, mut commands: Commands) {
    for (rail, parent) in &rails {
        if selected.get(parent.parent()).is_err() {
            commands.entity(rail).despawn();
        }
    }
}

/// **The regions of the shell, as `bevy_gooey` panels** (UI-30, #224).
///
/// Each is registered with the id a dock layout names. A `Dock` shows what the registry holds when it
/// is built and skips anything it cannot find (`DockPanelSkipped`), so this runs before the frame.
///
/// **Registered once, at `PreStartup`.** The spawn closure fills the panel; a region whose contents
/// the editor draws elsewhere — the viewport is a texture — gets the node that holds them, so the
/// dock's own arrangement is what a person drags.
fn register_the_shell_panels(mut registry: ResMut<bevy_gooey::PanelRegistry>) {
    for (id, title) in [
        ("inspector", "INSPECTOR"),
        ("viewport", "VIEWPORT"),
        ("place", "PLACE"),
        ("status", "STATUS"),
    ] {
        registry.register(id, title, move |c| {
            c.insert(bevy_gooey::DockPanel(id.to_owned()));
        });
    }
}

/// **The shell's arrangement**, as a dock layout a divider can move (UI-30, #224).
///
/// A left dock, the viewport in the middle with the most room, and a right dock — the shape the
/// hand-built frame has had since 2026-08-17, now data that gooey keeps and a person edits.
/// **A person moved a divider or a tab: their arrangement is saved** (UI-33, #227).
///
/// `bevy_gooey` reports every change a person makes to a `Dock` with `DockLayoutChanged`, carrying the
/// layout it now shows. Writing it to the user's file is what makes the arrangement survive a restart;
/// the editor reads it back through `SettingsLayer::shell_layout` on the next start.
///
/// **Nothing is written to a test or to a run with no file**: no `UserSettingsFile` means no place the
/// person asked the editor to keep anything, so the event is taken and dropped, and a headless test
/// that drags a divider does not write the real config.
pub fn save_the_layout(
    changed: On<bevy_gooey::DockLayoutChanged>,
    file: Option<Res<crate::settings::UserSettingsFile>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    let Some(file) = file else { return };
    // **`edit_layer`, not `write_key`.** `write_key` speaks the Settings screen's value vocabulary
    // (a switch, a number, a named choice), and a layout is none of those; `edit_layer` is the same
    // primitive underneath it — the file is read as the editor reads it, refused if it does not
    // read, checked to read back, and written atomically — with the layer itself in hand.
    let layout = changed.layout.clone();
    if let Err(why) = crate::setting_catalogue::edit_layer(&file.0, move |layer| {
        layer.shell_layout = Some(layout);
        Ok(())
    }) && let Some(state) = state.as_deref_mut()
    {
        state.status.problem(format!("the layout was not saved: {why}"));
    }
}

/// **A settings read reaches the dock**: a layout written by hand, or Reset's write, is what the shell
/// shows, the same way an edit of `text_scale` reaches the text.
///
/// It reads the resolved settings, not the `DockLayoutChanged` event, because there are two ways a
/// layout changes — a person dragging it (saved by [`save_the_layout`], which the dock already shows)
/// and a file a person edited (which only this can apply). Compared before writing, so a settled
/// editor costs one comparison.
pub fn follow_the_saved_layout(
    settings: Option<Res<crate::settings::EditorSettings>>,
    mut docks: Query<&mut bevy_gooey::Dock>,
) {
    let Some(settings) = settings else { return };
    let want = settings.shell_layout.clone().unwrap_or_else(shell_layout);
    for mut dock in &mut docks {
        if dock.0 != want {
            dock.0 = want.clone();
        }
    }
}

pub fn shell_layout() -> bevy_gooey::DockLayout {
    use bevy_gooey::{DockLayout, SplitAxis};
    DockLayout::split(
        SplitAxis::Horizontal,
        0.22,
        DockLayout::group(["inspector"]),
        DockLayout::split(
            SplitAxis::Horizontal,
            0.78,
            DockLayout::group(["viewport"]),
            DockLayout::group(["place"]),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A rail on a button that is not `Selected` is despawned; one on a `Selected` button stays.**
    #[test]
    fn a_rail_outlives_its_selection_never() {
        let mut app = App::new();
        app.add_systems(Update, clear_stale_rails);
        let kept = app.world_mut().spawn(bevy::ui::Selected).with_child(SelectedRail).id();
        let stale = app.world_mut().spawn_empty().with_child(SelectedRail).id();
        app.update();
        let rails = |app: &mut App, parent: Entity| {
            let world = app.world_mut();
            world.get::<Children>(parent).map_or(0, |c| c.iter().filter(|k| world.get::<SelectedRail>(*k).is_some()).count())
        };
        assert_eq!(rails(&mut app, kept), 1);
        assert_eq!(rails(&mut app, stale), 0);
    }
}
