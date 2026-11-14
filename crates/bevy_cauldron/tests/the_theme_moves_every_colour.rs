//! **A theme changes every colour the editor draws, live** (#428).
//!
//! `feedback-1794240972472` (the owner, Cmd+D, 2026-11-09): *"Changing the theme doesn't seem to change the
//! UI at all."* `chrome.rs` drew its headings, rows, hints, chips and status band with `chrome::Ink`, which
//! was read once from `editor_skin.yaml`, so the gooey widgets moved with a theme and nothing else did.
//!
//! # What these assert
//!
//! The editor is built, then the settings file is written with each of the three themes in turn, **live and
//! in every order**, and what is *on screen* is read back from the world: a heading's `TextColor`, a row's
//! and a chip's `BackgroundColor` and `BorderColor`, the status band's fill, a panel's edge, the window's
//! clear colour. Each is compared with the colour the theme's own skin gives that role (`Ink::of` of
//! `ui::skin_for`), **not** with "something changed": a colour that moved to the wrong place is a defect.
//!
//! - `the_samples_are_the_themes_own_colours` — per theme, the five samples match the skin, and they differ
//!   from the other two themes'.
//! - `nothing_is_left_in_another_themes_colours` — after every switch, no colour on screen is one that
//!   belongs to a *different* theme's palette and not the running one's. This is the whole of the owner's
//!   report as a rule: a heading left behind in dark text on cream would be in it.
//! - `every_theme_reads_on_screen` — WCAG contrast is computed from the rendered text and ground colours
//!   (4.5:1 for every theme, 7:1 for High contrast's body text), and `unreadable_pairs` is empty for all
//!   three, Parchment included.
//! - `a_theme_applies_in_the_frame_the_setting_lands` — no restart and no second frame.
//! - `a_theme_chosen_in_the_project_layer_reaches_the_ink` and `text_size_changes_no_colour`.
//! - `a_pictures_tint_and_a_clear_node_are_not_the_themes` — what is *not* the theme's stays put: an image's
//!   default white tint (which is also High contrast's `text`), a model's default white material and a
//!   transparent node, through High contrast -> Parchment -> Tool dark -> High contrast.
//! - `every_theme_carries_the_tones_the_editor_draws_with` — `Ink::of` answers magenta for a missing tone,
//!   so every theme file must name exactly the tones the editor's own skin does.
//!
//! **Through the file, not the resource**: `sync_settings` re-resolves `EditorSettings` from the files, so a
//! test that assigned the resource would be overwritten.
//!
//! **Motion is off** (`MotionSettings::duration_scale` 0): gooey eases a colour change over a third of a
//! second, which is right for a person and would make a frame-exact read a read of a blend.

use bevy::prelude::*;
use bevy_cauldron::chrome::{self, BandGround, Ink, PanelEdge, PanelGround, RowRest, TypeRole, text};
use bevy_cauldron::settings::{Theme, UserSettingsFile};
use bevy_gooey::GooeySkin;

use crate::fixtures;

/// Every theme the editor offers, in the order a person meets them.
const THEMES: [Theme; 3] = [Theme::ToolDark, Theme::Parchment, Theme::HighContrast];

/// The WCAG 2.2 floor for text (success criterion 1.4.3).
const TEXT_FLOOR: f32 = 4.5;
/// The enhanced floor (success criterion 1.4.6) High contrast's body text is held to. **Assumed decision**:
/// the issue names no number; 7:1 is what gooey's own `high_contrast_problems` and Xbox Accessibility
/// Guideline 102 ask of a high-contrast mode.
const HIGH_CONTRAST_TEXT_FLOOR: f32 = 7.0;
/// Seconds the file watcher's wall-clock poll is given to see a settings file change.
const WATCH_PATIENCE_S: u64 = 10;
/// Frames run after a switch so painters that repaint on a later frame (hover, selection, the focused
/// panel's edge) have all run on the new colours.
const SETTLE_FRAMES: usize = 6;
/// Least number of each sampled thing the editor must show, so an assertion over "each" is not over none.
const LEAST_SAMPLES: usize = 3;
/// Least number of words the contrast census must measure (the Map door shows about a hundred).
const LEAST_WORDS: usize = 60;
/// How far two sRGB channels may differ and still be the same colour: under one 8-bit step (1/255).
const SAME: f32 = 1.0e-3;

fn named(theme: Theme) -> &'static str {
    match theme {
        Theme::ToolDark => "ToolDark",
        Theme::Parchment => "Parchment",
        Theme::HighContrast => "HighContrast",
    }
}

/// The editor on `theme`, laid out, its settings file returned so a test can change it afterwards.
fn editor(tag: &str, theme: Theme) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let file = root.join(format!("user_config_{tag}_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(file.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(&file, format!("{{version: 1, theme: {}}}", named(theme))).expect("a settings file");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(file.clone()));
    app.insert_resource(bevy_gooey::MotionSettings { duration_scale: 0.0, reduced: false });
    for _ in 0..12 {
        app.update();
    }
    (app, file)
}

/// **A person chooses `theme`**: the settings file is written and the editor runs until it has taken it,
/// then a few frames more so every painter has run.
fn choose(app: &mut App, file: &std::path::Path, theme: Theme) {
    std::fs::write(file, format!("{{version: 1, theme: {}}}", named(theme))).expect("a settings file");
    let want = bevy_cauldron::ui::skin_for(theme).surface;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(WATCH_PATIENCE_S);
    while app.world().resource::<GooeySkin>().surface != want && std::time::Instant::now() < deadline {
        app.update();
    }
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

fn same(a: Color, b: Color) -> bool {
    let (a, b) = (Srgba::from(a), Srgba::from(b));
    (a.red - b.red).abs() <= SAME && (a.green - b.green).abs() <= SAME && (a.blue - b.blue).abs() <= SAME
}

/// The theme's ink, as its own skin says it — **not** `chrome::ink()`, which is what is being tested.
fn expected(theme: Theme) -> Ink {
    Ink::of(&bevy_cauldron::ui::skin_for(theme))
}

/// Every colour a theme's skin and ink own: the 28 ink fields and the skin's roles, ladder and tones.
fn palette(theme: Theme) -> Vec<Color> {
    let skin = bevy_cauldron::ui::skin_for(theme);
    let mut colours: Vec<Color> = expected(theme).fields().to_vec();
    colours.extend([
        skin.background,
        skin.surface,
        skin.surface_raised,
        skin.text,
        skin.text_muted,
        skin.accent,
        skin.accent_text,
        skin.danger,
        skin.focus_ring,
        skin.outline,
        skin.scrim,
        skin.hovered(),
        skin.pressed(),
        skin.card(),
        skin.preview_edge(),
    ]);
    colours.extend(skin.surfaces.iter().copied());
    colours.extend(skin.tones.values().copied());
    colours
}

/// The first opaque `BackgroundColor` at or above `entity`, composited over the window's clear colour:
/// the ground a word on `entity` is read against.
fn ground_under(world: &World, entity: Entity) -> Option<Color> {
    let mut at = entity;
    loop {
        if let Some(fill) = world.get::<BackgroundColor>(at)
            && Srgba::from(fill.0).alpha >= 0.9
        {
            let fill = Srgba::from(fill.0);
            let clear = Srgba::from(world.resource::<ClearColor>().0);
            return Some(Color::srgb(
                fill.red * fill.alpha + clear.red * (1.0 - fill.alpha),
                fill.green * fill.alpha + clear.green * (1.0 - fill.alpha),
                fill.blue * fill.alpha + clear.blue * (1.0 - fill.alpha),
            ));
        }
        at = world.get::<ChildOf>(at)?.parent();
    }
}

fn contrast(ink: Color, ground: Color) -> f32 {
    let c = |colour: Color| {
        let c = Srgba::from(colour);
        [c.red, c.green, c.blue]
    };
    bevy_gooey::contrast_ratio(c(ink), c(ground))
}

/// What the editor shows, read from the world.
#[derive(Default, Debug)]
struct Shown {
    /// A section heading: its words' colour, and the ground it is read on.
    headings: Vec<(Color, Option<Color>)>,
    /// A list row or chip at rest or under whatever the pointer is doing: its fill and its edge.
    rows: Vec<(Color, Color)>,
    /// What each of `rows` is, as the components on it, for a failure to say which one it was.
    row_names: Vec<String>,
    /// The chip shape: a `Button` carrying a `RowRest` — a row's fill and edge as well.
    chips: Vec<(Color, Color)>,
    /// A status or chrome band's fill.
    bands: Vec<Color>,
    /// A panel's ground.
    panels: Vec<Color>,
    /// A panel's edge.
    edges: Vec<Color>,
    /// Every word in the editor with the ground it is read on.
    words: Vec<(Color, Option<Color>)>,
}

fn shown(app: &mut App) -> Shown {
    let world = app.world_mut();
    let mut out = Shown::default();
    let mut headings = world.query::<(Entity, &TextColor, &TypeRole)>();
    let found: Vec<(Entity, Color, text::Role)> = headings.iter(world).map(|(e, c, r)| (e, c.0, r.role())).collect();
    for (entity, colour, role) in found {
        let ground = ground_under(world, entity);
        out.words.push((colour, ground));
        if role == text::HEADING {
            out.headings.push((colour, ground));
        }
    }
    let mut rows = world.query::<(Entity, &RowRest, &BackgroundColor, &BorderColor, Has<bevy::ui_widgets::Button>, Has<Children>)>();
    let found: Vec<(Entity, Color, Color, bool, bool)> =
        rows.iter(world).map(|(e, _, fill, border, button, children)| (e, fill.0, border.top, button, children)).collect();
    for (entity, fill, edge, button, children) in found {
        let names: Vec<String> = world.inspect_entity(entity).map(|c| c.map(|(_, info)| info.name().shortname().to_string()).collect()).unwrap_or_default();
        out.row_names.push(format!("{entity:?} [{}]", names.join(", ")));
        out.rows.push((fill, edge));
        if button && children {
            out.chips.push((fill, edge));
        }
    }
    let mut bands = world.query_filtered::<&BackgroundColor, With<BandGround>>();
    out.bands = bands.iter(world).map(|b| b.0).collect();
    let mut panels = world.query_filtered::<&BackgroundColor, With<PanelGround>>();
    out.panels = panels.iter(world).map(|b| b.0).collect();
    let mut edges = world.query_filtered::<&BorderColor, With<PanelEdge>>();
    out.edges = edges.iter(world).map(|b| b.top).collect();
    out
}

/// Every ink field of the *other* themes that the theme's own ink does not also hold.
fn other_inks(theme: Theme) -> Vec<Color> {
    let own = expected(theme).fields();
    THEMES.iter().filter(|t| **t != theme).flat_map(|t| expected(*t).fields()).filter(|c| !one_of(*c, &own)).collect()
}

/// **Which node this is**, for a failure to say: its words, its name, the components that are not layout
/// or visibility plumbing, and the same for its parents, nearest first.
fn describe(world: &World, entity: Entity) -> String {
    const PLUMBING: [&str; 14] = [
        "ChildOf", "Children", "AccessibilityNode", "Visibility", "InheritedVisibility", "ViewVisibility", "ComputedNode", "ComputedStackIndex",
        "UiGlobalTransform", "ContentSize", "UiTransform", "ComputedUiTargetCamera", "ComputedUiRenderTargetInfo", "FocusPolicy",
    ];
    let mut at = Some(entity);
    let mut chain: Vec<String> = Vec::new();
    while let Some(e) = at.filter(|_| chain.len() < 5) {
        let words = world.get::<Text>(e).map(|t| format!(" {:?}", t.0)).unwrap_or_default();
        let names: Vec<String> = world
            .inspect_entity(e)
            .map(|c| c.map(|(_, info)| info.name().shortname().to_string()).filter(|n| !n.starts_with("Inherited<") && !PLUMBING.contains(&n.as_str())).collect())
            .unwrap_or_default();
        chain.push(format!("{e:?}{words} [{}]", names.join(", ")));
        at = world.get::<ChildOf>(e).map(|c| c.parent());
    }
    chain.join("\n        < ")
}

fn one_of(colour: Color, set: &[Color]) -> bool {
    set.iter().any(|c| same(colour, *c))
}

/// **Each of the three themes, in every order a person could pick them, draws a heading, a row, a chip,
/// the status band, a panel and its edge in that theme's own colours.**
#[test]
fn the_samples_are_the_themes_own_colours() {
    let (mut app, file) = editor("theme_samples", Theme::ToolDark);
    // Every order: dark → parchment → high contrast → dark → high contrast → parchment, so a theme is
    // reached from each of the others and a "return to the default" is covered.
    let route = [Theme::ToolDark, Theme::Parchment, Theme::HighContrast, Theme::ToolDark, Theme::HighContrast, Theme::Parchment];
    let mut by_theme: Vec<(Theme, Shown)> = Vec::new();
    for theme in route {
        if theme != Theme::ToolDark || !by_theme.is_empty() {
            choose(&mut app, &file, theme);
        }
        let ink = expected(theme);
        let on = shown(&mut app);
        assert!(on.headings.len() >= LEAST_SAMPLES, "{theme:?}: the editor draws headings ({})", on.headings.len());
        assert!(on.rows.len() >= LEAST_SAMPLES, "{theme:?}: and rows ({})", on.rows.len());
        assert!(!on.bands.is_empty(), "{theme:?}: and a status band");
        assert!(!on.edges.is_empty(), "{theme:?}: and panel edges");
        for (colour, _) in &on.headings {
            assert!(one_of(*colour, &ink.fields()), "{theme:?}: a heading is drawn in one of the theme's inks: {colour:?}");
            assert!(!one_of(*colour, &other_inks(theme)), "{theme:?}: and not in a colour only another theme has: {colour:?}");
        }
        let fills = ink.fields();
        for (at, (fill, edge)) in on.rows.iter().enumerate() {
            assert!(one_of(*fill, &fills), "{theme:?}: a row's fill is one of the theme's inks: {fill:?} on {}", on.row_names[at]);
            let edges = [ink.panel_edge, ink.accent, ink.suggest, ink.danger, ink.edge_strong, ink.muted, ink.key, ink.labeled, Color::NONE];
            assert!(
                one_of(*edge, &edges) || Srgba::from(*edge).alpha == 0.0,
                "{theme:?}: a row's edge is one of the theme's: {edge:?}"
            );
        }
        for fill in &on.bands {
            assert!(same(*fill, ink.bar_bg), "{theme:?}: the status band is the theme's `bar_bg`: {fill:?} vs {:?}", ink.bar_bg);
        }
        let skin = bevy_cauldron::ui::skin_for(theme);
        for fill in &on.panels {
            assert!(same(*fill, skin.surface), "{theme:?}: a panel is the theme's `surface`: {fill:?}");
        }
        for edge in &on.edges {
            assert!(
                same(*edge, ink.panel_edge) || same(*edge, ink.accent),
                "{theme:?}: a panel's edge is the theme's `panel_edge` (or the `accent` of the one the keyboard is in): {edge:?}"
            );
        }
        assert!(
            same(app.world().resource::<ClearColor>().0, ink.void),
            "{theme:?}: the window behind the panels is the theme's `void`"
        );
        by_theme.push((theme, on));
    }
    // **Differ between themes**: the first heading, band and panel edge of each theme are not another's.
    for (a, sa) in &by_theme {
        for (b, sb) in &by_theme {
            if a == b {
                continue;
            }
            assert!(!same(sa.headings[0].0, sb.headings[0].0), "{a:?} and {b:?} draw a heading in different colours");
            assert!(!same(sa.bands[0], sb.bands[0]), "{a:?} and {b:?} draw the status band in different colours");
            assert!(!same(sa.rows[0].0, sb.rows[0].0), "{a:?} and {b:?} draw a row in different colours");
        }
    }
}

/// **No colour on screen belongs to another theme.** After each switch, every `BackgroundColor`,
/// `BorderColor` and `TextColor` in the world that is a colour of *some other* theme's palette is also a
/// colour of the running one (two palettes may share a colour — white, black) — otherwise it was left
/// behind.
#[test]
fn nothing_is_left_in_another_themes_colours() {
    let (mut app, file) = editor("theme_left_behind", Theme::ToolDark);
    for theme in [Theme::Parchment, Theme::HighContrast, Theme::ToolDark, Theme::Parchment] {
        choose(&mut app, &file, theme);
        let own = palette(theme);
        let others: Vec<(Theme, Vec<Color>)> = THEMES.iter().filter(|t| **t != theme).map(|t| (*t, palette(*t))).collect();
        let world = app.world_mut();
        let mut left: Vec<String> = Vec::new();
        let mut checked = 0usize;
        let mut found: Vec<(&str, Entity, Color)> = Vec::new();
        let mut fills = world.query::<(Entity, &BackgroundColor)>();
        found.extend(fills.iter(world).map(|(e, c)| ("fill", e, c.0)));
        let mut borders = world.query::<(Entity, &BorderColor)>();
        found.extend(borders.iter(world).map(|(e, c)| ("edge", e, c.top)));
        let mut texts = world.query::<(Entity, &TextColor)>();
        found.extend(texts.iter(world).map(|(e, c)| ("text", e, c.0)));
        for (what, entity, colour) in found {
            // A node with no fill has `rgba(0, 0, 0, 0)`, which is nobody's colour.
            if Srgba::from(colour).alpha == 0.0 {
                continue;
            }
            checked += 1;
            if one_of(colour, &own) {
                continue;
            }
            for (other, colours) in &others {
                if one_of(colour, colours) {
                    left.push(format!("{entity:?} {what} is {colour:?}, a colour of {other:?}\n        {}", describe(world, entity)));
                }
            }
        }
        assert!(checked > 200, "{theme:?}: the census looked at the editor's colours ({checked})");
        assert!(
            left.is_empty(),
            "{theme:?}: {} colour(s) on screen are another theme's:\n{}",
            left.len(),
            left.iter().take(40).cloned().collect::<Vec<_>>().join("\n")
        );
    }
}

/// **Every theme reads**, measured from the colours on screen: each word the editor draws has at least
/// 4.5:1 against the ground it is drawn on in every theme, High contrast's body text 7:1, and the library's
/// own check of every theme's skin (and the editor's inks against it) finds nothing.
#[test]
fn every_theme_reads_on_screen() {
    for theme in THEMES {
        let skin = bevy_cauldron::ui::skin_for(theme);
        assert_eq!(bevy_cauldron::ui::unreadable_pairs(&skin), Vec::<String>::new(), "{theme:?}: every ink clears every ground it is on");
        assert_eq!(skin.problems(), vec![], "{theme:?}: a well-formed skin");
    }
    assert_eq!(bevy_cauldron::ui::skin_for(Theme::HighContrast).high_contrast_problems(), vec![], "High contrast holds 7:1");

    let (mut app, file) = editor("theme_contrast", Theme::ToolDark);
    for theme in [Theme::Parchment, Theme::HighContrast, Theme::ToolDark] {
        choose(&mut app, &file, theme);
        let ink = expected(theme);
        let on = shown(&mut app);
        // **What this does not measure**: a word's own alpha, and a translucent layer between a word and the
        // ground `ground_under` finds (a scrim, a veil, a fade). Both would only lower a ratio, and the
        // layers are the theme's own `scrim`/`veil` whose RGB is checked against nothing here.
        let mut measured = 0usize;
        let mut failures: Vec<String> = Vec::new();
        for (colour, ground) in &on.words {
            let Some(ground) = ground else { continue };
            measured += 1;
            let ratio = contrast(*colour, *ground);
            // **The body text is `text`**: the theme's most legible ink, held to the enhanced floor in High
            // contrast. Every other ink (a label, a hint) is held to 4.5:1; Tool dark's quietest tones
            // are the only ones the editor's census has ever accepted lower, on a selected row, and none
            // of those is a heading or a status word.
            let floor = if theme == Theme::HighContrast && same(*colour, ink.text) { HIGH_CONTRAST_TEXT_FLOOR } else { TEXT_FLOOR };
            let quiet_on_selected = same(*ground, ink.row_selected) || same(*ground, ink.row_hover);
            if ratio < floor && !quiet_on_selected {
                failures.push(format!("{colour:?} on {ground:?} is {ratio:.2}:1, under {floor}:1"));
            }
        }
        assert!(measured >= LEAST_WORDS, "{theme:?}: the census measured the editor's words ({measured})");
        assert!(
            failures.is_empty(),
            "{theme:?}: {} word(s) are under their floor:\n{}",
            failures.len(),
            failures.iter().take(30).cloned().collect::<Vec<_>>().join("\n")
        );
    }
}

/// **A theme applies in the frame the setting lands**, with no restart and no second pass: the frame
/// `follow_the_theme` writes the skin is the frame the ink and everything drawn in it move.
#[test]
fn a_theme_applies_in_the_frame_the_setting_lands() {
    let (mut app, file) = editor("theme_same_frame", Theme::ToolDark);
    std::fs::write(&file, "{version: 1, theme: Parchment}").expect("a settings file");
    let want = bevy_cauldron::ui::skin_for(Theme::Parchment);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(WATCH_PATIENCE_S);
    // The frame the running skin first becomes Parchment's is the frame its ink must be too.
    while app.world().resource::<GooeySkin>().surface != want.surface && std::time::Instant::now() < deadline {
        app.update();
    }
    assert_eq!(app.world().resource::<GooeySkin>().surface, want.surface, "the settings file was taken");
    assert_eq!(chrome::ink().fields(), expected(Theme::Parchment).fields(), "the ink is the theme's in the same frame");
    let on = shown(&mut app);
    let ink = expected(Theme::Parchment);
    assert!(on.bands.iter().all(|b| same(*b, ink.bar_bg)), "and so is the status band already drawn");
    assert!(on.headings.iter().all(|(c, _)| one_of(*c, &ink.fields())), "and every heading is in the theme's ink");
}

/// **A theme chosen in the project's layer** (`.grimoire/settings`) is the running theme too, and
/// reaches the ink the same way.
#[test]
fn a_theme_chosen_in_the_project_layer_reaches_the_ink() {
    let root = fixtures::Fixture::new(&format!("theme_project_layer_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let user = root.join(format!("user_config_project_layer_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(user.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(&user, "{version: 1}").expect("a settings file");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(user));
    app.insert_resource(bevy_gooey::MotionSettings { duration_scale: 0.0, reduced: false });
    for _ in 0..12 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().project_dir.join(bevy_cauldron::settings::PROJECT_FILE);
    std::fs::write(&project, "{version: 1, theme: HighContrast}").expect("a project settings file");
    let want = bevy_cauldron::ui::skin_for(Theme::HighContrast).surface;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(WATCH_PATIENCE_S);
    while app.world().resource::<GooeySkin>().surface != want && std::time::Instant::now() < deadline {
        app.update();
    }
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    assert_eq!(chrome::ink().fields(), expected(Theme::HighContrast).fields(), "a project's theme is the editor's ink");
    let on = shown(&mut app);
    let ink = expected(Theme::HighContrast);
    assert!(on.bands.iter().all(|b| same(*b, ink.bar_bg)), "and the status band is drawn in it");
}

/// **Text size moves no colour**: a change of `text_scale` rewrites the running skin but not the ink, so
/// nothing already drawn is touched.
#[test]
fn text_size_changes_no_colour() {
    let (mut app, file) = editor("theme_text_size", Theme::Parchment);
    let before = (chrome::ink().fields(), shown(&mut app).headings.iter().map(|(c, _)| *c).collect::<Vec<_>>());
    std::fs::write(&file, "{version: 1, theme: Parchment, text_scale: 1.5}").expect("a settings file");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(WATCH_PATIENCE_S);
    while (app.world().resource::<GooeySkin>().text_scale - 1.5).abs() > f32::EPSILON && std::time::Instant::now() < deadline {
        app.update();
    }
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    let after = (chrome::ink().fields(), shown(&mut app).headings.iter().map(|(c, _)| *c).collect::<Vec<_>>());
    assert_eq!(before, after, "150% text leaves the ink and every heading's colour as they were");
}

/// **What is not the theme's stays put.** `ImageNode::default()`'s tint and a default `StandardMaterial` are
/// white, which is also High contrast's `text` (`LinearRgba(1, 1, 1)`); `Color::NONE` has the RGB of every black
/// field. A by-value repaint that asked about them would turn a picture or a model dark on leaving High contrast
/// (the owner's saved theme) and leave it so.
#[test]
fn a_pictures_tint_and_a_clear_node_are_not_the_themes() {
    let (mut app, file) = editor("theme_not_the_themes", Theme::HighContrast);
    let image = app.world_mut().spawn(ImageNode::default()).id();
    let tinted = app.world_mut().spawn(ImageNode::default().with_color(Color::WHITE)).id();
    let clear = app.world_mut().spawn((Node::default(), BackgroundColor(Color::NONE), BorderColor::all(Color::NONE))).id();
    let material = app.world_mut().resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial::default());
    for theme in [Theme::Parchment, Theme::ToolDark, Theme::HighContrast, Theme::Parchment] {
        choose(&mut app, &file, theme);
        for entity in [image, tinted] {
            let tint = app.world().get::<ImageNode>(entity).expect("the image is still there").color;
            assert!(same(tint, Color::WHITE), "{theme:?}: a picture's white tint is still white: {tint:?}");
        }
        let fill = app.world().get::<BackgroundColor>(clear).expect("the node is still there").0;
        let edge = app.world().get::<BorderColor>(clear).expect("the node is still there").top;
        assert_eq!(Srgba::from(fill).alpha, 0.0, "{theme:?}: a node with no fill is still clear: {fill:?}");
        assert_eq!(Srgba::from(edge).alpha, 0.0, "{theme:?}: and so is its edge: {edge:?}");
        let base = app.world().resource::<Assets<StandardMaterial>>().get(&material).expect("the material is still there").base_color;
        assert!(same(base, Color::WHITE), "{theme:?}: a default material is still white: {base:?}");
    }
}

/// **Every theme file names the tones the editor's own skin does** — `Ink::of` answers a missing tone in
/// magenta, so a theme file that dropped one would paint fuchsia only on the screen that used it.
#[test]
fn every_theme_carries_the_tones_the_editor_draws_with() {
    let own: Vec<String> = bevy_cauldron::ui::editor_skin().tones.keys().cloned().collect();
    assert!(own.len() >= 12, "the editor's own skin names its tones: {own:?}");
    for theme in THEMES {
        let skin = bevy_cauldron::ui::skin_for(theme);
        let named: Vec<String> = skin.tones.keys().cloned().collect();
        assert_eq!(named, own, "{theme:?}: the theme file names exactly the tones the editor's own skin does");
        let magenta = Color::srgb(1.0, 0.0, 1.0);
        assert!(
            !one_of(magenta, &Ink::of(&skin).fields()),
            "{theme:?}: no ink is the magenta that answers a missing tone"
        );
    }
}
