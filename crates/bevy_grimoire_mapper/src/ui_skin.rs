//! **A game's UI skin, previewed live** (#147).
//!
//! The data is `bevy_gooey`'s own [`GooeySkin`], written as RON in `<project>/family/bevy_gooey.ron`.
//! The crate's type is `#[serde(default)]`, so a partial file such as `(accent: Srgba((red: 0.2,
//! green: 0.6, blue: 0.9, alpha: 1.0)))` is the default skin with only that field changed.
//!
//! [`apply_ui_skin`] replaces the running [`GooeySkin`] resource when the file changes, and the
//! crate's own `apply_skin` restyles every widget carrying a `Role` that frame. A file that is not a
//! skin is a problem on the status line and the last good skin stays; a skin that breaks the crate's
//! own rules ([`GooeySkin::problems`]: WCAG 2.2 contrast, target size, unusable sizes) is applied and
//! each broken rule is a problem. Removing the file puts the crate's default skin back.

use bevy::prelude::*;
use bevy_gooey::GooeySkin;

use crate::editor::EditorState;
use crate::family_host::FamilySettings;

/// The crate this settings file belongs to, as `family.ron` names it.
pub const CRATE: &str = "bevy_gooey";

/// Reads a skin file's text as the crate's own type.
pub fn from_ron(text: &str) -> Result<GooeySkin, String> {
    ron::from_str(text).map_err(|e| format!("bevy_gooey skin: {e}"))
}

/// Copies a changed `family/bevy_gooey.ron` into the running [`GooeySkin`].
pub fn apply_ui_skin(settings: Res<FamilySettings>, mut state: ResMut<EditorState>, mut skin: ResMut<GooeySkin>, mut seen: Local<Option<String>>) {
    let Some(text) = settings.text.get(CRATE) else {
        if seen.take().is_some() {
            *skin = GooeySkin::default();
        }
        return;
    };
    if seen.as_ref() == Some(text) {
        return;
    }
    *seen = Some(text.clone());
    let parsed = match from_ron(text) {
        Ok(p) => p,
        Err(e) => {
            state.status.problem(format!("{e}; the previous skin stays"));
            return;
        }
    };
    for problem in parsed.problems() {
        state.status.problem(format!("bevy_gooey skin: {problem}"));
    }
    if *skin != parsed {
        *skin = parsed;
    }
}
