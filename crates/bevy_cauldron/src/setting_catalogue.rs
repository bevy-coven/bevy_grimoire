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

use crate::settings::{EditorSettings, GRID_STRENGTH_RANGE, KIND, SettingsLayer, TerminalAgent, TerminalCursor, Theme};

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

/// **The ends of the two size sliders**: 75% to 200%. A file may set any size
/// [`crate::settings::SCALE_RANGE`] allows, and a slider shows the nearest end of this range for one
/// outside it; the range is what a person can reach by moving the control.
pub const SIZE_SLIDER: (f32, f32) = (0.75, 2.0);

/// **How far one step of a scale slider moves it**: a twentieth, 5%.
pub const SCALE_STEP: f32 = 0.05;

/// The categories, in the order the screen's tabs show them.
pub const CATEGORIES: &[&str] = &["Appearance", "Accessibility", "Map", "Terminal"];

/// The names of [`Theme`]'s variants, in declaration order.
pub const THEMES: &[&str] = &["Tool dark", "Parchment", "High contrast"];

/// The names of [`TerminalCursor`]'s variants, in declaration order.
pub const TERMINAL_CURSORS: &[&str] = &["Block", "Underline", "Bar"];

/// The names of [`TerminalAgent`]'s variants, in declaration order.
pub const TERMINAL_AGENTS: &[&str] = &["Off", "Oh My Pi", "Custom command"];

/// **The scrollback lengths the screen offers** (#324), and their words. A file may set any length
/// [`crate::settings::SCROLLBACK_RANGE`] allows; the screen shows the nearest of these for one between.
pub const SCROLLBACK_CHOICES: &[usize] = &[1_000, 5_000, 10_000, 50_000];
pub const SCROLLBACK_NAMES: &[&str] = &["1,000 lines", "5,000 lines", "10,000 lines", "50,000 lines"];

/// **Every key a settings file may set.** The order is the order within a category.
pub const CATALOGUE: &[Setting] = &[
    Setting {
        key: "theme",
        label: "Theme",
        category: "Appearance",
        description: "The colours and look of the editor.",
        kind: Kind::Options(THEMES),
    },
    Setting {
        key: "text_scale",
        label: "Text size",
        category: "Appearance",
        description: "How large the text is. 100% is the size the editor was designed at.",
        kind: Kind::Scale { min: SIZE_SLIDER.0, max: SIZE_SLIDER.1 },
    },
    Setting {
        key: "interface_scale",
        label: "Interface size",
        category: "Appearance",
        description: "How large buttons and spacing are. 100% is the size the editor was designed at.",
        kind: Kind::Scale { min: SIZE_SLIDER.0, max: SIZE_SLIDER.1 },
    },
    Setting {
        key: "show_statistics",
        label: "Show statistics",
        category: "Appearance",
        description: "Show how many triangles are drawn, at the top right of the window.",
        kind: Kind::Switch,
    },
    Setting {
        key: "show_activity_bar",
        label: "Show activity bar",
        category: "Appearance",
        description: "Show the strip of workspace icons at the left edge of the window. Off, move between workspaces with the keys or the Window menu.",
        kind: Kind::Switch,
    },
    Setting {
        key: "reduced_motion",
        label: "Reduced motion",
        category: "Accessibility",
        description: "Dialogs, menus and panels stop sliding and growing and fade in place instead; colours still ease.",
        kind: Kind::Switch,
    },
    Setting {
        key: "follow_agent",
        label: "Follow the agent",
        category: "Map",
        description: "The view goes to each change the Familiar makes, one at a time, waiting while you are working in the viewport. Off, nothing moves and the look chip still offers Look.",
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
    Setting {
        key: "terminal_text_scale",
        label: "Terminal text size",
        category: "Terminal",
        description: "How large the terminal's text is. 100% is the size it ships at; the shell is told the new rows and columns.",
        kind: Kind::Scale { min: SIZE_SLIDER.0, max: SIZE_SLIDER.1 },
    },
    Setting {
        key: "terminal_cursor",
        label: "Terminal cursor",
        category: "Terminal",
        description: "The terminal's cursor when a program asks for none: a block over the character, an underline or a bar.",
        kind: Kind::Options(TERMINAL_CURSORS),
    },
    Setting {
        key: "terminal_cursor_blink",
        label: "Terminal cursor blinks",
        category: "Terminal",
        description: "The terminal's cursor blinks; off, it stays lit.",
        kind: Kind::Switch,
    },
    Setting {
        key: "terminal_scrollback",
        label: "Terminal scrollback",
        category: "Terminal",
        description: "How many lines of output each terminal keeps to scroll back through and to find in.",
        kind: Kind::Options(SCROLLBACK_NAMES),
    },
    Setting {
        key: "terminal_agent",
        label: "Terminal agent",
        category: "Terminal",
        description: "The agent the terminal's Start agent button starts, connected to the editor's tools and checked with a self test. The terminal starts no agent by itself: a session costs tokens to warm up. Custom command runs terminal_agent_command from settings.yaml, and is offered once a command is set.",
        kind: Kind::Options(TERMINAL_AGENTS),
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

/// Where `theme` sits in [`THEMES`], the order the Settings row offers.
///
/// `pub` because a message that has to name a theme must use the same word the row does — one table,
/// so a refusal and the setting cannot call Parchment two things.
pub fn theme_index(theme: Theme) -> usize {
    match theme {
        Theme::ToolDark => 0,
        Theme::Parchment => 1,
        Theme::HighContrast => 2,
    }
}

fn cursor_index(cursor: TerminalCursor) -> usize {
    match cursor {
        TerminalCursor::Block => 0,
        TerminalCursor::Underline => 1,
        TerminalCursor::Bar => 2,
    }
}

fn cursor_at(index: usize) -> TerminalCursor {
    match index {
        1 => TerminalCursor::Underline,
        2 => TerminalCursor::Bar,
        _ => TerminalCursor::Block,
    }
}

/// Where an agent sits in [`TERMINAL_AGENTS`], for a screen that shows or writes the choice. Public
/// because the Settings screen shows the terminal's effective agent (`settings_screen::refresh`) and
/// that is the one place the option order is defined.
pub fn agent_index(agent: TerminalAgent) -> usize {
    match agent {
        TerminalAgent::Off => 0,
        TerminalAgent::OhMyPi => 1,
        TerminalAgent::Custom => 2,
    }
}

fn agent_at(index: usize) -> TerminalAgent {
    match index {
        1 => TerminalAgent::OhMyPi,
        2 => TerminalAgent::Custom,
        _ => TerminalAgent::Off,
    }
}

/// The offered scrollback length nearest `lines`.
fn scrollback_index(lines: usize) -> usize {
    SCROLLBACK_CHOICES
        .iter()
        .enumerate()
        .min_by_key(|(_, choice)| choice.abs_diff(lines))
        .map_or(0, |(at, _)| at)
}

fn scrollback_at(index: usize) -> usize {
    SCROLLBACK_CHOICES.get(index).copied().unwrap_or(crate::terminal::SCROLLBACK)
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
            "follow_agent" => Value::On(self.follow_agent),
            "show_grid" => Value::On(self.show_grid),
            "grid_strength" => Value::Number(self.grid_strength),
            "show_statistics" => Value::On(self.show_statistics),
            "show_activity_bar" => Value::On(self.show_activity_bar),
            "terminal_text_scale" => Value::Number(self.terminal_text_scale),
            "terminal_cursor" => Value::Option(cursor_index(self.terminal_cursor)),
            "terminal_cursor_blink" => Value::On(self.terminal_cursor_blink),
            "terminal_scrollback" => Value::Option(scrollback_index(self.terminal_scrollback)),
            "terminal_agent" => Value::Option(agent_index(self.terminal_agent)),
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
            "follow_agent" => self.follow_agent.map(Value::On),
            "show_grid" => self.show_grid.map(Value::On),
            "grid_strength" => self.grid_strength.map(Value::Number),
            "show_statistics" => self.show_statistics.map(Value::On),
            "show_activity_bar" => self.show_activity_bar.map(Value::On),
            "terminal_text_scale" => self.terminal_text_scale.map(Value::Number),
            "terminal_cursor" => self.terminal_cursor.map(|c| Value::Option(cursor_index(c))),
            "terminal_cursor_blink" => self.terminal_cursor_blink.map(Value::On),
            "terminal_scrollback" => self.terminal_scrollback.map(|lines| Value::Option(scrollback_index(lines))),
            "terminal_agent" => self.terminal_agent.map(|a| Value::Option(agent_index(a))),
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
            ("follow_agent", Value::On(on)) => self.follow_agent = Some(on),
            ("show_grid", Value::On(on)) => self.show_grid = Some(on),
            ("grid_strength", Value::Number(n)) => self.grid_strength = Some(n),
            ("show_statistics", Value::On(on)) => self.show_statistics = Some(on),
            ("show_activity_bar", Value::On(on)) => self.show_activity_bar = Some(on),
            ("terminal_text_scale", Value::Number(n)) => self.terminal_text_scale = Some(n),
            ("terminal_cursor", Value::Option(i)) => self.terminal_cursor = Some(cursor_at(i)),
            ("terminal_cursor_blink", Value::On(on)) => self.terminal_cursor_blink = Some(on),
            ("terminal_scrollback", Value::Option(i)) => self.terminal_scrollback = Some(scrollback_at(i)),
            ("terminal_agent", Value::Option(i)) => self.terminal_agent = Some(agent_at(i)),
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
            "follow_agent" => self.follow_agent = None,
            "show_grid" => self.show_grid = None,
            "grid_strength" => self.grid_strength = None,
            "show_statistics" => self.show_statistics = None,
            "show_activity_bar" => self.show_activity_bar = None,
            "terminal_text_scale" => self.terminal_text_scale = None,
            "terminal_cursor" => self.terminal_cursor = None,
            "terminal_cursor_blink" => self.terminal_cursor_blink = None,
            "terminal_scrollback" => self.terminal_scrollback = None,
            "terminal_agent" => self.terminal_agent = None,
            _ => {}
        }
    }
}

/// **The agent choices a Terminal agent row offers** (#425): `Custom command` only once a command is set, or while it is the choice
/// the row already shows (so the row never misreports what is in use). Stepping through them can never land on a choice that would be
/// refused.
pub fn agent_choices(has_command: bool, current: TerminalAgent) -> &'static [&'static str] {
    if has_command || current == TerminalAgent::Custom { TERMINAL_AGENTS } else { &TERMINAL_AGENTS[..agent_index(TerminalAgent::Custom)] }
}

/// **Writes one key of the settings file at `path`**, or takes it out when `value` is `None`; every other
/// key in the file stays as the file has it.
///
/// The file is read first, as the editor reads it ([`SettingsLayer::read`]): one that does not read is
/// refused with the file and the field and **left exactly as it is**, because writing over it would
/// throw away what a person typed. A file that is not there is made, with its folder. The write is
/// atomic ([`bevy_grimoire_core::atomic_file::write`]), so the watcher never sees half a file.
pub fn write_key(path: &Path, key: &str, value: Option<Value>) -> Result<(), String> {
    edit_layer(path, |layer| match value {
        Some(v) if !layer.set(key, v) => Err(format!("`{key}` is not a {KIND} key that takes {v:?}")),
        // **A choice without a command is never saved** (#425): `Custom` runs `terminal_agent_command`, so a file that chooses it
        // without one is a terminal whose agent cannot start. Only this key is held to it, so a file that already has the problem
        // can still be changed everywhere else.
        Some(_) if key == "terminal_agent" && layer.terminal_agent == Some(TerminalAgent::Custom) && layer.terminal_agent_command.is_none() => {
            Err("Custom command needs a command to run: type it in the Terminal agent row first, nothing was saved".to_owned())
        }
        Some(_) => Ok(()),
        None => {
            layer.clear(key);
            Ok(())
        }
    })
}

/// **Changes the settings file at `path` through `change`**, with every guarantee of [`write_key`]: the
/// file is read as the editor reads it and left alone when it does not read, what `change` leaves must read
/// back, and the write is atomic.
pub fn edit_layer(path: &Path, change: impl FnOnce(&mut SettingsLayer) -> Result<(), String>) -> Result<(), String> {
    let file = path.to_string_lossy().replace('\\', "/");
    let mut layer = match std::fs::read_to_string(path) {
        Ok(text) => SettingsLayer::read(&file, &text).map_err(|r| format!("{file} does not read ({}): {}; not changed", r.field.as_deref().unwrap_or("file"), r.reason))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => SettingsLayer::default(),
        Err(e) => return Err(format!("{file}: {e}")),
    };
    change(&mut layer)?;
    // Refuse a value the next read would refuse: the file must read back.
    let text = bevy_grimoire_core::yaml::to_string(&layer).map_err(|p| p.line())?;
    SettingsLayer::read(&file, &text).map_err(|r| r.reason)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    bevy_grimoire_core::atomic_file::write(path, text).map_err(|e| format!("{file}: {e}"))
}
