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
use bevy::ui::{ComputedNode, UiGlobalTransform};

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
            (the_group_follows_the_armed_tool, mark_the_armed_tool, label_the_primary_group)
                .run_if(in_state(crate::screen::Screen::Editor)),
        )
        // After gooey has styled a chip, so the place it was given is the one this replaces.
        .add_systems(
            Update,
            stagger_the_chips.after(bevy_gooey::GooeySystems::Style).run_if(in_state(crate::screen::Screen::Editor)),
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

/// How many rows the chips of the bar stand in: two, because a chord is wider than the button it names
/// but narrower than two of them, so alternate buttons' chips can share a row.
const CHIP_ROWS: usize = 2;

/// The air between two chips, in logical px: a chip's own border is one device pixel, and two that touch
/// read as one.
const CHIP_GAP_PX: f32 = 1.0;

/// **Each button's shortcut chip stands over its own button, and none stands on another's** (#291).
///
/// Holding `K` shows a chord on every toolbar button (gooey's `ShortcutBadges`), drawn at the button's
/// right edge. A chord is wider than the 33 px button (`Ctrl+Shift+Z`, `Shift+B`), so every chip drew
/// over its neighbour's and the row read `Xift+B Ctrl+B…`. gooey places a chip from the skin and has no
/// idea its neighbour has one, so the bar's own host stands them: **centred on the button, in one of two
/// rows inside the bar**, a chip going to the row where it needs the least shove right of its centre.
/// Two rows cost no ground — the bar is tall enough for both, and nothing else stands on it — where a
/// row *below* the bar would stand on the door strip and its own chips.
///
/// Gooey writes a chip's place once, when it is styled, so this writes it again only when it differs:
/// a chip already where it belongs costs one comparison, and the system does nothing at all while no
/// chip stands (the query is empty).
fn stagger_the_chips(
    bars: Query<(&ComputedNode, &UiGlobalTransform), With<Toolbar>>,
    mut chips: Query<(Entity, &ChildOf, &ComputedNode, &mut Node), With<bevy_gooey::ShortcutBadge>>,
    buttons: Query<(&ComputedNode, &UiGlobalTransform, &ChildOf), Without<bevy_gooey::ShortcutBadge>>,
    parents: Query<&ChildOf>,
    toolbar: Query<(), With<Toolbar>>,
) {
    let Ok((bar, bar_at)) = bars.single() else {
        return;
    };
    let bar_rect = logical_rect(bar, bar_at);
    // The chips that are the bar's, with where their buttons are: a chip is a child of its button, and a
    // button is the bar's child, or its mode group's.
    let mut mine: Vec<(Entity, Rect, Vec2)> = Vec::new();
    for (chip, owner, node, _) in &chips {
        let Ok((button, button_at, _)) = buttons.get(owner.parent()) else {
            continue;
        };
        let on_the_bar = std::iter::successors(Some(owner.parent()), |e| parents.get(*e).ok().map(|p| p.parent()))
            .take(3)
            .any(|e| toolbar.contains(e));
        if !on_the_bar || node.size() == Vec2::ZERO {
            continue;
        }
        let size = node.size() * node.inverse_scale_factor;
        mine.push((chip, logical_rect(button, button_at), size));
    }
    mine.sort_by(|a, b| a.1.center().x.total_cmp(&b.1.center().x));

    let mut row_right = [f32::MIN; CHIP_ROWS];
    for (chip, button, size) in mine {
        let want_left = button.center().x - size.x * 0.5;
        // The row where the chip lands nearest its button's centre, and the left edge it gets there.
        let (row, left) = (0..CHIP_ROWS)
            .map(|r| (r, want_left.max(row_right[r] + CHIP_GAP_PX)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, want_left));
        row_right[row] = left + size.x;
        let rows_height = CHIP_ROWS as f32 * size.y + (CHIP_ROWS - 1) as f32 * CHIP_GAP_PX;
        let top = bar_rect.min.y + (bar_rect.height() - rows_height).max(0.0) * 0.5 + row as f32 * (size.y + CHIP_GAP_PX);
        let Ok((_, _, _, mut node)) = chips.get_mut(chip) else {
            continue;
        };
        let (want_x, want_y) = (Val::Px(left - button.min.x), Val::Px(top - button.min.y));
        if node.left != want_x {
            node.left = want_x;
        }
        if node.top != want_y {
            node.top = want_y;
        }
        if node.right != Val::Auto {
            node.right = Val::Auto;
        }
    }
}

/// A laid-out node's rectangle in logical px.
fn logical_rect(node: &ComputedNode, at: &UiGlobalTransform) -> Rect {
    Rect::from_center_size(at.translation * node.inverse_scale_factor, node.size() * node.inverse_scale_factor)
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
                        std::iter::once(icons::index(PLACE_ICON).map(|index| {
                            bevy_gooey::GroupOption::icon(
                                bevy_gooey::IconSource::Atlas { image: image.clone(), layout: layout.clone(), index },
                                bevy_gooey::Tooltip::new(PLACE_TITLE),
                            )
                        }))
                        .flatten()
                        .chain(tools().iter().filter_map(|(id, _)| {
                            let command = commands::by_id(id)?;
                            let index = icons::index(command.id)?;
                            Some(bevy_gooey::GroupOption::icon(
                                bevy_gooey::IconSource::Atlas { image: image.clone(), layout: layout.clone(), index },
                                bevy_gooey::Tooltip::new(command.title).with_shortcut(command.shortcut),
                            ))
                        })),
                        0,
                    ),
                ));
                // The group stands off the window's edge by that gap, in logical px (`Space::Px`).
                modes.insert(bevy_gooey::Spacing { margin: crate::chrome::margin_left(bevy_gooey::Space::Px(edge)), ..default() });
                // **The one labelled button that is not a mode stands with the modes**, before the
                // separator: `LABELLED_BUTTON`'s own doc says Fill is "which the brush's own row of
                // tools stands beside". Spawning it in the command loop below put a 46.7-px worded
                // button in a run of 30.8-px pictures — `feedback-1793927698061`, *"Why is the fill
                // button larger here? It looks out of place."* — because that loop is where the
                // pictures live and Fill is not one of them.
                //
                // It stays a one-shot command and not a group option: `toolbar.rs`'s header says Fill
                // is "a one-shot flood from the cursor, not a mode", and a lit Fill would claim a mode
                // the editor does not have.
                if let Some(command) = commands::by_id(LABELLED_BUTTON.0)
                    && let Some(index) = icons::index(command.id)
                {
                    parent.spawn((
                        ToolbarCommand(command.id),
                        bevy_gooey::ButtonVariant::Ghost,
                        bevy_gooey::icon_button(
                            bevy_gooey::IconSource::Atlas { image: image.clone(), layout: layout.clone(), index },
                            bevy_gooey::Tooltip::new(command.title).with_shortcut(command.shortcut),
                        ),
                    ));
                }
                parent.spawn(bevy_gooey::toolbar_separator());
                // **Everything else is a one-shot command**, so it is a button and never a member of a
                // group: a lit "Undo" would be lying about what `Ctrl+Z` did.
                for command in shown() {
                    // The modes are the group above, and Fill is the labelled button above it: this loop
                    // is the pictures, and neither belongs in it.
                    if tools().iter().any(|(id, _)| *id == command.id) || command.id == LABELLED_BUTTON.0 {
                        continue;
                    }
                    let Some(index) = icons::index(command.id) else {
                        continue;
                    };
                    parent.spawn((
                        ToolbarCommand(command.id),
                        bevy_gooey::ButtonVariant::Ghost,
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

/// The icon of the group's first option, **Place**: the tool every other one goes back to. It is the
/// tool the editor is in when none is armed, so it has no command of its own and no key — pressing the
/// armed tool's key again, or Esc, is the way back, and this option asks for the same.
const PLACE_ICON: &str = "map.place_mode";

/// What the Place option is called when the pointer rests on it.
const PLACE_TITLE: &str = "Place pieces";

/// How many options come before the tool commands in the mode group: Place.
const PLACE_OPTIONS: usize = 1;

/// The group option that stands for `tool`, or `None` for a tool the group has no option for (a
/// note). Place is option 0; the tool commands follow in [`tools`]'s order.
pub fn group_index(tool: Tool) -> Option<usize> {
    if tool == Tool::Place {
        return Some(0);
    }
    tools().iter().position(|(_, mode)| *mode == Some(tool)).map(|at| at + PLACE_OPTIONS)
}

/// **The command whose key puts `tool` down**, because every tool's key is a toggle: pressed while its
/// tool is armed, it returns to placing. `None` for Place, which is already where that leads.
fn leaving_command(tool: Tool) -> Option<&'static str> {
    tools()
        .iter()
        .find(|(_, mode)| *mode == Some(tool))
        .map(|(id, _)| *id)
        .or((tool == Tool::Note).then_some("map.note"))
}

/// The word under a tool's icon.
fn tool_label(tool: Tool) -> &'static str {
    match tool {
        Tool::Place => "Place",
        Tool::Move => "Move",
        Tool::Remove => "Erase",
        Tool::Clone => "Clone",
        Tool::Run => "Run",
        Tool::Note => "Note",
    }
}

/// The command whose button carries a word under its icon besides the tool modes: Fill, which the
/// brush's own row of tools stands beside.
const LABELLED_BUTTON: (&str, &str) = ("map.fill", "Fill");

/// On a button or option that already has its word under the icon, so the system that adds it runs
/// once per control.
#[derive(Component)]
struct Labelled;

/// **The primary group says what it is in words** (#302): Place, Move, Erase, Clone, Run and Fill each
/// carry their name under the icon, because a 16 px picture is a guess until it has been learned. The
/// rest of the bar (undo, save, view) are the pictures everyone already knows and keep only a tooltip.
///
/// gooey's icon button has no slot for a word, so the control's node turns to a column and a text child
/// goes under the icon. The skin styles the text by its role (a group option's like a tab's, so it
/// takes the accent's ink when the option is lit, and a button's a control's: a `ButtonText` is what the
/// dialogs and the pages call a button, and a toolbar's picture is not one of those); the control, its tooltip and its chip stay what they
/// were, so the tooltip still sits on the button.
fn label_the_primary_group(
    mut commands: Commands,
    groups: Query<&Children, With<ToolModes>>,
    buttons: Query<(Entity, &ToolbarCommand), Without<Labelled>>,
    done: Query<(), With<Labelled>>,
    mut nodes: Query<&mut Node>,
) {
    let mut label = |entity: Entity, words: &'static str, role: bevy_gooey::Role| {
        if let Ok(mut node) = nodes.get_mut(entity) {
            node.flex_direction = FlexDirection::Column;
        }
        commands.entity(entity).insert(Labelled).with_child((
            Text::new(words),
            role,
            bevy::text::TextLayout::new(bevy::text::Justify::Center, bevy::text::LineBreak::NoWrap),
            bevy::picking::Pickable::IGNORE,
        ));
    };
    let words: Vec<&'static str> = std::iter::once(tool_label(Tool::Place))
        .chain(tools().iter().filter_map(|(_, mode)| mode.map(tool_label)))
        .collect();
    for children in &groups {
        for (option, words) in children.iter().zip(words.iter().copied()) {
            if done.get(option).is_err() {
                label(option, words, bevy_gooey::Role::TabText);
            }
        }
    }
    for (button, command) in &buttons {
        if command.0 == LABELLED_BUTTON.0 {
            label(button, LABELLED_BUTTON.1, bevy_gooey::Role::ControlText);
        }
    }
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
    let Some(at) = group_index(state.tool) else {
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
    // Option 0 is Place, which is no command: the way to it is the armed tool's own key again.
    let id = match change.value.checked_sub(PLACE_OPTIONS) {
        Some(at) => tools().get(at).map(|(id, _)| *id),
        None => state.as_deref().and_then(|state| leaving_command(state.tool)),
    };
    let Some(command) = id.and_then(commands::by_id) else {
        return;
    };
    chords.queue(&command, *live);
    if let Some(state) = state.as_deref_mut() {
        state.status.note(format!("asked for {} from the toolbar", command.title));
    }
}
