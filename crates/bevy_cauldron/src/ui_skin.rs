//! **A project's game skin previews only inside the Game UI preview panel** (UI-03, #211).
//!
//! The data is `bevy_gooey`'s own [`GooeySkin`], written as YAML in `<project>/family/bevy_gooey.yaml`
//! — the file a game ships so its own UI matches its art. The crate's type is
//! `#[serde(default)]`, so a partial file such as `accent: !Srgba {red: 0.2, green: 0.6, blue: 0.9,
//! alpha: 1.0}` is the library's default skin with only that field changed.
//!
//! # The editor's chrome never takes it
//!
//! A project's skin is a **game**'s palette, and the editor is not the game: the Cauldron's panels,
//! menus and status band are the editor's own measured darks (UI-02, #210), which is why a light game
//! skin cannot make the editor unreadable. The preview is the one place a game's palette is shown, and
//! it shows it the way the library means it to be shown — a [`SkinScope`] on a subtree, so every
//! widget below it is drawn from that skin while the rest of the window keeps the running one.
//!
//! [`apply_ui_skin`] therefore puts the file's skin into [`GameSkin`], a resource of its own, and
//! never into the running [`GooeySkin`]. [`skin_scope`] is what reads it onto the panel, and it does
//! so **whether the panel is open or not**: a scope that only appeared when the panel opened would
//! restyle a frame late, and a test that edited the file would have to guess how many frames to wait.
//!
//! # What a person sees when the file is wrong
//!
//! A file that is not a skin is a problem on the status line and the last good game skin stays; a skin
//! that breaks the crate's own rules ([`GooeySkin::problems`]: WCAG 2.2 contrast, target size,
//! unusable sizes) is applied and each broken rule is a problem. Removing the file clears the preview
//! back to the editor's own skin, and the panel says so rather than drawing an empty box.

use bevy::prelude::*;
use bevy_gooey::{GooeySkin, SkinScope};

use crate::editor::EditorState;
use crate::family_host::FamilySettings;

/// The crate this settings file belongs to, as `family.yaml` names it.
pub const CRATE: &str = "bevy_gooey";

/// Reads a skin file's text as the crate's own type.
pub fn from_yaml(text: &str) -> Result<GooeySkin, String> {
    bevy_grimoire_core::yaml::from_str(text, "family/bevy_gooey.yaml").map_err(|p| format!("bevy_gooey skin: {}", p.line()))
}

/// **The project's game skin, as the file says it** (UI-03, #211).
///
/// A resource of its own, and deliberately **not** the running [`GooeySkin`]: a game's palette belongs
/// inside the Game UI preview panel and nowhere else, so this is the only thing that carries it and
/// [`skin_scope`] is the only thing that reads it onto the screen. `None` is a project with no
/// `family/bevy_gooey.yaml` — the panel then previews the editor's own skin and says there is nothing
/// shipped yet.
///
/// **It is not a `Local` on [`apply_ui_skin`]** because two systems and a test read it: the preview's
/// spawn, the panel's own labels, and `tests/a_project_skin_previews_only_in_the_panel.rs`.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct GameSkin(pub Option<GooeySkin>);

impl GameSkin {
    /// The skin the preview draws with: the project's own, or the editor's where a project ships none.
    ///
    /// Falling back to the editor's skin rather than to gooey's default is the panel's honest answer:
    /// a project that ships no game UI still has an editor, and the preview shows what the game would
    /// look like on the palette the person is already looking at.
    pub fn previewed(&self) -> GooeySkin {
        self.0.clone().unwrap_or_else(crate::ui::editor_skin)
    }
}

/// Copies a changed `family/bevy_gooey.yaml` into [`GameSkin`], and **never** into the running skin.
///
/// The running [`GooeySkin`] is the editor's own (UI-02, #210): a project's file is a game's palette
/// and reaches the window only through the Game UI preview panel's [`SkinScope`].
pub fn apply_ui_skin(
    settings: Res<FamilySettings>,
    mut state: ResMut<EditorState>,
    mut game: ResMut<GameSkin>,
    mut seen: Local<Option<String>>,
) {
    let Some(text) = settings.text.get(CRATE) else {
        if seen.take().is_some() {
            game.0 = None;
        }
        return;
    };
    if seen.as_ref() == Some(text) {
        return;
    }
    *seen = Some(text.clone());
    let parsed = match from_yaml(text) {
        Ok(p) => p,
        Err(e) => {
            state.status.problem(format!("{e}; the previous skin stays"));
            return;
        }
    };
    for problem in parsed.problems() {
        state.status.problem(format!("bevy_gooey skin: {problem}"));
    }
    if game.0.as_ref() != Some(&parsed) {
        game.0 = Some(parsed);
    }
}

/// **The subtree the project's skin is drawn on**, as a component to put on a node.
///
/// A [`SkinScope`] of the project's game skin, or none at all when the project ships no file — a scope
/// holding a copy of the editor's skin would be a second place the editor's palette is stated, and
/// gooey's rule is that no scope means the resource, which is already the editor's.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct ProjectSkinScope;

/// **Keeps the preview's scope on the file's skin** (UI-03, #211).
///
/// One expression for a scope's value, so the panel's spawn and every later frame agree: the scope is
/// inserted the frame the file lands, replaced when the file changes, and removed when the project
/// stops shipping one — which puts the preview back on the editor's own skin with nothing left behind.
/// A settled editor costs one comparison.
pub fn skin_scope(
    game: Res<GameSkin>,
    fresh: Query<Entity, (With<ProjectSkinScope>, Added<ProjectSkinScope>)>,
    scopes: Query<(Entity, Option<&SkinScope>), With<ProjectSkinScope>>,
    mut commands: Commands,
) {
    // **A settled editor does nothing.** Two things make a frame worth working: the file's skin moved,
    // or a panel carrying the marker was spawned since the last frame (the panel is spawned on demand,
    // so its first frame is always later than the file's). Everything else is a walk that can neither
    // find nor fix anything.
    if !game.is_changed() && fresh.is_empty() {
        return;
    }
    let wanted = game.0.clone();
    for (entity, held) in &scopes {
        match (&wanted, held) {
            (Some(want), Some(scope)) if scope.0 == *want => {}
            (Some(want), _) => {
                commands.entity(entity).insert(SkinScope(want.clone()));
            }
            // **No file, no scope**: an empty scope would be a second place the editor's palette is
            // stated, and gooey's rule is that no scope means the resource, which already is it.
            (None, Some(_)) => {
                commands.entity(entity).remove::<SkinScope>();
            }
            (None, None) => {}
        }
    }
}

pub struct GameUiPreviewPlugin;

impl Plugin for GameUiPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameSkin>()
            .add_systems(
                Update,
                (apply_ui_skin, skin_scope)
                    .chain()
                    .after(crate::family_host::Reloaded)
                    .before(bevy_gooey::GooeySystems::Style),
            );
    }
}
