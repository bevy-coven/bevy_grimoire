//! **Every panel header carries a "?" that opens that panel's page of the guide** (UI-91, #257).
//!
//! The written guide is `docs/guide/`, one page a subject. The Help menu lists every page, and a panel's header carries the
//! one page that covers what the panel holds, so the question a person has while looking at the panel opens the answer
//! instead of a browser search.
//!
//! **The list is data, and the tests keep it honest in both directions**: a page the guide has and the menu does not is a
//! page nobody can find, a heading the editor draws and the catalog does not name is a panel with no "?", and a catalog entry
//! whose file is gone is a "?" that opens a 404.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_cauldron::app_verbs::{AppVerb, LinksOff};
use bevy_cauldron::help::{self, PanelHelp};
use bevy_cauldron::menu_bar::EditorMenuBar;
use bevy_gooey::Activate;

use crate::fixtures;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<LinksOff>();
    updates(&mut app, 8);
    app
}

fn source(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn guide_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/guide")
}

/// Every heading the editor draws at the top of a panel: `chrome::title` for a left dock and `chrome::list_heading_in` for a
/// right one. Read from the source, so a panel added tomorrow is not silently without a "?".
fn drawn_panel_headings() -> Vec<String> {
    let mut found = Vec::new();
    for file in ["anim_tab.rs", "compose.rs", "editor.rs", "tiles.rs"] {
        for line in source(file).lines() {
            let trimmed = line.trim();
            let rest = trimmed
                .strip_prefix("crate::chrome::title(p, \"")
                .or_else(|| trimmed.strip_prefix("crate::chrome::list_heading_in(p, \""));
            if let Some(rest) = rest
                && let Some(word) = rest.split('"').next()
            {
                found.push(word.to_owned());
            }
        }
    }
    assert!(found.len() >= 5, "the panels' headings are read from the source: found {found:?}");
    found
}

#[test]
fn every_page_is_a_file_the_guides_index_lists() {
    let index = std::fs::read_to_string(guide_dir().join("README.md")).expect("the guide has an index");
    for page in help::PAGES {
        let path = guide_dir().join(page.file);
        assert!(path.exists(), "{} is listed in the Help menu and there is no such page", path.display());
        assert!(index.contains(page.file), "{} is not in docs/guide/README.md, so only the menu finds it", page.file);
        assert!(page.title.trim().ends_with(|c: char| !c.is_whitespace()), "{}'s title has trailing space", page.id);
    }
    // **And nothing in the directory is left out**: a page the menu cannot reach is a page only a
    // person who already knows the repository will read.
    let mut on_disk: Vec<String> = std::fs::read_dir(guide_dir())
        .expect("the guide is a directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".md") && name != "README.md")
        .collect();
    for page in help::PAGES {
        on_disk.retain(|name| name != page.file);
    }
    assert!(on_disk.is_empty(), "these pages are in docs/guide and no Help row opens them: {on_disk:?}");

    let mut ids: Vec<&str> = help::PAGES.iter().map(|p| p.id).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "two pages share an id");
}

#[test]
fn every_panel_the_editor_draws_has_a_page_and_every_page_opened_is_that_panels() {
    let headings = drawn_panel_headings();
    for heading in &headings {
        assert!(
            help::page_for_panel(heading).is_some(),
            "the editor draws a `{heading}` panel heading and the guide has no page for it, so that header would have no \"?\""
        );
    }
    let mut app = editor(&format!("panel_help_{}", std::process::id()));
    let buttons: Vec<(Entity, String)> = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &PanelHelp)>();
        q.iter(world).map(|(e, help)| (e, help.page.to_owned())).collect()
    };
    assert!(
        buttons.len() >= 2,
        "the Map door's two panels each carry a \"?\" on their heading: found {}",
        buttons.len()
    );
    for (entity, page) in buttons {
        let page = help::page(&page).unwrap_or_else(|| panic!("{page} is not a page of the guide"));
        let url = help::url(page);
        app.world_mut().commands().trigger(Activate { entity });
        updates(&mut app, 2);
        assert!(
            app.world().resource::<LinksOff>().0.contains(&url),
            "the \"?\" on a panel opens that panel's page ({url}); the run asked for {:?}",
            app.world().resource::<LinksOff>().0
        );
    }
}

#[test]
fn the_help_menu_lists_every_page_and_a_click_opens_it() {
    let mut app = editor(&format!("panel_help_menu_{}", std::process::id()));
    let rows: Vec<(Entity, String)> = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &AppVerb)>();
        q.iter(world)
            .filter_map(|(e, verb)| match verb {
                AppVerb::GuidePage(id) => Some((e, (*id).to_owned())),
                _ => None,
            })
            .collect()
    };
    assert_eq!(rows.len(), help::PAGES.len(), "Help lists one row for every page of the guide, and no row for anything else");
    for (entity, id) in rows {
        let page = help::page(&id).unwrap_or_else(|| panic!("the menu offers `{id}` and no page has it"));
        let title = page.title;
        // The row's words are the page's title: a menu is closed almost all the time, so this reads the
        // row's own label rather than asking the screen where it is drawn.
        assert_eq!(AppVerb::GuidePage(page.id).title(), title, "the row for {} says what the page is called", page.file);
        app.world_mut().commands().trigger(Activate { entity });
        updates(&mut app, 2);
        let url = help::url(page);
        assert!(app.world().resource::<LinksOff>().0.contains(&url), "choosing {title} opens {url}");
    }
    // The bar the rows were read from is the one on screen, not a second copy of it.
    assert_eq!(app.world_mut().query_filtered::<(), With<EditorMenuBar>>().iter(app.world()).count(), 1);
}

#[test]
fn a_panel_without_a_page_carries_no_button() {
    assert!(help::page_for_panel("NOT A PANEL").is_none(), "a heading nobody mapped gets no page");
    assert!(help::page("no.such.page").is_none());
}
