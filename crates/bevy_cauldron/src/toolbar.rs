//! **The toolbar** (UI-37, #231): the tool modes, undo and redo, save, and the view and capture
//! helpers, as a row of gooey icon buttons above the docks.
//!
//! **One body per verb, and it is the key's.** Every button carries the id of a registered command
//! (`crate::commands`) and, when it is clicked, hands that id to [`crate::command_run::Chords`] — the
//! same queue the command palette uses. The chord goes down a frame or two later and the editor's own
//! key handler judges it, exactly as if a person had pressed the key. So **a button cannot arm a tool
//! one way and its key another**: there is only one arming path, and it is the keyboard's.
//!
//! That is also why the toolbar holds no state of its own. Which button looks "on" is read from
//! `EditorState::tool` every frame, and a click changes nothing directly; the key press changes the
//! tool, and the buttons follow. The alternative — the button setting `Tool::Move` itself — is the
//! second code path `arm_tool` was written to delete.
//!
//! **Fill is not a tool.** `Tool` has six variants and no `Fill`; `Action::Fill` is a one-shot flood
//! from the cursor, not a mode. A Fill *toggle* would invent a mode to match a checklist line, so the
//! button runs the command `F` runs and refuses when the pointer is not over the map — see
//! `tests/a_tool_arms_the_same_from_a_button_and_a_key.rs`, which measured this.
//!
//! The bar's own look — height, gaps, the icon's tint and size — is `bevy_gooey`'s, from the running
//! `GooeySkin`. This module picks *which* commands it shows and marks the armed tool; it draws no
//! colour and sizes no glyph.

use bevy::prelude::*;

use crate::commands::{self, Command};
use crate::editor::{EditorState, Tool};
use crate::icons;

/// The id on a toolbar button, so a click can be resolved back to the command it stands for and a test
/// can find the armed tool's button by name rather than by position.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolbarCommand(pub &'static str);

/// **The toolbar is spawned once per editor**, at the foot of the door strip's parent — a band of its
/// own, under the chrome bar and above the docks, as `docs/ui/vision.md` §4 lays the workbench out.
#[derive(Component)]
pub struct Toolbar;

pub struct ToolbarPlugin;

impl Plugin for ToolbarPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<crate::command_run::CommandRunPlugin>() {
            app.add_plugins(crate::command_run::CommandRunPlugin);
        }
        app.add_systems(
            OnEnter(crate::screen::Screen::Editor),
            spawn_toolbar.after(crate::chrome::FrameSystems),
        )
        .add_systems(
            Update,
            (the_group_follows_the_armed_tool, mark_the_armed_tool)
                .run_if(in_state(crate::screen::Screen::Editor)),
        )
        // **The one dispatch.** gooey's `Activate` is a click, `Enter`, `Space` or a pad's south
        // button, so every way a person meets a button ends in the same observer.
        .add_observer(on_toolbar_activate)
        // **And one for a toggle group's choice.** `bevy_gooey` sends `ValueChange<usize>` from the
        // group, which is the index of the option chosen — not the command's id, so this maps it back
        // through [`tools`] rather than trusting the order in two places.
        .add_observer(on_tool_mode_chosen);
    }
}

/// **The tool modes' gooey toggle group**, so one option of it is lit at a time and the light is the
/// tool that is armed.
#[derive(Component)]
pub struct ToolModes;

/// **The commands the toolbar shows, in order**, with the tool modes first because they are what a
/// person reaches for while placing: the list is [`icons::TOOLBAR`]'s, so a command on the bar always
/// has a glyph (that parity is `tests/every_toolbar_command_has_an_icon.rs`'s).
pub fn shown() -> Vec<Command> {
    icons::TOOLBAR.iter().filter_map(|id| commands::by_id(id)).collect()
}

/// **The icon sheet, decoded once**, so a bar with no project and no working directory still has its
/// glyphs: the PNG is embedded in the binary ([`icons::ATLAS_PNG`]).
#[derive(Resource)]
pub struct ToolbarAtlas {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}

/// **Decode the sheet and spawn the bar**, once, on the frame the editor screen opens.
///
/// The bar is a gooey [`toolbar`](bevy_gooey::toolbar) inserted straight after the chrome bar, so it is
/// the window's shape and nothing a panel adds: the docks get what is left, as they did before there
/// was a toolbar.
pub fn spawn_toolbar(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    frame: Option<Res<crate::chrome::Frame>>,
    skin: Res<bevy_gooey::GooeySkin>,
) {
    let Some(frame) = frame else {
        return;
    };
    let image = match Image::from_buffer(
        icons::ATLAS_PNG,
        bevy::image::ImageType::Extension("png"),
        bevy::image::CompressedImageFormats::NONE,
        true,
        bevy::image::ImageSampler::default(),
        bevy::asset::RenderAssetUsages::default(),
    ) {
        Ok(image) => images.add(image),
        // A sheet that will not decode is a build fault, not a runtime one: the file is in the binary.
        // Say so once rather than spawning eleven blank buttons with no explanation.
        Err(error) => {
            bevy::log::error!("the toolbar's icon sheet does not decode: {error}");
            return;
        }
    };
    let layout = layouts.add(icons::layout());
    commands.insert_resource(ToolbarAtlas { image: image.clone(), layout: layout.clone() });
    // **The first button stands off the window's edge by a skin gap** (#290). gooey keeps a tooltip
    // `gap` clear of the window, and a box that cannot sit centred under a button at the bar's left end
    // slides along the button's edge; a button nearer the edge than that margin makes every below-the-
    // button place count as clipped, so the box was put *beside* the button, over its neighbours.
    let edge = skin.gap;

    let bar = commands
        .spawn((
            Toolbar,
            bevy_gooey::toolbar(Children::spawn(SpawnWith(move |parent: &mut bevy::ecs::relationship::RelatedSpawner<ChildOf>| {
                // **The tool modes are one gooey toggle group** (UI-37, #231): a group of options of
                // which exactly one is on, which is a tool mode exactly. Its value is written from
                // `EditorState` every frame by [`the_group_follows_the_armed_tool`], and a choice it
                // reports is handed to the same chord queue every other button uses.
                let mut modes = parent.spawn((
                    ToolModes,
                    bevy_gooey::toggle_group(
                        tools().iter().filter_map(|(id, _)| {
                            let command = commands::by_id(id)?;
                            let index = icons::index(command.id)?;
                            Some(bevy_gooey::GroupOption::icon(
                                bevy_gooey::IconSource::Atlas { image: image.clone(), layout: layout.clone(), index },
                                bevy_gooey::Tooltip::new(command.title).with_shortcut(command.shortcut),
                            ))
                        }),
                        0,
                    ),
                ));
                if let Some(mut node) = modes.get_mut::<Node>() {
                    node.margin.left = Val::Px(edge);
                }
                parent.spawn(bevy_gooey::toolbar_separator());
                // **Everything else is a one-shot command**, so it is a button and never a member of a
                // group: a lit "Undo" would be lying about what `Ctrl+Z` did.
                for command in shown() {
                    if tools().iter().any(|(id, _)| *id == command.id) {
                        continue;
                    }
                    let Some(index) = icons::index(command.id) else {
                        continue;
                    };
                    parent.spawn((
                        ToolbarCommand(command.id),
                        bevy_gooey::icon_button(
                            bevy_gooey::IconSource::Atlas { image: image.clone(), layout: layout.clone(), index },
                            bevy_gooey::Tooltip::new(command.title).with_shortcut(command.shortcut),
                        ),
                    ));
                    if command.id == "file.save" {
                        parent.spawn(bevy_gooey::toolbar_separator());
                    }
                }
            }))),
        ))
        .id();
    // Straight after the chrome bar: `insert_children` at index 1, because index 0 is that bar.
    commands.entity(frame.root).insert_children(1, &[bar]);
}

/// **One button pressed.** `bevy_gooey`'s `Activate` fires for a click, `Enter`, `Space` and a pad's
/// south button, so a person meets the button the same way whichever they use, and the chord that
/// goes down is the one the settings gave the command.
pub fn on_toolbar_activate(
    activate: On<bevy_gooey::Activate>,
    buttons: Query<&ToolbarCommand>,
    mut chords: ResMut<crate::command_run::Chords>,
    live: Res<crate::keys::Live>,
    mut state: Option<ResMut<EditorState>>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let Some(command) = commands::by_id(button.0) else {
        return;
    };
    chords.queue(&command, *live);
    if let Some(state) = state.as_deref_mut() {
        // "Asked for", as the palette says: the chord goes down a frame or two from now and the
        // dispatcher judges it then.
        state.status.note(format!("asked for {} from the toolbar", command.title));
    }
}

/// **The armed tool's button is lit**, and no other. Read from `EditorState`, never from the button, so
/// the light follows the key exactly: a click is a chord, the chord arms the tool, and this repaints
/// on the frame the tool changed.
///
/// **Only the tool modes are marked.** Undo, save and a snip are one-shot commands with no "on" state,
/// and a button that lit when pressed and stayed lit would be lying about what `Ctrl+Z` did.
pub fn mark_the_armed_tool(
    state: Res<EditorState>,
    buttons: Query<(&ToolbarCommand, &Children)>,
    mut icons: Query<&mut bevy_gooey::IconTone>,
) {
    if !state.is_changed() {
        return;
    }
    for (button, children) in &buttons {
        let Some(tone) = tools().iter().find(|(id, _)| *id == button.0).map(|(_, tone)| *tone) else {
            continue;
        };
        let on = tone == Some(state.tool);
        let want = if on { bevy_gooey::IconTone::Accent } else { bevy_gooey::IconTone::Text };
        for child in children.iter() {
            if let Ok(mut current) = icons.get_mut(child)
                && *current != want
            {
                *current = want;
            }
        }
    }
}

/// **Which button stands for which tool**, and `None` for a command that is not a mode. Written once so
/// [`mark_the_armed_tool`] and the test that reads the lights agree about it.
pub fn tools() -> [(&'static str, Option<Tool>); 4] {
    [
        ("map.move_mode", Some(Tool::Move)),
        ("map.remove_mode", Some(Tool::Remove)),
        ("map.clone_mode", Some(Tool::Clone)),
        ("map.run_mode", Some(Tool::Run)),
    ]
}

/// **The group's value is the armed tool**, written from `EditorState` every time the tool changes and
/// never from a click. A group that held its own choice would drift the moment a key changed the mode:
/// `B` would arm Move and leave Remove lit.
pub fn the_group_follows_the_armed_tool(
    state: Res<EditorState>,
    mut groups: Query<&mut bevy_gooey::ToggleGroupValue, With<ToolModes>>,
) {
    if !state.is_changed() {
        return;
    }
    let Some(at) = tools().iter().position(|(_, tool)| *tool == Some(state.tool)) else {
        return;
    };
    for mut value in &mut groups {
        if value.0 != at {
            value.0 = at;
        }
    }
}

/// **A choice in the group arms that tool**, through the same chord the key sends: the group's index is
/// mapped back to a command id, and the id goes into the queue. So a click on a group option, a click on
/// a toolbar button and a key press are one path, and the tool changes where the key handler says so.
pub fn on_tool_mode_chosen(
    change: On<bevy_gooey::ValueChange<usize>>,
    groups: Query<(), With<ToolModes>>,
    mut chords: ResMut<crate::command_run::Chords>,
    live: Res<crate::keys::Live>,
    mut state: Option<ResMut<EditorState>>,
) {
    if groups.get(change.source).is_err() {
        return;
    }
    let modes = tools();
    let Some(&(id, _)) = modes.get(change.value) else {
        return;
    };
    let Some(command) = commands::by_id(id) else {
        return;
    };
    chords.queue(&command, *live);
    if let Some(state) = state.as_deref_mut() {
        state.status.note(format!("asked for {} from the toolbar", command.title));
    }
}
