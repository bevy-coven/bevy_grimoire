//! **Hide the left panel, or everything but the world** (feedback-1794146831022).
//!
//! The note, from the editor: *"We should have a toggle that toggles into "Zen" mode, that hides all the UI and just sees
//! the game. Also, Cmd+B should toggle the left panel, like VSCode."*
//!
//! - **`Cmd+B` shows and hides the left panel** (the INSPECTOR column and its pane): the side bar's chord in VS Code, and in
//!   the same family as `Cmd+J` for the terminal. The viewport takes the room.
//! - **`Shift+Cmd+M` is Zen mode**: the menu bar, the chrome bar, the toolbar, the door's strip, both docks, the terminal, the
//!   status band and the empty-map prompt go, and the viewport is the whole window. The same chord brings them back, and a
//!   toast says so when the mode begins, since nothing on screen could.
//! - **View ▸ Show left panel** and **View ▸ Zen mode** are the same two, as ticks.
//!
//! # How a part goes
//!
//! **`Display::None`, and the frame is left as it is.** A hidden node lays out to nothing, which is also how the badges and
//! the census already tell a hidden control from a shown one (`chrome::Control`), and the viewport node (which the world is
//! drawn through, `surface`) grows by flexbox into what the others leave. What each was before is kept on the node
//! ([`Folded`]) and put back, so a part that was hidden for another reason (the collapsed terminal) stays hidden after Zen.
//! **The view is enforced every frame it is on**: a part another system raises (the terminal's own chord) is folded again the
//! next frame. The shell's split gives the first pane the left panel's share through [`SplitFraction`] and [`SplitMin`];
//! hiding the pane alone would leave the viewport the rest of that share, so both are put to nothing while it is hidden and
//! back to what they were when it returns.
//!
//! **Session state, not a setting.** A person who left Zen mode on would open the editor to a bare viewport and no menu; the
//! left panel's state is likewise the session's. Neither is written to the user's file.

use bevy::prelude::*;
use bevy_gooey::{Modifiers, Role, Shortcut, SplitFraction, SplitMin};

use crate::chrome::{Frame, FrameSystems, ShellSplit};
use crate::confirm::Confirm;
use crate::empty_state::EmptyMapPrompt;
use crate::keys::{Context, Live};
use crate::menu_bar::EditorMenuBar;
use crate::screen::Screen;
use crate::terminal_pane::TerminalPaneRoot;
use crate::toolbar::Toolbar;

/// **The chord that shows and hides the left panel**: Command+B on a Mac, Ctrl+B elsewhere. VS Code's Toggle Primary Side Bar.
/// `Run` used to hold it (`keys.rs`); it is `Shift+Cmd+B` now.
pub const LEFT_PANEL_CHORD: Shortcut = Shortcut::new(Modifiers::PRIMARY, KeyCode::KeyB);

/// **The chord that begins and ends Zen mode**: Shift+Command+M on a Mac, Shift+Ctrl+M elsewhere. A letter and a modifier pair
/// that no binding of `keys.rs` takes, so nothing else fires with it.
pub fn zen_chord() -> Shortcut {
    Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, KeyCode::KeyM)
}

/// What the person has hidden of the shell this session.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellView {
    /// The left panel (and its pane) is hidden.
    pub left_hidden: bool,
    /// Zen mode: everything but the viewport is hidden.
    pub zen: bool,
}

impl ShellView {
    /// Is the left panel's pane out of the layout? Zen takes it with everything else.
    pub fn left_is_hidden(self) -> bool {
        self.left_hidden || self.zen
    }

    /// Are the bands, the toolbar, the right dock and the terminal out of the layout?
    pub fn the_rest_is_hidden(self) -> bool {
        self.zen
    }
}

/// **What a node's `display` was before the shell view folded it**, put back when the view lets go of it.
#[derive(Component, Debug)]
pub struct Folded(pub Display);

/// The View menu's tick for the left panel.
#[derive(Component)]
pub struct LeftPanelItem;

/// The View menu's tick for Zen mode.
#[derive(Component)]
pub struct ZenItem;

/// Everything Zen mode folds that has no field in the [`Frame`]: found by what it is.
type ZenParts = Or<(With<EditorMenuBar>, With<Toolbar>, With<TerminalPaneRoot>, With<EmptyMapPrompt>)>;

pub struct ShellViewPlugin;

impl Plugin for ShellViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShellView>().add_systems(
            Update,
            (toggle_on_the_chords, fold_the_shell.after(FrameSystems), tick_the_menu_rows)
                .chain()
                .run_if(in_state(Screen::Editor)),
        );
    }
}

/// **The two chords.** Not while a text field has the keys (a chord typed into a field is the field's) nor over a question.
fn toggle_on_the_chords(
    keys: Res<ButtonInput<KeyCode>>,
    live: Res<Live>,
    confirm: Res<Confirm>,
    bindings: Res<bevy_gooey::BindingSettings>,
    mut view: ResMut<ShellView>,
    mut commands: Commands,
) {
    let left = LEFT_PANEL_CHORD.just_pressed(&keys);
    let zen = zen_chord().just_pressed(&keys);
    if !(left || zen) || live.0 == Context::Typing || confirm.is_open() {
        return;
    }
    if left {
        view.left_hidden = !view.left_hidden;
    }
    if zen {
        view.zen = !view.zen;
        if view.zen {
            commands.spawn(bevy_gooey::toast(format!("Zen mode: {} brings the editor back", bindings.chord(zen_chord()))));
        }
    }
}

/// **Fold what the view hides, and unfold what it has let go of.** Every frame the view is on, the same answer, so a part
/// another system raised is folded again; a settled shell costs one comparison per part.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn fold_the_shell(
    view: Res<ShellView>,
    frame: Option<Res<Frame>>,
    parts: Query<Entity, ZenParts>,
    splits: Query<&Children, With<ShellSplit>>,
    roles: Query<&Role>,
    folded: Query<(Entity, &Folded)>,
    mut nodes: Query<&mut Node>,
    mut shares: Query<(&mut SplitFraction, &mut SplitMin), With<ShellSplit>>,
    mut kept_share: Local<Option<(f32, f32)>>,
    mut commands: Commands,
) {
    let Some(frame) = frame else { return };
    let mut fold: Vec<Entity> = Vec::new();
    if view.the_rest_is_hidden() {
        fold.extend(parts.iter());
        fold.extend([frame.chrome_bar, frame.door_strip, frame.status, frame.right]);
    }
    if view.left_is_hidden() {
        // **The first pane and the divider only**: the children come `[pane, divider, pane]`, and the second pane holds the viewport.
        for kids in &splits {
            let own = |kid: &Entity| roles.get(*kid).is_ok_and(|role| matches!(role, Role::SplitPane | Role::SplitDivider));
            fold.extend(kids.iter().filter(own).take(2));
        }
    }

    for entity in &fold {
        let Ok(mut node) = nodes.get_mut(*entity) else { continue };
        if node.display != Display::None {
            if !folded.contains(*entity) {
                commands.entity(*entity).insert(Folded(node.display));
            }
            node.display = Display::None;
        }
    }
    for (entity, before) in &folded {
        if fold.contains(&entity) {
            continue;
        }
        if let Ok(mut node) = nodes.get_mut(entity) {
            node.display = before.0;
        }
        commands.entity(entity).remove::<Folded>();
    }

    // The left pane's share of the split goes to the viewport while the pane is out of the layout.
    for (mut fraction, mut min) in &mut shares {
        if view.left_is_hidden() {
            if kept_share.is_none() {
                *kept_share = Some((fraction.0, min.first));
            }
            if fraction.0 != 0.0 {
                fraction.0 = 0.0;
            }
            if min.first != 0.0 {
                min.first = 0.0;
            }
        } else if let Some((was, least)) = kept_share.take() {
            fraction.0 = was;
            min.first = least;
        }
    }
}

/// **The ticks say what the view is**, whichever way it changed: a chord, or the row.
fn tick_the_menu_rows(
    view: Res<ShellView>,
    left: Query<(Entity, Has<bevy_gooey::Checked>), With<LeftPanelItem>>,
    zen: Query<(Entity, Has<bevy_gooey::Checked>), With<ZenItem>>,
    mut commands: Commands,
) {
    let mut tick = |item: Entity, ticked: bool, want: bool| match (want, ticked) {
        (true, false) => {
            commands.entity(item).insert(bevy_gooey::Checked);
        }
        (false, true) => {
            commands.entity(item).remove::<bevy_gooey::Checked>();
        }
        _ => {}
    };
    for (item, ticked) in &left {
        tick(item, ticked, !view.left_hidden);
    }
    for (item, ticked) in &zen {
        tick(item, ticked, view.zen);
    }
}

/// **View ▸ Show left panel**: the tick is the panel shown.
pub fn toggle_the_left_panel(change: On<bevy_gooey::ValueChange<bool>>, mut view: ResMut<ShellView>) {
    view.left_hidden = !change.value;
}

/// **View ▸ Zen mode.**
pub fn toggle_zen(change: On<bevy_gooey::ValueChange<bool>>, mut view: ResMut<ShellView>) {
    view.zen = change.value;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Zen takes the left panel with it, and the left panel alone takes nothing else.**
    #[test]
    fn the_view_says_what_is_hidden() {
        for left_hidden in [false, true] {
            for zen in [false, true] {
                let view = ShellView { left_hidden, zen };
                assert_eq!(view.left_is_hidden(), left_hidden || zen, "{view:?}");
                assert_eq!(view.the_rest_is_hidden(), zen, "{view:?}");
            }
        }
    }

    /// **The two chords are different keys and neither is the other's with a modifier added**: a press of one never reads as the
    /// other (`Shortcut::just_pressed` asks for exactly its modifiers).
    #[test]
    fn the_chords_do_not_overlap() {
        assert_ne!(LEFT_PANEL_CHORD, zen_chord());
        assert_ne!(LEFT_PANEL_CHORD.key, zen_chord().key);
    }
}
