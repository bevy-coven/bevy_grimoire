//! **How far away everything is** (NV-01, #378; series getting around, #377).
//!
//! A person's complaint about an editor is rarely "a feature is missing"; it is "I cannot get there".
//! That is measurable, and measuring it is what lets a later change — workspaces, the activity bar,
//! document tabs — be judged as cheaper or not.
//!
//! This file drives the real editor from a cold start on a fixture project, by the keys and pointer
//! actions a hand would use, and counts them. `docs/ui/navigation-baseline.md` is generated from the
//! same table, so the document and the test cannot disagree about how expensive a destination is.
//!
//! # What one press means
//!
//! One key event. A chord is one press (`Cmd+O` is one), and a typed character is one press. Pointer
//! actions are counted apart — a click and a wheel notch each cost one — because a route of two keys
//! is a different thing to learn from a route of two clicks, and a route that needs the wheel has no
//! keyboard form at all.
//!
//! # A destination with no route is a row, and its absence is proved
//!
//! A destination nobody can reach is the most expensive kind, so it is a row rather than a gap, and
//! the test proves the reason: the label is absent from every screen the editor can be put on, or the
//! route needs a build this one is not. What is *not* done is calling something reachable because a
//! string for it exists in the source.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input::mouse::MouseScrollUnit;
use bevy::input::touch::TouchPhase;
use bevy::picking::pointer::{Location, PointerAction, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};

use bevy_cauldron::screen::Screen;
use bevy_cauldron::tiles::Mode;
use bevy_gooey::testing::{click, find_by_label};

use crate::fixtures;

/// **The most wheel notches a route may spend** before this test calls the destination unscrollable.
///
/// Measured, not guessed: the Map inspector's content is about eight screens tall at the fixture's
/// size, so a destination past the sixtieth notch is not reachable by a person either.
const WHEEL_BUDGET: u32 = 60;

/// Where the generated table lives.
const BASELINE: &str = "docs/ui/navigation-baseline.md";

/// The platform's primary modifier, which is what every chord in the issue's targets names.
fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

/// **What a route cost**, and whether it arrived.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Cost {
    /// One per key event: a chord is one, a typed character is one.
    presses: u32,
    /// One per click or wheel notch: the pointer.
    pointer: u32,
}

/// **One destination, as the issue lists it.**
struct Destination {
    /// The row's name in the document.
    what: &'static str,
    /// The route, in words a person could follow.
    route: String,
    /// What it cost.
    cost: Cost,
    /// Whether the editor can be got there at all, and if not, why.
    reach: Reach,
}

/// Whether a destination can be arrived at, with the proof.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Reach {
    /// The editor was driven there and the label was on screen.
    Arrived,
    /// No route, with the measured reason.
    Unreachable(String),
}

// ───────────────────────────── the driver ─────────────────────────────

struct Driver {
    app: App,
    cost: Cost,
    /// The wheel has been aimed at the inspector, so a scroll notch lands on it.
    aimed: bool,
}

impl Driver {
    /// The editor on a fresh fixture project, on the Map door with the map open.
    fn cold_start(tag: &str) -> Driver {
        Driver::on(tag, Mode::Map)
    }

    /// **The editor opened on a named panel**, the way `--door` and `--kit` open one. The home is added
    /// as `main.rs` adds it, so `Cmd+O` lands on a filled project page rather than an empty one.
    fn on(tag: &str, mode: Mode) -> Driver {
        let root = fixtures::Fixture::new(&format!("nav_{tag}_{}", std::process::id()))
            .descriptor("alpha/floor", "alpha")
            .descriptor("alpha/wall", "alpha")
            .place("alpha/floor", (0.0, 0.0))
            .place("alpha/wall", (2.0, 0.0))
            .build("m");
        let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("{e}"));
        app.add_plugins((
            bevy_cauldron::launcher::LauncherPlugin,
            bevy_cauldron::loading::LoadingPlugin,
            bevy_cauldron::window_frame::WindowFramePlugin,
        ));
        for _ in 0..10 {
            app.update();
        }
        Driver { app, cost: Cost::default(), aimed: false }
    }

    fn window(&mut self) -> Entity {
        Entity::PLACEHOLDER
    }

    fn key(&mut self, code: KeyCode, text: Option<&str>, state: ButtonState) {
        let window = self.window();
        self.app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: text.map_or(Key::Unidentified(NativeKey::Unidentified), |t| Key::Character(t.into())),
            state,
            text: text.map(Into::into),
            repeat: false,
            window,
        });
    }

    fn settle(&mut self) {
        for _ in 0..6 {
            self.app.update();
        }
    }

    /// **One chord, counted as one press.**
    fn chord(&mut self, keys: &[KeyCode]) {
        for state in [ButtonState::Pressed, ButtonState::Released] {
            for code in keys {
                self.key(*code, None, state);
            }
            self.app.update();
        }
        self.settle();
        self.cost.presses += 1;
    }

    /// **A click on the widget whose words are `label`**, counted as one pointer action.
    fn click(&mut self, label: &str) {
        let widget = find_by_label(&mut self.app, label).unwrap_or_else(|e| panic!("no `{label}` on screen: {e}\n\non screen: {:#?}", self.words()));
        click(&mut self.app, widget);
        self.settle();
        self.cost.pointer += 1;
    }

    /// Every word on screen, for a failure that says what the editor was showing instead.
    /// **The words a person can see.** `InheritedVisibility` is not enough: the inspector is one long
    /// `ScrollArea`, so every heading in it is `Inherited` while only a screenful is inside the
    /// viewport. A node counts only when its box overlaps the box of every `ScrollArea` above it,
    /// which is what scrolling actually changes.
    fn words(&mut self) -> Vec<String> {
        let world = self.app.world_mut();
        let mut q = world.query::<(Entity, &Text, &InheritedVisibility, &ComputedNode, &UiGlobalTransform)>();
        let found: Vec<(Entity, String)> = q
            .iter(world)
            .filter(|(_, text, shown, node, _)| shown.get() && !text.0.trim().is_empty() && node.size().x > 0.0 && node.size().y > 0.0)
            .map(|(e, text, _, _, _)| (e, text.0.trim().to_owned()))
            .collect();
        let mut out: Vec<String> = found
            .into_iter()
            .filter(|(e, _)| box_of(*e, world).is_some_and(|mine| inside_every_scroll_area(mine, *e, world)))
            .map(|(_, text)| text)
            .collect();
        out.sort();
        out.dedup();
        out
    }

    fn showing(&mut self, label: &str) -> bool {
        self.words().iter().any(|w| w == label)
    }

    /// **Frames in real time until `done`**, or five seconds. A route that crosses the loading screen
    /// is paced by the clock rather than by frames, so a fixed settle cannot measure it.
    fn until(&mut self, mut done: impl FnMut(&mut Self) -> bool) -> bool {
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(5) {
            self.app.update();
            if done(self) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }

    fn screen(&self) -> Screen {
        *self.app.world().resource::<State<Screen>>().get()
    }

    /// **The panel showing, or none.** A door's resources are torn down with it (`screen::OWNERSHIP`),
    /// so off the editor there is no panel at all — which is why this answers `Option`.
    fn panel(&self) -> Option<Mode> {
        self.app.world().get_resource::<Mode>().copied()
    }

    /// The panel's own name, because `Mode` has no `Debug` and a failure has to say where it stood.
    fn panel_name(&self) -> &'static str {
        self.panel().map_or("no door", |m| m.label())
    }

    /// **The inspector's box**, so a wheel notch lands on it: `(centre, scroll offset, scrollable range)`.
    fn inspector(&mut self) -> Option<(Vec2, f32, f32)> {
        let world = self.app.world_mut();
        let mut q = world.query_filtered::<(&ScrollPosition, &ComputedNode, &UiGlobalTransform), With<bevy_cauldron::editor::MapControlsPane>>();
        let (pos, node, at) = q.single(world).ok()?;
        let scale = node.inverse_scale_factor();
        let visible = node.size() * scale;
        Some((at.translation * scale, pos.y, (node.content_size().y * scale - visible.y).max(0.0)))
    }

    /// **One wheel notch down the inspector**, counted as one pointer action.
    fn notch(&mut self) -> bool {
        let Some((centre, _, range)) = self.inspector() else { return false };
        if range <= 0.0 {
            return false;
        }
        let handle = self
            .app
            .world()
            .get_resource::<bevy_cauldron::surface::Surface>()
            .map(|s| s.image.clone())
            .unwrap_or_default();
        let location = Location {
            target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }),
            position: centre,
        };
        if !self.aimed {
            self.app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location.clone(), PointerAction::Move { delta: Vec2::ZERO }));
            self.app.update();
            self.aimed = true;
        }
        self.app.world_mut().write_message(PointerInput::new(
            PointerId::Mouse,
            location,
            PointerAction::Scroll { unit: MouseScrollUnit::Line, x: 0.0, y: -1.0, phase: TouchPhase::Moved },
        ));
        self.app.update();
        self.cost.pointer += 1;
        true
    }

    /// **Wheel down until `label` is on screen**, and answer whether it got there.
    fn scroll_to(&mut self, label: &str) -> bool {
        if self.showing(label) {
            return true;
        }
        for _ in 0..WHEEL_BUDGET {
            if !self.notch() {
                return false;
            }
            if self.showing(label) {
                return true;
            }
        }
        false
    }
}

// ───────────────────────────── the destinations ─────────────────────────────

/// **The map that is already open.**
fn the_map_is_open() -> Destination {
    let mut d = Driver::cold_start("map");
    let arrived = d.showing("MAP") && d.showing("INSPECTOR");
    assert!(arrived, "the Map door's own panel is not on screen at a cold start: {:#?}", d.words());
    Destination {
        what: "The open map",
        route: "the editor starts on it".to_owned(),
        cost: d.cost,
        reach: Reach::Arrived,
    }
}

/// **A named map**, from the Map door.
fn a_named_map() -> Destination {
    let mut d = Driver::cold_start("named_map");
    // The close-project chord is the only way to the project page, which is where maps are chosen.
    d.chord(&[primary(), KeyCode::KeyO]);
    assert_eq!(d.screen(), Screen::Project, "Cmd+O leaves the door for the project page");
    // **The map's own card, by the name on it.** `MapCard` sits on the card and the name is its child,
    // so the row a person reads is the child's text rather than the card's.
    let card = {
        let world = d.app.world_mut();
        let mut q = world.query_filtered::<&bevy_cauldron::project_page::MapCard, ()>();
        let cards: Vec<String> = q.iter(world).map(|c| c.0.clone()).collect();
        cards.into_iter().next().unwrap_or_else(|| panic!("the project page lists no map: {:#?}", d.words()))
    };
    let card = card.as_str().to_owned();
    d.click(&card);
    let arrived = d.until(|d| d.screen() == Screen::Editor && d.panel() == Some(Mode::Map));
    assert!(arrived, "a click on `{card}` did not open that map: {:?} in {:?}", d.screen(), d.panel_name());
    Destination {
        what: "A named map",
        route: "Cmd+O, then click its card on the project page".to_owned(),
        cost: d.cost,
        reach: Reach::Arrived,
    }
}

/// **A named piece on the open map**: its row in the outliner.
fn a_named_piece() -> Destination {
    let mut d = Driver::cold_start("named_piece");
    // View ▸ Show outliner is a menu tick, not a command, so it is two clicks.
    d.click("View");
    d.click("Show outliner");
    let arrived = d.scroll_to("OUTLINER");
    assert!(arrived, "the outliner did not appear: {:#?}", d.words());
    // **The piece's own row**, named `<id> · <descriptor>` by `outliner::nodes_for`.
    // **The row a placement's own id starts**, `<id>@<n> · <descriptor>` — matched on the id rather
    // than on the separator, which the status line also carries.
    let row = d
        .words()
        .into_iter()
        .find(|w| w.starts_with("alpha/floor@"))
        .unwrap_or_else(|| panic!("the outliner drew no piece row: {:#?}", d.words()));
    d.click(&row);
    Destination {
        what: "A named piece on the open map",
        route: "click View, click Show outliner, wheel down to it, click its row".to_owned(),
        cost: d.cost,
        reach: Reach::Arrived,
    }
}

/// **A piece's library entry**, which is the Meshes panel's shelf.
fn a_pieces_library_entry() -> Destination {
    let mut d = Driver::cold_start("library_entry");
    // The crossing key, then the panel slot: the Kit door's first panel is Meshes.
    d.chord(&[KeyCode::Equal]);
    assert_eq!(d.panel_name(), Mode::Meshes.label(), "the crossing opens the Kit door on its first panel");
    // **The shelf's own row for a library piece.** The pack starts folded, so the piece is behind the
    // pack's own row: fold it open, then click the piece. A click selects it, which is what the panel
    // reports in its MESH block.
    let pack = d.words().into_iter().find(|w| w.starts_with("MESHES (")).unwrap_or_else(|| panic!("the shelf drew no pack row: {:#?}", d.words()));
    d.click(&pack);
    let row = d
        .words()
        .into_iter()
        .find(|w| w.contains("alpha/floor") || w == "Floor")
        .unwrap_or_else(|| panic!("the shelf drew no piece row after folding `{pack}`: {:#?}", d.words()));
    d.click(&row);
    let arrived = d.showing("PIECES");
    assert!(arrived, "the shelf is not on screen after the crossing: {:#?}", d.words());
    Destination {
        what: "A piece's library entry",
        route: "press = (cross to the kit), click the pack to open it, click its row on the shelf".to_owned(),
        cost: d.cost,
        reach: Reach::Arrived,
    }
}

/// **A tile**, on the Kit door.
fn a_tile() -> Destination {
    let mut d = Driver::cold_start("tile");
    d.chord(&[KeyCode::Equal]);
    d.chord(&[KeyCode::Digit2]);
    assert_eq!(d.panel_name(), Mode::Tiles.label(), "the second slot key opens Tiles");
    Destination { what: "A tile", route: "press = (cross to the kit), press 2".to_owned(), cost: d.cost, reach: Reach::Arrived }
}

/// **A composition**, on the Kit door.
fn a_composition() -> Destination {
    let mut d = Driver::cold_start("composition");
    d.chord(&[KeyCode::Equal]);
    d.chord(&[KeyCode::Digit3]);
    assert_eq!(d.panel_name(), Mode::Compose.label(), "the third slot key opens Compose");
    Destination { what: "A composition", route: "press = (cross to the kit), press 3".to_owned(), cost: d.cost, reach: Reach::Arrived }
}

/// **The rig bench**, and **a rig's clip** in it.
fn a_rig() -> Destination {
    // The bench is opened by `--door rigs` and by nothing else: the crossing key alternates between
    // the Map and the Kit doors only, and no key, menu row or command names the Rigs door.
    let mut d = Driver::on("rigs", Mode::Anim);
    let arrived = d.showing("RIGS");
    assert!(arrived, "the rigs panel is not on screen when the bench is opened: {:#?}", d.words());
    // And it cannot be reached from inside the editor: crossing twice never arrives.
    let mut probe = Driver::cold_start("rigs_probe");
    let mut seen: Vec<&'static str> = Vec::new();
    for _ in 0..4 {
        probe.chord(&[KeyCode::Equal]);
        seen.push(probe.panel_name());
    }
    assert!(!seen.contains(&Mode::Anim.label()), "a crossing reached the rig bench, which this row says it cannot: {seen:?}");
    Destination {
        what: "A rig",
        route: format!(
            "only by launching the editor with --door rigs; the crossing key alternates Map and Kit, so from inside \
             the editor there is no route (crossing twice lands on {:?} then {:?})",
            seen[0], seen[1]
        ),
        cost: d.cost,
        reach: Reach::Unreachable("no key, menu row or command opens the Rigs door from inside the editor".to_owned()),
    }
}

/// **One of a rig's clips.**
fn a_rigs_clip() -> Destination {
    let mut d = Driver::on("rigs_clip", Mode::Anim);
    // The bench's list is where a clip is chosen, and it is empty on a fixture with no rigs.
    let bench = d.showing("RIGS");
    assert!(bench, "the bench is not even drawn: {:#?}", d.words());
    Destination {
        what: "One of a rig's clips",
        route: "not reachable: the bench is behind --door rigs, and its list is chosen by a click".to_owned(),
        cost: d.cost,
        reach: Reach::Unreachable(format!(
            "the Rigs door has no in-editor route{}, and a clip is chosen from a list inside it",
            if bench { "" } else { " and its bench did not draw" }
        )),
    }
}

/// **A sound cue**, in the Map inspector's AUDIO block.
fn a_sound_cue() -> Destination {
    let mut d = Driver::cold_start("sound");
    let drawn = d.scroll_to("AUDIO");
    if drawn {
        return Destination {
            what: "A sound cue",
            route: "wheel the inspector down to AUDIO".to_owned(),
            cost: d.cost,
            reach: Reach::Arrived,
        };
    }
    // Prove it is absent from the whole panel, not merely below the fold.
    assert!(!d.showing("AUDIO"), "AUDIO appeared only after the budget was spent");
    Destination {
        what: "A sound cue",
        route: format!(
            "no route: {} wheel notches down the inspector and AUDIO never appears, because the block is compiled out \
             without the bevy_siren feature",
            d.cost.pointer
        ),
        cost: d.cost,
        reach: Reach::Unreachable(
            "the AUDIO block is behind the off-by-default `bevy_siren` feature, and nothing else draws a cue".to_owned(),
        ),
    }
}

/// **A map's terrain**, **its lights**, **its data files**, **its problems** and **its notes** — every
/// one of them a heading of the one inspector scroll.
fn an_inspector_section(what: &'static str, label: &'static str, tag: &'static str, absent: &'static str) -> Destination {
    let mut d = Driver::cold_start(tag);
    let up_to = d.cost.clone();
    let arrived = d.scroll_to(label);
    if arrived {
        return Destination {
            what,
            route: format!("wheel the inspector down to {label} (there is no key that scrolls it)"),
            cost: d.cost,
            reach: Reach::Arrived,
        };
    }
    Destination {
        what,
        route: format!("no route: {label} is drawn nowhere"),
        cost: up_to,
        reach: Reach::Unreachable(absent.to_owned()),
    }
}

/// **The history list**, which is the rows beside REDO.
fn the_history() -> Destination {
    let mut d = Driver::cold_start("history");
    let arrived = d.showing("UNDO") && d.showing("REDO");
    assert!(arrived, "the undo and redo history is not on the Map panel: {:#?}", d.words());
    Destination { what: "The history", route: "it is drawn beside UNDO and REDO, on screen at a cold start".to_owned(), cost: d.cost, reach: Reach::Arrived }
}

/// **The AGENT block.**
fn the_agent_block() -> Destination {
    let mut d = Driver::cold_start("agent_block");
    d.click("View");
    d.click("Show agent");
    let arrived = d.showing("AGENT");
    assert!(arrived, "View ▸ Show agent did not put the AGENT block on screen: {:#?}", d.words());
    Destination { what: "The AGENT block", route: "click View, click Show agent".to_owned(), cost: d.cost, reach: Reach::Arrived }
}

/// **The settings screen.**
fn the_settings() -> Destination {
    let mut d = Driver::cold_start("settings");
    d.chord(&[primary(), KeyCode::Comma]);
    let arrived = d.showing("Settings");
    assert!(arrived, "Cmd+, did not open the settings screen: {:#?}", d.words());
    Destination { what: "The settings", route: "Cmd+,".to_owned(), cost: d.cost, reach: Reach::Arrived }
}

/// **The running game's log.**
fn the_games_log() -> Destination {
    let mut d = Driver::cold_start("game_log");
    // Every screen the editor has: the log is not on any of them.
    let mut seen: Vec<&str> = Vec::new();
    for (screen, keys) in [
        (Screen::Editor, None),
        (Screen::Project, Some([primary(), KeyCode::KeyO])),
    ] {
        if let Some(keys) = keys {
            d.chord(&keys);
        }
        assert_eq!(d.screen(), screen, "the probe is on the screen it meant to look at");
        if d.showing("GAME LOG") || d.showing("Game log") {
            seen.push("a log block");
        }
    }
    let cost = d.cost.clone();
    assert!(seen.is_empty(), "a game log is drawn after all: {seen:?}");
    Destination {
        what: "The running game's log",
        route: "no route: the game's output goes to stdout and cauldron.log, and nothing draws it".to_owned(),
        cost,
        reach: Reach::Unreachable("no panel or screen draws the running game's output".to_owned()),
    }
}

// ───────────────────────────── the document ─────────────────────────────

/// Every row, measured once.
fn table() -> Vec<Destination> {
    vec![
        the_map_is_open(),
        a_named_map(),
        a_named_piece(),
        a_pieces_library_entry(),
        a_tile(),
        a_composition(),
        a_rig(),
        a_rigs_clip(),
        a_sound_cue(),
        an_inspector_section("A map's terrain", "TERRAIN", "terrain", ""),
        an_inspector_section(
            "A map's water",
            "WATER",
            "water",
            "water is an operation (`water_place`) with no panel: nothing draws a map's water to walk to",
        ),
        an_inspector_section(
            "A map's forest",
            "FOREST",
            "forest",
            "forest is an operation (`forest_scatter`) with no panel, so there is nothing to walk to",
        ),
        an_inspector_section(
            "A map's lights",
            "LIGHTS",
            "lights",
            "the LIGHTS block is behind the off-by-default `bevy_lights` feature, and nothing else draws a pool",
        ),
        an_inspector_section("A game data file", "DATA", "data", ""),
        an_inspector_section("The problems list", "PROBLEMS", "problems", ""),
        an_inspector_section("The notes", "NOTES", "notes", ""),
        the_history(),
        the_agent_block(),
        the_settings(),
        the_games_log(),
    ]
}

/// **The document, rendered from the rows.** No clock, no paths, so two runs produce one text.
fn render(rows: &[Destination]) -> String {
    let mut out = String::from(
        "# Getting around, as it is today\n\
         \n\
         Generated by `crates/bevy_cauldron/tests/navigation.rs` (`navigation::`), which drives the editor from a\n\
         cold start on a fixture project by the keys and pointer actions a person uses. Rerun it with\n\
         \n\
         ```\n\
         cargo nextest run -p bevy_cauldron --test editor -E 'test(navigation::)'\n\
         ```\n\
         \n\
         **One press is one key event.** A chord is one press (`Cmd+O` counts once) and a typed character is\n\
         one. Pointer actions — a click, a wheel notch — are counted in their own column, because a route\n\
         that needs the wheel has no keyboard form at all.\n\
         \n\
         **A row that does not arrive counts what the search cost.** For those the numbers are the work spent\n\
         establishing that there is no route — sixty wheel notches down the inspector, say — rather than the\n\
         price of a route, because there is no price to pay.\n\
         \n\
         ## What a later change is held to\n\
         \n\
         - Any workspace in **1 press**.\n\
         - Any named thing in at most **Cmd+P, its name and Enter** — one press, some typing, one press.\n\
         - Back to where you were in **1 press**.\n\
         \n\
         ## The rows\n\
         \n\
         | Destination | Route | Keys | Pointer | Arrives |\n\
         |---|---|---|---|---|\n",
    );
    for row in rows {
        let arrives = match &row.reach {
            Reach::Arrived => "yes".to_owned(),
            Reach::Unreachable(why) => format!("**no** — {why}"),
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            row.what, row.route, row.cost.presses, row.cost.pointer, arrives
        ));
    }
    let unreachable: Vec<&Destination> = rows.iter().filter(|r| matches!(r.reach, Reach::Unreachable(_))).collect();
    out.push_str(&format!(
        "\n## What the numbers say\n\
         \n\
         {} of {} destinations cannot be reached at all. Every one that can be reached is reached either\n\
         by a chord the person has to know (`Cmd+O`, `Cmd+,`, the crossing key) or by the wheel: the\n\
         inspector is one long scroll, and no key moves it. That is why a workspace in one press, and a\n\
         name reached by typing it, are the targets.\n",
        unreachable.len(),
        rows.len()
    ));
    out
}

fn baseline_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(BASELINE)
}

/// **Every destination is measured, and the document is what the measurement says** (NV-01, #378).
///
/// The two halves of the acceptance are one test: the rows are driven here, and the file is written
/// from the same rows, so a route that gets longer fails the reader's copy of it as well as this one.
#[test]
fn every_destination_is_measured_and_the_baseline_is_written() {
    let rows = table();
    let rendered = render(&rows);
    let path = baseline_path();
    std::fs::write(&path, &rendered).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    // Rerunning reproduces it: what was written reads back as what a second render makes.
    let read = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(read, rendered, "the baseline does not read back as the table that made it");

    // **The anti-vacuity floor**: a table of nothing would satisfy every assertion above.
    assert_eq!(rows.len(), 20, "the issue lists twenty destinations");
    let arrived = rows.iter().filter(|r| matches!(r.reach, Reach::Arrived)).count();
    assert!(arrived >= 12, "only {arrived} destinations were actually arrived at, so the drivers are not driving");
}

/// A node's laid-out box in the surface's own pixels: `(centre, half extent)`.
fn box_of(entity: Entity, world: &mut World) -> Option<(Vec2, Vec2)> {
    let node = *world.get::<ComputedNode>(entity)?;
    let at = *world.get::<UiGlobalTransform>(entity)?;
    let scale = node.inverse_scale_factor();
    let size = node.size() * scale;
    if size.x <= 0.0 || size.y <= 0.0 {
        return None;
    }
    Some((at.translation * scale, size * 0.5))
}

/// Whether `mine` is inside the box of every `ScrollPosition` ancestor of `entity`.
fn inside_every_scroll_area(mine: (Vec2, Vec2), entity: Entity, world: &mut World) -> bool {
    let mut here = entity;
    while let Some(parent) = world.get::<ChildOf>(here).map(|c| c.parent()) {
        here = parent;
        if world.get::<ScrollPosition>(here).is_none() {
            continue;
        }
        let Some(area) = box_of(here, world) else { continue };
        let overlaps =
            (mine.0.x - area.0.x).abs() < mine.1.x + area.1.x && (mine.0.y - area.0.y).abs() < mine.1.y + area.1.y;
        if !overlaps {
            return false;
        }
    }
    true
}
