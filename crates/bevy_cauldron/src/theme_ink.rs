//! **A theme changes every colour the editor draws, live** (#428).
//!
//! # What was wrong
//!
//! Picking another theme in Settings changed gooey's widgets and nothing else. `chrome.rs` draws its
//! headings, rows, hints, chips and status band with [`crate::chrome::Ink`], and `Ink` was read once from
//! `editor_skin.yaml` and never again, so those colours were the dark theme's whatever ran. The owner's
//! saved theme was High contrast and the editor looked as it always had (`feedback-1794240972472`:
//! *"Changing the theme doesn't seem to change the UI at all"*).
//!
//! # What it is now
//!
//! There is one path. [`follow_the_skin`] watches the running [`GooeySkin`]; when its colours change it
//! rebuilds the ink from it ([`Ink::of`], the function that built the original) and moves **everything
//! already drawn** from the old ink onto the new one. Painters that read [`crate::chrome::ink`] each frame
//! (every gizmo, every hover and selection painter) follow by themselves from the next read; painters
//! that wrote a colour once at spawn (a heading, a chip, a panel's edge, the status band) are moved here.
//!
//! **A colour is moved only if it is the old ink's.** [`Ink::remap`] answers "what is this colour in the
//! new ink?" for a colour that is a field of the old one (or one at another alpha), and `None` for
//! anything else — a measured colour, a game's, a terminal program's — which is left alone. So nothing
//! here needs to know which entity is a heading and which is a chip: if it was drawn in the theme's
//! `text`, it is drawn in the new theme's `text`.
//!
//! # What is not moved
//!
//! **An image's tint** ([`ImageNode`]) is the picture's, not the theme's: its default is white, which a
//! by-value match cannot tell from High contrast's white `text`, so a card's picture would be retinted to
//! near-black by leaving High contrast. **A `StandardMaterial`'s base colour** is the same case (a model's default is white), so the
//! animation stage's ghost keeps the tint it was spawned with. And a colour at alpha 0 ([`Color::NONE`]) is nobody's.
//!
//! # What is moved
//!
//! A UI node's [`BackgroundColor`], [`BorderColor`], [`TextColor`], [`Outline`];
//! gooey's [`InkOverride`] (which restyles the node it is on); the colours the editor's own components
//! carry so that a painter can put them back ([`Ground`], [`RowRest`], [`BadgeRest`]); the window's
//! [`ClearColor`] and a camera's custom clear; and the Feathers theme's tokens.
//!
//! # When
//!
//! In `PostUpdate`, which is after every `Update` system and after their commands have been applied, so
//! everything spawned this frame is in the world and is drawn in the old ink: the swap and the move are
//! one step with nothing spawned between them. A colour written *after* it (a painter's next frame) reads
//! the new ink.

use bevy::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy_gooey::{GooeySkin, InkOverride};

use crate::badges::BadgeRest;
use crate::chrome::{self, Ground, Ink, RowRest};

/// **Every place the editor stores a colour it drew from the ink.**
#[derive(SystemParam)]
pub struct Painted<'w, 's> {
    fills: Query<'w, 's, &'static mut BackgroundColor>,
    borders: Query<'w, 's, &'static mut BorderColor>,
    texts: Query<'w, 's, &'static mut TextColor>,
    outlines: Query<'w, 's, &'static mut Outline>,
    overrides: Query<'w, 's, &'static mut InkOverride>,
    grounds: Query<'w, 's, &'static mut Ground>,
    rests: Query<'w, 's, &'static mut RowRest>,
    badges: Query<'w, 's, &'static mut BadgeRest>,
    cameras: Query<'w, 's, &'static mut Camera>,
}

/// Moves one colour from the old ink to the new, writing only when it differs, so a node the ink does
/// not touch is not marked changed.
fn moved(from: &Ink, to: &Ink, colour: Color) -> Option<Color> {
    Ink::remap(from, to, colour).filter(|next| *next != colour)
}

impl Painted<'_, '_> {
    /// Every stored colour that was `from`'s becomes `to`'s.
    fn move_onto(&mut self, from: &Ink, to: &Ink) {
        let on = |c: Color| moved(from, to, c);
        for mut fill in &mut self.fills {
            if let Some(next) = on(fill.0) {
                fill.0 = next;
            }
        }
        for mut border in &mut self.borders {
            let sides = [border.top, border.right, border.bottom, border.left].map(&on);
            if sides.iter().any(Option::is_some) {
                let [top, right, bottom, left] = sides;
                border.top = top.unwrap_or(border.top);
                border.right = right.unwrap_or(border.right);
                border.bottom = bottom.unwrap_or(border.bottom);
                border.left = left.unwrap_or(border.left);
            }
        }
        for mut text in &mut self.texts {
            if let Some(next) = on(text.0) {
                text.0 = next;
            }
        }
        for mut outline in &mut self.outlines {
            if let Some(next) = on(outline.color) {
                outline.color = next;
            }
        }
        for mut ink in &mut self.overrides {
            let [fill, text, edge] = [ink.fill, ink.ink, ink.edge].map(|c| c.and_then(&on));
            if fill.is_some() || text.is_some() || edge.is_some() {
                ink.fill = fill.or(ink.fill);
                ink.ink = text.or(ink.ink);
                ink.edge = edge.or(ink.edge);
            }
        }
        for mut ground in &mut self.grounds {
            if let Some(next) = on(ground.0) {
                ground.0 = next;
            }
        }
        for mut rest in &mut self.rests {
            if let Some(next) = on(rest.0) {
                rest.0 = next;
            }
        }
        for mut rest in &mut self.badges {
            let [fill, edge] = [rest.0, rest.1].map(&on);
            if fill.is_some() || edge.is_some() {
                rest.0 = fill.unwrap_or(rest.0);
                rest.1 = edge.unwrap_or(rest.1);
            }
        }
        for mut camera in &mut self.cameras {
            if let bevy::camera::ClearColorConfig::Custom(colour) = camera.clear_color
                && let Some(next) = on(colour)
            {
                camera.clear_color = bevy::camera::ClearColorConfig::Custom(next);
            }
        }
    }
}

/// **The running skin's colours are the editor's ink, and what was drawn in the old one moves** (#428).
///
/// A pass whose skin did not change costs one flag read; one whose skin changed but not its colours (a
/// text size) builds the ink, finds it equal to the one running and returns.
pub fn follow_the_skin(
    skin: Res<GooeySkin>,
    mut painted: Painted,
    clear: Option<ResMut<ClearColor>>,
    feathers: Option<ResMut<bevy::feathers::theme::UiTheme>>,
) {
    if !skin.is_changed() {
        return;
    }
    let next = Ink::of(&skin);
    let previous = chrome::ink();
    if previous == next {
        return;
    }
    chrome::swap_ink(next);
    painted.move_onto(&previous, &next);
    if let Some(mut clear) = clear {
        clear.0 = next.void;
    }
    if let Some(mut feathers) = feathers {
        // Seeded from the ink, which is the new one now.
        *feathers = bevy::feathers::theme::UiTheme(chrome::theme());
    }
}

/// Adds [`follow_the_skin`], last in the frame.
pub struct ThemeInkPlugin;

impl Plugin for ThemeInkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, follow_the_skin);
    }
}
