//! **The one plugin that owns the editor's UI library** (UI-01, #209).
//!
//! The editor's UI is `bevy_gooey`, in every build. [`EditorUiPlugin`] is the only place that adds
//! gooey's plugins, so a later step of the UI series (a theme, a command palette, a menu bar) adds
//! its gooey plugin here and nowhere else.

use bevy::prelude::*;
use bevy_gooey::{GooeyNavPlugin, GooeySkin, GooeyStylePlugin};

/// The editor's own skin, as data: `src/editor_skin.yaml`.
const EDITOR_SKIN: &str = include_str!("editor_skin.yaml");

/// **The editor's look as `bevy_gooey`'s own type** (UI-02, #210): the colours and measures
/// `chrome.rs` draws with, the elevation ladder in `surfaces` and the sizes a compact tool wants.
/// `GooeySkin::problems()` is the check that the ladder is a ladder and the ink is readable.
pub fn editor_skin() -> GooeySkin {
    crate::ui_skin::from_yaml(EDITOR_SKIN).unwrap_or_else(|e| panic!("src/editor_skin.yaml does not read: {e}"))
}

/// **Parchment's colours**, as data: `src/editor_theme_parchment.yaml` (#428).
const PARCHMENT_LOOK: &str = include_str!("editor_theme_parchment.yaml");

/// **High contrast's colours**, as data: `src/editor_theme_high_contrast.yaml` (#428).
const HIGH_CONTRAST_LOOK: &str = include_str!("editor_theme_high_contrast.yaml");

/// The least contrast the editor's own ink may have against a theme's ground: WCAG 2.2, success
/// criterion 1.4.3, the floor `bevy_gooey::problems()` holds a skin's own text to.
const LEAST_TEXT_CONTRAST: f32 = bevy_gooey::TEXT_CONTRAST;

/// **The editor's skin for a theme** (UI-73, #251; every colour of it, #428): `editor_skin.yaml` for
/// [`Theme::ToolDark`], and for the other two that file's sizes under the theme's own colours.
///
/// A theme's colours are a file, like Tool dark's: its eight-step ladder, its `hover`, `pressed` and `card`
/// fills and the editor's own tones (`key`, `label`, `muted`, `suggest`, ...). A theme that set only a
/// preset's dozen colours would leave those tones the dark theme's, and the chrome draws with them — that
/// was the whole of #428: near-white labels on Parchment's cream. `surface`, `text` and `accent` are still
/// gooey's preset's own (`GooeySkin::parchment()`, `GooeySkin::high_contrast()`); the rest are set so every
/// ink the editor draws reads on every ground it is drawn on, which [`unreadable_pairs`] asks.
///
/// **The sizes stay the editor's.** [`GooeySkin::with_colours_of`](bevy_gooey::GooeySkin::with_colours_of)
/// takes a theme's colours and keeps this skin's metrics, so changing the theme never moves a layout: a
/// person who set 200% text and then asks for high contrast has both. It does not carry the tones, which
/// are copied here.
///
/// Reads and parses its file on each call: callers ask when the theme changes, never per frame.
///
/// [`Theme::ToolDark`]: crate::settings::Theme::ToolDark
pub fn skin_for(theme: crate::settings::Theme) -> GooeySkin {
    use crate::settings::Theme;
    let (look, file) = match theme {
        Theme::ToolDark => return editor_skin(),
        Theme::Parchment => (PARCHMENT_LOOK, "src/editor_theme_parchment.yaml"),
        Theme::HighContrast => (HIGH_CONTRAST_LOOK, "src/editor_theme_high_contrast.yaml"),
    };
    let look = crate::ui_skin::from_yaml(look).unwrap_or_else(|e| panic!("{file} does not read: {e}"));
    let mut skin = editor_skin().with_colours_of(&look);
    skin.tones = look.tones;
    skin
}

/// **The inks the editor's own labels are drawn in, named** — the side a theme's ground is measured
/// against (UI-73, #251), read from `skin` by [`chrome::Ink::of`](crate::chrome::Ink::of), the same
/// function the chrome paints from, so what is measured is what is drawn.
pub fn editor_inks(skin: &GooeySkin) -> Vec<(&'static str, Color)> {
    let ink = crate::chrome::Ink::of(skin);
    vec![
        ("text", ink.text),
        ("text_muted", ink.dim),
        ("accent", ink.accent),
        ("danger", ink.danger),
        ("key", ink.key),
        ("label", ink.label),
        ("labeled", ink.labeled),
        ("muted", ink.muted),
        ("suggest", ink.suggest),
    ]
}

/// **The grounds a theme puts under the editor's own ink**: its `surface` — a panel, composited onto
/// its background because a panel is what a label is drawn on — and its `surface_raised`, which is a
/// row, a header and a slot.
fn grounds_of(skin: &GooeySkin) -> [(&'static str, Color); 2] {
    [("surface", skin.solid_surface()), ("surface_raised", skin.surface_raised)]
}

/// **One sRGB colour's channels**, as [`bevy_gooey::contrast_ratio`] takes them. Alpha is dropped: the
/// rule [`GooeySkin::problems`](bevy_gooey::GooeySkin::problems) keeps, and `solid_surface` has
/// already composited the ground.
fn channels(colour: Color) -> [f32; 3] {
    let c = bevy::color::Srgba::from(colour);
    [c.red, c.green, c.blue]
}

/// **Every pair a theme's ground would drop below [`LEAST_TEXT_CONTRAST`]**, as
/// `"text on surface 1.12:1"`, over [`editor_inks`] and then a panel's ground and a row's.
///
/// Empty for each of the three themes the editor offers (`tests/the_theme_moves_every_colour.rs`):
/// every ink the chrome draws is one of the theme's own, so there is nothing a theme's ground can leave
/// unreadable. Anything in it is a theme file that was edited into an unreadable pair.
pub fn unreadable_pairs(theme_skin: &GooeySkin) -> Vec<String> {
    let grounds = grounds_of(theme_skin);
    let mut out = Vec::new();
    for (ink_name, ink) in editor_inks(theme_skin) {
        for (ground_name, ground) in grounds {
            let ratio = bevy_gooey::contrast_ratio(channels(ink), channels(ground));
            if ratio < LEAST_TEXT_CONTRAST {
                out.push(format!("{ink_name} on {ground_name} {ratio:.2}:1"));
            }
        }
    }
    out
}

/// **The theme and the text size reach the running skin** (UI-73, #251): the settings resolve into
/// [`crate::settings::EditorSettings`], and this is how a choice in the settings screen — or a file a
/// person edited — becomes the skin a widget draws from. `crate::theme_ink::follow_the_skin` then
/// moves the editor's own ink, and everything drawn in it, onto that skin the same frame (#428).
///
/// **A settled editor costs one comparison.** The theme and the size last decided are kept in `Local`s,
/// and a pass whose two inputs are the ones already answered returns before [`skin_for`] is called:
/// parsing a theme's yaml sixty times a second, to write what is already there, is work with no answer
/// at the end of it.
///
/// **The colours and the scale are written separately, and that is not tidiness.** [`skin_for`] builds
/// the theme's colours at the editor's own sizes, so a person's 200% text would be reset by a theme
/// change if the two were assigned together.
///
/// **The first pass under Tool dark writes no colours.** [`EditorUiPlugin`] has already put the
/// editor's own skin in as the running one, so a run that has only ever seen the default leaves it
/// exactly as it was found. A *return* to Tool dark after another theme is still an application.
///
/// **A project's `family/bevy_gooey.yaml` is not this system's business** (UI-03, #211). That file is a
/// *game's* palette: it lives in [`crate::ui_skin::GameSkin`] and reaches the window only through the
/// Game UI preview panel's `SkinScope`, so the running skin here is always the person's theme whatever
/// a project ships.
///
/// **Every theme is honoured** (#428). It used to refuse Parchment, because `chrome.rs`'s labels were
/// the dark theme's near-white whatever ran and a cream ground left them at 1.12:1. They are the
/// running skin's now, so there is nothing to refuse.
///
/// It runs after the settings are read and before gooey styles, so a theme chosen this frame is the
/// skin this frame's widgets are painted from.
pub fn follow_the_theme(
    settings: Option<Res<crate::settings::EditorSettings>>,
    mut skin: ResMut<GooeySkin>,
    // The theme this system last decided, so a settled editor returns at once — and so a run that has
    // only ever seen the default leaves the skin exactly as `EditorUiPlugin` put it.
    mut applied: Local<Option<crate::settings::Theme>>,
) {
    let Some(settings) = settings else { return };
    let theme = settings.theme;
    // **One comparison is the whole frame.** Nothing moved — not the theme, not the size — so nothing
    // is measured, parsed or written.
    if applied.as_ref() == Some(&theme) && (skin.text_scale - settings.text_scale).abs() < f32::EPSILON {
        return;
    }
    let first_default = theme == crate::settings::Theme::default() && applied.is_none();
    *applied = Some(theme);
    // The colours `EditorUiPlugin` inserted stay for a first pass under the default; otherwise they are
    // the theme's.
    let mut want = if first_default { skin.clone() } else { skin_for(theme) };
    // The theme's colours *and* the person's scale, in one value: the theme is built at the editor's
    // own sizes, so the scale is written on top of them, never instead of them.
    want.text_scale = settings.text_scale;
    if *skin != want {
        *skin = want;
    }
}

/// **How the editor spells a chord: in words, `Cmd+Z`, never the symbol `⌘Z`** (`feedback-1794097488318`, and
/// `feedback-1793927744511` before it; `bevy-coven/bevy_gooey#45`).
///
/// gooey spells a Mac's chords with Apple's symbols by default (`⌘⌃⌥⇧` and `↩⌫⌦⎋⇥`), and a face that does not
/// map a symbol draws the missing-glyph box, which the owner twice reported as "a `?`". The face this editor
/// ships, Fira Mono, lacks `⌘ ⌃ ⌥ ↩ ⎋ ⇥`, six of the thirteen, and `⌘` is on nearly every chord. Only the host
/// knows which face it ships, so gooey makes the spelling a host setting
/// ([`bevy_gooey::BindingSettings::chord_glyphs`]); this is the editor's answer. It reaches every place gooey
/// draws a chord: a tooltip's keycap, a menu row, the badges and the palette. On a platform that is not a Mac
/// the two spellings are the same string.
pub fn editor_binding_settings() -> bevy_gooey::BindingSettings {
    bevy_gooey::BindingSettings { chord_glyphs: bevy_gooey::ChordGlyphs::Words, ..default() }
}

/// Adds gooey's style plugin and its navigation plugin, puts the editor's skin in as the running one and adds the system that applies the project's skin file ([`crate::ui_skin::apply_ui_skin`]).
pub struct EditorUiPlugin;

/// **The wheel scrolls a panel** (#283): `bevy_ui_widgets::ScrollAreaPlugin`, which is the machinery
/// that turns a wheel event over a `ScrollArea` into a `ScrollPosition`.
///
/// `chrome::scroll_list` has put a `ScrollArea` and `overflow: scroll_y()` on every scrollable box in
/// this editor since it was written, and drew a scrollbar beside it — but `DefaultPlugins` does not
/// include `UiWidgetsPlugins`, and nothing here added this one, so **no panel answered the wheel at
/// all**. Measured on big 2026-11-03 at 1280x720: the INSPECTOR's `ENVIRONMENT` heading sits at
/// y = 898 in a 720 px window, and ten wheel notches over the panel left it at 898.5. Every section
/// below `EDGES` was unreachable, to a person as much as to a storyboard.
///
/// Its own plugin rather than a line inside [`EditorUiPlugin`] so a test can add exactly this and ask
/// whether it arrived, which is what `tests/the_wheel_scrolls_a_panel.rs` does.
pub struct ScrollWheelPlugin;

impl Plugin for ScrollWheelPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy::ui_widgets::ScrollAreaPlugin>() {
            app.add_plugins(bevy::ui_widgets::ScrollAreaPlugin);
        }
    }
}

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<GooeyStylePlugin>() {
            app.add_plugins(GooeyStylePlugin);
        }
        app.add_plugins(ScrollWheelPlugin);
        // **Menus need it** (UI-36, #230): gooey registers the context-menu click, the popovers' anchoring
        // and their closing in the navigation plugin. The editor's arrow keys stay its own because gooey's
        // only act on widgets that opt in (`AutoDirectionalNavigation`), and the viewport is not one.
        if !app.is_plugin_added::<GooeyNavPlugin>() {
            app.add_plugins(GooeyNavPlugin);
        }
        // **The settings screen** (UI-71, #249): Cmd/Ctrl+, over any screen.
        app.add_plugins((crate::settings_screen::SettingsScreenPlugin, crate::project_config::ProjectConfigPlugin, crate::keymap_screen::KeymapScreenPlugin));
        app.insert_resource(editor_skin());
        // **The editor's own ink follows the running skin** (#428): the last system of every frame moves
        // everything drawn in the old ink onto the new one when a theme changes the skin's colours.
        app.add_plugins(crate::theme_ink::ThemeInkPlugin);
        app.insert_resource(editor_binding_settings());
        // **A person's arrangement, kept and restored** (UI-33, #227). The observer saves each change
        // gooey reports; the system is what a file edit or Reset reaches the dock through.
        app.add_observer(save_the_layout);
        app.add_systems(
            Update,
            follow_the_saved_layout.after(crate::settings::sync_settings).before(bevy_gooey::GooeySystems::Style),
        );
        // **The shell's regions as gooey panels** (UI-30, #224). A `Dock` shows only what a
        // `PanelRegistry` names, so the regions the editor draws are registered here — the one place
        // that owns the UI library — and the dock that arranges them is spawned in
        // `chrome::frame`, where the outer layout already lives.
        //
        // The contents keep the editor's own colours for now; UI-02 (#210) moves those onto the skin.
        // What changes here is the *arrangement*: a region can be dragged and resized by gooey rather
        // than by a hand-built flex row nothing can move.
        app.add_systems(PreStartup, register_the_shell_panels);
        // **`K` is the reveal, and gooey owns the rule** (UI-41, #235; bevy-coven/bevy_gooey#36).
        //
        // `ShortcutBadges` is ExposeHK's overlay, and until G-32 it could only be shown by a
        // *modifier* held exactly — `Modifiers::held(keys) == hold`. This editor's overlay key is
        // `K`, a plain key (`keys::Action::Shortcuts`, and `a_key_still_fires_while_k_is_held` pins
        // that `K`+verb still fires), so its own `keys::pressed` poll was the only way to reveal it.
        // `Reveal::Key` says the same thing in the crate, and `BadgesShown` reports it back.
        //
        // **Zero delay**: `K` is not a chord prefix but the overlay's own key, and the 0.3 s default
        // exists to stop a modifier flashing the screen on the way to `Ctrl+S`. Nothing types `K`
        // to reach a chord, so the delay would only make the overlay feel slow.
        app.insert_resource(bevy_gooey::ShortcutBadges {
            hold: bevy_gooey::Reveal::Key(bevy::input::keyboard::KeyCode::KeyK),
            delay_seconds: 0.0,
        });
        // **Refusals stand at the top of the viewport** (UI-39, #233), where the card they replace stood.
        app.insert_resource(bevy_gooey::ToastSettings { anchor: bevy_gooey::ToastAnchor::TopCenter, ..default() });
        // **The project's game skin, and the preview's scope on it** (UI-03, #211). Its own plugin,
        // because the file is a game's palette: it reaches the window only through the Game UI preview
        // panel, never the editor's chrome.
        app.add_plugins(crate::ui_skin::GameUiPreviewPlugin);
        // **The person's text size reaches the editor's own labels** (UI-73, #251): `chrome::font`
        // reads one atomic, so a size chosen in Settings enlarges them with gooey's own widgets.
        app.add_systems(
            Update,
            follow_the_text_size.after(crate::settings::sync_settings).before(bevy_gooey::GooeySystems::Style),
        );
        // **And a label that was already drawn follows it** (UI-73, #251): `follow_the_text_size`
        // moves the atomic `chrome::font` reads, which is every label spawned *after* the change. A
        // label spawned before it keeps the size it was built at, so this rewrites every `TypeRole`
        // from its role the frame the setting differs from the one last drawn. After
        // `follow_the_text_size`, so the two agree about the scale being applied.
        app.add_systems(
            Update,
            crate::chrome::resize_the_labels
                .after(follow_the_text_size)
                .before(bevy_gooey::GooeySystems::Style),
        );
        // **The player's Reduced motion reaches gooey** (UI-R01): travel lands, colours and fades still ease.
        app.add_systems(
            Update,
            follow_the_motion_setting.after(crate::settings::sync_settings).before(bevy_gooey::GooeySystems::Style),
        );
        // **The theme reaches the running skin** (UI-73, #251), after the settings are read. A project's
        // `family/bevy_gooey.yaml` is not consulted: it is the *game's* palette and lives in `GameSkin`
        // for the Game UI preview (UI-03, #211), so the running skin here is always the person's theme.
        app.add_systems(
            Update,
            follow_the_theme
                .after(crate::settings::sync_settings)
                .after(crate::ui_skin::skin_scope)
                .before(bevy_gooey::GooeySystems::Style),
        );
        // Before gooey's colour motion, which leads every colour to the last one written: a button this paints
        // is that colour's target, and colours it writes after the motion would be a frame ahead of it.
        app.add_systems(
            Update,
            (paint_marked_buttons, clear_stale_rails).chain().after(bevy_gooey::GooeySystems::Style).before(bevy_gooey::update_color_motion),
        );
    }
}

/// A [`bevy_gooey::button`] that is the screen's primary action: gooey's [`bevy_gooey::ButtonVariant::Submit`] (the accent fill,
/// lifting under the pointer; UI-R03, #333). A screen has at most one. This replaced the editor's own marker and repaint
/// system, which existed only because gooey had one kind of button (bevy-coven/bevy_gooey#40).
pub fn submit_button(text: impl Into<String>) -> impl Bundle {
    (bevy_gooey::button(text), bevy_gooey::ButtonVariant::Submit)
}

/// Repaints a [`bevy::ui::Selected`] button (a card or a navigation row that is the chosen one: the skin's hovered fill, plus a
/// [`SelectedRail`]). It runs after gooey's restyle, which does not restyle on a colour change, so the two never fight; a pressed
/// selected button keeps gooey's own pressed fill.
///
/// **The rail is why this exists at all now.** A fill a few ramp steps lighter is not a selected state a person can see:
/// measured in the shipped 0.2.4 screenshot on 2026-11-05, `Projects` — the selected tab — was not tellable from `Learn`. A
/// shape can be, and the palette already has one (`crate::chrome::SELECT_RAIL_W`), so a selected button gains a strip down its
/// leading edge. It is inserted by this system rather than by each caller: every `Selected` button already means "this one",
/// so every one of them should say it the same way.
#[derive(Component)]
struct SelectedRail;

#[allow(clippy::type_complexity)]
fn paint_marked_buttons(
    skin: Res<bevy_gooey::GooeySkin>,
    mut buttons: Query<
        (Entity, &mut BackgroundColor, Has<bevy::ui::Pressed>, Has<bevy::ui::InteractionDisabled>, Option<&Children>),
        (With<bevy_gooey::Role>, With<bevy::ui::Selected>),
    >,
    mut rails: Query<&mut BackgroundColor, (With<SelectedRail>, Without<bevy_gooey::Role>)>,
    parents: Query<&ChildOf>,
    opacities: Query<&bevy_gooey::Opacity>,
    mut commands: Commands,
) {
    for (entity, mut fill, pressed, disabled, children) in &mut buttons {
        // **A button in a dialog that is fading out fades with it.** gooey multiplies the colours it writes by
        // the `Opacity` at and above a node; these are written after it, so they take the same factor, or the
        // chosen tab would stand solid in a card that is going.
        let faded = {
            let (mut factor, mut at) = (1.0_f32, entity);
            loop {
                if let Ok(opacity) = opacities.get(at) {
                    factor *= opacity.factor();
                }
                match parents.get(at) {
                    Ok(parent) => at = parent.parent(),
                    Err(_) => break factor,
                }
            }
        };
        let with_opacity = |color: Color| if faded < 1.0 { color.with_alpha(color.alpha() * faded) } else { color };
        let want = if disabled {
            skin.surface
        } else if pressed {
            continue;
        } else {
            skin.hovered()
        };
        let want = with_opacity(want);
        if fill.0 != want {
            fill.0 = want;
        }
        // **The rail, for a button that is the chosen one.**
        let wants = !disabled;
        let has = children.is_some_and(|c| c.iter().any(|child| rails.get(child).is_ok()));
        if wants && has {
            for child in children.into_iter().flat_map(|c| c.iter()) {
                if let Ok(mut rail) = rails.get_mut(child)
                    && rail.0 != with_opacity(skin.accent)
                {
                    rail.0 = with_opacity(skin.accent);
                }
            }
        }
        match (wants, has) {
            (true, false) => {
                commands.entity(entity).with_child((
                    SelectedRail,
                    Node {
                        // PLACES-ITSELF-OK: a rail pinned to its own button's leading edge, the way
                        // `crate::chrome::SELECT_RAIL_W` is drawn on a kit's row — it is not a panel.
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(crate::chrome::SELECT_RAIL_W),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(with_opacity(skin.accent)),
                    bevy::picking::Pickable::IGNORE,
                ));
            }
            (false, true) => {
                for child in children.into_iter().flat_map(|c| c.iter()) {
                    if rails.get(child).is_ok() {
                        commands.entity(child).despawn();
                    }
                }
            }
            _ => {}
        }
    }
}

/// **Takes the rail off a button that is no longer `Selected`.** [`paint_marked_buttons`] only visits buttons that still carry
/// `Selected`, so the rail of a button that has just lost it was never removed: choosing the next sidebar tab left the old one
/// railed too (`feedback-1793927950267`).
fn clear_stale_rails(rails: Query<(Entity, &ChildOf), With<SelectedRail>>, selected: Query<(), With<bevy::ui::Selected>>, mut commands: Commands) {
    for (rail, parent) in &rails {
        if selected.get(parent.parent()).is_err() {
            commands.entity(rail).despawn();
        }
    }
}

/// **The regions of the shell, as `bevy_gooey` panels** (UI-30, #224).
///
/// Each is registered with the id a dock layout names. A `Dock` shows what the registry holds when it
/// is built and skips anything it cannot find (`DockPanelSkipped`), so this runs before the frame.
///
/// **Registered once, at `PreStartup`.** The spawn closure fills the panel; a region whose contents
/// the editor draws elsewhere — the viewport is a texture — gets the node that holds them, so the
/// dock's own arrangement is what a person drags.
/// **The reload panel's own rhythm**, in multiples of the skin's gap: half a step, because its lines
/// are one datum each and a full step reads as separate paragraphs (UI-61, #247).
pub const RELOAD_ROW_GAP: f32 = 0.5;

fn register_the_shell_panels(mut registry: ResMut<bevy_gooey::PanelRegistry>) {
    for (id, title) in [
        ("inspector", "INSPECTOR"),
        ("viewport", "VIEWPORT"),
        ("place", "PLACE"),
        ("status", "STATUS"),
    ] {
        registry.register(id, title, move |c| {
            c.insert(bevy_gooey::DockPanel(id.to_owned()));
        });
    }
    // **The bottom dock's own panel, and the first one with contents** (UI-61, #247).
    //
    // The four above are placeholders: the editor draws those regions itself, so their pages are empty
    // by design. This one is a real panel — gooey's dock builds it, gooey's tab names it and gooey's
    // body sizes it — and `editor::refresh_reload_list` fills the column it spawns with what became of
    // each file's latest edit. The lines keep the editor's ink because gooey has no per-severity text
    // role (its `Role::Tone` is a fill): the *panel* is the crate's, and that is what this issue asks.
    registry.register("reload", "RELOAD", |c| {
        c.insert((
            bevy_gooey::DockPanel("reload".to_owned()),
            crate::editor::ReloadPanel,
        ))
        .with_child((bevy_gooey::column(RELOAD_ROW_GAP), crate::editor::ReloadList));
    });
}

/// **The shell's arrangement for a workspace**, as a dock layout a divider can move (UI-30, #224;
/// per workspace since NV-02, #379).
///
/// Each workspace has its own — Map gives the inspector a column, Kit gives the library more room,
/// World puts its panels under the view. The shape the hand-built frame has had since 2026-08-17 is
/// Map's, because that is where a person lands.
/// **A person moved a divider or a tab: their arrangement is saved** (UI-33, #227).
///
/// `bevy_gooey` reports every change a person makes to a `Dock` with `DockLayoutChanged`, carrying the
/// layout it now shows. Writing it to the user's file is what makes the arrangement survive a restart;
/// the editor reads it back through `SettingsLayer::workspace_layout` on the next start.
///
/// **Nothing is written to a test or to a run with no file**: no `UserSettingsFile` means no place the
/// person asked the editor to keep anything, so the event is taken and dropped, and a headless test
/// that drags a divider does not write the real config.
pub fn save_the_layout(
    changed: On<bevy_gooey::DockLayoutChanged>,
    arrangement: Query<(), With<crate::chrome::ArrangementDock>>,
    file: Option<Res<crate::settings::UserSettingsFile>>,
    workspace: Option<Res<crate::workspace::Workspace>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    let Some(file) = file else { return };
    // **A drag in the bottom dock is not the workspace's arrangement** (UI-61, #247): it is that dock's
    // own, and it does not move the split that places the inspector and the viewport.
    if arrangement.get(changed.entity).is_err() {
        return;
    }
    // **`edit_layer`, not `write_key`.** `write_key` speaks the Settings screen's value vocabulary
    // (a switch, a number, a named choice), and a layout is none of those; `edit_layer` is the same
    // primitive underneath it — the file is read as the editor reads it, refused if it does not
    // read, checked to read back, and written atomically — with the layer itself in hand.
    // **Keyed by the workspace that is open**, so a drag in Map moves nothing in Kit.
    let Some(workspace) = workspace.map(|w| *w) else { return };
    let layout = changed.layout.clone();
    let key = workspace.id().to_owned();
    if let Err(why) = crate::setting_catalogue::edit_layer(&file.0, move |layer| {
        layer.workspace_layout.insert(key, layout);
        Ok(())
    }) && let Some(state) = state.as_deref_mut()
    {
        state.status.problem(format!("the layout was not saved: {why}"));
    }
}

/// **A settings read reaches the dock**: a layout written by hand, or Reset's write, is what the shell
/// shows, the same way an edit of `text_scale` reaches the text.
///
/// It reads the resolved settings, not the `DockLayoutChanged` event, because there are two ways a
/// layout changes — a person dragging it (saved by [`save_the_layout`], which the dock already shows)
/// and a file a person edited (which only this can apply). Compared before writing, so a settled
/// editor costs one comparison.
pub fn follow_the_saved_layout(
    settings: Option<Res<crate::settings::EditorSettings>>,
    workspace: Option<Res<crate::workspace::Workspace>>,
    // **The arrangement's dock, and only it** (UI-61, #247): a second gooey `Dock` stands at the foot
    // of the workbench, and writing the workspace's layout into that one would replace the panels it
    // holds with the split that arranges the shell.
    mut docks: Query<&mut bevy_gooey::Dock, With<crate::chrome::ArrangementDock>>,
) {
    let Some(settings) = settings else { return };
    let workspace = workspace.as_deref().copied().unwrap_or_default();
    // **This workspace's arrangement, else its own default.** The `Dock` is rebuilt in the same
    // frame a workspace is entered, so the layout on screen is the one the person left.
    let want = settings
        .workspace_layout
        .get(workspace.id())
        .cloned()
        .unwrap_or_else(|| workspace.default_layout());
    for mut dock in &mut docks {
        if dock.0 != want {
            dock.0 = want.clone();
        }
    }
}

/// **The person's text size reaches every label the editor draws by hand** (UI-73, #251).
///
/// gooey scales its own widgets' type by the skin's `text_scale`; the editor's hand-drawn labels read
/// [`crate::chrome::font`], which has no system parameter to reach a setting through. So the setting is
/// copied into the module's one atomic here, and a settled editor costs one comparison.
///
/// `None` is a harness that never added the settings, and it leaves the scale at 1.0 — the size the
/// design was made at.
fn follow_the_text_size(settings: Option<Res<crate::settings::EditorSettings>>, mut applied: Local<Option<f32>>) {
    let Some(settings) = settings else { return };
    if applied.is_some_and(|last| (last - settings.text_scale).abs() < f32::EPSILON) {
        return;
    }
    *applied = Some(settings.text_scale);
    crate::chrome::set_text_scale(settings.text_scale);
}

/// **Reduced motion is the player's setting, and gooey's `MotionSettings::reduced` is how it is kept**: with it on,
/// what travels (a dialog's rise, a panel's slide, a widget moved by layout) lands at once and colours and fades
/// still ease, which is what WCAG 2.3.3 asks and what keeps a theme switch from popping. Only the one flag is
/// written, so `duration_scale` stays whatever the host set; compared first, so a settled editor writes nothing.
pub fn follow_the_motion_setting(settings: Option<Res<crate::settings::EditorSettings>>, mut motion: ResMut<bevy_gooey::MotionSettings>) {
    let Some(settings) = settings else { return };
    if motion.reduced != settings.reduced_motion {
        motion.reduced = settings.reduced_motion;
    }
}

pub fn shell_layout(workspace: crate::workspace::Workspace) -> bevy_gooey::DockLayout {
    workspace.default_layout()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A rail on a button that is not `Selected` is despawned; one on a `Selected` button stays.**
    #[test]
    fn a_rail_outlives_its_selection_never() {
        let mut app = App::new();
        app.add_systems(Update, clear_stale_rails);
        let kept = app.world_mut().spawn(bevy::ui::Selected).with_child(SelectedRail).id();
        let stale = app.world_mut().spawn_empty().with_child(SelectedRail).id();
        app.update();
        let rails = |app: &mut App, parent: Entity| {
            let world = app.world_mut();
            world.get::<Children>(parent).map_or(0, |c| c.iter().filter(|k| world.get::<SelectedRail>(*k).is_some()).count())
        };
        assert_eq!(rails(&mut app, kept), 1);
        assert_eq!(rails(&mut app, stale), 0);
    }

    /// **Every chord the editor can spell on a Mac is made of codepoints the face it ships maps** (`feedback-1794097488318`).
    ///
    /// The relation is between two things the build holds: the words a chord is drawn with, and the character map of the
    /// face they are drawn in. A codepoint the face does not map is a missing-glyph box, whatever the layout does. Written
    /// over the modifiers a chord can carry and the named keys gooey gives a glyph on a Mac (`↩⌫⌦⎋⇥↑↓←→`), so one more
    /// symbol in gooey's table shows up here as a missing one, and independent of that table: the face is the measurement.
    #[test]
    fn every_chord_spelled_on_a_mac_is_drawable_in_the_face_the_editor_ships() {
        use bevy_gooey::{Modifiers, Platform, Shortcut};
        let face = ttf_parser::Face::parse(include_bytes!("../assets/fonts/FiraMono-Regular.ttf"), 0)
            .unwrap_or_else(|e| panic!("the shipped face does not parse: {e}"));
        let settings = bevy_gooey::BindingSettings { platform: Some(Platform::Mac), ..editor_binding_settings() };
        let modifiers = [
            Modifiers::NONE,
            Modifiers::CTRL,
            Modifiers::SHIFT,
            Modifiers::ALT,
            Modifiers::META,
            Modifiers::CTRL | Modifiers::SHIFT | Modifiers::ALT | Modifiers::META,
        ];
        let keys = [
            KeyCode::KeyZ,
            KeyCode::Digit1,
            KeyCode::Comma,
            KeyCode::Enter,
            KeyCode::Backspace,
            KeyCode::Delete,
            KeyCode::Escape,
            KeyCode::Tab,
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
        ];
        let mut missing = Vec::new();
        for modifiers in modifiers {
            for key in keys {
                let words = settings.chord(Shortcut::new(modifiers, key));
                missing.extend(
                    words.chars().filter(|c| face.glyph_index(*c).is_none()).map(|c| format!("{words:?}: U+{:04X} {c}", c as u32)),
                );
            }
        }
        assert!(missing.is_empty(), "a chord spelled for a Mac has codepoints the face lacks:\n  {}", missing.join("\n  "));
    }
}
