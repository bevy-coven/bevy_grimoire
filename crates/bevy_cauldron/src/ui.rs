//! **The one plugin that owns the editor's UI library** (UI-01, #209).
//!
//! The editor's UI is `bevy_gooey`, in every build. [`EditorUiPlugin`] is the only place that adds
//! gooey's plugins, so a later step of the UI series (a theme, a command palette, a menu bar) adds
//! its gooey plugin here and nowhere else.

use bevy::prelude::*;
use bevy_gooey::{GooeyNavPlugin, GooeySkin, GooeyStylePlugin};

/// The editor's own skin, as data: `src/editor_skin.ron`.
const EDITOR_SKIN: &str = include_str!("editor_skin.ron");

/// **The editor's look as `bevy_gooey`'s own type** (UI-02, #210): the colours and measures
/// `chrome.rs` draws with, the elevation ladder in `surfaces` and the sizes a compact tool wants.
/// `GooeySkin::problems()` is the check that the ladder is a ladder and the ink is readable.
pub fn editor_skin() -> GooeySkin {
    crate::ui_skin::from_ron(EDITOR_SKIN).unwrap_or_else(|e| panic!("src/editor_skin.ron does not read: {e}"))
}

/// The least contrast the editor's own ink may have against a theme's ground: WCAG 2.2, success
/// criterion 1.4.3, the floor `bevy_gooey::problems()` holds a skin's own text to.
const LEAST_TEXT_CONTRAST: f32 = bevy_gooey::TEXT_CONTRAST;

/// **The editor's skin for a theme** (UI-73, #251): its own measured colours for [`Theme::ToolDark`],
/// and `bevy_gooey`'s preset for the two the editor does not ship.
///
/// Tool dark is the editor's own `editor_skin.ron` because the editor's darks are *measured*
/// (`chrome.rs`'s elevation ladder and the ink contracts its census holds), and gooey's `tool()` is a
/// different set of them. Parchment and High contrast are the library's presets whole: they are the
/// accessibility answer, and a host that re-derived one would be a second, unchecked palette.
///
/// **The sizes stay the editor's.** [`GooeySkin::with_colours_of`](bevy_gooey::GooeySkin::with_colours_of)
/// takes a preset's colours and keeps this skin's metrics, so changing the theme never moves a layout:
/// a person who set 200% text and then asks for high contrast has both. The one exception is high
/// contrast's **opaque** surfaces — its look is what makes a faint ground into a solid one — which
/// `with_colours_of` carries with the rest of the colours.
///
/// **This is the skin a theme *means*, not necessarily the one the editor runs.** Which of them can
/// be honours is [`honoured_from`]'s question, and it has to be asked: `chrome.rs` still draws its
/// labels from its own constants (the last half of #210, which needs the protected colour tests
/// re-pinned), so a light theme's ground would leave every `chrome::TEXT` label at about 1.1:1.
pub fn skin_for(theme: crate::settings::Theme) -> GooeySkin {
    use crate::settings::Theme;
    match theme {
        Theme::ToolDark => editor_skin(),
        Theme::Parchment => editor_skin().with_colours_of(&GooeySkin::parchment()),
        Theme::HighContrast => editor_skin().with_colours_of(&GooeySkin::high_contrast()),
    }
}

/// **The inks the editor's own labels are drawn in, named** — the side a theme's ground is measured
/// against (UI-73, #251).
///
/// **Why a theme needs measuring at all.** `chrome.rs` declares 28 colour constants and draws its
/// headings, rows, hints, chips and status band with them, not with the skin; #210's acceptance is
/// that they all come from the skin, and until it is met those colours are the editor's ink whatever
/// the running skin says. So Parchment — whose `surface` is cream and whose `text` is dark brown —
/// would paint every panel cream while each `chrome::TEXT` label over it stayed near-white:
/// **1.12:1**, measured on [`skin_for`]`(Theme::Parchment).surface`. Tool dark and High contrast both
/// clear the floor (`muted` on `surface_raised` is the tightest pair in each, 4.61:1 and 4.94:1), so
/// Parchment is the one theme the editor cannot honour yet.
///
/// **Read from [`editor_skin`], not from the constants**, and the two are the same colour by protected
/// assertion: `tests/editor_skin.rs`, `tests/the_skin_carries_the_editors_other_colours.rs` and
/// `tests/the_skin_carries_the_editors_state_colours.rs` pin every field and tone of the editor's skin
/// to the constant of the same meaning. Going through the skin here is therefore not an
/// approximation — it is the same nine colours under the name a skin can carry, so this function
/// cannot drift from the ink the chrome paints while those assertions hold.
///
/// A tone the skin does not carry is left out rather than guessed: `the_skin_carries_the_editors_other_colours.rs`
/// asserts all five, and a check that invented a colour for a missing one would be measuring a
/// palette nothing draws.
pub fn editor_inks() -> Vec<(&'static str, Color)> {
    let skin = editor_skin();
    let mut inks = vec![
        ("text", skin.text),
        ("text_muted", skin.text_muted),
        ("accent", skin.accent),
        ("danger", skin.danger),
    ];
    // The named tones `chrome.rs` draws labels with, in the order `editor_skin.ron` lists them.
    for tone in ["key", "label", "labeled", "muted", "suggest"] {
        if let Some(colour) = skin.tone(tone) {
            inks.push((tone, colour));
        }
    }
    inks
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
/// Empty is the editor saying it can honour this theme today; anything in it is the reason it cannot.
/// [`honoured_from`] turns the first into the one sentence a person reads.
pub fn unreadable_pairs(theme_skin: &GooeySkin) -> Vec<String> {
    let grounds = grounds_of(theme_skin);
    let mut out = Vec::new();
    for (ink_name, ink) in editor_inks() {
        for (ground_name, ground) in grounds {
            let ratio = bevy_gooey::contrast_ratio(channels(ink), channels(ground));
            if ratio < LEAST_TEXT_CONTRAST {
                out.push(format!("{ink_name} on {ground_name} {ratio:.2}:1"));
            }
        }
    }
    out
}

/// **The running skin this theme can be honoured as, and why not when it cannot** (UI-73, #251).
///
/// The editor has one set of inks it can paint — [`editor_inks`], which are `chrome.rs`'s constants
/// until #210 finishes moving them onto the skin. So a theme is applied only when the ground it would
/// paint still leaves those inks readable, and one that would not is refused here naming the pair that
/// failed — **one path, and a real validation rather than a fallback**: the alternative, painting
/// Parchment's cream under the editor's near-white labels at 1.12:1 and calling it a theme, is a
/// screen nobody can read.
///
/// `Ok(skin)` is what to write into the running [`GooeySkin`]; `Err(why)` is the sentence the caller
/// says. The caller is [`follow_the_theme`], which puts it on the status line and in one log warning.
///
/// **The sentence names no issue and no code word**, because a person reads it: it says which theme,
/// which pair, and that this is temporary.
pub fn honoured_from(theme: crate::settings::Theme) -> Result<GooeySkin, String> {
    let skin = skin_for(theme);
    let unreadable = unreadable_pairs(&skin);
    let Some(first) = unreadable.first() else {
        return Ok(skin);
    };
    Err(format!(
        "{} cannot be drawn yet: its ground leaves the editor's own labels at {first}, and 4.5:1 is \
         the least that can be read. The colours keep the theme they had. This is temporary — the \
         editor's labels have to take their colour from the theme before a light one can be shown.",
        theme_name(theme),
    ))
}

/// What a person calls a theme, for the sentence [`honoured_from`] refuses one with.
///
/// The names are [`crate::setting_catalogue::THEMES`]', so a refusal and the Settings row say the same
/// word — a person told "Parchment cannot be drawn" can find Parchment in the list.
pub fn theme_name(theme: crate::settings::Theme) -> &'static str {
    use crate::settings::Theme;
    match theme {
        Theme::ToolDark => crate::setting_catalogue::THEMES[0],
        Theme::Parchment => crate::setting_catalogue::THEMES[1],
        Theme::HighContrast => crate::setting_catalogue::THEMES[2],
    }
}

/// **The theme and the text size reach the running skin** (UI-73, #251): the settings resolve into
/// [`crate::settings::EditorSettings`], and this is how a choice in the settings screen — or a file a
/// person edited — becomes the skin a widget draws from.
///
/// **A settled editor costs one comparison.** The theme, the size and the refusal last decided are
/// kept in `Local`s, and a pass whose three inputs are the ones already answered returns before
/// [`skin_for`] is called: parsing `editor_skin.ron` and measuring nine inks against two grounds sixty
/// times a second, to write what is already there, is work with no answer at the end of it.
///
/// **The colours and the scale are written separately, and that is not tidiness.** [`skin_for`] builds
/// the theme's colours at the editor's own sizes, so a person's 200% text would be reset by a theme
/// change if the two were assigned together — and a project that ships `family/bevy_gooey.ron` gets the
/// scale *without* the colours, which is the half a bare `return` used to drop.
///
/// **The first pass under Tool dark writes no colours.** [`EditorUiPlugin`] has already put the
/// editor's own skin in as the running one, and removing a project's skin file clears the running skin
/// to gooey's own default (`ui_skin::apply_ui_skin`), which `tests/family_gooey_skin.rs` pins. A
/// *return* to Tool dark after another theme is still an application.
///
/// **A theme the editor cannot honour is refused, once, and said.** See [`honoured_from`]: Parchment's
/// ground would leave the editor's own inks at 1.12:1 while `chrome.rs`'s constants are still what
/// paints them, so choosing it keeps the colours that were running and puts the reason on the status
/// line, with one log warning for the person who edited a settings file by hand. The warning is raised
/// for a *changed* refusal only, so an editor sitting on a refused theme is silent.
///
/// It runs after the settings are read and before gooey styles, so a theme chosen this frame is the
/// skin this frame's widgets are painted from.
pub fn follow_the_theme(
    settings: Option<Res<crate::settings::EditorSettings>>,
    family: Option<Res<crate::family_host::FamilySettings>>,
    state: Option<ResMut<crate::editor::EditorState>>,
    mut skin: ResMut<GooeySkin>,
    // The theme this system last decided, so a settled editor returns at once — and so a run that has
    // only ever seen the default leaves the skin exactly as `EditorUiPlugin` put it.
    mut applied: Local<Option<crate::settings::Theme>>,
    // The refusal last said, so a settled editor does not warn every frame and a *new* one is said.
    mut refused: Local<Option<String>>,
) {
    let Some(settings) = settings else { return };
    let theme = settings.theme;
    // **One comparison is the whole frame.** Nothing moved, so nothing is measured, parsed or written.
    if applied.as_ref() == Some(&theme) && skin.text_scale == settings.text_scale {
        return;
    }
    let first_default = applied.is_none() && theme == crate::settings::Theme::default();
    *applied = Some(theme);
    // **A project's skin is the running skin, and the text size is still the person's.** A project that
    // ships `family/bevy_gooey.ron` has said what its chrome looks like, so the theme setting does not
    // overwrite its colours — but the scale is the person's, and it is written here rather than skipped
    // with the rest, which is what the early `return` used to do.
    if family.as_deref().is_some_and(|family| family.text.contains_key(crate::ui_skin::CRATE)) {
        if skin.text_scale != settings.text_scale {
            skin.text_scale = settings.text_scale;
        }
        return;
    }
    let mut want = if first_default {
        // The colours `EditorUiPlugin` inserted stay; only the person's scale is this system's to write.
        skin.clone()
    } else {
        match honoured_from(theme) {
            Ok(skin) => {
                *refused = None;
                skin
            }
            Err(why) => {
                if refused.as_deref() != Some(why.as_str()) {
                    // The status line is where this editor says things, so a person who chose the theme
                    // in the Settings screen reads why there; the log warning is for a settings file
                    // edited by hand, where no screen was asked and no screen will answer.
                    if let Some(mut state) = state {
                        state.status.problem(why.clone());
                    }
                    bevy::log::warn!("{why}");
                }
                *refused = Some(why);
                // The colours that were running stay: a refused theme is not a theme, it is a theme
                // this editor cannot paint. Only the person's scale moves.
                skin.clone()
            }
        }
    };
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
        app.add_systems(
            Update,
            crate::ui_skin::apply_ui_skin
                .after(crate::family_host::Reloaded)
                .before(bevy_gooey::GooeySystems::Style),
        );
        // **The player's Reduced motion reaches gooey** (UI-R01): travel lands, colours and fades still ease.
        app.add_systems(
            Update,
            follow_the_motion_setting.after(crate::settings::sync_settings).before(bevy_gooey::GooeySystems::Style),
        );
        // **The theme and the text size reach the skin** (UI-73, #251), after the settings are read and
        // after the project's skin file, so a project's `family/bevy_gooey.ron` still wins.
        app.add_systems(
            Update,
            follow_the_theme
                .after(crate::settings::sync_settings)
                .after(crate::ui_skin::apply_ui_skin)
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
}

/// **The shell's arrangement**, as a dock layout a divider can move (UI-30, #224).
///
/// A left dock, the viewport in the middle with the most room, and a right dock — the shape the
/// hand-built frame has had since 2026-08-17, now data that gooey keeps and a person edits.
/// **A person moved a divider or a tab: their arrangement is saved** (UI-33, #227).
///
/// `bevy_gooey` reports every change a person makes to a `Dock` with `DockLayoutChanged`, carrying the
/// layout it now shows. Writing it to the user's file is what makes the arrangement survive a restart;
/// the editor reads it back through `SettingsLayer::shell_layout` on the next start.
///
/// **Nothing is written to a test or to a run with no file**: no `UserSettingsFile` means no place the
/// person asked the editor to keep anything, so the event is taken and dropped, and a headless test
/// that drags a divider does not write the real config.
pub fn save_the_layout(
    changed: On<bevy_gooey::DockLayoutChanged>,
    file: Option<Res<crate::settings::UserSettingsFile>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    let Some(file) = file else { return };
    // **`edit_layer`, not `write_key`.** `write_key` speaks the Settings screen's value vocabulary
    // (a switch, a number, a named choice), and a layout is none of those; `edit_layer` is the same
    // primitive underneath it — the file is read as the editor reads it, refused if it does not
    // read, checked to read back, and written atomically — with the layer itself in hand.
    let layout = changed.layout.clone();
    if let Err(why) = crate::setting_catalogue::edit_layer(&file.0, move |layer| {
        layer.shell_layout = Some(layout);
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
    mut docks: Query<&mut bevy_gooey::Dock>,
) {
    let Some(settings) = settings else { return };
    let want = settings.shell_layout.clone().unwrap_or_else(shell_layout);
    for mut dock in &mut docks {
        if dock.0 != want {
            dock.0 = want.clone();
        }
    }
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

pub fn shell_layout() -> bevy_gooey::DockLayout {
    use bevy_gooey::{DockLayout, SplitAxis};
    DockLayout::split(
        SplitAxis::Horizontal,
        0.22,
        DockLayout::group(["inspector"]),
        DockLayout::split(
            SplitAxis::Horizontal,
            0.78,
            DockLayout::group(["viewport"]),
            DockLayout::group(["place"]),
        ),
    )
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
