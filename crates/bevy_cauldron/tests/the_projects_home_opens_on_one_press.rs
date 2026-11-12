//! **A press on a project card opens it** (feedback-1794103491165, #344).
//!
//! From a person's capture: *"I have to click on this twice, why?"*, pointing at a card that already
//! wore the accent rail that says it is chosen, with nothing opened. Measured on the installed
//! Cauldron 0.5.0: one left press left `grimoire/state.screen` at `launcher`, a second made it
//! `project`.
//!
//! Every other control on the home — `New project`, `Open folder…`, `Open the example`, `Take the
//! tour` — acts on one press, and so does a card pressed from the keyboard. So a press opens, and the
//! rail is what says *this one* for a card that has the keyboard or the pointer.
//!
//! # What is proved here
//!
//! 1. **One press opens** — the window is pointed at that folder and the screen moves on, on the same
//!    press.
//! 2. **The keyboard route is unchanged**: a card that has the keyboard opens on a press too, which
//!    it already did.
//! 3. **The rail is the keyboard's, not a second selection**: the card with input focus wears
//!    `Selected`, and no other card does — a rail that outlived the focus would be the invisible
//!    state the two-press step was built on.
//! 4. **A second press does not open it twice.** The two-press step's second half is gone, and the
//!    guard it used to carry went with it, so this pins the thing that guard was hiding: pressing a
//!    card twice quickly queues **one** open, not two, and the home does not carry the second press
//!    into a second crossing.

use std::path::{Path, PathBuf};

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::ui::Selected;
use bevy::ui_widgets::Activate;
use bevy_cauldron::launcher::{LauncherPlugin, ProjectCard};
use bevy_cauldron::project_assets::{PointedAt, ProjectRoot};
use bevy_cauldron::recent::{self, RecentProjects};
use bevy_cauldron::screen::Screen;

use crate::fixtures;

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("grimoire_one_press_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A project folder named `name` under `dir`: `assets/grimoire` is all the launcher asks for.
fn project(dir: &Path, name: &str) -> PathBuf {
    let root = dir.join(name);
    std::fs::create_dir_all(root.join("assets/grimoire")).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
    root.canonicalize().unwrap_or_else(|e| panic!("{e}"))
}

/// An editor app on the Projects home, whose recent list holds `projects` (the last one opened first).
fn on_the_home(tag: &str, scratch: &Scratch, projects: &[&Path]) -> App {
    let root = fixtures::Fixture::new(&format!("onepress_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let file = scratch.0.join("recent.ron");
    for p in projects {
        recent::record(&file, p).unwrap_or_else(|e| panic!("{e}"));
    }
    app.insert_resource(RecentProjects(file)).add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..4 {
        app.update();
    }
    app
}

/// The card whose folder is `path`.
fn card_of(app: &mut App, path: &Path) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &ProjectCard)>();
    q.iter(world)
        .find(|(_, card)| card.path == path)
        .map(|(entity, _)| entity)
        .unwrap_or_else(|| panic!("no card for {}", path.display()))
}

fn press(app: &mut App, card: Entity) {
    app.world_mut().commands().trigger(Activate { entity: card });
    app.update();
    app.update();
}

fn screen_of(app: &App) -> Screen {
    *app.world().resource::<State<Screen>>().get()
}

/// The folder the window is pointed at, which is what "opens that project" means in this process.
fn pointed_at(app: &App) -> Option<PathBuf> {
    app.world().get_resource::<PointedAt>().map(|at| at.root.clone())
}

/// **1. One press opens the project the card names.**
#[test]
fn one_press_on_a_card_opens_that_project() {
    let scratch = Scratch::new("open");
    let (alpha, beta) = (project(&scratch.0, "alpha"), project(&scratch.0, "beta"));
    let mut app = on_the_home("open", &scratch, &[&alpha, &beta]);
    assert_eq!(screen_of(&app), Screen::Launcher, "the home is up, and nothing has been opened");

    let card = card_of(&mut app, &beta);
    press(&mut app, card);

    assert_eq!(
        pointed_at(&app),
        Some(beta.clone()),
        "one press on a card opens that folder, as every other control on the home acts on one press"
    );
    assert_ne!(screen_of(&app), Screen::Launcher, "and the screen moves on");
    // The list follows the real `ProjectRoot` too, so a second open of the same folder cannot be made
    // necessary by the home's own bookkeeping.
    assert_eq!(
        app.world().get_resource::<ProjectRoot>().and_then(|r| r.current()),
        Some(beta.clone()),
        "the process's own project root is the folder the press named"
    );
}

/// **2. A press from the keyboard opens too**, which is the route that already worked — kept so the
/// change cannot be "one press for the pointer, still two for the hand on the arrows".
#[test]
fn a_press_from_the_keyboard_opens_the_card_with_focus() {
    let scratch = Scratch::new("keyboard");
    let (alpha, beta) = (project(&scratch.0, "alpha"), project(&scratch.0, "beta"));
    let mut app = on_the_home("keyboard", &scratch, &[&alpha, &beta]);
    let card = card_of(&mut app, &alpha);
    app.world_mut().resource_mut::<InputFocus>().set(card, FocusCause::Navigated);
    for _ in 0..2 {
        app.update();
    }
    press(&mut app, card);
    assert_eq!(pointed_at(&app), Some(alpha.clone()), "a card reached by Tab or an arrow opens on a press");
}

/// **3. The rail says which card has the keyboard**, and only that one: the two-press step's
/// invisible `Launcher::selected` is gone, so the mark on screen is the focus itself.
#[test]
fn the_rail_wears_the_card_with_the_keyboard() {
    let scratch = Scratch::new("rail");
    let (alpha, beta) = (project(&scratch.0, "alpha"), project(&scratch.0, "beta"));
    let mut app = on_the_home("rail", &scratch, &[&alpha, &beta]);

    let marked = |app: &mut App| -> Vec<Entity> {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, (With<ProjectCard>, With<Selected>)>();
        q.iter(world).collect()
    };
    let beta_card = card_of(&mut app, &beta);
    assert!(marked(&mut app).is_empty(), "no card is chosen before anything reaches one");

    app.world_mut().resource_mut::<InputFocus>().set(beta_card, FocusCause::Navigated);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(marked(&mut app), vec![beta_card], "the card with the keyboard wears the rail");

    let alpha_card = card_of(&mut app, &alpha);
    app.world_mut().resource_mut::<InputFocus>().set(alpha_card, FocusCause::Navigated);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(marked(&mut app), vec![alpha_card], "and the rail moves with it, leaving nothing behind");
}

/// **4. Pressing a card twice opens the project once**, in one frame and across two.
///
/// Before #344 a first press chose and a second opened, so a repeat could only reach "chosen again".
/// Now both presses name an open, and the guard the old step carried went with it — so what a repeat
/// does has to be pinned rather than assumed. Counted at the crossing itself, not at the request:
/// `open_requested` reads `.last()`, so two `OpenRequest`s in one frame would still be one crossing
/// and a count of messages would say otherwise about the same picture.
#[test]
fn a_second_press_on_the_same_card_opens_it_once() {
    let scratch = Scratch::new("twice");
    let (alpha, beta) = (project(&scratch.0, "alpha"), project(&scratch.0, "beta"));
    let mut app = on_the_home("twice", &scratch, &[&alpha, &beta]);
    let card = card_of(&mut app, &beta);

    // **Every time the editor crosses into a project**, read off the state machine rather than
    // inferred: a second crossing is invisible in `PointedAt`, which would be set to the same folder.
    #[derive(Resource, Default)]
    struct Crossings(Vec<PathBuf>);
    app.init_resource::<Crossings>();
    app.add_systems(
        Update,
        |mut crossings: ResMut<Crossings>, screen: Res<State<Screen>>, at: Option<Res<PointedAt>>| {
            if screen.is_changed() && *screen.get() == Screen::Project
                && let Some(at) = at
            {
                crossings.0.push(at.root.clone());
            }
        },
    );

    // **Both presses before a frame runs**, which is what a person clicking twice quickly does.
    app.world_mut().commands().trigger(Activate { entity: card });
    app.world_mut().commands().trigger(Activate { entity: card });
    for _ in 0..6 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<Crossings>().0,
        vec![beta.clone()],
        "the card was pressed twice and the editor crossed {:?}, so it opened the project more than once",
        app.world().resource::<Crossings>().0
    );

    // **And a press that lands after the crossing** is on a card the home no longer draws: the entity
    // is despawned with it, so the trigger reaches nothing and the editor stays where it is.
    app.world_mut().commands().trigger(Activate { entity: card });
    for _ in 0..6 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<Crossings>().0,
        vec![beta.clone()],
        "a press after the crossing opened the project again"
    );
    assert_eq!(pointed_at(&app), Some(beta), "and the folder is the one the card named");
    assert_eq!(screen_of(&app), Screen::Project, "the editor is on it");
}
