//! **The terminal's pane**: a compact dock across the bottom of the editor's window.
//!
//! `Ctrl+\`` raises it and lowers it. It is a dock and not a modal — the point of the feature is that
//! the person keeps editing while the shell is open, so a pane that stole the whole window would be
//! the alt-tab it exists to replace.
//!
//! # What is in it
//!
//! | part | what it is |
//! |---|---|
//! | the grip | a 4 px edge on top, dragged to resize; clamped to `[TERMINAL_MIN_H, TERMINAL_MAX_H]` |
//! | the bar | `Terminal` on the left; one chip per session, then `+` and `×`, on the right |
//! | the body | the shell's rows, **one `Text` per run of one colour** |
//! | the caret | a 2 px block where the shell's cursor is, blinking on `Time<Real>` |
//!
//! **One `Text` per run of one colour in a row, never per cell.** A 200 × 50 pane is ten thousand
//! cells; ten thousand entities is a frame-time cliff, and ten thousand `Text`s is worse. A compiler
//! line is three to six runs, so a row's entity count is bounded by its *colour changes* rather than
//! by its width, and a single-colour row is one entity however long it is.
//!
//! # It survives the door
//!
//! The **pane** is a `Node` and the door sweep takes it ([`crate::screen::scene_roots`]); the
//! **shells** are a resource ([`crate::terminal::Terminal`]) and nothing sweeps a resource. That is
//! the whole persistence story, and it is the right one: a `cargo build` started on the Map door is
//! still running when the person crosses to Kit, and the pane they raise there draws the same
//! scrollback because it is the same `TerminalScreen`. Nothing has to be excluded from the sweep, so
//! there is no exclusion list to drift.
//!
//! # Narrow is a layout behaviour, not a hope
//!
//! The column count is `floor(body_width / cell_width)` from a measured cell
//! ([`crate::terminal_view::measure_the_cell`]), and the shell is told every time it changes —
//! `cargo`'s own wrapping then does the work, and the emulator re-wraps its scrollback so a prompt
//! that was one row at 200 columns is drawn as four at 50 rather than clipped. The pane never draws a
//! cell outside its rect: a clipped terminal loses the right-hand end of every error message with no
//! way to scroll to it.

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;
use bevy::ui::ComputedNode;

use crate::chrome;
use crate::terminal::Terminal;
use crate::terminal_view::CellMetrics;

/// **The pane's resting height**, logical px — about twenty rows, which is a compiler error and the
/// five lines of context around it. A person's drag replaces it for the session.
pub const TERMINAL_DEFAULT_H: f32 = 320.0;

/// **The floor**: about eight rows. Below this a prompt plus one line of output does not fit, and a
/// terminal showing a truncated prompt is worse than one that stops shrinking.
pub const TERMINAL_MIN_H: f32 = 96.0;

/// **The ceiling**, so a pane dragged to the top stops being a terminal and becomes the window. It is
/// a fixed count and deliberately **not** a promise of "the map keeps half the window": that is the
/// limitation `a_dock_resize_never_covers_the_viewport` already states about `SplitMin`, and a bound
/// computed from the body's height is a system this slice does not add.
pub const TERMINAL_MAX_H: f32 = 640.0;

/// **Below this many columns the pane is narrow**: the child is told a small width and reflows into
/// it. It is a *reading* of the situation rather than a second layout path — the drawing is
/// identical, only the column count differs — but it is what the compactness test asserts against.
pub const TERMINAL_NARROW_COLS: u16 = 60;

/// The pane's own bar height, logical px. Fixed, like [`chrome`]'s two bars and for the same reason:
/// a bar that changed height with its content would move the shell under the person's hands.
const TERMINAL_BAR_H: f32 = 24.0;

/// The grip's thickness, logical px.
const TERMINAL_GRIP_H: f32 = 4.0;

/// How long the caret stays lit, in seconds — half a second each way, the platform convention.
const CARET_PERIOD: f32 = 1.0;

/// The caret's width, logical px. Two, because one pixel on a HiDPI surface is invisible and three is
/// a letter.
const CARET_W: f32 = 2.0;

/// **The pane's root.** The one marker every query here keys on.
///
/// `Reflect`, like `chrome::ViewportSlot`, because **BRP is how an agent looks at this editor**: a
/// marker with no reflection is a pane `world.query` cannot find, which makes the terminal invisible to
/// the one tool built for seeing it. That is not a test convenience — `tools/demo/record.py` aims by
/// querying, and a pane nothing can query is a pane no storyboard can press.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalPaneRoot;

/// The bar, so the chips can be rebuilt without walking the tree.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalBar;

/// The node the rows are rebuilt into.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalBody;

/// One drawn row, and which row of the shell's screen it is.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalRow(pub usize);

/// The caret's node.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalCaret;

/// The grip, dragged to resize the pane.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalGrip;

/// A chip in the bar, and the tab it stands for.
#[derive(Component, Clone, Copy)]
pub struct TerminalTabChip(pub usize);

/// The `+` chip: another shell, started in the open project.
#[derive(Component)]
pub struct TerminalNewTab;

/// **The `cd` chip: move the drawn shell to the project the editor is pointed at now.**
///
/// The shell is *not* moved when a project opens — a running `cargo build` belongs to the project it
/// was started in, and yanking its working directory would break it. So the pane offers the move
/// instead: it types `cd '<root>'` into the shell, which is a keystroke the person could type
/// themselves, and the shell's own history records it.
#[derive(Component)]
pub struct TerminalCdChip;

/// The `×` chip: close the tab being drawn, and with it its shell.
#[derive(Component)]
pub struct TerminalCloseTab;

/// **The terminal has the keyboard**, which is the one state this feature must not get wrong.
///
/// Written by this module, read by [`crate::editor::sense_context`] — which folds it into the same
/// `typing` flag a text field sets, so **no editor binding fires while a shell has the keys**. A
/// shell that typed `d` and moved the camera would be unusable, and `Live(Context::Typing, …)` is
/// the single gate the whole census already respects; a second swallow written here would be the
/// two-paths failure this crate spends its refusals on.
///
/// Two chords survive, checked in [`crate::terminal_view::type_keys`] *before* the swallow because
/// they are read there rather than through the census: `Ctrl+\`` closes, `Ctrl+Shift+V` pastes.
#[derive(Resource, Default, Reflect)]
#[reflect(Resource, Default)]
pub struct TerminalFocus(pub bool);

impl TerminalFocus {
    /// Take the keyboard.
    pub fn take(&mut self) {
        self.0 = true;
    }

    /// Give it back to the editor.
    pub fn release(&mut self) {
        self.0 = false;
    }

    /// Does the terminal own the keys this frame?
    pub fn holds(&self) -> bool {
        self.0
    }
}

/// The pane's own working state: the size it last asked for, and the caret's blink.
#[derive(Resource, Default, Reflect)]
#[reflect(Resource, Default)]
pub struct TerminalPane {
    cols: u16,
    rows: u16,
    /// Seconds into the caret's blink cycle.
    blink: f32,
    /// Where a drag began, in the body's cell coordinates, while a selection is being made.
    drag_from: Option<(usize, usize)>,
}

impl TerminalPane {
    /// The cell count the pane last asked the shell for.
    pub fn cells(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    /// Record the cell count this frame's layout produced.
    pub fn set_cells(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
    }

    /// Where a selection drag started, if one is in progress.
    pub fn drag_from(&self) -> Option<(usize, usize)> {
        self.drag_from
    }
}

/// Registers the pane, its painting systems, and the chords that raise and lower it.
pub struct TerminalPanePlugin;

impl Plugin for TerminalPanePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerminalFocus>()
            .init_resource::<TerminalPane>()
            // **Registered, so BRP can name them.** `world.list_resources` and `world.get_resources`
            // only see types the app has reflected, and an agent reading this editor's state is the
            // reason `bevy_fairy` exists. A `Terminal` nothing can read is a shell an agent can start
            // and never observe.
            .register_type::<TerminalFocus>()
            .register_type::<TerminalPane>()
            .register_type::<TerminalPaneRoot>()
            .register_type::<TerminalRow>()
            .register_type::<TerminalBar>()
            .register_type::<TerminalBody>()
            .register_type::<TerminalCaret>()
            .register_type::<TerminalGrip>()
            // **The frame is the editor's alone** (`chrome::spawn_frame` runs `OnEnter(Editor)`); the
            // launcher and the project page build their own roots, so there is no body to dock into
            // there and nothing to spawn against.
            .add_systems(
                OnEnter(crate::screen::Screen::Editor),
                spawn_the_pane.after(chrome::FrameSystems),
            )
            // **The keyboard goes back when the door closes.** A focus flag that outlived its pane
            // would leave every editor binding refused with no shell on screen to explain it — the
            // bug `leaving_a_tab_puts_the_name_prompt_down` exists for, one level up.
            .add_systems(OnExit(crate::screen::Screen::Editor), give_the_keyboard_back)
            .add_systems(
                Update,
                (blink_the_caret,).run_if(the_pane_is_up),
            )
            // The two chords that are the pane's own. Read from `ButtonInput` here rather than
            // through `keys::BINDINGS`: the census is the table of what a *tab* does, and a chord
            // that belongs to the window is the case `command_palette::chords` already set the
            // precedent for. `no_context_gains_a_key` is therefore untouched.
            .add_systems(
                Update,
                (toggle_on_the_chord, close_the_tab_on_its_chord)
                    .in_set(crate::terminal_view::TerminalChordSet),
            )
            .add_observer(a_click_takes_the_keyboard)
            .add_observer(a_click_elsewhere_gives_it_back)
            .add_observer(a_drag_begins)
            .add_observer(a_drag_extends)
            .add_observer(a_drag_ends)
            .add_observer(the_wheel_scrolls_the_scrollback)
            .add_observer(the_grip_resizes)
            .add_observer(a_chip_selects_its_tab)
            .add_observer(a_chip_adds_a_tab)
            .add_observer(a_chip_moves_the_shell)
            .add_observer(a_chip_closes_a_tab);
    }
}

/// Is a pane on screen this frame? Every per-frame system asks it, so a lowered terminal costs one
/// empty query.
fn the_pane_is_up(panes: Query<(), With<TerminalPaneRoot>>) -> bool {
    !panes.is_empty()
}

/// **Build the pane**, as the body's last row.
///
/// A row of the frame's own column rather than a floating layer: the body already stacks the docks
/// and the viewport, so "docked across the bottom" is one child appended to it. A `GlobalZIndex`
/// layer would paint over the status band and the docks' scrollbars, and z-order is the dialogs' — see
/// [`chrome::MODAL_Z`].
fn spawn_the_pane(
    mut commands: Commands,
    frame: Option<Res<chrome::Frame>>,
    frame_children: Query<&Children>,
    terminal: Res<Terminal>,
    mut focus: ResMut<TerminalFocus>,
) {
    let Some(frame) = frame else {
        return;
    };
    // **A pane belongs to a screen.** The door sweep takes the old root; this spawns the new one, and
    // the shells in `Terminal` are untouched — that is the whole of what "survives the door" means.
    //
    // **It is parented to the frame's ROOT, not its body, and that is not a detail.**
    // `chrome::Frame::body` is the `flex_direction: Row` that holds the two docks and the viewport, so a
    // child of it is a *row sibling*: the pane was laid out 2560 px wide and pushed off the right edge
    // of a 2560 px window, over the inspector and the map. Measured on the released 0.2.8 build on
    // `mac_air`: size 2560 x 768, centre x 1718, spanning 438..2998. The root is the `Column` that
    // already stacks the chrome bar, the door strip, the body and the status band, so a bottom dock is
    // a row between `body` and `status` — which is what this inserts.
    //
    // **It arrives lowered.** The pane is a dock a person raises, not something that takes a fifth of
    // the window the first time they open a project — so the node exists from the door's first frame
    // and `Ctrl+\`` is what shows it. That is also why the shells are not touched here: a shell that
    // was running when the door closed is still running, and raising the pane draws its scrollback.
    // **The status band's index**, so the pane lands between the body and the band. Found by asking the
    // frame's own children rather than assuming "last but one": a frame that gained a row would
    // otherwise put the terminal in the wrong place, silently.
    let band_at = frame_children
        .get(frame.root)
        .map(|kids| {
            kids.iter()
                .position(|child| child == frame.status)
                .unwrap_or(usize::MAX)
        })
        .unwrap_or(usize::MAX);
    build_the_pane(&mut commands, frame.root, terminal.height(), &terminal, band_at);
    // A door that opens never has the terminal's keys: a lowered pane that still owned the keyboard
    // would be a dead editor with nothing on screen saying why.
    focus.release();
}

/// Build the pane under `body`, returning its root. **The one place a pane is made**, lowered.
fn build_the_pane(
    commands: &mut Commands,
    parent: Entity,
    height: f32,
    terminal: &Terminal,
    // The index in `parent`'s children the pane goes at — the status band's, so the band stays last.
    before: usize,
) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(height),
                min_height: Val::Px(TERMINAL_MIN_H),
                flex_direction: FlexDirection::Column,
                // **Never shrinks.** The height is the person's, and a body that shrank it would turn
                // a remembered 320 px into whatever the docks left over.
                flex_shrink: 0.0,
                border: UiRect::top(Val::Px(chrome::EDGE_W)),
                // **Lowered until the chord raises it.** `Display::None` rather than `Visibility`,
                // because a hidden-but-laid-out pane would still take its height out of the body and
                // the map would be 320 px shorter for a terminal nobody has asked for.
                display: Display::None,
                ..default()
            },
            BackgroundColor(chrome::PANEL_BG),
            BorderColor::all(chrome::PANEL_EDGE),
            // **`Pickable::default()`, not `IGNORE`.** The body has to be clickable to take the
            // keyboard and draggable to select — and `IGNORE` skips an entity *and everything below
            // it*, the trap `chrome::spawn_frame` records for the shell split's divider.
            Pickable::default(),
            TerminalPaneRoot,
            Name::new("terminal"),
        ))
        .id();

    let grip = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(TERMINAL_GRIP_H),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(chrome::PANEL_EDGE),
            TerminalGrip,
            Pickable::default(),
            Name::new("terminal grip"),
        ))
        .id();

    let bar = commands
        .spawn((
            bevy::prelude::Node {
                width: Val::Percent(100.0),
                height: Val::Px(TERMINAL_BAR_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(chrome::GAP_TIGHT),
                padding: UiRect::axes(Val::Px(chrome::MARGIN), Val::Px(0.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(chrome::BAR_BG),
            TerminalBar,
            Pickable::default(),
            Name::new("terminal bar"),
        ))
        .id();

    let menu = build_the_menu(commands);
    let body_node = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                // **Clipped, and not a `ScrollArea`.** The scrollback belongs to the emulator
                // (`TerminalScreen::scroll`): a layout scroll would move the *drawn* rows while the
                // shell's history lives in the grid, which is two answers to one question. `clip`
                // is what stops a row painting over the status band.
                overflow: Overflow::clip(),
                padding: UiRect::axes(Val::Px(chrome::MARGIN), Val::Px(crate::terminal_view::BODY_PAD)),
                ..default()
            },
            BackgroundColor(chrome::PANEL_BG),
            TerminalBody,
            // **A right-click offers the three things a terminal's clipboard needs.** A shell with no
            // way to copy its output is a toy; gooey's `context_menu` is the editor's one menu
            // machinery, so this is the same widget every other menu uses.
            bevy_gooey::ContextMenu(menu),
            Pickable::default(),
            Name::new("terminal body"),
        ))
        .id();

    let caret = commands
        .spawn((
            Node {
                // The caret is not a panel: it marks one cell of the shell's grid, exactly as a
                // list's scrollbar overlay sits on its viewport, and `draw_the_terminal` moves it.
                // PLACES-ITSELF-OK: a caret over one cell is not a panel `chrome::Frame` places.
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(CARET_W),
                height: Val::Px(crate::terminal_view::TERMINAL_CELL_H),
                ..default()
            },
            BackgroundColor(chrome::ACCENT),
            TerminalCaret,
            Pickable::IGNORE,
            Name::new("terminal caret"),
        ))
        .id();

    commands.entity(body_node).add_child(caret);
    commands.entity(root).add_children(&[grip, bar, body_node]);
    // **Above the status band, below the body.** `insert_children` at the band's index keeps the band
    // at the foot of the window, where it has been since the frame was written; an `add_child` would
    // put the terminal under it. `insert_children` clamps the index, so `usize::MAX` is "last".
    commands.entity(parent).insert_children(before, &[root]);
    fill_the_bar(commands, bar, terminal);
    root
}

/// **The terminal's context menu**: Copy, Paste, Clear.
///
/// Built once per pane and named by the body's [`bevy_gooey::ContextMenu`], so a right-click anywhere in
/// the scrollback opens it at the pointer — gooey's own behaviour, not a second implementation.
///
/// **Copy takes the selection, and says so when there is none** by staying disabled rather than
/// silently doing nothing: a menu item that looks live and does nothing is the failure mode
/// `commands.rs`'s `enabled` exists to prevent. It reads the selection when the menu opens, which is
/// what `On<Add<ContextTarget>>` is for.
fn build_the_menu(commands: &mut Commands) -> Entity {
    commands
        .spawn(bevy_gooey::context_menu(children![
            (
                bevy_gooey::menu_item("Copy"),
                bevy_gooey::observe(copy_the_selection),
            ),
            (
                bevy_gooey::menu_item("Paste"),
                bevy_gooey::observe(paste_the_clipboard),
            ),
            (
                bevy_gooey::menu_item("Clear"),
                bevy_gooey::observe(clear_the_scrollback),
            ),
        ]))
        .id()
}

/// Copy the selection to the system clipboard.
fn copy_the_selection(
    _click: On<bevy_gooey::Activate>,
    terminal: Option<Res<Terminal>>,
) {
    let Some(terminal) = terminal else {
        return;
    };
    let Some(tab) = terminal.tab() else {
        return;
    };
    let Some(text) = tab.screen().selection_text() else {
        return;
    };
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(text);
    }
}

/// Paste the clipboard, bracketed when the child asked for it.
fn paste_the_clipboard(
    _click: On<bevy_gooey::Activate>,
    mut terminal: ResMut<Terminal>,
) {
    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        return;
    };
    let Ok(text) = clipboard.get_text() else {
        return;
    };
    let bracketed = terminal
        .tab()
        .is_some_and(|tab| tab.screen().bracketed_paste());
    let mut bytes = Vec::new();
    if bracketed {
        bytes.extend_from_slice(b"\x1b[200~");
        bytes.extend_from_slice(text.as_bytes());
        bytes.extend_from_slice(b"\x1b[201~");
    } else {
        bytes.extend_from_slice(text.as_bytes());
    }
    terminal.write(&bytes);
}

/// **Clear the scrollback** — the shell's history, not its screen: the pane keeps showing what is
/// live, and the lines above stop costing anything to keep.
fn clear_the_scrollback(_click: On<bevy_gooey::Activate>, mut terminal: ResMut<Terminal>) {
    if let Some(tab) = terminal.tab_mut() {
        tab.screen_mut().clear_scrollback();
    }
}

/// **The bar**: the pane's name on the left, its sessions and its two controls on the right.
fn fill_the_bar(commands: &mut Commands, bar: Entity, terminal: &Terminal) {
    commands.entity(bar).despawn_children();
    commands.entity(bar).with_children(|bar| {
        chrome::list_heading(bar, "Terminal");
        // A spacer, so the chips sit on the right the way every other bar in this editor puts them.
        bar.spawn(Node {
            flex_grow: 1.0,
            ..default()
        });
        // **A chip per session, and the two controls through the same builder.** `chrome::chip`
        // carries `RowRest`, so rest / hover / pressed are painted by `style_list_rows` without this
        // pane knowing what hover looks like — the fourth shape a hand-rolled `Button` would be,
        // and `chrome_census::a_clickable_is_one_of_the_three_shapes` fails the build on exactly that.
        for (index, tab) in terminal.tabs().iter().enumerate() {
            let active = index == terminal.active();
            let ink = if active { chrome::ACCENT } else { chrome::TEXT };
            let fill = if active { chrome::ROW_SELECTED } else { chrome::BAR_BG };
            chrome::chip(
                bar,
                TerminalTabChip(index),
                tab.title(),
                chrome::text::CONTROL,
                ink,
                fill,
                chrome::PANEL_EDGE,
            );
        }
        // **`cd` only when it would do something.** The chip names the directory it would move to, so a
        // person reads where it goes rather than trusting a verb; with no open project, or a shell
        // already in it, there is nothing to offer and no chip.
        let want = terminal.next_cwd();
        if terminal.tab().is_some_and(|tab| tab.wants_cd_to(&want)) {
            let name = want
                .file_name()
                .map(|n| format!("cd {}", n.to_string_lossy()))
                .unwrap_or_else(|| "cd".to_owned());
            chrome::chip(
                bar,
                TerminalCdChip,
                &name,
                chrome::text::CONTROL,
                chrome::ACCENT,
                chrome::BAR_BG,
                chrome::PANEL_EDGE,
            );
        }
        chrome::chip(
            bar,
            TerminalNewTab,
            "+",
            chrome::text::CONTROL,
            chrome::TEXT,
            chrome::BAR_BG,
            chrome::PANEL_EDGE,
        );
        chrome::chip(
            bar,
            TerminalCloseTab,
            "x",
            chrome::text::CONTROL,
            chrome::DANGER,
            chrome::BAR_BG,
            chrome::PANEL_EDGE,
        );
    });
}
/// **`Ctrl+\`` raises the pane, or lowers it when it already has the keys.**
///
/// Three cases, and each is what a person means:
///
/// - **lowered** — show it and take the keyboard; the pump starts a shell if there is none;
/// - **raised and focused** — lower it, and give the editor its keys back;
/// - **raised and unfocused** — take the keys, which is how you get back into a shell you clicked away
///   from without reaching for the mouse again.
///
/// One pane per door, always spawned and always a child of the frame's body; raising is
/// `Display::Flex`, so there is no second spawn path and nothing to keep in step.
fn toggle_on_the_chord(
    keys: Res<ButtonInput<KeyCode>>,
    mut panes: Query<&mut Node, With<TerminalPaneRoot>>,
    mut focus: ResMut<TerminalFocus>,
) {
    let chord = bevy_gooey::Shortcut::new(bevy_gooey::Modifiers::PRIMARY, KeyCode::Backquote);
    if !chord.just_pressed(&keys) {
        return;
    }
    let Some(mut node) = panes.iter_mut().next() else {
        return;
    };
    let lowered = node.display == Display::None;
    if lowered {
        node.display = Display::Flex;
        focus.take();
        return;
    }
    if focus.holds() {
        node.display = Display::None;
        focus.release();
        return;
    }
    // Raised but unfocused: the press means "put me back in the shell", not "hide it".
    focus.take();
}

/// **`Ctrl+Shift+\`` closes the tab**, leaving the pane up — which is what a person wants after
/// typing `exit` into a shell that is now gone.
fn close_the_tab_on_its_chord(
    keys: Res<ButtonInput<KeyCode>>,
    terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    let chord = bevy_gooey::Shortcut::new(
        bevy_gooey::Modifiers::PRIMARY | bevy_gooey::Modifiers::SHIFT,
        KeyCode::Backquote,
    );
    if !chord.just_pressed(&keys) {
        return;
    }
    let mut terminal = terminal;
    terminal.close_active();
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// Give the keyboard back when the door closes.
fn give_the_keyboard_back(mut focus: ResMut<TerminalFocus>) {
    focus.release();
}

/// Redraw the bar after the session list changed.
fn redraw_the_bar(commands: &mut Commands, bar: &Query<Entity, With<TerminalBar>>, terminal: &Terminal) {
    if let Ok(bar) = bar.single() {
        fill_the_bar(commands, bar, terminal);
    }
}

/// **A click anywhere in the pane puts the keyboard in the shell** — including the bar, because a
/// person who clicked the chip to switch sessions means to type into the one they picked.
fn a_click_takes_the_keyboard(
    click: On<bevy::picking::events::PointerClick>,
    ancestors: Query<&ChildOf>,
    roots: Query<(), With<TerminalPaneRoot>>,
    mut focus: ResMut<TerminalFocus>,
) {
    if roots.contains(click.entity)
        || ancestors.iter_ancestors(click.entity).any(|up| roots.contains(up))
    {
        focus.take();
    }
}

/// **A click anywhere outside it gives the keys back**, or the editor would be dead to the keyboard
/// with nothing on screen saying why.
fn a_click_elsewhere_gives_it_back(
    click: On<bevy::picking::events::PointerClick>,
    ancestors: Query<&ChildOf>,
    roots: Query<(), With<TerminalPaneRoot>>,
    mut focus: ResMut<TerminalFocus>,
) {
    if !focus.holds() {
        return;
    }
    let inside = roots.contains(click.entity)
        || ancestors.iter_ancestors(click.entity).any(|up| roots.contains(up));
    if !inside {
        focus.release();
    }
}

/// **A drag over the body selects**, which is the only way to copy a compiler error out.
///
/// The UI picking backend reports `hit.position` **normalized to the node**, `(-0.5, -0.5)` at its
/// top-left (`bevy_ui-0.20.0/src/picking_backend.rs`'s own header), so a cell is that plus a half,
/// times the node's logical size, divided by the cell size. No second camera projection.
///
/// Three observers rather than one with three event parameters: an `On<E>` observer is registered per
/// event type, and a system taking two of them is a system two fires would each run entirely.
fn a_drag_begins(
    start: On<bevy::picking::events::PointerDragStart>,
    body: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<TerminalBody>>,
    metrics: Res<CellMetrics>,
    mut pane: ResMut<TerminalPane>,
    mut terminal: ResMut<Terminal>,
) {
    if start.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    let Ok((node, _)) = body.get(start.entity) else {
        return;
    };
    let Some(at) = cell_under(&start.hit, node, &metrics) else {
        return;
    };
    pane.drag_from = Some(at);
    if let Some(tab) = terminal.tab_mut() {
        tab.screen_mut().select(at, at);
    }
}

/// Extend the selection while the button is down.
fn a_drag_extends(
    drag: On<bevy::picking::events::PointerDrag>,
    body: Query<(), With<TerminalBody>>,
    metrics: Res<CellMetrics>,
    pane: Res<TerminalPane>,
    mut terminal: ResMut<Terminal>,
) {
    if drag.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    let Some(from) = pane.drag_from else {
        return;
    };
    if body.get(drag.entity).is_err() {
        return;
    }
    // **`PointerDrag` carries no `hit`** — only the start, enter, over and end events do — so the
    // pointer's displacement is `distance`, measured from the drag's start in screen pixels. Adding it
    // to the starting cell is the whole conversion: there is no second camera projection here because
    // the pane is UI, and a UI pixel is a cell once divided by the cell's measured size.
    let scale = metrics.scale();
    let (rows, cols) = (
        (drag.distance.y / (metrics.height() * scale)).round() as i32,
        (drag.distance.x / (metrics.width() * scale)).round() as i32,
    );
    let to = (
        (from.0 as i32 + rows).max(0) as usize,
        (from.1 as i32 + cols).max(0) as usize,
    );
    if let Some(tab) = terminal.tab_mut() {
        tab.screen_mut().select(from, to);
    }
}

/// Forget the anchor, and keep what was selected — the copy is the next thing the person does.
fn a_drag_ends(drag: On<bevy::picking::events::PointerDragEnd>, mut pane: ResMut<TerminalPane>) {
    if drag.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    pane.drag_from = None;
}

/// The cell the pointer is over, from the UI backend's normalized hit position.
fn cell_under(
    hit: &bevy::picking::backend::HitData,
    node: &ComputedNode,
    metrics: &CellMetrics,
) -> Option<(usize, usize)> {
    let normalized = hit.position?;
    let size = node.size() * node.inverse_scale_factor();
    let local = Vec2::new(normalized.x + 0.5, normalized.y + 0.5) * size;
    Some((
        (local.y / metrics.height()).max(0.0) as usize,
        (local.x / metrics.width()).max(0.0) as usize,
    ))
}

/// **The wheel scrolls the scrollback**, three lines a notch — the same cadence the editor's lists use.
fn the_wheel_scrolls_the_scrollback(
    mut scroll: On<bevy::picking::events::PointerScroll>,
    ancestors: Query<&ChildOf>,
    bodies: Query<(), With<TerminalBody>>,
    mut terminal: ResMut<Terminal>,
) {
    let over = bodies.contains(scroll.entity)
        || ancestors.iter_ancestors(scroll.entity).any(|up| bodies.contains(up));
    if !over {
        return;
    }
    // Consumed: the pane is a terminal, and its scrollback is not the docks'.
    scroll.propagate(false);
    let lines = (scroll.y.round() as i32) * 3;
    if let Some(tab) = terminal.tab_mut() {
        tab.screen_mut().scroll(lines);
    }
}

/// **Drag the grip to resize**, clamped to a terminal's range. The height is remembered on the
/// resource, so the next door's pane opens where this one was left.
fn the_grip_resizes(
    drag: On<bevy::picking::events::PointerDrag>,
    grip: Query<(), With<TerminalGrip>>,
    mut panes: Query<&mut Node, With<TerminalPaneRoot>>,
    mut terminal: ResMut<Terminal>,
) {
    if !grip.contains(drag.entity) || drag.delta.y == 0.0 {
        return;
    }
    // Dragging **up** makes the pane taller, so the delta is subtracted: y grows downward.
    let want = terminal.height() - drag.delta.y;
    terminal.set_height(want);
    let height = terminal.height();
    for mut node in &mut panes {
        node.height = Val::Px(height);
    }
}

/// **A chip selects its session.**
fn a_chip_selects_its_tab(
    mut click: On<bevy::picking::events::PointerClick>,
    chips: Query<&TerminalTabChip>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    let Ok(&TerminalTabChip(index)) = chips.get(click.entity) else {
        return;
    };
    click.propagate(false);
    terminal.select(index);
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// **`+` starts another shell**, in the open project.
fn a_chip_adds_a_tab(
    mut click: On<bevy::picking::events::PointerClick>,
    plus: Query<(), With<TerminalNewTab>>,
    pane: Res<TerminalPane>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    if !plus.contains(click.entity) {
        return;
    }
    click.propagate(false);
    // **The cells the pane is actually drawing.** A shell started at a guessed 80 x 24 would print a
    // prompt wrapped for a width the pane is not showing and be resized one frame later, which is a
    // visible flash on every `+`. `TerminalPane` is written by the view's own resize system, so this
    // is the measurement the last frame made.
    let (cols, rows) = measured_cells(&pane);
    let cwd = terminal.next_cwd();
    terminal.spawn(cols, rows, &cwd);
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// **The cell count a shell is started with.**
///
/// Not simply what the pane last measured, and the difference is a bug that shipped for an afternoon:
/// on the frame `Ctrl+\`` raises the pane, `Display` has just gone from `None` to `Flex`, so the body
/// has **no layout yet** and measures zero — and this clamped that to one cell and started a shell
/// believing it was one column wide. The prompt then drew as a single column of characters, which
/// looked exactly like a terminal that had not started.
///
/// So a measure below a usable minimum is **not** believed: the fallback is 80 x 24, the size every
/// terminal program is written against, and the pane's own resize corrects it on the next frame with a
/// real measurement. That is one honest frame of the wrong width rather than a session that is wrong
/// until something resizes it.
pub(crate) fn measured_cells(pane: &TerminalPane) -> (u16, u16) {
    const USABLE_MIN_COLS: u16 = 20;
    const USABLE_MIN_ROWS: u16 = 5;
    let (cols, rows) = pane.cells();
    if cols < USABLE_MIN_COLS || rows < USABLE_MIN_ROWS {
        return (FALLBACK_COLS, FALLBACK_ROWS);
    }
    (cols, rows)
}

/// The size a shell is started at before the pane has ever been laid out. 80 x 24 is not an arbitrary
/// number: it is the default every terminfo entry declares and every program is tested against.
const FALLBACK_COLS: u16 = 80;
const FALLBACK_ROWS: u16 = 24;

/// **`cd <project>` in the drawn shell**, as a keystroke.
///
/// Quoted with single quotes, which POSIX shells take literally, and with any embedded quote escaped the
/// POSIX way (`'\''`) — a project directory with a space or an apostrophe in it is ordinary, and a chip
/// that broke on one would be a chip nobody trusts. The newline is what runs it.
fn a_chip_moves_the_shell(
    mut click: On<bevy::picking::events::PointerClick>,
    cd: Query<(), With<TerminalCdChip>>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    if !cd.contains(click.entity) {
        return;
    }
    click.propagate(false);
    let root = terminal.next_cwd();
    let quoted = root.to_string_lossy().replace('\'', "'\\''");
    terminal.write(format!("cd '{quoted}'\n").as_bytes());
    // **The pane records what it told the shell**, because only the shell knows where it went.
    if let Some(tab) = terminal.tab_mut() {
        tab.offered_cd(root);
    }
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// **`×` closes the session being drawn**, and with it the shell (`Drop` hangs the child up).
fn a_chip_closes_a_tab(
    mut click: On<bevy::picking::events::PointerClick>,
    close: Query<(), With<TerminalCloseTab>>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    if !close.contains(click.entity) {
        return;
    }
    click.propagate(false);
    terminal.close_active();
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// **The caret blinks on real time**, so a paused-but-visible editor still has a live cursor: the
/// shell on the other end is running whether the game clock is or not.
fn blink_the_caret(
    time: Res<Time<Real>>,
    mut pane: ResMut<TerminalPane>,
    carets: Query<&mut Node, With<TerminalCaret>>,
) {
    pane.blink = (pane.blink + time.delta_secs()) % CARET_PERIOD;
    let on = pane.blink < CARET_PERIOD * 0.5;
    let want = if on { Display::Flex } else { Display::None };
    for mut node in carets {
        if node.display != want {
            node.display = want;
        }
    }
}
