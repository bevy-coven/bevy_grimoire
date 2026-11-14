//! **Drawing the terminal, and the one rule that must not be got wrong: the keys.**
//!
//! Four systems, in the order they matter:
//!
//! 1. [`pump_the_terminal`] — every shell's output into its screen; the emulator's replies back out.
//! 2. [`measure_the_cell`] — one glyph's advance, once per font, from the shipped face.
//! 3. [`resize_the_terminal`] — the pane's size → a cell count → `TIOCSWINSZ`, **only on a change**.
//! 4. [`draw_the_terminal`] — rebuild the rows, and only when the screen is dirty.
//!
//! # The key swallow
//!
//! [`type_keys`] reads the raw [`KeyboardInput`] stream and, while [`TerminalFocus`] holds, turns every
//! key into the bytes a terminal sends. **No editor binding fires** — not because each one is
//! checked, but because the editor's own gate is `Live(Context::Typing, …)` and
//! [`crate::editor::sense_context`] folds the terminal into the same `typing` flag a text field sets.
//! One gate, in the place every binding already respects.
//!
//! That is also why this file does **not** clear the message stream. The editor's fields drain before
//! leaving precisely because a message they did not read is still there next frame; the terminal reads
//! what it wants and leaves the rest, and the `Typing` flag is what stops the rest from acting.
//!
//! # What a terminal sends for a key
//!
//! Not the character. `Enter` is `\r`, `Backspace` is `\x7f`, an arrow is `ESC [ A`, `Ctrl+C` is
//! `\x03`. This is the VT100 vocabulary and it is not negotiable: a shell in raw mode (`vim`, `less`,
//! `fzf`) reads those sequences and nothing else, and a pane that sent `"Enter"` would leave the
//! program waiting for a byte that never came. [`key_bytes`] is the whole table, and it is data.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::ui::ComputedNode;

use crate::chrome;
use crate::terminal::{CaretShape, Terminal, SCROLLBACK};
use crate::terminal_pane::{
    TerminalBody, TerminalCaret, TerminalFind, TerminalFocus, TerminalPane, TerminalRow, FIND_CHORD,
};

/// **One cell's advance**, logical px: the shipped face is `FiraMono`, whose advance is `0.6 em` — the
/// ratio `capture_feedback_1793905698793`'s test measured independently, stated once here for the
/// terminal. [`measure_the_cell`] replaces it with a real measurement on the first frame that has a
/// laid-out glyph, so a project's own face is honoured.
pub const TERMINAL_CELL_W: f32 = chrome::text::LABEL.px() * 0.6;

/// **One cell's height**, logical px: Bevy 0.20's default `LineHeight` is `RelativeToFont(1.2)`, the
/// same rule [`chrome::BODY_LINE_H`] states.
pub const TERMINAL_CELL_H: f32 = chrome::text::LABEL.px() * 1.2;

/// **Where the rows begin inside the body, logical px, and one number for every side of the boundary:**
/// the window's panel margin across, a small inset down.
///
/// The pane writes it as the body's padding; [`resize_the_terminal`] subtracts it before dividing by a cell,
/// the caret adds it before multiplying by one, and `terminal_pane::cell_under` subtracts it from a press. It
/// was a single 2 px written on both axes here while the pane padded its sides by `chrome::MARGIN`, and that
/// cost three things on mac_air (feedback-1794011769640): the shell was told of 4 more columns than a row
/// holds, so omp's full-width rule widened the pane without end; the caret stood 1.7 cells left of the
/// cursor; and a press selected the character two cells right of the one under it.
pub const BODY_INSET: Vec2 = Vec2::new(chrome::MARGIN, 2.0);

/// **The terminal's face** (#323): FiraMono Nerd Font Mono, the Nerd Fonts patch of the face the editor
/// ships (SIL OFL 1.1; the licence and the release note sit beside it in `assets/fonts/`). Prompts
/// (starship, powerlevel10k), `eza --icons` and `lazygit` draw Nerd Font icons from the Private Use
/// Area, which plain FiraMono has none of, so they drew as boxes. The `Mono` variant keeps every icon
/// one cell wide, so the grid the shell was told about is the grid drawn; and the Latin glyphs are
/// FiraMono's own, so the terminal reads like the rest of the window.
pub const TERMINAL_FONT: Handle<Font> = bevy::asset::uuid_handle!("6b9f3f0e-2d47-4a8c-b1e5-3c0d9a7e5f21");

/// The face's bytes, compiled in: a project carries no terminal font of its own.
const TERMINAL_FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/FiraMonoNerdFontMono-Regular.otf");

/// **A `TextFont` in the terminal's face**, at the size every terminal row is drawn at: the label role
/// times the person's terminal text size (#324, `CellMetrics::text_scale`).
pub fn terminal_font(scale: f32) -> TextFont {
    TextFont {
        font: TERMINAL_FONT.into(),
        ..chrome::font_scaled(chrome::text::LABEL, scale)
    }
}

/// Put the terminal's face in the font assets under [`TERMINAL_FONT`], before anything draws a row.
fn install_the_terminal_face(mut fonts: ResMut<Assets<Font>>) {
    if let Err(why) = fonts.insert(TERMINAL_FONT.id(), Font::from_bytes(TERMINAL_FONT_BYTES.to_vec())) {
        error!("the terminal's face could not be installed, so its rows draw in no face: {why}");
    }
}

/// **The chord set**, so the pane's own chords can be ordered after the key swallow rather than racing
/// it. The swallow consumes a `Ctrl+\`` press by *not* forwarding it (it is the close chord), and the
/// toggle reads the same `ButtonInput`; the order below is what makes one press one act.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TerminalChordSet;

/// **The caret's query filter**, named because Clippy is right that it is more than a signature
/// should carry inline — and because a `Without<TerminalRow>` beside a `With<TerminalCaret>` is a
/// claim about two markers not overlapping, which deserves a name.
type CaretFilter = (With<TerminalCaret>, Without<TerminalRow>);

/// Registers the terminal's drawing and key systems.
pub struct TerminalViewPlugin;

impl Plugin for TerminalViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CellMetrics>()
            .init_resource::<Drawn>()
            .add_systems(Startup, install_the_terminal_face)
            // **The swallow is last in `Phase::Text`**, after the editor's own fields have read what
            // they are going to read: a field that is open has the `Typing` flag, so the terminal's
            // keys are already refused by then, and a shell's are the only ones left.
            .add_systems(Update, type_keys.after(crate::keys::Phase::Text))
            // **The pump runs before anything draws**, in `PreUpdate` so a frame's output is what that
            // frame shows rather than one behind.
            .add_systems(PreUpdate, pump_the_terminal)
            .add_systems(
                Update,
                (
                    // The person's settings first (#324): a new text size invalidates the measurement
                    // below, and a new default caret is the screens' before they are drawn. After the
                    // settings are read, so the first frame's agent choice is the file's, not the default.
                    take_the_terminal_settings.after(crate::settings::sync_settings),
                    // The measurement first, so the column count this frame uses a real advance.
                    measure_the_cell,
                    resize_the_terminal.after(measure_the_cell),
                    draw_the_terminal.after(resize_the_terminal),
                )
                    .chain(),
            );
    }
}

/// **The measured advance of one glyph**, which is what the pane's column count is computed from.
///
/// Measured once, from a laid-out `Text` holding one `M` in the shipped face, and remeasured whenever
/// the font changes ([`measure_the_cell`] compares the size it saw). A hard-coded 6.6 px would be
/// right until somebody chose a project face with a different advance, at which point every column
/// would drift by a fraction of a cell per character and the pane would look broken in a way nobody
/// could name.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CellMetrics {
    width: f32,
    height: f32,
    /// Physical pixels per logical pixel, as the text pipeline laid the glyphs out.
    scale: f32,
    measured: bool,
    /// **The person's terminal text size** (#324), a multiple of the shipped size. A change of it
    /// repaints every row at the new size and measures the cell again.
    text_scale: f32,
}

impl Default for CellMetrics {
    fn default() -> Self {
        Self {
            width: TERMINAL_CELL_W,
            height: TERMINAL_CELL_H,
            scale: 1.0,
            measured: false,
            text_scale: 1.0,
        }
    }
}

impl CellMetrics {
    /// **How many physical pixels one logical pixel is.** `PointerDrag::distance` is reported in
    /// physical screen pixels while the cell is a logical measurement, so the two need one bridge —
    /// and the bridge is the layout's own scale factor, which is what `ComputedNode` carries.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// One cell's advance, logical pixels.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// One cell's height, logical pixels.
    pub fn height(&self) -> f32 {
        self.height
    }

    /// Has a real glyph been measured, or is this still the ratio?
    pub fn is_measured(&self) -> bool {
        self.measured
    }

    /// The terminal's text size, a multiple of the shipped size (#324).
    pub fn text_scale(&self) -> f32 {
        self.text_scale
    }
}

/// **The person's terminal settings, given to the terminal when they change** (#324).
///
/// `EditorSettings` changes only when a settings file resolves to something new (`settings::sync_settings`),
/// so this does its work once per edit. The default caret goes to every screen and to every shell opened
/// later; a new text size repaints every row at that size and measures the cell again, and the column
/// and row count that follows is told to the shell by [`resize_the_terminal`]. The agent choice is the
/// pane's ([`pump_the_terminal`]), and no shell opens until this has run once.
fn take_the_terminal_settings(
    settings: Option<Res<crate::settings::EditorSettings>>,
    mut terminal: ResMut<Terminal>,
    mut metrics: ResMut<CellMetrics>,
    mut drawn: ResMut<Drawn>,
    mut collapse_taken: Local<Option<bool>>,
) {
    let Some(settings) = settings else {
        // An app with no settings has none to wait for: its terminal opens plain shells.
        if !terminal.settings_taken() {
            terminal.set_agent(crate::settings::TerminalAgent::Off, None);
        }
        return;
    };
    if !settings.is_changed() {
        return;
    }
    terminal.set_default_caret(settings.terminal_cursor.shape());
    terminal.set_scrollback(settings.terminal_scrollback);
    // Before `set_agent`, which marks the settings as given and so lets the first shell open.
    terminal.set_shell(settings.terminal_shell.as_ref().map(|program| crate::terminal::ShellChoice {
        program: program.clone(),
        args: settings.terminal_shell_args.clone(),
    }));
    terminal.set_agent(settings.terminal_agent, settings.terminal_agent_command.clone());
    // **The collapse a person left, when a file states one** (#424), taken when the *setting* changes, never again: the pane's own
    // raise and lower (a chord, the status bar's label) write the file, and a later read of that same value must not undo a raise
    // made since. A layer that states nothing leaves the pane as it starts, collapsed.
    if settings.terminal_collapsed != *collapse_taken {
        *collapse_taken = settings.terminal_collapsed;
        if let Some(collapsed) = settings.terminal_collapsed {
            terminal.set_collapsed(collapsed);
        }
    }
    if metrics.text_scale != settings.terminal_text_scale {
        metrics.text_scale = settings.terminal_text_scale;
        metrics.measured = false;
        drawn.rows.clear();
        drawn.stamp = 0;
    }
}

/// **Measure one glyph, once.**
///
/// The pane draws `Text` nodes in the shell's own font, so a `TextLayoutInfo` for one of them *is* the
/// measurement: its `glyphs` carry the advance per character. The first frame that has one is
/// measured; after that this reads a `bool`.
///
/// **Falls back to the ratio** when no row has been drawn yet (an empty scrollback is no rows at all),
/// which is what makes the first frame's column count sane rather than zero — and the fallback is
/// replaced the moment there is a glyph to measure, so it can never be the answer for long.
fn measure_the_cell(
    mut metrics: ResMut<CellMetrics>,
    rows: Query<(&bevy::text::TextLayoutInfo, &TextFont), With<TerminalRow>>,
) {
    if metrics.measured {
        return;
    }
    // **Only a row already drawn at the current size is a measurement** (#324): after the text size
    // changes, a row still laid out at the old one would be measured and kept for good.
    let size = terminal_font(metrics.text_scale).font_size;
    let Some((layout, advance)) = rows
        .iter()
        .filter(|(_, font)| font.font_size == size)
        .find_map(|(layout, _)| advance_of(layout).map(|advance| (layout, advance)))
    else {
        return;
    };
    // Physical pixels in, logical out, which is what `scale_factor` is for.
    let scale = if layout.scale_factor > 0.0 {
        layout.scale_factor
    } else {
        1.0
    };
    let width = (advance / scale).max(1.0);
    let height = (layout.size.y / scale).max(1.0);
    if (width - metrics.width).abs() < 0.01 && (height - metrics.height).abs() < 0.01 {
        metrics.measured = true;
        return;
    }
    metrics.width = width;
    metrics.height = height;
    metrics.scale = scale;
    metrics.measured = true;
}

/// A step between two neighbouring pens shorter than this, physical px, is a mark drawn over the cell
/// before it (a combining accent), not a cell of its own.
const PEN_STEP_MIN_PX: f32 = 0.5;

/// **One cell's advance in a laid-out row, physical px**: the shortest step between two neighbouring
/// glyphs' pens, or `None` for a row with no such pair.
///
/// **The pen, not the ink.** A glyph's `position` is the centre of its ink, `size / 2 + pen + offset`
/// (`bevy_text-0.20.0/src/pipeline.rs:430`), so the distance between two centres is the advance plus the
/// difference in the two letters' ink. Measured that way the cell came out as wide as the first two
/// letters of the row it read said: 6.0 logical px from `ladvien@` (an `l` and an `a`), 6.42 from `sh-5.3$`
/// (an `s` and an `h`), whose pens are 7.2 physical px apart at scale 1.2, which is 6.0. A cell 7 % wide put
/// the caret further right of its column the further along the row it was. Taking each glyph's half-width
/// and bearing back off gives its pen. **The shortest step** because in a monospace face every step is a
/// whole number of cells.
fn advance_of(layout: &bevy::text::TextLayoutInfo) -> Option<f32> {
    let pen = |glyph: &bevy::text::PositionedGlyph| {
        glyph.position.x - glyph.atlas_info.rect.width() / 2.0 - glyph.atlas_info.offset.x
    };
    layout
        .glyphs
        .windows(2)
        .map(|pair| pen(&pair[1]) - pen(&pair[0]))
        .filter(|step| *step >= PEN_STEP_MIN_PX)
        .min_by(f32::total_cmp)
}

/// **The pane's size → a cell count → `TIOCSWINSZ`, only when it changed.**
///
/// The "only when it changed" is the whole reason this is a comparison: the body is laid out every
/// frame, and an unconditional resize would be an ioctl per frame that makes `cargo`'s progress line
/// redraw sixty times a second.
fn resize_the_terminal(
    metrics: Res<CellMetrics>,
    bodies: Query<&ComputedNode, With<TerminalBody>>,
    mut pane: ResMut<TerminalPane>,
    mut terminal: ResMut<Terminal>,
) {
    let Ok(body) = bodies.single() else {
        return;
    };
    let inner = body.size() * body.inverse_scale_factor();
    let usable = (inner - BODY_INSET * 2.0).max(Vec2::ZERO);
    // **One column minimum, one row minimum**, because a zero in either is a division by zero inside
    // the grid. The pane's own `TERMINAL_MIN_H` makes a zero unreachable from a drag; a test that
    // resizes the surface to something absurd is where it is reachable, and clamping is right there.
    // **A body that has not been laid out yet is not a one-cell terminal.** `floor(0 / 6.6)` is 0 and
    // the emulator's own floor is 1, which is a real size it will faithfully draw — so a pane measuring
    // zero would resize a live shell down to a single column and leave it there until the next real
    // layout. Refusing the measurement is what makes "not yet measured" mean "ask again next frame".
    if usable.x < metrics.width() * 2.0 || usable.y < metrics.height() * 2.0 {
        return;
    }
    let cols = ((usable.x / metrics.width()).floor() as u16).max(2);
    let rows = ((usable.y / metrics.height()).floor() as u16).max(2);
    if (cols, rows) == pane.cells() {
        return;
    }
    pane.set_cells(cols, rows);
    terminal.resize(cols, rows);
}

/// **Drain every shell into its screen, and start the first one the first time the pane is drawn.**
///
/// Every tab, not just the drawn one: a `cargo build` in a background tab that nobody is looking at
/// must not stall on a full pipe because the pane was elsewhere. This is also where a program's
/// question gets its answer, which is what makes `tput cols` (and anything else that reports) work.
///
/// # When a shell starts
///
/// **The first frame the pane is raised**, not when it is spawned. The pane is a child of the frame
/// from the door's first frame but starts lowered, so `is_empty` plus a raised pane is exactly "a person
/// asked for a terminal and there is nothing in it yet" — and a project opened and left without ever
/// pressing the chord costs no `fish` at all. A shell is not cheap (tens of milliseconds and a few
/// megabytes) and a session can open twenty doors.
fn pump_the_terminal(
    mut terminal: ResMut<Terminal>,
    panes: Query<&Node, With<crate::terminal_pane::TerminalPaneRoot>>,
    pane: Res<TerminalPane>,
    project: Option<Res<crate::project::Project>>,
    view: Option<Res<crate::settings::SettingsView>>,
    root: Option<Res<crate::project_assets::ProjectRoot>>,
) {
    let raised = panes.iter().any(|node| node.display != Display::None);
    // **Nor before the open project's own settings have been looked at** (#339). The door's pane is drawn two frames
    // before its `Project` exists (the root is pointed at before the first frame, the project is built in the editor's
    // first), and the settings are taken in that gap with `EditorSettings` the user's layer alone: a shell started
    // there is a plain one for a project that chooses an agent, and a shell already open is not the empty pane that
    // invites the agent. So: a project on its way (a root, no `Project` yet) is waited for, and so is the look at its
    // file. An app with no root, or that does not sync settings, has no layer to wait for.
    let project_layer_in = if project.is_some() {
        view.as_deref().is_none_or(|view| view.project_looked())
    } else {
        root.as_deref().is_none_or(|root| root.current().is_none())
    };

    // **Not before the settings are read**, or a person who chose an agent gets a plain first shell.
    if raised && terminal.settings_taken() && project_layer_in {
        // The cells the pane is drawing, so the shell's first prompt is wrapped for the width it will
        // actually be shown at rather than for a guessed 80 and then resized a frame later.
        let (cols, rows) = crate::terminal_pane::measured_cells(&pane);
        // **No agent starts by itself.** A session costs tokens to warm up, so with an agent chosen the
        // pane stays empty and shows an `Invoke Agent` button (`terminal_pane`), and only its press opens
        // the agent tab. A person who is just looking around the game pays nothing.
        if !terminal.invites_the_agent() && terminal.is_empty() {
            let cwd = terminal.next_cwd();
            terminal.spawn(cols.max(2), rows.max(1), &cwd);
        }
    }
    if terminal.is_empty() {
        return;
    }
    terminal.pump();
}

/// **Rebuild the drawn rows, but only when the screen changed.**
///
/// Every frame: one `bool` read and one stamp comparison. On a new build of the screen: one `Vec`
/// comparison per drawn row, and one `Text` per **run of same-coloured cells** for each row that
/// differs. A typical compiler line is three to six runs; a settled prompt is two entities.
///
/// # The rows are kept, not rebuilt
///
/// A row's node is spawned once and reused; only its children (the colour runs) are replaced, and only
/// when that row's cells actually differ from what was drawn. That is what makes a `cargo build`
/// scrolling past cost one entity-tree edit per changed row rather than one per frame for the whole
/// pane — and it is why [`Drawn`] keeps a copy of the rows the pane is showing.
// A Bevy system's parameters are what it reads and writes; the caret's shape and focus (#320) are two
// more of them, and bundling them into a `SystemParam` would hide which resources this one touches.
#[allow(clippy::too_many_arguments)]
fn draw_the_terminal(
    metrics: Res<CellMetrics>,
    mut terminal: ResMut<Terminal>,
    mut drawn: ResMut<Drawn>,
    bodies: Query<Entity, With<TerminalBody>>,
    mut rows: Query<(Entity, &TerminalRow, &mut Node, &mut Visibility)>,
    carets: Query<(&mut Node, &mut Visibility, &mut BackgroundColor), CaretFilter>,
    mut glyphs: Query<(&mut Text, &mut TextFont, &mut TextColor), With<crate::terminal_pane::TerminalCaretGlyph>>,
    focus: Res<TerminalFocus>,
    mut commands: Commands,
) {
    let Ok(body) = bodies.single() else {
        return;
    };
    // **A still frame copies nothing.** `lines()` rebuilds only when the emulator is dirty; the rows
    // are then read in place, beside `drawn` (another resource, so the two borrows are disjoint), and
    // [`Drawn::stamp`] says whether they are the very build already on screen. An earlier version
    // cloned every row twice a frame to compare them, shell idle or not (#308).
    let Some(tab) = terminal.tab_mut() else {
        return;
    };
    tab.screen_mut().lines();
    let Some(tab) = terminal.tab() else {
        return;
    };
    let screen = tab.screen();
    let lines = screen.built_lines();
    let stamp = screen.stamp();
    let caret = screen.cursor();
    let problem = tab.problem();
    let drawn = &mut *drawn;
    // **A new screen is a new body**, and what was drawn into the old one went with it. The terminal is on
    // every screen (#314), so the home's pane, the loader's and the editor's are three bodies in a minute:
    // the rows `Drawn` kept are despawned entities, and a stamp that matches would leave the new pane empty
    // while the shell is full. Forget them, and the new body is drawn from scratch on this frame.
    if drawn.body != Some(body) {
        *drawn = Drawn { body: Some(body), ..Drawn::default() };
    }

    // **The rows are created once and then only repainted.**
    //
    // The `rows` query cannot see a row this system spawned — a `Commands` spawn lands after the
    // system returns — so a loop that asked the query every frame would spawn a fresh row per line per
    // frame forever. That was measured live: 23 `TerminalRow` entities existed and `world.query` found
    // none, because every one had been replaced before the frame ended and every row was empty.
    //
    // So the row entities are kept on [`Drawn`], and the query is only the *first* frame's source.
    if drawn.row_entities.len() < lines.len() {
        for (entity, row, _, _) in &rows {
            if drawn.row_entities.len() >= lines.len() {
                break;
            }
            if !drawn.row_entities.iter().any(|(e, _)| *e == entity) {
                drawn.row_entities.push((entity, row.0));
            }
        }
    }
    while drawn.row_entities.len() < lines.len() {
        let index = drawn.row_entities.len();
        let node = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    // **Logical, like every `Val::Px`**: Bevy multiplies it by the target's scale
                    // factor and `UiScale`, exactly as it does the row's text, so `metrics.height()`
                    // here is one text line on screen. The glyph line's *physical* height was used
                    // once, on the belief that a `Node`'s height is in surface pixels; at the
                    // editor's `UiScale` of 1.2 that drew every row 20 % taller than the cell the
                    // shell was sized for, so the prompt fell below the pane and the caret drifted
                    // above the cursor (#310).
                    height: Val::Px(metrics.height()),
                    flex_direction: FlexDirection::Row,
                    flex_shrink: 0.0,
                    ..default()
                },
                TerminalRow(index),
                Pickable::IGNORE,
                Name::new(format!("terminal row {index}")),
            ))
            .id();
        commands.entity(body).add_child(node);
        drawn.row_entities.push((node, index));
    }
    // Visibility and height follow the row count every frame; both are compared before writing,
    // because `Node` is change-detected by the layout system.
    for (entity, index) in &drawn.row_entities {
        let Ok((_, _, mut node, mut visibility)) = rows.get_mut(*entity) else {
            continue;
        };
        let want = if *index < lines.len() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
        if node.height != Val::Px(metrics.height()) {
            node.height = Val::Px(metrics.height());
        }
    }

    // **Only a new build of the screen is looked at, and only its changed rows are repainted.** A
    // frame whose stamp is the one on screen compares nothing and copies nothing; a new build compares
    // each row with what `Drawn` holds and repaints only those that differ.
    //
    // **One `Text` per row with a `TextSpan` per colour run**, not one node per run. A row of four
    // differently-coloured runs as four sibling nodes is four flex items whose widths are guesses at
    // the font's metrics — and the guess was wrong in the live editor: the runs wrapped and overlapped
    // each other, because a forced `Val::Px(6.0 * cells)` is a *different* advance from what the shaper
    // uses. `TextSpan` is Bevy's own answer: one layout, one advance, colours per span. `TextLayout` is
    // no-wrap, so a long row is clipped by the body's `Overflow::clip` and never re-flowed by the
    // layout engine.
    if drawn.stamp != stamp {
        for (entity, index) in &drawn.row_entities {
            let Some(line) = lines.get(*index) else {
                continue;
            };
            if drawn.rows.get(*index) == Some(line) {
                continue;
            }
            commands.entity(*entity).despawn_children();
            let runs = colour_runs(line);
            commands
                .entity(*entity)
                .insert((
                    Text::new(String::new()),
                    TextLayout::no_wrap(),
                    TextColor(chrome::ink().text),
                    terminal_font(metrics.text_scale()),
                    Pickable::IGNORE,
                ))
                .with_children(|row| {
                    for run in &runs {
                        // **Every span names the terminal's face** (#323). A `TextSpan` is a node of its
                        // own with its own `TextFont` (bevy_text's `TextSpan` requires one), so a span
                        // without it is drawn in the editor's face whatever the row's root says — and the
                        // spans are where all of a row's text is. Measured live: the icons a prompt
                        // prints drew as boxes while only the empty root named the Nerd Font.
                        let mut span = row.spawn((
                            TextSpan::new(run.text.clone()),
                            TextColor(run.ink),
                            terminal_font(metrics.text_scale()),
                            Pickable::IGNORE,
                        ));
                        // **A ground only where the cells paint one** (#309): an inverse run, a
                        // coloured background, the selection. The pane is already its default ground,
                        // so a plain run carries none and costs no extra quad.
                        if let Some(ground) = run.ground {
                            span.insert(bevy::text::TextBackgroundColor(ground));
                        }
                    }
                });
        }
        lines.clone_into(&mut drawn.rows);
        drawn.stamp = stamp;
    }

    // **The caret sits where the shell's cursor is**, in cells from the body's top-left, in the shape
    // the program asked for (#320). Without the keyboard it is an outline of its cell whatever its
    // shape, VS Code's `cursorStyleInactive` default, so a glance says the keys are elsewhere. A block is
    // a tint of the accent with the covered character drawn on it in its own ink, so the word stays
    // readable through the blink (`terminal_pane::CARET_BLOCK_ALPHA`); a bar and an underline cover no
    // character and keep the accent's full strength.
    let caret_visible = caret.visible && caret.row < lines.len();
    let (cell_w, cell_h) = (metrics.width(), metrics.height());
    let held = focus.holds();
    let (width, height, drop) = match (held, caret.shape) {
        (true, CaretShape::Bar) => (crate::terminal_pane::CARET_W, cell_h, 0.0),
        (true, CaretShape::Underline) => (
            cell_w,
            crate::terminal_pane::CARET_UNDERLINE_H,
            cell_h - crate::terminal_pane::CARET_UNDERLINE_H,
        ),
        (true, CaretShape::Block) | (false, _) => (cell_w, cell_h, 0.0),
    };
    let solid_block = held && caret.shape == CaretShape::Block;
    let border = if held {
        UiRect::ZERO
    } else {
        UiRect::all(Val::Px(crate::terminal_pane::CARET_OUTLINE_W))
    };
    let ground = match (held, caret.shape) {
        (false, _) => Color::NONE,
        (true, CaretShape::Block) => chrome::ink().accent.with_alpha(crate::terminal_pane::CARET_BLOCK_ALPHA),
        (true, _) => chrome::ink().accent,
    };
    for (mut node, mut visibility, mut fill) in carets {
        let want = if caret_visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
        let left = Val::Px(BODY_INSET.x + caret.column as f32 * cell_w);
        let top = Val::Px(BODY_INSET.y + caret.row as f32 * cell_h + drop);
        if node.left != left {
            node.left = left;
        }
        if node.top != top {
            node.top = top;
        }
        if node.width != Val::Px(width) {
            node.width = Val::Px(width);
        }
        if node.height != Val::Px(height) {
            node.height = Val::Px(height);
        }
        if node.border != border {
            node.border = border;
        }
        if fill.0 != ground {
            fill.0 = ground;
        }
    }
    // The character under a block, and its ink: the row's cell at the caret's column. A row is trimmed of
    // its trailing blanks, so a caret past the end covers a space, which is drawn as nothing.
    let (covered, ink) = if solid_block {
        lines
            .get(caret.row)
            .and_then(|line| line.cells.get(caret.column))
            .filter(|cell| cell.ch != ' ')
            .map_or((String::new(), chrome::ink().text), |cell| (String::from(cell.ch), cell.fg))
    } else {
        (String::new(), chrome::ink().text)
    };
    // The glyph is drawn at the rows' size, so a block over a large cell holds a large character.
    let glyph_font = terminal_font(metrics.text_scale());
    for (mut glyph, mut font, mut glyph_ink) in &mut glyphs {
        if glyph.0 != covered {
            glyph.0.clone_from(&covered);
        }
        if font.font_size != glyph_font.font_size {
            font.font_size = glyph_font.font_size;
        }
        if glyph_ink.0 != ink {
            glyph_ink.0 = ink;
        }
    }

    // **A refusal is drawn in the pane's ink**, in the first row, so a shell that would not start is a
    // sentence on screen rather than an empty box. It replaces the first row's content because there
    // is no shell to have written anything.
    if let Some(problem) = problem
        && let Some((first, _)) = drawn.row_entities.first()
    {
        commands.entity(*first).despawn_children();
        commands.entity(*first).with_child((
            Text::new(problem.to_owned()),
            TextColor(chrome::ink().danger),
            chrome::typed(chrome::text::LABEL),
            TextLayout::no_wrap(),
            Pickable::IGNORE,
        ));
    }
}

/// **The rows the pane is showing**, so a rebuild is a comparison rather than a repaint.
#[derive(Resource, Default)]
pub struct Drawn {
    rows: Vec<crate::terminal::Line>,
    /// The screen's stamp for the build in `rows`, so a frame whose screen did not rebuild skips the
    /// row comparison and the copy altogether.
    stamp: u64,
    /// The row entities, kept because a `Commands` spawn is invisible to this system's own query for
    /// the rest of the frame — see [`draw_the_terminal`].
    row_entities: Vec<(Entity, usize)>,
    /// The body these rows were drawn into: a screen's own pane, rebuilt on every screen (#314).
    body: Option<Entity>,
}

/// One run of same-coloured characters in a row.
struct Run {
    text: String,
    ink: Color,
    /// The run's own background, or `None` on the pane's default ground.
    ground: Option<Color>,
}

/// **Split a row into runs of one colour.** The reason a 200-column pane is a handful of entities
/// rather than two hundred.
///
/// A run is a maximal span of one ink **on one ground**; the text is what the shaper lays out, so no
/// width is computed here at all — an advance guessed at this level is the bug `TextSpan` exists to
/// avoid. The ground splits runs too, because Bevy draws a span's `TextBackgroundColor` behind that
/// span's glyphs and nothing else: an inverse word inside a plain line is its own run (#309).
fn colour_runs(line: &crate::terminal::Line) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for cell in &line.cells {
        let ground = cell.has_ground().then_some(cell.bg);
        match runs.last_mut() {
            Some(run) if run.ink == cell.fg && run.ground == ground => run.text.push(cell.ch),
            _ => runs.push(Run {
                text: cell.ch.to_string(),
                ink: cell.fg,
                ground,
            }),
        }
    }
    runs
}

/// **A clipboard gesture in the terminal**, which is the pane's and never the shell's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardChord {
    Paste,
    Copy,
    SelectAll,
}

/// **The chords for each gesture** (feedback-1794024642593). Command+V, Command+C and Command+A first,
/// which is what every Mac terminal answers to; Command is `bevy_gooey::Modifiers::META`, the Super key off
/// a Mac, which a desktop rarely gives an app (the precedent is `terminal_pane::CLEAR_CHORD`). Then the
/// primary modifier with Shift, the Linux terminals' Ctrl+Shift+V, Ctrl+Shift+C and (GNOME's)
/// Ctrl+Shift+A, because Ctrl+V, Ctrl+C and Ctrl+A are the shell's own.
const CLIPBOARD_CHORDS: [(ClipboardChord, KeyCode); 3] = [
    (ClipboardChord::Paste, KeyCode::KeyV),
    (ClipboardChord::Copy, KeyCode::KeyC),
    (ClipboardChord::SelectAll, KeyCode::KeyA),
];

/// **The clipboard gesture the press of `key` makes**, if it makes one. Keyed to the event's key so one
/// press acts once, when Command arrives in the same frame as the letter.
pub fn clipboard_chord(keys: &ButtonInput<KeyCode>, key: KeyCode) -> Option<ClipboardChord> {
    use bevy_gooey::{Modifiers, Shortcut};
    let &(chord, _) = CLIPBOARD_CHORDS.iter().find(|(_, chord_key)| *chord_key == key)?;
    let command = Shortcut::new(Modifiers::META, key);
    let primary_shift = Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, key);
    (command.just_pressed(keys) || primary_shift.just_pressed(keys)).then_some(chord)
}

/// **Is Command held?** A key pressed under it is a chord, and never the shell's letter: a Mac keyboard
/// reports Command+Z as `z`, and no Mac terminal sends it on. On a Mac this is also the primary modifier,
/// whose own chords (`Cmd+\``, `Cmd+J`, `Cmd+F`, `Cmd+K`) are read before it.
pub fn command_held(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight)
}

/// **The Command chords that edit the line a shell is reading**, as a Mac terminal sends them (feedback-1794146958145: *"I pressed
/// Cmd+Backspace (Mac) and it did not clear the line to the beginning. We need our terminal to automatically respect terminal
/// conventions"*). Every other Command chord is the pane's or the window's and never the shell's ([`command_held`]); these three
/// are what VS Code's and iTerm2's terminals send on a Mac, as the control codes readline binds:
///
/// |chord|sends|readline's name|
/// |---|---|---|
/// |`Cmd+Backspace`|`^U`|unix-line-discard: clear to the start of the line|
/// |`Cmd+Left`|`^A`|beginning-of-line|
/// |`Cmd+Right`|`^E`|end-of-line|
///
/// Only with Command alone: a Shift, Option or Ctrl held with it is another gesture. (`Option+Backspace` is the word before the
/// cursor and already goes as `ESC DEL`.)
pub fn line_editing_chord(keys: &ButtonInput<KeyCode>, key: KeyCode) -> Option<&'static [u8]> {
    let others = [KeyCode::ShiftLeft, KeyCode::ShiftRight, KeyCode::AltLeft, KeyCode::AltRight, KeyCode::ControlLeft, KeyCode::ControlRight];
    if !command_held(keys) || keys.any_pressed(others) {
        return None;
    }
    match key {
        KeyCode::Backspace => Some(b"\x15"),
        KeyCode::ArrowLeft => Some(b"\x01"),
        KeyCode::ArrowRight => Some(b"\x05"),
        _ => None,
    }
}

/// **The keys, and the swallow.**
///
/// While [`TerminalFocus`] holds, every keystroke becomes the bytes a terminal sends, and the editor
/// sees none of them — the gate is [`crate::editor::sense_context`]'s `Typing` flag, which is the one
/// every binding already respects.
///
/// **The chords the pane keeps for itself are checked first**, because they are the pane's and not the
/// shell's: `Ctrl+\`` closes, and the clipboard's ([`clipboard_chord`]) paste, copy and select all. A key
/// pressed with Command held goes no further ([`command_held`]). Everything else — every letter, `Ctrl+C`,
/// the arrows, `Tab` — goes to the child unchanged.
fn type_keys(
    mut events: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mut terminal: ResMut<Terminal>,
    focus: Res<TerminalFocus>,
    find: Res<TerminalFind>,
) {
    if !focus.holds() {
        return;
    }
    // **The find box has the keys** (#321): they are its query, and none of them is the shell's. Read
    // and dropped rather than left, or the shell would be sent them the frame the box closed. The frame
    // an Escape closed the box is the box's too: an Escape sent on would make readline read the next
    // key as Meta.
    if find.has_the_keys() || find.closed_this_frame() {
        events.clear();
        return;
    }
    // **The close chord is the pane's**, and it is read from `ButtonInput` by `toggle_on_the_chord`
    // (which runs in `TerminalChordSet`, after this system). Forwarding it to the shell as well would
    // send a `\x1c`-ish byte every time somebody lowered the pane.
    let closes = bevy_gooey::Shortcut::new(bevy_gooey::Modifiers::PRIMARY, KeyCode::Backquote);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let mut out: Vec<u8> = Vec::new();
    for event in events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        // **A bare `PageUp`/`PageDown` pages the pane's scrollback**, and `Shift` sends the sequence to
        // the child so `less` and `vim` still page themselves. Without this the wheel was the only way
        // to read a build that had scrolled past, and a keyboard-only author had no way at all.
        if !shift {
            match event.key_code {
                KeyCode::PageUp => {
                    if let Some(tab) = terminal.tab_mut() {
                        tab.screen_mut().scroll_page(true);
                    }
                    continue;
                }
                KeyCode::PageDown => {
                    if let Some(tab) = terminal.tab_mut() {
                        tab.screen_mut().scroll_page(false);
                    }
                    continue;
                }
                _ => {}
            }
        }
        if closes.just_pressed(&keys)
            || crate::terminal_pane::COLLAPSE_CHORD.just_pressed(&keys)
            || FIND_CHORD.just_pressed(&keys)
            || crate::terminal_pane::CLEAR_CHORD.just_pressed(&keys)
        {
            // The pane's own chords: `toggle_on_the_chord` and `collapse_on_the_chord` read the same
            // `ButtonInput` this frame, so this system's job is only to **not** forward them to the
            // shell — on Linux Ctrl+J is also a newline, and collapsing the pane must not run a line.
            continue;
        }
        match clipboard_chord(&keys, event.key_code) {
            Some(ClipboardChord::Paste) => {
                if let Ok(mut clipboard) = arboard::Clipboard::new()
                    && let Ok(text) = clipboard.get_text()
                {
                    // **A paste is bracketed when the child asked for it** (`ESC[?2004h`), which is how a
                    // shell tells a multi-line insertion apart from a person typing `Enter` between lines.
                    // Sending it unbracketed would run the second line before the person read the first.
                    if bracketed_paste(&terminal) {
                        out.extend_from_slice(b"\x1b[200~");
                        out.extend_from_slice(text.as_bytes());
                        out.extend_from_slice(b"\x1b[201~");
                    } else {
                        out.extend_from_slice(text.as_bytes());
                    }
                }
                continue;
            }
            Some(ClipboardChord::Copy) => {
                if let Some(tab) = terminal.tab()
                    && let Some(text) = tab.screen().selection_text()
                    && let Ok(mut clipboard) = arboard::Clipboard::new()
                {
                    let _ = clipboard.set_text(text);
                }
                continue;
            }
            Some(ClipboardChord::SelectAll) => {
                if let Some(tab) = terminal.tab_mut() {
                    tab.screen_mut().select_all();
                }
                continue;
            }
            None => {}
        }
        if command_held(&keys) {
            // A Command chord is never the shell's letter, except the three that edit its line.
            if let Some(bytes) = line_editing_chord(&keys, event.key_code) {
                out.extend_from_slice(bytes);
            }
            continue;
        }
        let app_cursor = terminal.tab().is_some_and(|tab| tab.screen().app_cursor());
        out.extend_from_slice(&key_bytes(event, &keys, app_cursor));
    }
    if !out.is_empty() {
        terminal.write(&out);
    }
}

/// Did the child ask for bracketed paste? `ESC[?2004h` sets `BRACKETED_PASTE`.
///
/// A paste that is bracketed is wrapped in `ESC[200~`/`ESC[201~`, which is how a shell or an editor
/// running in the shell tells a multi-line insertion apart from a person typing `Enter` between the
/// lines. Sending it unbracketed would run the second line before the person had read the first —
/// the reason `vim` and every REPL ask for this mode.
fn bracketed_paste(terminal: &Terminal) -> bool {
    terminal
        .tab()
        .is_some_and(|tab| tab.screen().bracketed_paste())
}

/// **The bytes a terminal sends for one key event.**
///
/// Data, not logic: the VT100 vocabulary, and the order of the tests is the order the cases must be
/// tried in (`Enter` before `Character`, because a keyboard reports both).
///
/// Every sequence here is what `xterm-256color`'s terminfo declares, which is the `TERM` the session
/// is opened with ([`crate::terminal::pty`]) — so what a program is told and what actually arrives
/// cannot disagree.
fn key_bytes(event: &KeyboardInput, keys: &ButtonInput<KeyCode>, app_cursor: bool) -> Vec<u8> {
    use crate::terminal::input::{CursorKey, Mods, cursor_key};
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let mods = Mods { shift, alt, ctrl };
    let mut out = Vec::new();

    match &event.logical_key {
        Key::Enter => out.push(b'\r'),
        Key::Tab => out.push(b'\t'),
        Key::Space => out.push(b' '),
        Key::Backspace => out.push(0x7f),
        Key::Escape => out.push(0x1b),
        // A shell in raw mode reads these; `vim`, `less` and `fzf` are what they are for. **Spelled
        // for the mode the program set** (#311): an ncurses program in keypad mode reads `ESC [ A` as
        // Escape and `[`, and a modifier is a parameter (`ESC [ 1 ; 5 C` is Ctrl+Right, a word right).
        Key::ArrowUp => out.extend(cursor_key(CursorKey::Up, mods, app_cursor)),
        Key::ArrowDown => out.extend(cursor_key(CursorKey::Down, mods, app_cursor)),
        Key::ArrowRight => out.extend(cursor_key(CursorKey::Right, mods, app_cursor)),
        Key::ArrowLeft => out.extend(cursor_key(CursorKey::Left, mods, app_cursor)),
        Key::Home => out.extend(cursor_key(CursorKey::Home, mods, app_cursor)),
        Key::End => out.extend(cursor_key(CursorKey::End, mods, app_cursor)),
        // **`PageUp`/`PageDown` page the pane's own scrollback**, which is what a terminal does; a
        // full-screen program that wants the sequence gets it with `Shift`, so `less` still pages
        // itself. `key_bytes` cannot scroll (it returns bytes), so the caller recognises these two
        // before it gets here — see `type_keys`.
        // With `Shift`, a full-screen program pages itself; a bare press is handled in `type_keys`
        // before it reaches here, because paging the pane is not a byte sequence.
        Key::PageUp if shift => out.extend_from_slice(b"\x1b[5~"),
        Key::PageDown if shift => out.extend_from_slice(b"\x1b[6~"),
        Key::Insert => out.extend_from_slice(b"\x1b[2~"),
        Key::Delete => out.extend_from_slice(b"\x1b[3~"),
        // **A `Ctrl` chord over a letter is its ASCII control code**, which is the oldest rule in
        // terminal input and the one that makes `Ctrl+C` an interrupt: `a` is 0x61, and `Ctrl+a` is
        // 0x01. `Ctrl+Space` is NUL, which a shell reads as the same "clear the line".
        Key::Character(text) => {
            if ctrl {
                if let Some(first) = text.chars().next()
                    && first.is_ascii_alphabetic()
                {
                    let code = (first.to_ascii_lowercase() as u8) & 0x1f;
                    out.push(code);
                }
                // `Ctrl+Space` and the punctuation a shell binds (`Ctrl+\` is 0x1c) are not letters;
                // forwarding their raw form would type the character instead of running the command.
            } else {
                out.extend_from_slice(text.as_bytes());
            }
        }
        // **`event.text` is the authoritative characters**, and it is the only place some input
        // arrives: an IME composition, a dead key that combined, and an injected key from an agent
        // (`bevy_fairy/input`) all report `Key::Unidentified` with the text in this field. Reading the
        // logical key alone made those keystrokes vanish — measured live: `world.query` showed the
        // shell running and every typed character going nowhere.
        _ => {
            match &event.text {
                Some(text) if !text.is_empty() => {
                    if ctrl {
                        if let Some(first) = text.chars().next()
                            && first.is_ascii_alphabetic()
                        {
                            out.push((first.to_ascii_lowercase() as u8) & 0x1f);
                        }
                    } else {
                        out.extend_from_slice(text.as_bytes());
                    }
                }
                // A modifier's own press is not text: forwarding it would send a stray byte per chord.
                _ => return Vec::new(),
            }
        }
    }
    // **`Alt` is the `ESC` prefix**, which is how every terminal emulator spells Meta.
    if alt && !out.is_empty() && out[0] != 0x1b {
        out.insert(0, 0x1b);
    }
    out
}

/// **The scrollback is bounded, and the bound is stated.** A `cargo build -v` produces megabytes; a
/// terminal that kept them all would leak in a program meant to run for a working day.
const _: usize = SCROLLBACK;

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::text::{GlyphAtlasInfo, PositionedGlyph, TextLayoutInfo};

    use super::*;

    /// A laid-out glyph whose pen is at `pen` and whose ink is `ink` wide, `bearing` right of the pen:
    /// `bevy_text-0.20.0/src/pipeline.rs:430` gives the ink's centre, `size / 2 + pen + offset`.
    fn glyph(pen: f32, ink: f32, bearing: f32) -> PositionedGlyph {
        PositionedGlyph {
            position: Vec2::new(ink / 2.0 + pen + bearing, 0.0),
            atlas_info: GlyphAtlasInfo {
                texture: AssetId::default(),
                rect: Rect::new(0.0, 0.0, ink, 1.0),
                offset: Vec2::new(bearing, 0.0),
                is_alpha_mask: true,
            },
            section_index: 0,
            line_index: 0,
        }
    }

    /// **The cell is the pens' advance, whichever letters the measured row starts with.** The numbers are
    /// `sh-5.3$`'s first two glyphs as Bevy laid them out at scale 1.2: an `s` of 6 px ink from its pen, an
    /// `h` of 5 px ink 1 px right of its pen, the pens 7.2 px apart. Their ink centres are 7.7 px apart, which
    /// was taken for a cell of 6.42 logical px.
    #[test]
    fn a_cell_is_the_advance_between_two_pens_not_between_two_inks() {
        const SCALE: f32 = 1.2;
        const ADVANCE_PX: f32 = 7.2;
        let mut world = World::new();
        world.init_resource::<CellMetrics>();
        let layout = TextLayoutInfo {
            scale_factor: SCALE,
            glyphs: vec![glyph(0.0, 6.0, 0.0), glyph(ADVANCE_PX, 5.0, 1.0)],
            size: Vec2::new(2.0 * ADVANCE_PX, 15.0),
            ..default()
        };
        world.spawn((TerminalRow(0), terminal_font(1.0), layout));
        world.run_system_once(measure_the_cell).unwrap_or_else(|e| panic!("{e}"));
        let metrics = world.resource::<CellMetrics>();
        assert!(metrics.is_measured(), "a laid-out row is a measurement");
        assert!(
            (metrics.width() - ADVANCE_PX / SCALE).abs() < 1e-4,
            "one cell is {} logical px (the pens' advance), and was measured as {}",
            ADVANCE_PX / SCALE,
            metrics.width()
        );
    }

    /// `keys` with `held` down and `key` pressed this frame.
    fn pressed(held: &[KeyCode], key: KeyCode) -> ButtonInput<KeyCode> {
        let mut keys = ButtonInput::<KeyCode>::default();
        for k in held {
            keys.press(*k);
        }
        keys.press(key);
        keys
    }

    /// feedback-1794024642593: a Mac terminal pastes with Cmd+V, copies with Cmd+C and selects all with Cmd+A.
    #[test]
    fn command_v_pastes_command_c_copies_and_command_a_selects_all() {
        for (key, chord) in [
            (KeyCode::KeyV, ClipboardChord::Paste),
            (KeyCode::KeyC, ClipboardChord::Copy),
            (KeyCode::KeyA, ClipboardChord::SelectAll),
        ] {
            let keys = pressed(&[KeyCode::SuperLeft], key);
            assert_eq!(clipboard_chord(&keys, key), Some(chord), "Command+{key:?}");
            assert!(command_held(&keys), "Command+{key:?} is a chord, never a letter for the shell");
        }
    }

    /// The platform's primary modifier with Shift keeps the chords it had: Ctrl+Shift+V and Ctrl+Shift+C off a Mac.
    #[test]
    fn primary_shift_still_pastes_and_copies() {
        let primary = crate::keys::MOD_KEYS[0];
        assert_eq!(clipboard_chord(&pressed(&[primary, KeyCode::ShiftLeft], KeyCode::KeyV), KeyCode::KeyV), Some(ClipboardChord::Paste));
        assert_eq!(clipboard_chord(&pressed(&[primary, KeyCode::ShiftLeft], KeyCode::KeyC), KeyCode::KeyC), Some(ClipboardChord::Copy));
    }

    /// A letter, and Ctrl with a letter, are the shell's: Ctrl+V is readline's quoted insert, Ctrl+C an interrupt.
    #[test]
    fn a_letter_and_a_control_letter_stay_the_shells() {
        for held in [&[][..], &[KeyCode::ControlLeft][..]] {
            for key in [KeyCode::KeyV, KeyCode::KeyC, KeyCode::KeyA] {
                let keys = pressed(held, key);
                assert_eq!(clipboard_chord(&keys, key), None, "{held:?} + {key:?}");
                assert!(!command_held(&keys), "{held:?} + {key:?}");
            }
        }
    }

    /// **Command with Backspace, Left and Right edit the line the shell is reading** (feedback-1794146958145): `^U`, `^A` and `^E`,
    /// what VS Code's and iTerm2's terminals send on a Mac.
    #[test]
    fn command_backspace_left_and_right_send_the_line_editing_codes() {
        for (key, bytes) in [(KeyCode::Backspace, b"\x15"), (KeyCode::ArrowLeft, b"\x01"), (KeyCode::ArrowRight, b"\x05")] {
            for command in [KeyCode::SuperLeft, KeyCode::SuperRight] {
                assert_eq!(line_editing_chord(&pressed(&[command], key), key), Some(&bytes[..]), "{command:?} + {key:?}");
            }
        }
    }

    /// **Only Command alone, and only those three keys**: another modifier with it is another gesture, a bare key is the shell's own
    /// (Backspace is `DEL`, an arrow its escape sequence), and no other Command chord is sent on.
    #[test]
    fn other_chords_are_not_line_editing() {
        for held in [
            &[KeyCode::SuperLeft, KeyCode::ShiftLeft][..],
            &[KeyCode::SuperLeft, KeyCode::AltLeft][..],
            &[KeyCode::SuperLeft, KeyCode::ControlLeft][..],
            &[][..],
            &[KeyCode::AltLeft][..],
        ] {
            for key in [KeyCode::Backspace, KeyCode::ArrowLeft, KeyCode::ArrowRight] {
                assert_eq!(line_editing_chord(&pressed(held, key), key), None, "{held:?} + {key:?}");
            }
        }
        for key in [KeyCode::KeyU, KeyCode::KeyA, KeyCode::Delete, KeyCode::ArrowUp, KeyCode::Home] {
            assert_eq!(line_editing_chord(&pressed(&[KeyCode::SuperLeft], key), key), None, "Command + {key:?}");
        }
    }
}
