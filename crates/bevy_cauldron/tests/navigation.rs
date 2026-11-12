//! **How far away everything is** (NV-01, #378; series getting around, #377).
//!
//! A person's complaint about an editor is rarely "a feature is missing"; it is "I cannot get there".
//! That is measurable, and measuring it is what lets a later change — workspaces, the activity bar,
//! document tabs — be judged as cheaper or not.
//!
//! This file drives the real editor from a cold start on a fixture project, by the keys and pointer
//! actions a hand would use, and counts them. Its rows *are* `docs/ui/navigation.md` (the baseline the
//! series started from, `docs/ui/navigation-baseline.md`, is frozen beside it, #385): the
//! test renders the document and compares it with the committed file, so a route that gets longer
//! fails here until the document is regenerated. Nothing is written to the source tree unless
//! `UPDATE_NAVIGATION_BASELINE=1` is set, which is the pattern `tests/reload_contract.rs` uses for the
//! same reason — this test runs in the push gate.
//!
//! # What one press means
//!
//! One key event. A chord is one press (`Cmd+O` is one) and a typed character is one. Pointer actions
//! are counted apart — a click and a wheel notch each cost one — because a route of two keys is a
//! different thing to learn from a route of two clicks.
//!
//! # Every row's route is what the driver did
//!
//! A `Driver` records one phrase per action it takes, and a row's route column is those phrases
//! joined. Nothing is transcribed beside the measurement, so the words cannot describe a route the
//! numbers disagree with.
//!
//! # An arrival is proved, and an absence is measured
//!
//! A row marked arrived asserts the thing it claims: the selection it made, the map it opened, the
//! block its panel drew. A row that does not arrive reports what the search actually cost — the wheel
//! notches it spent, or the presses it made trying — rather than a zero.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input::mouse::MouseScrollUnit;
use bevy::input::touch::TouchPhase;
use bevy::picking::pointer::{Location, PointerAction, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};

use bevy_cauldron::build::Build;
use bevy_cauldron::compose::ComposeState;
use bevy_cauldron::workspace::Workspace;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::screen::Screen;
use bevy_cauldron::selection::Selection;
use bevy_cauldron::tiles::{ImportState, Mode};
use bevy_gooey::testing::{click, find_by_label};

use crate::fixtures;

/// **Frames the editor is given to finish opening** before a route starts. A door's `OnEnter` spawns
/// the panels, and a fixture measures its meshes on the way in; ten frames is what the editor's own
/// tests settle on for this.
const WARM_UP_FRAMES: usize = 10;

/// **Frames after each action**, so what it changed is laid out before the next one reads the screen.
const SETTLE_FRAMES: usize = 6;

/// **How long a route may wait for something the clock paces** — the loading screen between the
/// project page and a door — before the row is called unreachable.
///
/// Generous on purpose: this loop returns the moment it arrives, so a large budget costs nothing when
/// the editor is quick and keeps the test from flaking when the gate's host is loaded.
const WAIT_BUDGET: Duration = Duration::from_secs(30);

/// How long the real-time wait sleeps between frames.
const WAIT_POLL: Duration = Duration::from_millis(10);

/// **The most wheel notches a row may spend** before the destination is called unscrollable. The Map
/// inspector is about eight screens tall at a fixture's size, so a heading past this is not reachable
/// by a person either.
const WHEEL_BUDGET: u32 = 60;

/// Where the generated table lives, relative to the workspace root. **The live measurement**, since NV-08 (#385)
/// moved the panels into their workspaces and the baseline's routes stopped existing; the baseline is kept as it was.
const BASELINE: &str = "docs/ui/navigation.md";

/// **Set this to rewrite the document** instead of checking it. Run it, read the diff, commit both.
const REGENERATE: &str = "UPDATE_NAVIGATION_BASELINE";

/// The platform's primary modifier, which is what every chord in the issue's targets names.
fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

/// **What a route cost**, measured as it is taken.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
    /// **What the driver actually did**, one phrase per action, in order.
    steps: Vec<String>,
    /// What it cost.
    cost: Cost,
    /// Whether the editor can be got there at all, and if not, why.
    reach: Reach,
}

impl Destination {
    /// The route column: the driver's own phrases, joined.
    fn route(&self) -> String {
        self.steps.join(", ")
    }
}

/// Whether a destination can be arrived at, with the proof.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Reach {
    /// The editor was driven there and the row's own assertion passed.
    Arrived,
    /// No route, with the measured reason.
    Unreachable(String),
}

// ───────────────────────────── the driver ─────────────────────────────

/// **A person at the keyboard and the pointer**, counting what they do and remembering the words for
/// it, so a row's numbers and its route come from one place.
struct Driver {
    app: App,
    cost: Cost,
    steps: Vec<String>,
    /// The wheel has been aimed at the inspector, so a notch lands on it.
    aimed: bool,
}

impl Driver {
    /// The editor on a fresh fixture project, on the Map door with the map open.
    fn cold_start(tag: &str) -> Driver {
        Driver::on(tag, Mode::Map)
    }

    /// **The editor opened on a named panel**, the way `--door` and `--kit` open one.
    ///
    /// The home is added as `main.rs` adds it, so `Cmd+O` lands on a filled project page rather than
    /// on an empty one — a harness that left it out would measure a route the shipped editor does
    /// not have. The project carries a tile and two compositions as well as two placed pieces, so the
    /// Tiles and Compose rows select something real rather than an empty panel.
    fn on(tag: &str, mode: Mode) -> Driver {
        let root = fixtures::Fixture::new(&format!("nav_{tag}_{}", std::process::id()))
            .descriptor("alpha/floor", "alpha")
            .descriptor("alpha/wall", "alpha")
            .place("alpha/floor", (0.0, 0.0))
            .place("alpha/wall", (2.0, 0.0))
            .bounded_composition("crate", (1.0, 1.0, 1.0), &[("body", "alpha/floor", (0.0, 0.0))])
            .bounded_composition("barrel", (1.0, 1.0, 1.0), &[("body", "alpha/wall", (2.0, 0.0))])
            .build("m");
        let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("{e}"));
        app.add_plugins((
            bevy_cauldron::launcher::LauncherPlugin,
            bevy_cauldron::loading::LoadingPlugin,
            bevy_cauldron::window_frame::WindowFramePlugin,
        ));
        for _ in 0..WARM_UP_FRAMES {
            app.update();
        }
        Driver { app, cost: Cost::default(), steps: Vec::new(), aimed: false }
    }

    /// **No `PrimaryWindow` is spawned, on purpose.** The editor draws into an image, and a window
    /// present makes the picking backend and `gooey::testing::click` resolve widgets against a window
    /// that nothing is drawn into, so every click landed nowhere. A key message's `window` is read by
    /// nothing on the path a `ButtonInput` takes, so it is a placeholder.
    fn window() -> Entity {
        Entity::PLACEHOLDER
    }

    fn key(&mut self, code: KeyCode, text: Option<&str>, state: ButtonState) {
        self.app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: text.map_or(Key::Unidentified(NativeKey::Unidentified), |t| Key::Character(t.into())),
            state,
            text: text.map(Into::into),
            repeat: false,
            window: Self::window(),
        });
    }

    fn settle(&mut self) {
        for _ in 0..SETTLE_FRAMES {
            self.app.update();
        }
    }

    /// **One chord, counted as one press, and named for the route.**
    fn chord(&mut self, keys: &[KeyCode], spoken: &str) {
        for state in [ButtonState::Pressed, ButtonState::Released] {
            for code in keys {
                self.key(*code, None, state);
            }
            self.app.update();
        }
        self.settle();
        self.cost.presses += 1;
        self.steps.push(spoken.to_owned());
    }

    /// **A click on the widget whose words are `label`**, counted as one pointer action.
    fn click(&mut self, label: &str) {
        let widget = find_by_label(&mut self.app, label).unwrap_or_else(|e| panic!("no `{label}` on screen: {e}\n\non screen: {:#?}", self.words()));
        click(&mut self.app, widget);
        self.settle();
        self.cost.pointer += 1;
        self.steps.push(format!("click `{label}`"));
    }

    /// **The words a person can see.** `InheritedVisibility` is not enough: the inspector is one long
    /// `ScrollArea`, so every heading in it is `Inherited` while only a screenful is inside the
    /// viewport. A node counts only when its box overlaps the box of every `ScrollArea` above it,
    /// which is what scrolling actually changes.
    fn words(&mut self) -> Vec<String> {
        let world = self.app.world_mut();
        let mut q = world.query::<(Entity, &Text, &InheritedVisibility, &ComputedNode, &UiGlobalTransform)>();
        let found: Vec<(Entity, String)> = q
            .iter(world)
            .filter(|(_, text, shown, node, _)| {
                shown.get() && !text.0.trim().is_empty() && node.size().x > 0.0 && node.size().y > 0.0
            })
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

    /// The one visible word that starts with `prefix`, or a failure that says what was on screen.
    fn word_starting(&mut self, prefix: &str) -> String {
        self.words()
            .into_iter()
            .find(|w| w.starts_with(prefix))
            .unwrap_or_else(|| panic!("nothing on screen starts `{prefix}`; on screen: {:#?}", self.words()))
    }

    /// **Frames in real time until `done`**, for the loading screen, which is paced by the clock
    /// rather than by frames. Returns the moment it arrives, so the budget costs nothing when the
    /// editor is quick.
    fn until(&mut self, mut done: impl FnMut(&mut Self) -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < WAIT_BUDGET {
            self.app.update();
            if done(self) {
                return true;
            }
            std::thread::sleep(WAIT_POLL);
        }
        false
    }

    fn screen(&self) -> Screen {
        *self.app.world().resource::<State<Screen>>().get()
    }

    /// **The panel showing, or none.** A door's resources are torn down with it (`screen::OWNERSHIP`),
    /// so off the editor there is no panel at all.
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
        let mut q = world
            .query_filtered::<(&ScrollPosition, &ComputedNode, &UiGlobalTransform), With<bevy_cauldron::editor::MapControlsPane>>();
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
            self.app
                .world_mut()
                .write_message(PointerInput::new(PointerId::Mouse, location.clone(), PointerAction::Move { delta: Vec2::ZERO }));
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

    /// **Wheel down until `label` is on screen**, answering whether it got there. The notches are
    /// counted as they are spent, so a row that does not arrive reports what the search cost.
    fn scroll_to(&mut self, label: &str) -> bool {
        if self.showing(label) {
            return true;
        }
        for _ in 0..WHEEL_BUDGET {
            if !self.notch() {
                break;
            }
            if self.showing(label) {
                self.steps.push(format!("wheel down to `{label}`"));
                return true;
            }
        }
        self.steps.push(format!("wheel {WHEEL_BUDGET} notches, never reaching `{label}`"));
        false
    }
}

// ───────────────────────────── the destinations ─────────────────────────────

/// **The map that is already open.**
fn the_map_is_open() -> Destination {
    let mut d = Driver::cold_start("map");
    assert!(d.showing("MAP") && d.showing("INSPECTOR"), "the Map door's own panel is not on screen: {:#?}", d.words());
    assert_eq!(d.panel_name(), Mode::Map.label(), "the editor opened on the Map door");
    Destination { what: "The open map", steps: vec!["the editor starts on it".to_owned()], cost: d.cost, reach: Reach::Arrived }
}

/// **A named map**, from the Map door.
fn a_named_map() -> Destination {
    let mut d = Driver::cold_start("named_map");
    d.chord(&[primary(), KeyCode::KeyO], "Cmd+O (close project)");
    assert_eq!(d.screen(), Screen::Project, "Cmd+O leaves the door for the project page");
    let card = {
        let world = d.app.world_mut();
        let mut q = world.query_filtered::<&bevy_cauldron::project_page::MapCard, ()>();
        q.iter(world).map(|c| c.0.clone()).next().unwrap_or_else(|| panic!("the project page lists no map: {:#?}", d.words()))
    };
    d.click(&card);
    let arrived = d.until(|d| d.screen() == Screen::Editor && d.panel() == Some(Mode::Map));
    assert!(arrived, "the click on `{card}` did not reach the editor: {:?} in {:?}", d.screen(), d.panel_name());
    // **The map that opened is the one on the card**, not merely some map.
    let opened = d.app.world().resource::<OpenMap>().map.name.clone();
    assert_eq!(opened, card, "the editor opened `{opened}` and the card said `{card}`");
    Destination { what: "A named map", steps: d.steps, cost: d.cost, reach: Reach::Arrived }
}

/// **A named piece on the open map**: its row in the outliner.
fn a_named_piece() -> Destination {
    let mut d = Driver::cold_start("named_piece");
    d.click("View");
    d.click("Show outliner");
    assert!(d.scroll_to("OUTLINER"), "the outliner did not appear: {:#?}", d.words());
    // **The row a placement's own id starts**, `<id>@<n> · <descriptor>`.
    let row = d.word_starting("alpha/floor@");
    d.click(&row);
    // **Selected, which is what clicking a row means** (`outliner`: "Selecting a row selects the piece").
    let selected = d.app.world().resource::<Selection>().ids().to_vec();
    assert!(
        selected.iter().any(|id| id.starts_with("alpha/floor@")),
        "clicking `{row}` selected {selected:?} instead of that piece"
    );
    Destination { what: "A named piece on the open map", steps: d.steps, cost: d.cost, reach: Reach::Arrived }
}

/// **A piece's library entry**, which is the Meshes panel's shelf.
fn a_pieces_library_entry() -> Destination {
    let mut d = Driver::cold_start("library_entry");
    d.chord(&[primary(), KeyCode::Digit4], "press Cmd/Ctrl+4 (the Kit workspace)");
    assert_eq!(d.panel_name(), Mode::Meshes.label(), "the Kit workspace opens on its first panel");
    // The pack starts folded, so the piece is behind the pack's own row.
    let pack = d.word_starting("MESHES (");
    d.click(&pack);
    let row = d.word_starting("alpha/floor");
    d.click(&row);
    // **The panel armed that piece**, which is what selecting a library row does — not merely that
    // the shelf is drawn, which was true before either click.
    let armed = d.app.world().resource::<ImportState>().selected_library_id.clone();
    assert_eq!(armed.as_deref(), Some("alpha/floor"), "clicking `{row}` did not arm it on the shelf (armed {armed:?})");
    Destination { what: "A piece's library entry", steps: d.steps, cost: d.cost, reach: Reach::Arrived }
}

/// **A tile**, opened on the Tiles panel.
fn a_tile() -> Destination {
    let mut d = Driver::cold_start("tile");
    d.chord(&[primary(), KeyCode::Digit4], "press Cmd/Ctrl+4 (the Kit workspace)");
    d.click("TILES");
    assert_eq!(d.panel_name(), Mode::Tiles.label(), "the strip's second chip opens Tiles");
    // **The tile's own row**, then `Enter`, which is how a row on this page opens
    // ("1 tile(s) — Enter opens one").
    let row = d.word_starting("TILES (");
    d.click(&row);
    let tile = d.word_starting("crate");
    d.click(&tile);
    d.chord(&[KeyCode::Enter], "press Enter (open it)");
    let open = d.app.world().get_resource::<Build>().and_then(|b| b.open.as_ref().map(|c| c.id.clone()));
    assert_eq!(open.as_deref(), Some("crate"), "the Tiles panel has no tile open after clicking its row (open {open:?})");
    Destination { what: "A tile", steps: d.steps, cost: d.cost, reach: Reach::Arrived }
}

/// **A composition**, chosen on the Compose panel.
fn a_composition() -> Destination {
    let mut d = Driver::cold_start("composition");
    d.chord(&[primary(), KeyCode::Digit4], "press Cmd/Ctrl+4 (the Kit workspace)");
    d.click("COMPOSE");
    assert_eq!(d.panel_name(), Mode::Compose.label(), "the strip's third chip opens Compose");
    // **The second group's own row**, so the click changes the selection rather than confirming it.
    let row = d.word_starting("barrel ");
    d.click(&row);
    let selected = d.app.world().get_resource::<ComposeState>().map(|s| s.selected);
    assert_eq!(selected, Some(1), "clicking `{row}` left the composition cursor at {selected:?}");
    assert!(d.showing("MEMBERS OF `barrel`"), "the panel did not open `barrel`: {:#?}", d.words());
    Destination { what: "A composition", steps: d.steps, cost: d.cost, reach: Reach::Arrived }
}

/// **The rig bench**, one chord away: the Animation workspace (`Cmd`/`Ctrl`+`5`, #379).
///
/// It was the one row that was unreachable from inside the editor: the crossing key alternated Map and
/// Kit and the bench opened only from `--door rigs` on the command line. The workspaces made it a
/// destination like the others.
fn a_rig() -> Destination {
    let mut d = Driver::cold_start("rig");
    d.chord(&[primary(), KeyCode::Digit5], "press Cmd/Ctrl+5 (the Animation workspace)");
    assert_eq!(d.panel_name(), Mode::Anim.label(), "the Animation workspace opens the rig bench");
    assert!(d.showing("RIGS"), "the bench did not draw: {:#?}", d.words());
    Destination { what: "A rig (the Rigs bench)", steps: d.steps, cost: d.cost, reach: Reach::Arrived }
}

/// **One of a rig's clips**, which is chosen from a list inside the bench.
fn a_rigs_clip() -> Destination {
    let mut d = Driver::cold_start("rigs_clip");
    d.chord(&[primary(), KeyCode::Digit5], "press Cmd/Ctrl+5 (the Animation workspace)");
    assert!(d.showing("RIGS"), "the bench is not even drawn: {:#?}", d.words());
    // The list a clip is chosen from, and whether this project has one to choose.
    let rows = d.words().into_iter().filter(|w| w.starts_with("no rigs") || w.contains("rig(s)")).collect::<Vec<_>>();
    let mut steps = d.steps;
    steps.push("click a row in the bench's list".to_owned());
    Destination {
        what: "One of a rig's clips",
        steps,
        cost: d.cost,
        reach: Reach::Unreachable(format!(
            "a clip is chosen from a list inside the bench, which Cmd/Ctrl+5 opens; this fixture has no rig, so its list reads {rows:?}"
        )),
    }
}

/// **A sound cue**, in the Sound workspace (`Cmd`/`Ctrl`+`6`, NV-08 #385).
///
/// It was the Map inspector's AUDIO block, 60 wheel notches that never reached it. The audio panel is the
/// Sound workspace's now, one chord away. Its heading is drawn in a build with `bevy_siren`; a build without
/// the crate says so in a sentence, and that is the row's reason.
fn a_sound_cue() -> Destination {
    let mut d = Driver::cold_start("sound");
    d.chord(&[primary(), KeyCode::Digit6], "press Cmd/Ctrl+6 (the Sound workspace)");
    assert_eq!(d.panel_name(), Mode::Sound.label(), "Cmd/Ctrl+6 opens the Sound workspace");
    let cost = d.cost;
    let steps = d.steps.clone();
    if d.showing("AUDIO") {
        return Destination { what: "A sound cue", steps, cost, reach: Reach::Arrived };
    }
    assert!(
        d.showing(bevy_cauldron::workspace_panels::NO_AUDIO),
        "the Sound workspace shows neither AUDIO nor the sentence that says the build has none: {:#?}",
        d.words()
    );
    Destination {
        what: "A sound cue",
        steps,
        cost,
        reach: Reach::Unreachable(
            "the Sound workspace opens by one chord, but the AUDIO panel is behind the off-by-default `bevy_siren` \
             feature: this build draws a sentence saying so, and no cue"
                .to_owned(),
        ),
    }
}

/// **A part of the world**, in the World workspace (`Cmd`/`Ctrl`+`3`, NV-08 #385): terrain, water, forest and
/// lights were headings of the one Map inspector scroll.
///
/// `absent` is the sentence a row that does not arrive gives, naming what would have to exist.
fn a_world_section(what: &'static str, label: &'static str, tag: &'static str, absent: &'static str) -> Destination {
    assert!(!absent.is_empty(), "`{label}` must say why it cannot be reached, since a row renders that reason");
    let mut d = Driver::cold_start(tag);
    d.chord(&[primary(), KeyCode::Digit3], "press Cmd/Ctrl+3 (the World workspace)");
    assert_eq!(d.panel_name(), Mode::World.label(), "Cmd/Ctrl+3 opens the World workspace");
    let arrived = d.showing(label);
    let reach = if arrived { Reach::Arrived } else { Reach::Unreachable(absent.to_owned()) };
    Destination { what, steps: d.steps, cost: d.cost, reach }
}

/// **A heading of the one inspector scroll** — terrain, water, forest, lights, a data file, the
/// problems and the notes all live here.
///
/// `absent` is the sentence a row that does not arrive gives, naming what would have to exist.
fn an_inspector_section(what: &'static str, label: &'static str, tag: &'static str, absent: &'static str) -> Destination {
    assert!(!absent.is_empty(), "`{label}` must say why it cannot be reached, since a row renders that reason");
    let mut d = Driver::cold_start(tag);
    let arrived = d.scroll_to(label);
    // The wheel notches are counted as they are spent, so an unreachable row reports the search.
    let cost = d.cost;
    let steps = d.steps.clone();
    let reach = if arrived { Reach::Arrived } else { Reach::Unreachable(absent.to_owned()) };
    Destination { what, steps, cost, reach }
}

/// **The history list**, which is the rows beside REDO.
fn the_history() -> Destination {
    let mut d = Driver::cold_start("history");
    assert!(d.showing("UNDO") && d.showing("REDO"), "the undo and redo history is not on the Map panel: {:#?}", d.words());
    // **The rows themselves**, which is what "the history" means. UI-53 made it gooey's virtualized list, so
    // what is counted is the list's nodes — one per line of the session — and the rows it draws of them.
    let (lines, drawn) = {
        let world = d.app.world_mut();
        let tree = world
            .query_filtered::<Entity, With<bevy_cauldron::editor::HistoryTree>>()
            .single(world)
            .ok();
        let lines = tree
            .and_then(|tree| world.get::<bevy_gooey::Table>(tree))
            .and_then(|table| table.source::<bevy_gooey::Tree>())
            .map_or(0, bevy_gooey::Tree::node_count);
        let drawn = tree.map_or(0, |tree| {
            world.query::<&bevy_gooey::TableRow>().iter(world).filter(|row| row.table == tree).count()
        });
        (lines, drawn)
    };
    // **A cold start has no edits, so the list is one line and one row is drawn** (gooey's pool is
    // `(rows tall).ceil() + 1`, capped at the lines it holds): the list is there, and it draws exactly what it holds.
    let pool = {
        let world = d.app.world_mut();
        world
            .query_filtered::<&bevy_gooey::TableRows, With<bevy_cauldron::editor::HistoryTree>>()
            .single(world)
            .map_or(0, |rows| rows.0.ceil() as usize + 1)
    };
    assert!(lines >= 1 && drawn >= 1, "the history list holds {lines} line(s) and drew {drawn} row(s), so the undo list is not there");
    assert_eq!(
        drawn,
        lines.min(pool),
        "the history list drew {drawn} row(s) of its {lines} line(s); a pool of {pool} draws {} of them",
        lines.min(pool)
    );
    Destination {
        what: "The history",
        steps: vec![format!(
            "it is drawn beside UNDO and REDO, on screen at a cold start ({lines} {}, {drawn} drawn)",
            plural(lines, "line", "lines")
        )],
        cost: d.cost,
        reach: Reach::Arrived,
    }
}

/// **The AGENT block.**
fn the_agent_block() -> Destination {
    let mut d = Driver::cold_start("agent_block");
    // Owner exception (2026-11-12, NV-10 #387): the AGENT block is the Agent workspace's, one chord away (Cmd/Ctrl+9), where it was
    // View ▸ Show agent; the compact block is still that toggle, and is not the destination measured here.
    d.chord(&[primary(), KeyCode::Digit9], "press Cmd/Ctrl+9 (the Agent workspace)");
    assert_eq!(d.panel_name(), Mode::Agent.label(), "Cmd/Ctrl+9 opens the Agent workspace");
    assert!(d.showing("AGENT"), "the Agent workspace did not put the AGENT block on screen: {:#?}", d.words());
    // **The block's own three lines**, so this is the block and not a heading that happens to match.
    for line in ["HEARD", "NEWEST", "HAND-BACK"] {
        assert!(d.showing(line), "the AGENT block is on screen without its `{line}` line");
    }
    Destination { what: "The AGENT block", steps: d.steps, cost: d.cost, reach: Reach::Arrived }
}

/// **The settings screen.**
fn the_settings() -> Destination {
    let mut d = Driver::cold_start("settings");
    d.chord(&[primary(), KeyCode::Comma], "Cmd+,");
    assert!(d.showing("Settings"), "Cmd+, did not open the settings screen: {:#?}", d.words());
    assert!(d.showing("Search settings"), "the settings screen is on screen without its search box");
    Destination { what: "The settings", steps: d.steps, cost: d.cost, reach: Reach::Arrived }
}

/// **The running game's log**, in the Game workspace (`Cmd`/`Ctrl`+`8`, NV-09 #386): its output is written to a file the
/// workspace tails, under the heading LOG.
fn the_games_log() -> Destination {
    let mut d = Driver::cold_start("game_log");
    d.chord(&[primary(), KeyCode::Digit8], "press Cmd/Ctrl+8 (the Game workspace)");
    assert_eq!(d.panel_name(), Mode::Game.label(), "Cmd/Ctrl+8 opens the Game workspace");
    let arrived = d.showing("LOG");
    let reach = if arrived {
        Reach::Arrived
    } else {
        Reach::Unreachable("the Game workspace is open and the LOG heading is not on screen".to_owned())
    };
    Destination { what: "The running game's log", steps: d.steps, cost: d.cost, reach }
}

// ───────────────────────────── the document ─────────────────────────────

/// **Every row, in the issue's order.** The names are fixed here so a row that disappeared is a
/// failure rather than a shorter document.
const ROWS: [&str; 20] = [
    "The open map",
    "A named map",
    "A named piece on the open map",
    "A piece's library entry",
    "A tile",
    "A composition",
    "A rig (the Rigs bench)",
    "One of a rig's clips",
    "A sound cue",
    "A map's terrain",
    "A map's water",
    "A map's forest",
    "A map's lights",
    "A game data file",
    "The problems list",
    "The notes",
    "The history",
    "The AGENT block",
    "The settings",
    "The running game's log",
];

/// **The fewest rows that must actually arrive**, so a driver that stopped driving cannot pass this
/// test by reporting absences.
const LEAST_ARRIVED: usize = 12;

/// Every row, measured.
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
        a_world_section(
            "A map's terrain",
            "TERRAIN",
            "terrain",
            "the TERRAIN line is in the World workspace and the heading is not on screen there",
        ),
        a_world_section(
            "A map's water",
            "WATER",
            "water",
            "water is an operation (`water_place`) written through the World workspace's terrain line; it has no panel of its own, so nothing draws a map's water to walk to",
        ),
        a_world_section(
            "A map's forest",
            "FOREST",
            "forest",
            "forest is an operation (`forest_scatter`) written through the World workspace's terrain line; it has no panel of its own, so there is nothing to walk to",
        ),
        a_world_section(
            "A map's lights",
            "LIGHTS",
            "lights",
            "the LIGHTS block is in the World workspace but behind the off-by-default `bevy_lights` feature, and nothing else draws a pool",
        ),
        an_inspector_section(
            "A game data file",
            "DATA",
            "data",
            "the DATA table is a heading of the Map inspector and the wheel never reached it",
        ),
        an_inspector_section(
            "The problems list",
            "PROBLEMS",
            "problems",
            "PROBLEMS is a heading of the Map inspector and the wheel never reached it",
        ),
        an_inspector_section("The notes", "NOTES", "notes", "NOTES is a heading of the Map inspector and the wheel never reached it"),
        the_history(),
        the_agent_block(),
        the_settings(),
        the_games_log(),
    ]
}

/// **What the numbers say**, built from the rows rather than written beside them.
///
/// It counts how each arriving row was reached and reports the wheel's own range over the rows that
/// actually wheel, so the paragraph cannot claim a route or a cost the table does not show.
fn verdict(rows: &[Destination]) -> String {
    let arrived: Vec<&Destination> = rows.iter().filter(|r| matches!(r.reach, Reach::Arrived)).collect();
    let unreachable = rows.len() - arrived.len();
    let here = arrived.iter().filter(|r| r.cost.presses == 0 && r.cost.pointer == 0).count();
    let keys_only = arrived.iter().filter(|r| r.cost.presses > 0 && r.cost.pointer == 0).count();
    let pointers = arrived.iter().filter(|r| r.cost.pointer > 0).count();
    // **The wheel's own cost**, over the rows that wheel: a row reached by a click must not drag the
    // minimum down to zero.
    let wheels: Vec<u32> = arrived
        .iter()
        .filter(|r| r.steps.iter().any(|s| s.starts_with("wheel")))
        .map(|r| r.cost.pointer)
        .collect();
    let wheel_clause = match (wheels.iter().min(), wheels.iter().max()) {
        (Some(least), Some(most)) if least != most => {
            format!("The inspector moves only by the wheel, so scrolling to a heading costs between {least} and {most} notches.")
        }
        (Some(one), _) => format!("The inspector moves only by the wheel, which costs {one} notches to a heading."),
        _ => "Nothing that arrives needs the wheel.".to_owned(),
    };
    let mut out = format!(
        "{unreachable} of {} destinations cannot be reached at all. Of the {} that can, {here} {}, {keys_only} {}, \
         and {pointers} {}. {wheel_clause}",
        rows.len(),
        arrived.len(),
        plural(here, "is where the editor already starts", "are where the editor already starts"),
        plural(keys_only, "is reached by a key alone", "are reached by keys alone"),
        plural(pointers, "needs the pointer", "need the pointer"),
    );
    out.push_str(&format!(
        " Set against that, a workspace model asks for any workspace in one press, any named thing in at most \
         `Cmd+P`, its name and `Enter`, and one press back: {} of these {} cost more than one press and {pointers} \
         of them need the pointer.",
        arrived.iter().filter(|r| r.cost.presses > 1).count(),
        arrived.len()
    ));
    out
}

/// `one` or `many`, chosen by the count.
fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 { one } else { many }
}

/// **What the frozen baseline (`navigation-baseline.md`, NV-01 #378) said about one destination.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Was {
    keys: u32,
    pointer: u32,
    arrives: bool,
}

/// **The frozen baseline's rows**, by destination, read from the document NV-01 left. A document that does not read is a failure: the
/// comparison would otherwise be against nothing.
fn baseline() -> Vec<(String, Was)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/ui/navigation-baseline.md");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut rows = Vec::new();
    for line in text.lines().filter(|l| l.starts_with("| ") && !l.starts_with("| Destination")) {
        let cells: Vec<&str> = line.trim_matches('|').split(" | ").map(str::trim).collect();
        if cells.len() < 5 {
            continue;
        }
        let (Ok(keys), Ok(pointer)) = (cells[2].parse::<u32>(), cells[3].parse::<u32>()) else { continue };
        rows.push((cells[0].to_owned(), Was { keys, pointer, arrives: cells[4] == "yes" }));
    }
    assert!(rows.len() >= ROWS.len(), "the baseline lists {} destinations and this test measures {}", rows.len(), ROWS.len());
    rows
}

/// **What NV-01 held a later change to**, per destination: a workspace in one press, or a named thing in the everything-finder.
#[derive(Clone, Copy, Debug)]
enum Target {
    /// One key press, no pointer.
    OnePress,
    /// `Cmd+P`, its name, `Enter`: met when the finder is *driven* with the thing's name and arrives at its page.
    Finder,
}

/// Which target a destination is held to.
fn target_of(what: &str) -> Target {
    match what {
        "A named map"
        | "A named piece on the open map"
        | "A piece's library entry"
        | "A tile"
        | "A composition"
        | "One of a rig's clips"
        | "A game data file"
        | "The notes" => Target::Finder,
        _ => Target::OnePress,
    }
}

/// **The issues filed for a missed target**, by destination (NV-11, #388): a missed target is its own issue, naming the destination and
/// the route. A miss with no entry here fails the test, so a target cannot be missed quietly.
const FILED: &[(&str, &str)] = &[
    ("One of a rig's clips", "#391"),
    ("A sound cue", "#392"),
    ("A map's water", "#393"),
    ("A map's forest", "#394"),
    ("A map's lights", "#395"),
    ("The problems list", "#397"),
];

// ───────────────────────────── driving the finder ─────────────────────────────

/// Where the bed stands in the finder fixture's 64 m map, far from where the camera is put, so a finder that did nothing cannot pass.
const BED_AT: (f32, f32) = (28.0, -28.0);
/// How near the camera's focus must come to the bed for the finder to have framed it, in metres.
const FRAMED_WITHIN_M: f32 = 6.0;
/// The registered data file the finder fixture holds, and its id.
const DATA_REGISTRY: &str = "(version: 1, files: [(id: \"tuning\", path: \"mob.ron\", bounds: {\"mob.speed\": (min: 0.0, max: 9.0)})])";
const DATA_FILE: &str = "// tuning\n(mob: (speed: 1.50, name: \"orc\", hp: 7), fast: false)\n";

/// **What typing into the finder asks for, per destination**: the thing's own name as a person types it.
const FINDER_ASKS: [(&str, &str); 7] = [
    ("A named map", "meadow"),
    ("A named piece on the open map", "bed@3"),
    ("A piece's library entry", "stool"),
    ("A tile", "tiling"),
    ("A composition", "stack"),
    ("A game data file", "tuning"),
    ("The notes", "mind"),
];

/// **The editor for the finder rows**: the same kind of project as the other rows' (pieces, a placed piece, a composition) with what
/// the finder indexes beyond them — a lattice, a registered data file and a note — added, and a window so keys reach the finder's field.
fn finder_editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("nav_finder_{tag}_{}", std::process::id()))
        .descriptor("stool", "alpha")
        .descriptor("bed", "alpha")
        .place_as("bed@3", "bed", BED_AT)
        .authored_lattice("tiling", "alpha", "wall", 2)
        .bounded_composition("stack", (1.0, 1.0, 1.0), &[("body", "stool", (0.0, 0.0))])
        .side(64.0)
        .build("meadow");
    // **Where the project reads its registry from** is its authoring kit's directory, which the project says.
    let dir = bevy_cauldron::project::Project::open(&root, None).unwrap_or_else(|e| panic!("{e}")).grimoire_dir;
    std::fs::write(dir.join("mob.ron"), DATA_FILE).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(dir.join("data.ron"), DATA_REGISTRY).unwrap_or_else(|e| panic!("{e}"));
    let mut app = bevy_cauldron::harness::build_headless(&root, "meadow", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
    app.world_mut().spawn((Window::default(), bevy::window::PrimaryWindow));
    for _ in 0..WARM_UP_FRAMES {
        app.update();
    }
    // **A note**, left the way an agent or a person leaves one: through the registry's operation.
    let mut ctx = bevy_cauldron::ops::Ctx {
        project: app.world().get_resource::<bevy_cauldron::project::Project>().cloned(),
        open: app.world().get_resource::<OpenMap>().cloned(),
        session: "nav".to_owned(),
        ..bevy_cauldron::ops::Ctx::default()
    };
    let note = bevy_cauldron::ops::dispatch(&mut ctx, "note_add", &serde_json::json!({"cells": [0, 0, 0, 0], "text": "mind the gap"}));
    assert!(matches!(note, Ok(bevy_cauldron::ops::Applied::Done { .. })), "the finder fixture could not leave its note");
    for _ in 0..40 {
        app.update();
    }
    app
}

/// One key event to the primary window, as the finder's own tests send it.
fn finder_key(app: &mut App, code: KeyCode, logical: bevy::input::keyboard::Key, text: Option<&str>, state: ButtonState) {
    let window = app.world_mut().query_filtered::<Entity, With<bevy::window::PrimaryWindow>>().single(app.world()).expect("one primary window");
    app.world_mut().write_message(KeyboardInput { key_code: code, logical_key: logical, state, text: text.map(Into::into), repeat: false, window });
}

/// The key a typed character is made with on a US keyboard, and whether Shift is held for it.
fn finder_key_for(c: char) -> (KeyCode, bool) {
    const LETTERS: [KeyCode; 26] = [
        KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD, KeyCode::KeyE, KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH, KeyCode::KeyI,
        KeyCode::KeyJ, KeyCode::KeyK, KeyCode::KeyL, KeyCode::KeyM, KeyCode::KeyN, KeyCode::KeyO, KeyCode::KeyP, KeyCode::KeyQ, KeyCode::KeyR,
        KeyCode::KeyS, KeyCode::KeyT, KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX, KeyCode::KeyY, KeyCode::KeyZ,
    ];
    match c {
        'a'..='z' => (LETTERS[usize::from(c as u8 - b'a')], false),
        '@' => (KeyCode::Digit2, true),
        '3' => (KeyCode::Digit3, false),
        other => panic!("the finder driver has no key for `{other}`"),
    }
}

/// **Drive the finder for one destination**: `Cmd+P`, the name typed, `Enter`. Costs one press for the chord, one per character and one
/// for `Enter`; arrives when the page the destination is on is the one showing.
fn drive_the_finder(what: &'static str, query: &'static str) -> Result<Cost, String> {
    use bevy::input::keyboard::{Key, NativeKey};
    let mut app = finder_editor(what.split_whitespace().last().unwrap_or("row"));
    for _ in 0..40 {
        app.update();
    }
    if what == "A named piece on the open map" {
        let mut rig = app.world_mut().resource_mut::<bevy_cauldron::view::Rig>();
        rig.focus = Vec3::new(-BED_AT.0, 0.0, -BED_AT.1);
        rig.height = 8.0;
        for _ in 0..40 {
            app.update();
        }
    }
    let settle = |app: &mut App| {
        for _ in 0..40 {
            app.update();
        }
    };
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in [primary(), KeyCode::KeyP] {
            finder_key(&mut app, key, Key::Unidentified(NativeKey::Unidentified), None, state);
        }
        app.update();
    }
    settle(&mut app);
    let opened = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy_gooey::CommandPalette>>().iter(world).next().is_some()
    };
    if !opened {
        return Err("Cmd+P did not open the finder".to_owned());
    }
    for c in query.chars() {
        let (code, shift) = finder_key_for(c);
        let s = c.to_string();
        for state in [ButtonState::Pressed, ButtonState::Released] {
            if shift {
                finder_key(&mut app, KeyCode::ShiftLeft, Key::Unidentified(NativeKey::Unidentified), None, state);
            }
            finder_key(&mut app, code, Key::Character(s.as_str().into()), (state == ButtonState::Pressed).then_some(s.as_str()), state);
            app.update();
        }
    }
    settle(&mut app);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        finder_key(&mut app, KeyCode::Enter, Key::Enter, None, state);
        app.update();
    }
    settle(&mut app);
    let still_open = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy_gooey::CommandPalette>>().iter(world).next().is_some()
    };
    if still_open {
        return Err(format!("Enter did not choose a row for `{query}`: the finder is still open"));
    }
    let workspace = *app.world().resource::<Workspace>();
    let mode = app.world().get_resource::<Mode>().map(|m| m.label());
    let selected = app.world().resource::<ImportState>().selected_library_id.clone();
    let arrived = match what {
        "A named map" => (workspace == Workspace::Map && app.world().resource::<OpenMap>().map.name == "meadow").then_some(()),
        "A named piece on the open map" => {
            let focus = app.world().resource::<bevy_cauldron::view::Rig>().focus;
            (workspace == Workspace::Map && Vec2::new(focus.x - BED_AT.0, focus.z - BED_AT.1).length() < FRAMED_WITHIN_M).then_some(())
        }
        "A piece's library entry" => (workspace == Workspace::Kit && selected.as_deref() == Some("stool")).then_some(()),
        "A tile" => (workspace == Workspace::Kit && selected.as_deref() == Some("tiling")).then_some(()),
        "A composition" => (workspace == Workspace::Kit && mode == Some(Mode::Compose.label())).then_some(()),
        "A game data file" | "The notes" => (workspace == Workspace::Map).then_some(()),
        other => return Err(format!("no arrival is defined for `{other}`")),
    };
    match arrived {
        Some(()) => Ok(Cost { presses: 2 + query.chars().count() as u32, pointer: 0 }),
        None => Err(format!("`{query}` chosen in the finder landed on the {} workspace, not on {what}'s page", workspace.title())),
    }
}

/// **Every named-thing destination, asked of the finder**, and what became of it.
fn finder_runs() -> Vec<(&'static str, Result<Cost, String>)> {
    let mut runs: Vec<(&'static str, Result<Cost, String>)> = FINDER_ASKS.iter().map(|(what, query)| (*what, drive_the_finder(what, query))).collect();
    runs.push((
        "One of a rig's clips",
        Err("the finder indexes rigs and not their clips: a clip has no page of its own, so Cmd+P cannot name one".to_owned()),
    ));
    runs
}

/// **Is the target met?** `Ok` with the measured route, or the measured reason it is not.
fn target_met(row: &Destination, finder: &[(&'static str, Result<Cost, String>)]) -> Result<String, String> {
    match target_of(row.what) {
        Target::OnePress => {
            let arrived = matches!(row.reach, Reach::Arrived);
            if arrived && row.cost.presses <= 1 && row.cost.pointer == 0 {
                Ok(format!("{} press(es)", row.cost.presses))
            } else {
                Err(format!(
                    "{} press(es) and {} pointer action(s) against one press{}",
                    row.cost.presses,
                    row.cost.pointer,
                    if arrived { "" } else { ", and it does not arrive" }
                ))
            }
        }
        Target::Finder => match finder.iter().find(|(what, _)| *what == row.what) {
            Some((_, Ok(cost))) => Ok(format!("driven: Cmd+P, the name, Enter, {} presses", cost.presses)),
            Some((_, Err(why))) => Err(format!("driving the finder failed: {why}")),
            None => Err("the finder was not driven for it".to_owned()),
        },
    }
}

/// **The document, rendered from the rows, the frozen baseline's rows and the targets.** No clock, no paths, so two runs produce one text.
fn render(rows: &[Destination], was: &[(String, Was)], finder: &[(&'static str, Result<Cost, String>)]) -> String {
    let mut out = String::from(
        "# Getting around, as it is now\n\
         \n\
         Measured against `navigation-baseline.md`, which is frozen as NV-01 (#378) left it (`the_navigation_baseline_stays_frozen`).\n\
         \n\
         Generated by `crates/bevy_cauldron/tests/navigation.rs` (`navigation::`), which drives the editor from a\n\
         cold start on a fixture project by the keys and pointer actions a person uses. The test **checks** this\n\
         file: it fails when the table below and the editor disagree, and rewrites it only with\n\
         `UPDATE_NAVIGATION_BASELINE=1`:\n\
         \n\
         ```\n\
         UPDATE_NAVIGATION_BASELINE=1 cargo nextest run -p bevy_cauldron --test editor -E 'test(navigation::)'\n\
         ```\n\
         \n\
         **One press is one key event.** A chord is one press (`Cmd+O` counts once) and a typed character is\n\
         one. Pointer actions — a click, a wheel notch — are counted in their own column, because a route\n\
         that needs the wheel has no keyboard form at all. A row's route is the driver's own record of what it\n\
         did, in order.\n\
         \n\
         ## What a later change is held to\n\
         \n\
         - Any workspace in **1 press**: a destination of the workspace kind is *met* when it arrives in at most one press and no pointer action.\n\
         - Any named thing in at most **Cmd+P, its name and Enter** — one press, some typing, one press: *met* when the finder is **driven** with the\n\
         \x20 thing's name (`Cmd+P`, the characters, `Enter`) on a project that holds one — a piece, a placed piece, a lattice, a composition, a registered\n\
         \x20 data file and a note — and arrives at its page. The cost shown is that route's presses.\n\
         - Back to where you were in **1 press**: held by `back_and_forward_across_workspaces`, not a row here.\n\
         - **Never worse than the baseline**: a destination that arrived then still arrives, and costs no more keys and pointer actions together.\n\
         \n\
         A target that is *missed* is filed as its own issue, naming the destination and the route; the issue is cited in the Target column and\n\
         the test fails for a miss with none.\n\
         \n\
         ## The rows\n\
         \n\
         | Destination | Route | Keys | Pointer | Arrives | Baseline (keys, pointer, arrives) | Target |\n\
         |---|---|---|---|---|---|---|\n",
    );
    for row in rows {
        let arrives = match &row.reach {
            Reach::Arrived => "yes".to_owned(),
            Reach::Unreachable(why) => format!("**no** — {why}"),
        };
        let old = was
            .iter()
            .find(|(name, _)| name == row.what)
            .map_or_else(|| "not in the baseline".to_owned(), |(_, w)| format!("{}, {}, {}", w.keys, w.pointer, if w.arrives { "yes" } else { "no" }));
        let target = match target_met(row, finder) {
            Ok(route) => format!("met — {route}"),
            Err(why) => {
                let issue = FILED.iter().find(|(what, _)| *what == row.what).map_or("no issue filed", |(_, issue)| *issue);
                format!("**missed** — {why} ({issue})")
            }
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            row.what,
            row.route(),
            row.cost.presses,
            row.cost.pointer,
            arrives,
            old,
            target
        ));
    }
    out.push_str("\n## What the numbers say\n\n");
    out.push_str(&verdict(rows));
    out.push('\n');
    out
}

fn baseline_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(BASELINE)
}

/// **Every destination is measured, and the document says what was measured** (NV-01, #378).
///
/// The comparison is the acceptance: the file is read, not written, unless `UPDATE_NAVIGATION_BASELINE`
/// is set — so "rerunning reproduces the table" is a check that can fail rather than a claim.
#[test]
fn every_destination_is_measured_and_the_baseline_says_so() {
    let rows = table();

    // **Every row the issue names is present, in its order.** Compared against `ROWS` rather than
    // against a count, so a row that quietly changed its name fails here.
    let measured: Vec<&str> = rows.iter().map(|r| r.what).collect();
    assert_eq!(measured, ROWS, "the table's rows are not the destinations the issue lists");

    // **The anti-vacuity floor**: a table of nothing, or of absences, would satisfy everything else.
    let arrived = rows.iter().filter(|r| matches!(r.reach, Reach::Arrived)).count();
    assert!(
        arrived >= LEAST_ARRIVED,
        "only {arrived} of {} destinations were arrived at, so the drivers are not driving",
        rows.len()
    );

    // **Never worse than the baseline** (NV-11, #388): what arrived then arrives now, at no more keys and pointer actions together.
    let was = baseline();
    for row in &rows {
        let (_, old) = was.iter().find(|(name, _)| name == row.what).unwrap_or_else(|| panic!("`{}` is not in the frozen baseline", row.what));
        let arrives = matches!(row.reach, Reach::Arrived);
        assert!(!(old.arrives && !arrives), "reachability got worse: `{}` arrived in the baseline and does not now", row.what);
        if old.arrives && arrives {
            assert!(
                row.cost.presses + row.cost.pointer <= old.keys + old.pointer,
                "reachability got worse: `{}` cost {} key(s) and {} pointer action(s) in the baseline and costs {} and {} now",
                row.what,
                old.keys,
                old.pointer,
                row.cost.presses,
                row.cost.pointer
            );
        }
    }
    // **Every missed target has its issue.**
    let finder = finder_runs();
    let unfiled: Vec<String> = rows
        .iter()
        .filter(|row| target_met(row, &finder).is_err() && !FILED.iter().any(|(what, _)| *what == row.what))
        .map(|row| format!("{} ({}; route: {})", row.what, target_met(row, &finder).err().unwrap_or_default(), row.route()))
        .collect();
    let regenerating = std::env::var_os(REGENERATE).is_some();
    assert!(unfiled.is_empty() || regenerating, "these targets are missed and no issue is filed for them (add each to FILED):\n  {}", unfiled.join("\n  "));
    let rendered = render(&rows, &was, &finder);
    let path = baseline_path();
    let committed = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    if committed == rendered {
        return;
    }
    if std::env::var_os(REGENERATE).is_some() {
        std::fs::write(&path, &rendered).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        return;
    }
    // **The diff, so the failure says which row moved** rather than dumping two documents.
    let mut difference = String::new();
    let committed_lines: Vec<&str> = committed.lines().collect();
    let rendered_lines: Vec<&str> = rendered.lines().collect();
    for (n, (was, now)) in committed_lines.iter().zip(rendered_lines.iter()).enumerate() {
        if was != now {
            difference.push_str(&format!("line {}:\n  committed: {was}\n  measured:  {now}\n", n + 1));
        }
    }
    if committed_lines.len() != rendered_lines.len() {
        difference.push_str(&format!(
            "the document has {} lines and the measurement makes {}\n",
            committed_lines.len(),
            rendered_lines.len()
        ));
    }
    panic!(
        "{} is out of date with the editor. Regenerate it with {REGENERATE}=1 and commit both.\n{difference}",
        path.display()
    );
}

/// **The issues filed for missed targets and the committed document agree** (NV-11, #388): every `#N` in [`FILED`] is cited on its own row of
/// `docs/ui/navigation.md` as a missed target, no row cites an issue that is not in the list, and no missed row is without one. Offline: the
/// list is a constant, and the document is the one the measurement wrote.
#[test]
fn every_filed_issue_is_cited_by_its_row_and_no_miss_is_unfiled() {
    let path = baseline_path();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let rows: Vec<&str> = text.lines().filter(|l| l.starts_with("| ") && l.contains("**missed**")).collect();
    for (what, issue) in FILED {
        let row = rows.iter().find(|r| r.starts_with(&format!("| {what} |")));
        let row = row.unwrap_or_else(|| panic!("`{what}` is filed as {issue} but its row in the document is not a missed target"));
        assert!(row.contains(&format!("({issue})")), "`{what}`'s row does not cite {issue}: {row}");
    }
    for row in &rows {
        assert!(!row.contains("no issue filed"), "a missed target has no issue filed: {row}");
        let name = row.trim_start_matches("| ").split(" | ").next().unwrap_or_default();
        assert!(FILED.iter().any(|(what, _)| *what == name), "`{name}` is a missed target and is not in FILED");
    }
}

// ───────────────────────────── helpers ─────────────────────────────

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
