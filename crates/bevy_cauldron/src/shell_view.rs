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
//! # The left panel is the shell split's collapsed pane (UI-R06, #336)
//!
//! **Hiding the left panel collapses the shell's gooey split**, as Enter on its divider does, and showing it restores
//! it: the panes slide on gooey's spring ([`bevy_gooey::SplitGlide`]) instead of the viewport jumping a pane's width, and the
//! collapsed pane **peeks** — the pointer resting on the divider floats the pane over the viewport at the width a restore would
//! give it, with the viewport still (gooey's `SplitPeek`, #69). **The shell asks for it with gooey's [`CollapseSplit`]**
//! (G-77, #88), triggered on the split: the one path the divider's own Enter, South button and double press take, so
//! [`collapse_the_left_pane`] synthesises no key and reaches into nothing of gooey's.
//!
//! **Two numbers are the shell's to hold while it is collapsed.** The pane's floor ([`SplitMin::first`], which keeps a dragged
//! pane wide enough for a panel) is put to nothing so the pane can shut, and given back when a restore has landed; the
//! fraction a restore returns to is raised to that floor first, so the pane opens to the width the panel needs and the
//! glide has somewhere to go that the floor does not cut short.
//!
//! **A shut pane shows no dock.** Once the collapse has landed, nothing is peeking and the pane is as wide as nothing, the left
//! dock is `Display::None` ([`Folded`]): a node that lays out to nothing is how the badges and the census already tell a
//! hidden control from a shown one (`chrome::Control`), and a dock clipped to no width but still laid out would have its
//! badges drawn over the world and its widgets in the Tab order. A peek shows it again for as long as it floats.
//!
//! # How the rest goes
//!
//! **`Display::None`, and the frame is left as it is.** A hidden node lays out to nothing and the viewport node (which the world
//! is drawn through, `surface`) grows by flexbox into what the others leave. What each was before is kept on the node
//! ([`Folded`]) and put back, so a part that was hidden for another reason (the collapsed terminal) stays hidden after Zen.
//! **The view is enforced every frame it is on**: a part another system raises (the terminal's own chord) is folded again the
//! next frame. Zen also folds the split's first pane and its divider, so no sliver of either stands in a bare window.
//!
//! **Session state, not a setting.** A person who left Zen mode on would open the editor to a bare viewport and no menu; the
//! left panel's state is likewise the session's. Neither is written to the user's file.

use bevy::prelude::*;
use bevy_gooey::{CollapseSplit, Modifiers, Role, Shortcut, SplitDivider, SplitFraction, SplitGlide, SplitMin, SplitPeek, SplitSaved};

use crate::chrome::{BOTTOM_DOCK_H, BottomDock, Frame, FrameSystems, ShellSplit, WORKBENCH_FLOOR_H};
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
    /// **The bottom dock is asked for** (View ▸ Show reload panel): shown whatever the room, which is
    /// what a person on a short window with the terminal open (the default) has to do to read the reload state.
    pub reload_shown: bool,
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

/// The View menu's tick for the reload panel: the bottom dock, asked for.
#[derive(Component)]
pub struct ReloadPanelItem;

/// Everything Zen mode folds that has no field in the [`Frame`]: found by what it is.
type ZenParts = Or<(With<EditorMenuBar>, With<Toolbar>, With<TerminalPaneRoot>, With<EmptyMapPrompt>)>;

pub struct ShellViewPlugin;

impl Plugin for ShellViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShellView>().add_systems(
            Update,
            (
                toggle_on_the_chords,
                collapse_the_left_pane.after(FrameSystems),
                give_the_bottom_dock_its_room.after(FrameSystems),
                fold_the_shell.after(FrameSystems),
                tick_the_menu_rows,
            )
                .chain()
                .run_if(in_state(Screen::Editor)),
        );
    }
}

/// **The two chords.** Not while a text field has the keys (a chord typed into a field is the field's), nor over a question, nor
/// when the shell has the keyboard and the chord is on Ctrl (a control code it reads, `TerminalFocus::owns`: off a Mac every
/// primary chord). **Zen mode gives the terminal's keyboard back**, or the keys that move the camera would go on reaching a shell
/// nobody can see.
fn toggle_on_the_chords(
    keys: Res<ButtonInput<KeyCode>>,
    live: Res<Live>,
    confirm: Res<Confirm>,
    bindings: Res<bevy_gooey::BindingSettings>,
    mut focus: ResMut<crate::terminal_pane::TerminalFocus>,
    mut view: ResMut<ShellView>,
    mut commands: Commands,
) {
    let left = LEFT_PANEL_CHORD.just_pressed(&keys) && !focus.owns(&LEFT_PANEL_CHORD);
    let zen = zen_chord().just_pressed(&keys) && !focus.owns(&zen_chord());
    if !(left || zen) || live.0 == Context::Typing || confirm.is_open() {
        return;
    }
    if left {
        view.left_hidden = !view.left_hidden;
    }
    if zen {
        view.zen = !view.zen;
        if view.zen {
            focus.release();
            commands.spawn(bevy_gooey::toast(format!("Zen mode: {} brings the editor back", bindings.chord(zen_chord()))));
        }
    }
}

/// Where the shell's collapse of the left pane stands. The float is the pane's floor, [`SplitMin::first`], as the shell
/// authored it.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
enum Collapse {
    /// The pane is open, or collapsed by someone else.
    #[default]
    Open,
    /// The shell pressed Enter on the divider to collapse, and gooey has not answered yet.
    Asking(f32),
    /// The shell's collapse stands (or is sliding in); the floor is given back at a restore.
    Held(f32),
    /// A restore is under way; the floor is given back when its glide has landed.
    Giving(f32),
}

/// **Ask the shell split to collapse its first pane, or give it back**: gooey's [`CollapseSplit`], the one path the divider's
/// Enter, South button and double press take too, so the slide, the saved fraction, the peek and the one
/// `ValueChange<f32>` are the same whoever asks. Asking for the state the split is in does nothing.
fn ask_the_split(commands: &mut Commands, split: Entity, collapsed: bool) {
    commands.trigger(CollapseSplit { entity: split, collapsed });
}

/// What the shell reads of its split to collapse it.
type ShellSplitRead = (
    Entity,
    &'static mut SplitFraction,
    &'static mut SplitMin,
    &'static ComputedNode,
    &'static Children,
    Has<SplitSaved>,
    Has<SplitGlide>,
);

/// **Collapse the left pane while the view says it is hidden, restore it when it is not** — see the module's header.
///
/// One state per frame: [`Collapse`]. A person who docks the peek, or drags the divider, while the view says hidden has
/// opened the pane by hand, and the view follows them (the tick and the chord agree) rather than collapsing it again.
/// **A split that is not the one it last looked at starts over** (the frame is spawned on each door, with its own pane open
/// and its own floor): the view is session state and stays, so the new split is collapsed to match it, not mistaken for one
/// a person has opened.
fn collapse_the_left_pane(
    mut view: ResMut<ShellView>,
    mut splits: Query<ShellSplitRead, With<ShellSplit>>,
    roles: Query<&Role>,
    dividers: Query<&ComputedNode, With<SplitDivider>>,
    mut state: Local<Collapse>,
    mut looked_at: Local<Option<Entity>>,
    mut commands: Commands,
) {
    let Ok((split, mut fraction, mut min, node, kids, collapsed, gliding)) = splits.single_mut() else { return };
    if *looked_at != Some(split) {
        *looked_at = Some(split);
        *state = Collapse::Open;
    }
    let Some(divider) = kids.iter().find(|kid| matches!(roles.get(*kid), Ok(Role::SplitDivider))) else { return };
    let want = view.left_is_hidden();
    match *state {
        Collapse::Open if want => {
            // Laid out, or there is no room to answer for: gooey's collapse reads the same room and does nothing without it.
            let bar = dividers.get(divider).map_or(0.0, |d| d.size().x * d.inverse_scale_factor());
            let extent = node.size().x * node.inverse_scale_factor() - bar;
            if extent <= 0.0 || collapsed {
                return;
            }
            let floor = min.first;
            // What a restore gives back is never under the floor, so the pane opens to the width its panel needs.
            let at_least = (floor / extent).clamp(0.0, 1.0);
            if fraction.0 < at_least {
                fraction.0 = at_least;
            }
            min.first = 0.0;
            ask_the_split(&mut commands, split, true);
            *state = Collapse::Asking(floor);
        }
        Collapse::Open => {}
        Collapse::Asking(floor) => {
            if collapsed {
                *state = Collapse::Held(floor);
            } else {
                // gooey did not take it (no room yet): the floor goes back and the next frame asks again.
                min.first = floor;
                *state = Collapse::Open;
            }
        }
        Collapse::Held(floor) => {
            if collapsed && !want {
                ask_the_split(&mut commands, split, false);
                *state = Collapse::Giving(floor);
            } else if !collapsed && !gliding {
                // Docked by the person: the pane is open and the view says so.
                if want && !view.zen {
                    view.left_hidden = false;
                }
                *state = Collapse::Giving(floor);
            }
        }
        Collapse::Giving(floor) => {
            if collapsed && want {
                // Hidden again before it opened: collapse from where it is drawn.
                ask_the_split(&mut commands, split, true);
                *state = Collapse::Asking(floor);
            } else if !collapsed && !gliding {
                min.first = floor;
                *state = Collapse::Open;
            }
        }
    }
}

/// **The bottom dock: asked for, or given the room** (UI-61, #247). The one writer of its `display`.
///
/// A 148-px row under the body is taken from the body, and at the editor's reference window (1280 x
/// 720, terminal open) that left the inspector 24 px of visible height and the Kit tabs' badge legends
/// under the dock. So, unless the person has asked for it (View ▸ Show reload panel, [`ShellView::reload_shown`]),
/// the dock is a row only when the workbench keeps [`WORKBENCH_FLOOR_H`] beside it: `Display::None`
/// otherwise, which lays out to nothing and leaves no text for the census to find drawn outside a
/// clip. **The room is the body plus the dock's own row, whether it is shown or not**, so showing it
/// does not change the answer the next frame (no flicker at the threshold).
///
/// **Zen mode hides it, and the frame Zen ends on does not read the room.** [`fold_the_shell`] does not
/// touch the dock, so there is no stored display to put back (that put back a stale `Flex` for one frame
/// when the window had shrunk in Zen); and on that frame the layout is still Zen's, whose body is the
/// whole window, so a dock nobody asked for stays hidden until the next frame has measured the real one.
fn give_the_bottom_dock_its_room(
    view: Res<ShellView>,
    frame: Option<Res<Frame>>,
    laid: Query<&ComputedNode>,
    mut docks: Query<&mut Node, With<BottomDock>>,
    mut zen_last_frame: Local<bool>,
) {
    let Some(frame) = frame else { return };
    let Ok(mut dock) = docks.get_mut(frame.bottom) else { return };
    let just_left_zen = std::mem::replace(&mut *zen_last_frame, view.the_rest_is_hidden()) && !view.the_rest_is_hidden();
    let display = if view.the_rest_is_hidden() {
        Display::None
    } else if view.reload_shown {
        Display::Flex
    } else if just_left_zen {
        Display::None
    } else {
        let Ok(body) = laid.get(frame.body) else { return };
        let interface_px = |node: &ComputedNode| node.size().y * node.inverse_scale_factor();
        let own_row = if dock.display == Display::None { 0.0 } else { laid.get(frame.bottom).map_or(0.0, interface_px) };
        if interface_px(body) + own_row >= WORKBENCH_FLOOR_H + BOTTOM_DOCK_H { Display::Flex } else { Display::None }
    };
    if dock.display != display {
        dock.display = display;
    }
}

/// **Fold what the view hides, and unfold what it has let go of.** Every frame the view is on, the same answer, so a part
/// another system raised is folded again; a settled shell costs one comparison per part.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn fold_the_shell(
    view: Res<ShellView>,
    frame: Option<Res<Frame>>,
    parts: Query<Entity, ZenParts>,
    splits: Query<(&Children, Has<SplitSaved>, Has<SplitGlide>, &SplitPeek), With<ShellSplit>>,
    roles: Query<&Role>,
    folded: Query<(Entity, &Folded)>,
    mut nodes: Query<&mut Node>,
    mut commands: Commands,
) {
    let Some(frame) = frame else { return };
    let mut fold: Vec<Entity> = Vec::new();
    if view.the_rest_is_hidden() {
        fold.extend(parts.iter());
        fold.extend([frame.chrome_bar, frame.door_strip, frame.status, frame.right]);
    }
    for (kids, collapsed, gliding, peek) in &splits {
        if view.zen {
            // **The first pane and the divider only**: the children come `[pane, divider, pane]`, and the second pane holds the viewport.
            let own = |kid: &Entity| roles.get(*kid).is_ok_and(|role| matches!(role, Role::SplitPane | Role::SplitDivider));
            fold.extend(kids.iter().filter(own).take(2));
        }
        // **A shut pane shows no dock**: collapsed, landed, and nothing floating over the viewport.
        if view.left_is_hidden() && collapsed && !gliding && !peek.is_open() {
            fold.push(frame.left);
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
}

/// **The ticks say what the view is**, whichever way it changed: a chord, or the row.
fn tick_the_menu_rows(
    view: Res<ShellView>,
    left: Query<(Entity, Has<bevy_gooey::Checked>), With<LeftPanelItem>>,
    zen: Query<(Entity, Has<bevy_gooey::Checked>), With<ZenItem>>,
    reload: Query<(Entity, Has<bevy_gooey::Checked>), With<ReloadPanelItem>>,
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
    for (item, ticked) in &reload {
        tick(item, ticked, view.reload_shown);
    }
}

/// **View ▸ Show left panel**: the tick is the panel shown.
pub fn toggle_the_left_panel(change: On<bevy_gooey::ValueChange<bool>>, mut view: ResMut<ShellView>) {
    view.left_hidden = !change.value;
}

/// **View ▸ Show reload panel**: the tick is the bottom dock asked for.
pub fn toggle_the_reload_panel(change: On<bevy_gooey::ValueChange<bool>>, mut view: ResMut<ShellView>) {
    view.reload_shown = change.value;
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
                let view = ShellView { left_hidden, zen, ..ShellView::default() };
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
