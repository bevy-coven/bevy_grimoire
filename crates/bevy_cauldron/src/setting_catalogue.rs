//! **What the settings screen lists: every key of a settings file, with its words** (UI-71, #249).
//!
//! A file ([`SettingsLayer`]) says only the keys it sets. The screen needs more than a struct to draw a
//! key: what it is called, which category it sits under, what it does, what its default is, and what
//! kind of control sets it. That is [`Setting`], and [`CATALOGUE`] is the one list of them, so the
//! screen, its search and its Reset read the same table and a key added to [`SettingsLayer`] is listed
//! here in the same commit (`tests/the_settings_screen_lists_every_key.rs` checks the two agree).
//!
//! # A value, whatever the key's type
//!
//! [`Value`] is a number, a switch or a choice among named options, so [`SettingsLayer::get`],
//! [`SettingsLayer::set`] and [`SettingsLayer::clear`] reach any key by its name without the screen
//! knowing the field. [`write_key`] is the only place a settings file is written from the screen: it
//! reads the file as the editor reads it, changes one key, and writes the file back atomically, and it
//! refuses to touch a file that does not read, as every writer in this crate does.

use std::path::Path;

use crate::settings::{EditorSettings, GRID_STRENGTH_RANGE, KIND, SCALE_RANGE, SettingsLayer, Theme};

/// One key's value: what a control shows and what a write carries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    On(bool),
    Number(f32),
    /// The index of one of the key's named options.
    Option(usize),
}

/// How a key is set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Switch,
    /// A number between two ends, set by a slider; shown as a percentage.
    Scale { min: f32, max: f32 },
    /// One of a few named values.
    Options(&'static [&'static str]),
}

/// One key of a settings file, in the words the screen shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Setting {
    /// The key as the file spells it.
    pub key: &'static str,
    /// The words on the control: sentence case, unique among settings.
    pub label: &'static str,
    /// The tab it is listed under.
    pub category: &'static str,
    /// What it does, in a sentence.
    pub description: &'static str,
    pub kind: Kind,
}

/// The categories, in the order the screen's tabs show them.
pub const CATEGORIES: &[&str] = &["Appearance", "Accessibility", "Map"];

/// The names of [`Theme`]'s variants, in declaration order.
pub const THEMES: &[&str] = &["Tool dark", "Parchment", "High contrast"];

/// **Every key a settings file may set.** The order is the order within a category.
pub const CATALOGUE: &[Setting] = &[
    Setting {
        key: "theme",
        label: "Theme",
        category: "Appearance",
        description: "The look of the editor's chrome. The theme settings (UI-73) apply it.",
        kind: Kind::Options(THEMES),
    },
    Setting {
        key: "text_scale",
        label: "Text size",
        category: "Appearance",
        description: "Text as a multiple of the size the design was drawn at. The theme settings (UI-73) apply it.",
        kind: Kind::Scale { min: SCALE_RANGE.0, max: SCALE_RANGE.1 },
    },
    Setting {
        key: "interface_scale",
        label: "Interface size",
        category: "Appearance",
        description: "Spacing and targets as a multiple of the size the design was drawn at. The theme settings (UI-73) apply it.",
        kind: Kind::Scale { min: SCALE_RANGE.0, max: SCALE_RANGE.1 },
    },
    Setting {
        key: "reduced_motion",
        label: "Reduced motion",
        category: "Accessibility",
        description: "Changes land without easing. The theme settings (UI-73) apply it.",
        kind: Kind::Switch,
    },
    Setting {
        key: "show_grid",
        label: "Grid lines",
        category: "Map",
        description: "Draw the map's grid over the ground in the viewport.",
        kind: Kind::Switch,
    },
    Setting {
        key: "grid_strength",
        label: "Grid strength",
        category: "Map",
        description: "How strongly the grid is drawn; less is fainter.",
        kind: Kind::Scale { min: GRID_STRENGTH_RANGE.0, max: GRID_STRENGTH_RANGE.1 },
    },
];

/// The setting with this key.
pub fn find(key: &str) -> Option<&'static Setting> {
    CATALOGUE.iter().find(|s| s.key == key)
}

/// **Does `query` find this setting?** Every word of the query is somewhere in the setting's label,
/// key, category or description, whatever the case.
pub fn matches(setting: &Setting, query: &str) -> bool {
    let hay = format!("{} {} {} {}", setting.label, setting.key, setting.category, setting.description).to_lowercase();
    query.split_whitespace().all(|word| hay.contains(&word.to_lowercase()))
}

/// The settings `query` finds, in catalogue order; all of them when the query is empty. With a
/// `category`, only that tab's.
pub fn search(query: &str, category: Option<&str>) -> Vec<&'static Setting> {
    CATALOGUE
        .iter()
        .filter(|s| category.is_none_or(|c| s.category == c))
        .filter(|s| matches(s, query))
        .collect()
}

fn theme_index(theme: Theme) -> usize {
    match theme {
        Theme::ToolDark => 0,
        Theme::Parchment => 1,
        Theme::HighContrast => 2,
    }
}

fn theme_at(index: usize) -> Theme {
    match index {
        1 => Theme::Parchment,
        2 => Theme::HighContrast,
        _ => Theme::ToolDark,
    }
}

impl EditorSettings {
    /// The value this key has now.
    pub fn value(&self, key: &str) -> Option<Value> {
        Some(match key {
            "theme" => Value::Option(theme_index(self.theme)),
            "text_scale" => Value::Number(self.text_scale),
            "interface_scale" => Value::Number(self.interface_scale),
            "reduced_motion" => Value::On(self.reduced_motion),
            "show_grid" => Value::On(self.show_grid),
            "grid_strength" => Value::Number(self.grid_strength),
            _ => return None,
        })
    }
}

/// The value a key has when no file sets it.
pub fn default_value(key: &str) -> Option<Value> {
    EditorSettings::default().value(key)
}

/// **A value as the screen says it.**
pub fn say(setting: &Setting, value: Value) -> String {
    match (setting.kind, value) {
        (Kind::Switch, Value::On(on)) => if on { "On" } else { "Off" }.to_owned(),
        (Kind::Scale { .. }, Value::Number(n)) => format!("{}", (n * 100.0).round() / 100.0),
        (Kind::Options(names), Value::Option(i)) => names.get(i).copied().unwrap_or("?").to_owned(),
        _ => "?".to_owned(),
    }
}

impl SettingsLayer {
    /// The value this layer sets for `key`, or `None` when it leaves the key out.
    pub fn get(&self, key: &str) -> Option<Value> {
        match key {
            "theme" => self.theme.map(|t| Value::Option(theme_index(t))),
            "text_scale" => self.text_scale.map(Value::Number),
            "interface_scale" => self.interface_scale.map(Value::Number),
            "reduced_motion" => self.reduced_motion.map(Value::On),
            "show_grid" => self.show_grid.map(Value::On),
            "grid_strength" => self.grid_strength.map(Value::Number),
            _ => None,
        }
    }

    /// Sets `key` to `value`; false when there is no such key or the value is of another kind.
    pub fn set(&mut self, key: &str, value: Value) -> bool {
        match (key, value) {
            ("theme", Value::Option(i)) => self.theme = Some(theme_at(i)),
            ("text_scale", Value::Number(n)) => self.text_scale = Some(n),
            ("interface_scale", Value::Number(n)) => self.interface_scale = Some(n),
            ("reduced_motion", Value::On(on)) => self.reduced_motion = Some(on),
            ("show_grid", Value::On(on)) => self.show_grid = Some(on),
            ("grid_strength", Value::Number(n)) => self.grid_strength = Some(n),
            _ => return false,
        }
        true
    }

    /// Leaves `key` out of the layer: the layer below decides it again.
    pub fn clear(&mut self, key: &str) {
        match key {
            "theme" => self.theme = None,
            "text_scale" => self.text_scale = None,
            "interface_scale" => self.interface_scale = None,
            "reduced_motion" => self.reduced_motion = None,
            "show_grid" => self.show_grid = None,
            "grid_strength" => self.grid_strength = None,
            _ => {}
        }
    }
}

/// **Writes one key of the settings file at `path`**, or takes it out when `value` is `None`; every other
/// key in the file stays as the file has it.
///
/// The file is read first, as the editor reads it ([`SettingsLayer::read`]): one that does not read is
/// refused with the file and the field and **left exactly as it is**, because writing over it would
/// throw away what a person typed. A file that is not there is made, with its folder. The write is
/// atomic ([`bevy_grimoire_core::atomic_file::write`]), so the watcher never sees half a file.
pub fn write_key(path: &Path, key: &str, value: Option<Value>) -> Result<(), String> {
    let file = path.to_string_lossy().replace('\\', "/");
    let mut layer = match std::fs::read_to_string(path) {
        Ok(text) => SettingsLayer::read(&file, &text).map_err(|r| format!("{file} does not read ({}): {}; not changed", r.field.as_deref().unwrap_or("file"), r.reason))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => SettingsLayer::default(),
        Err(e) => return Err(format!("{file}: {e}")),
    };
    match value {
        Some(v) => {
            if !layer.set(key, v) {
                return Err(format!("`{key}` is not a {KIND} key that takes {v:?}"));
            }
        }
        None => layer.clear(key),
    }
    // Refuse a value the next read would refuse: the file must read back.
    let text = ron::ser::to_string_pretty(&layer, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
    SettingsLayer::read(&file, &text).map_err(|r| format!("{key}: {}", r.reason))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    bevy_grimoire_core::atomic_file::write(path, text).map_err(|e| format!("{file}: {e}"))
}
