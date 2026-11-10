//! **The terminal's agent is a setting** (`terminal_agent`, `terminal_agent_command`): the screen lists
//! the harness, the file alone names a custom command, and a blank command is refused at the door.

use bevy_cauldron::setting_catalogue::{self, Kind, TERMINAL_AGENTS};
use bevy_cauldron::settings::{EditorSettings, SettingsLayer, TerminalAgent};

const COMMAND: &str = "claude --mcp-config $CAULDRON_MCP_CONFIG";

#[test]
fn a_file_chooses_the_agent_and_its_command() {
    let layer = SettingsLayer::read(
        "t",
        &format!("{{version: 1, terminal_agent: OhMyPi, terminal_agent_command: \"{COMMAND}\"}}"),
    )
    .unwrap_or_else(|r| panic!("{r:?}"));
    let settings = EditorSettings::resolve(None, Some(&layer));
    assert_eq!(settings.terminal_agent, TerminalAgent::OhMyPi);
    assert_eq!(settings.terminal_agent_command.as_deref(), Some(COMMAND));
    assert_eq!(EditorSettings::default().terminal_agent, TerminalAgent::Off, "a terminal opens a shell until an agent is chosen");
}

#[test]
fn a_blank_command_is_refused_with_its_field() {
    let refusal = SettingsLayer::read("t", "{version: 1, terminal_agent_command: \"  \"}")
        .expect_err("a command of spaces runs nothing, so the file is refused");
    assert_eq!(refusal.field.as_deref(), Some("terminal_agent_command"));
}

#[test]
fn the_screen_offers_the_harnesses_and_not_the_command() {
    let setting = setting_catalogue::find("terminal_agent").expect("the screen lists the agent");
    assert_eq!(setting.kind, Kind::Options(TERMINAL_AGENTS));
    assert_eq!(setting.category, "Terminal");
    assert!(
        setting_catalogue::find("terminal_agent_command").is_none(),
        "the screen has no text control, so the command is the file's alone"
    );
}
