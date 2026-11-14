//! **The Design workspace: the tree, the rendered page and the board** (SG-05, #404; the series #399).
//!
//! `Cmd`/`Ctrl`+`0` opens a tenth workspace: the design tree on the left, the selected file rendered by gooey's `markdown_view` in the middle
//! with its pictures in place, and that place's dream board beside it as a card grid. **Edit** swaps the page for the file editor and **Done**
//! saves it and renders it again; a change on disk under unsaved typing offers Reload or Keep mine; "As an agent reads it" renders the chain
//! as `grimoire_style` serves it; Go to anything lands on a topic's name here; a dropped picture goes on the selected place's board.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_cauldron::design_view::{DesignView, Page};
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::keys::{Action, binding};
use bevy_cauldron::workspace::Workspace;
use bevy_gooey::{CardRef, CodeEditor, CodeEditorValue, MarkdownOutline, MarkdownPicture, MarkdownView};
use bevy_grimoire_core::board::{self, Board, Entry};
use bevy_grimoire_core::design::{self, Tree};
use bevy_grimoire_core::style;
use bevy_grimoire_core::trust::{Decision, TrustStore};

use crate::fixtures;

const SETTLE: usize = 14;
/// One frame of the test clock, in seconds: the file watchers run on the real clock, which the manual strategy drives.
const FRAME_S: f32 = 1.0 / 60.0;

const GAME: &str = "design/style.md";
const LEVEL: &str = "design/levels/m/style.md";
const FOG: &str = "design/levels/m/board/fog.png";
const DUSK: &str = "design/levels/m/board/dusk.png";
const GONE: &str = "design/levels/m/board/gone.png";
const LEVEL_TEXT: &str = "# Yard\n\n## Light\n\nSodium light on wet concrete.\n\n![the fog's colour, not its density](board/fog.png \"fog\")\n\n## Mood\n\nQuiet.\n\n![how the light leaves the roofs first](board/dusk.png)\n";

fn press(app: &mut App, action: Action) {
    let row = binding(action);
    let mut keys = vec![row.key];
    if row.needs_mod {
        keys.push(if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft });
    }
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for code in &keys {
            app.world_mut().write_message(KeyboardInput {
                key_code: *code,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        app.update();
        app.update();
    }
    // The editor's state is rebuilt by a workspace switch, so the person's store of this test is named again once Design is open.
    let store = app.world().resource::<bevy_cauldron::project::Project>().root.join("trusted.yaml");
    app.world_mut().resource_mut::<EditorState>().trust.file = Some(store);
    settle(app);
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn picture(root: &std::path::Path, rel: &str) {
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().expect("a folder")).unwrap_or_else(|e| panic!("{e}"));
    image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 120, 40, 255])).save(&full).unwrap_or_else(|e| panic!("{}: {e}", full.display()));
}

fn write(root: &std::path::Path, rel: &str, text: &str) {
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().expect("a folder")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, text).unwrap_or_else(|e| panic!("{e}"));
}

/// **A project with a design tree**: the game, the level `m`, a board of three pictures (one restricted, one whose file is not here) and a page citing two.
fn project(tag: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    write(&root, GAME, "# The game\n\nGrey concrete, sodium light.\n");
    write(&root, "design/dialogue.md", "# Dialogue\n\nShort lines.\n");
    write(&root, LEVEL, LEVEL_TEXT);
    picture(&root, FOG);
    picture(&root, DUSK);
    let mut level = Board::default();
    let mut fog = Entry::new(FOG, "the fog's colour, not its density");
    fog.licence = Some("CC BY 4.0".to_owned());
    level.add(fog, None).unwrap_or_else(|e| panic!("{e}"));
    let mut dusk = Entry::new(DUSK, "how the light leaves the roofs first");
    dusk.restricted = true;
    level.add(dusk, None).unwrap_or_else(|e| panic!("{e}"));
    level.add(Entry::new(GONE, "a picture whose file is not on this machine"), None).unwrap_or_else(|e| panic!("{e}"));
    level.save(&root.join(board::file_of("design/levels/m"))).unwrap_or_else(|e| panic!("{e}"));
    root
}

fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    // **A surface the size of a window**: on the harness's default one the workspace's widgets are laid out where a pointer cannot reach them.
    harness::resize_surface(&mut app, 1536, 1400).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME_S)));
    // **A trust store of the test's own**, so nothing here reads or writes the person's.
    let store = root.join("trusted.yaml");
    TrustStore::default().save(&store).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<EditorState>().trust.file = Some(store);
    settle(&mut app);
    app
}

/// **The row of the design tree that reads `label`**, when one does: the box a pointer is over when it is on that row.
///
/// **Why a tree row is not found by `find_by_label`** (measured, #404 follow-up): that helper answers with the nearest ancestor of the words that takes focus
/// (`TabIndex`), and the table's *rows* do not take focus, the table does. So `find_by_label("m")` is the whole tree (329 x 87 px, three rows), and a click
/// at its centre lands on the middle row, `dialogue`, which is where the click "on `m`" went. A click on the row itself selects `m`.
fn tree_row(app: &mut App, label: &str) -> Option<Entity> {
    let texts: Vec<Entity> = {
        let world = app.world_mut();
        world.query::<(Entity, &Text, &InheritedVisibility)>().iter(world).filter(|(_, text, shown)| text.0.trim() == label.trim() && shown.get()).map(|(e, ..)| e).collect()
    };
    let mut rows: Vec<Entity> = texts
        .into_iter()
        .filter_map(|text| std::iter::successors(Some(text), |e| app.world().get::<ChildOf>(*e).map(ChildOf::parent)).find(|e| app.world().get::<bevy_gooey::TableRow>(*e).is_some()))
        .collect();
    rows.sort();
    rows.dedup();
    match rows.as_slice() {
        [] => None,
        [one] => Some(*one),
        many => panic!("{} tree rows read `{label}`: {many:?}", many.len()),
    }
}

/// **Put the pointer over `entity`, as a mouse does**, when the app has a window.
///
/// With no window `bevy_gooey::testing::click` moves the pointer itself. Once one has been spawned (typing needs it), `surface::retarget_pointer` takes the pointer's
/// place from the window's cursor (`view::sense_pointer`) every frame and overwrites the helper's move, so the press lands where the *cursor* is and misses the widget
/// (measured: a click on `Done` hit nothing and the editor stayed open; a `CursorMoved` message does not move it, for it is not the window's state). The window's cursor
/// is set to the widget's centre, in physical pixels, which is what the helper's own move would have been.
fn put_the_pointer_over(app: &mut App, entity: Entity) {
    let window = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy::window::PrimaryWindow>>().iter(world).next()
    };
    let Some(window) = window else { return };
    let centre = app.world().get::<bevy::ui::UiGlobalTransform>(entity).unwrap_or_else(|| panic!("{entity} is laid out")).translation;
    app.world_mut().get_mut::<Window>(window).unwrap_or_else(|| panic!("the primary window")).set_physical_cursor_position(Some(centre.as_dvec2()));
    app.update();
}

/// **Click the one visible widget that reads `label`** with the pointer, where it is drawn: a tree row, a button, a toggle. Every click is a real one: the pointer
/// goes to the widget's centre and presses there, so a row that is laid out under another, or a button that is clipped, fails the test as it would fail a person.
fn click(app: &mut App, label: &str) {
    let entity = match tree_row(app, label) {
        Some(row) => row,
        None => bevy_gooey::testing::find_by_label(app, label).unwrap_or_else(|e| panic!("`{label}`: {e}")),
    };
    put_the_pointer_over(app, entity);
    bevy_gooey::testing::click(app, entity);
    settle(app);
}

fn visible(app: &mut App, label: &str) -> bool {
    bevy_gooey::testing::find_by_label(app, label).is_ok()
}

fn selected(app: &App) -> Option<String> {
    app.world().resource::<DesignView>().selected().map(str::to_owned)
}

fn page_source(app: &mut App) -> String {
    let world = app.world_mut();
    world.query::<&MarkdownView>().single(world).map(|v| v.source().to_owned()).unwrap_or_else(|e| panic!("one page is drawn: {e}"))
}

#[test]
fn cmd_0_opens_design_and_a_level_selected_by_its_label_shows_its_headings_pictures_and_board() {
    let root = project("design_page");
    let mut app = editor(&root);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Map, "the editor opens on the Map");
    press(&mut app, Action::WorkspaceDesign);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Design, "Cmd/Ctrl+0 is the Design workspace");
    assert_eq!(selected(&app).as_deref(), Some(GAME), "it opens on the game's page");

    // The level's row exists, labelled `m`.
    assert!(visible(&mut app, "m"), "the tree has a row labelled `m`");
    // The level, by the label of its row in the tree.
    click(&mut app, "m");
    assert_eq!(selected(&app).as_deref(), Some(LEVEL), "a click on the level's row selects its page");

    let world = app.world_mut();
    let headings: Vec<String> = world.query::<&MarkdownOutline>().single(world).map(|o| o.0.iter().map(|e| e.title.clone()).collect()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(headings, ["Yard", "Light", "Mood"], "the page's headings");
    let pictures: Vec<(String, bool)> = world.query::<(&MarkdownPicture, Has<ImageNode>)>().iter(world).map(|(p, image)| (p.dest().to_owned(), image)).collect();
    assert_eq!(pictures.len(), 2, "the page's two pictures: {pictures:?}");
    assert!(pictures.iter().all(|(_, image)| *image), "both pictures are image boxes, not the box a missing one is: {pictures:?}");

    // The board shows the same pictures with their captions, a licence line, a badge and a mark.
    let cards = world.query::<&CardRef>().iter(world).count();
    assert_eq!(cards, 3, "the board's three pictures are three cards");
    for words in [
        "the fog's colour, not its density",
        "how the light leaves the roofs first",
        "CC BY 4.0",
        "Restricted: not shipped with the game",
        "Missing on this machine",
    ] {
        assert!(visible(&mut app, words), "the board says `{words}`");
    }
}

#[test]
fn the_page_and_the_tree_readers_list_the_same_pictures_in_the_same_order_for_every_fixture_page() {
    let pages = [
        LEVEL_TEXT,
        "# A\n\n![one](a.png)\n\ntext ![two](sub/b.jpg \"t\") more\n\n[![three](c.png)](https://example.com)\n",
        "# ![in a heading](h.png)\n\n| a | b |\n|---|---|\n| ![in a cell](cell.png) | x |\n\n> ![in a quote](q.png)\n\n- ![in a list](l.png)\n",
        "![ref][r]\n\n[r]: via/ref.png \"titled\"\n\n```\n![not a picture](code.png)\n```\n",
        "no pictures at all\n",
    ];
    for page in pages {
        let ours: Vec<String> = design::pictures(page, "design/levels/m/style.md").into_iter().map(|p| p.cited).collect();
        let gooeys: Vec<String> = bevy_gooey::markdown::images(page).into_iter().map(|(_, dest, _)| dest).collect();
        assert_eq!(ours, gooeys, "the two readers disagree about the pictures of {page:?}");
    }
}

#[test]
fn edit_typing_and_done_write_the_file_and_render_the_new_text() {
    let root = project("design_edit");
    let mut app = editor(&root);
    press(&mut app, Action::WorkspaceDesign);
    click(&mut app, "m");
    click(&mut app, "Edit page");
    assert!(!visible(&mut app, "Edit page") && visible(&mut app, "Done"), "Edit swaps the page for the editor, and the button for Done");
    {
        let world = app.world_mut();
        assert!(world.query::<&MarkdownView>().iter(world).next().is_none(), "the page is taken away while the file editor holds the file");
        let editor = world.query_filtered::<Entity, With<CodeEditor>>().single(world).unwrap_or_else(|e| panic!("one editor: {e}"));
        world.resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    }
    settle(&mut app);
    // Typing needs a window to name; one is spawned after the first clicks. (A click after it puts the cursor over the widget first: see `put_the_pointer_over`.)
    app.world_mut().spawn((Window::default(), bevy::window::PrimaryWindow));
    bevy_gooey::testing::type_text(&mut app, "Dusk first. ");
    settle(&mut app);
    let typed = {
        let world = app.world_mut();
        world.query::<&CodeEditorValue>().single(world).map(|v| v.text().to_owned()).unwrap_or_else(|e| panic!("{e}"))
    };
    assert!(typed.contains("Dusk first. "), "the person typed in the editor: {typed:?}");
    assert_eq!(std::fs::read_to_string(root.join(LEVEL)).unwrap_or_default(), LEVEL_TEXT, "nothing is written until Done");

    click(&mut app, "Done");
    let on_disk = std::fs::read_to_string(root.join(LEVEL)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(on_disk, typed, "Done writes what was typed");
    assert_eq!(page_source(&mut app), typed, "and renders it again");
}

#[test]
fn a_change_on_disk_during_edit_offers_reload_or_keep_mine_and_overwrites_nothing() {
    let root = project("design_conflict");
    let mut app = editor(&root);
    press(&mut app, Action::WorkspaceDesign);
    click(&mut app, "m");
    click(&mut app, "Edit page");
    {
        let world = app.world_mut();
        let editor = world.query_filtered::<Entity, With<CodeEditor>>().single(world).unwrap_or_else(|e| panic!("one editor: {e}"));
        world.resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    }
    settle(&mut app);
    app.world_mut().spawn((Window::default(), bevy::window::PrimaryWindow));
    bevy_gooey::testing::type_text(&mut app, "Mine. ");
    settle(&mut app);

    // Someone else writes the file: the question is asked, and the disk is not touched by the editor.
    let theirs = "# Theirs\n\nWritten by an agent.\n";
    write(&root, LEVEL, theirs);
    for _ in 0..40 {
        app.update();
    }
    assert!(visible(&mut app, "Reload") && visible(&mut app, "Keep mine"), "a change on disk under unsaved typing asks Reload or Keep mine");
    assert_eq!(std::fs::read_to_string(root.join(LEVEL)).unwrap_or_default(), theirs, "the editor overwrote nothing");

    // Done is refused while the question stands, and writes nothing.
    click(&mut app, "Done");
    assert_eq!(std::fs::read_to_string(root.join(LEVEL)).unwrap_or_default(), theirs, "a save while the question is open writes nothing");
    assert!(app.world().resource::<EditorState>().status.problem_text().contains("Reload or Keep mine"), "and says why");

    click(&mut app, "Reload");
    assert!(!visible(&mut app, "Keep mine"), "answered");
    let held = {
        let world = app.world_mut();
        world.query::<&CodeEditorValue>().single(world).map(|v| v.text().to_owned()).unwrap_or_else(|e| panic!("{e}"))
    };
    assert_eq!(held, theirs, "Reload takes the disk's text");
}

#[test]
fn as_an_agent_reads_it_shows_the_chain_exactly_as_grimoire_style_returns_it() {
    let root = fixtures::Fixture::new(&format!("design_agent_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    write(&root, GAME, "# The game\n\nGrey concrete, sodium light.\n");
    write(&root, "design/regions/north/style.md", "# North\n\nSalt on everything.\n");
    write(&root, "design/regions/north/levels/m/style.md", "# The yard\n\nThe wall is the horizon.\n");
    let mut app = editor(&root);
    // The person has allowed the game's and the level's files, and not the region's: it is withheld, and the page shows the line an agent reads.
    let store = root.join("trusted.yaml");
    let mut trusted = TrustStore::default();
    for file in [GAME, "design/regions/north/levels/m/style.md"] {
        let text = std::fs::read_to_string(root.join(file)).unwrap_or_else(|e| panic!("{e}"));
        trusted.trust_text(&root, file, &text).unwrap_or_else(|e| panic!("{e}"));
    }
    trusted.save(&store).unwrap_or_else(|e| panic!("{e}"));

    press(&mut app, Action::WorkspaceDesign);
    click(&mut app, "m");
    assert_eq!(selected(&app).as_deref(), Some("design/regions/north/levels/m/style.md"));
    let toggle = bevy_gooey::testing::find_by_label(&mut app, "As an agent reads it").unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().commands().trigger(bevy_gooey::ValueChange { source: toggle, value: true, is_final: true });
    settle(&mut app);
    assert!(app.world().resource::<DesignView>().as_agent());

    let tree = Tree::read(&root);
    let mut decide = |path: &str, text: &str| -> Result<Decision, String> {
        TrustStore::load(&store).and_then(|s| s.text_decision(&root, path, text))
    };
    let served = style::compose(&tree, &style::Request { map: Some("m".to_owned()), at: None, topic: None }, &mut decide).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(page_source(&mut app), served.text, "the page is the text grimoire_style returns for this level");
    let order: Vec<&str> = served.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(order, [GAME, "design/regions/north/style.md", "design/regions/north/levels/m/style.md"], "the game's, region's and level's files, in the order the chain is read");
    assert!(served.text.contains("waiting for the person"), "the withheld region file is the line an agent is told, not its text");
    assert!(!served.text.contains("Salt on everything"), "and its text is not in the page");
    assert!(matches!(app.world().resource::<DesignView>().snapshot().page, Page::Agent(_)));
}

#[test]
fn go_to_anything_on_a_topics_name_opens_it_here_and_a_style_link_reveal_opens_the_level() {
    use bevy_cauldron::goto::{GotoIndex, Kind, show_in_world};
    let root = project("design_goto");
    let mut app = editor(&root);
    let topic = app.world().resource::<GotoIndex>().of_kind(Kind::Design).find(|e| e.title == "dialogue").cloned().unwrap_or_else(|| panic!("`dialogue` is a row of the finder"));
    show_in_world(app.world_mut(), &topic);
    settle(&mut app);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Design, "a topic of the game opens in the Design workspace");
    assert_eq!(selected(&app).as_deref(), Some("design/dialogue.md"));
    assert!(app.world().resource::<GotoIndex>().of_kind(Kind::Design).any(|e| e.title == "m"), "every place is a row too");

    // The Map inspector's Style link asks the reveal for the level's page; it lands here.
    app.world_mut().resource_mut::<bevy_cauldron::reveal::Revealing>().request_by_person("style_link", &bevy_cauldron::ops::Shown::File { path: LEVEL.to_owned(), lines: None });
    settle(&mut app);
    assert_eq!(selected(&app).as_deref(), Some(LEVEL));
}

/// **`grimoire_state` as an agent reads it**, serialized.
fn state_text(app: &mut App) -> String {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/state").cloned() else {
        panic!("grimoire/state is not registered");
    };
    app.world_mut().run_system_with(id, Some(serde_json::json!({}))).unwrap_or_else(|e| panic!("{e:?}")).unwrap_or_else(|e| panic!("{e:?}")).to_string()
}

fn link(app: &mut App, dest: &str, kind: bevy_gooey::markdown::LinkKind) {
    let view = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<MarkdownView>>().single(world).unwrap_or_else(|e| panic!("one page: {e}"))
    };
    app.world_mut().commands().trigger(bevy_gooey::MarkdownLink { entity: view, dest: dest.to_owned(), kind });
    settle(app);
}

const NOT_OPENED_LEAVES: &str = "NOT OPENED: a link in the page leaves the project";
const NOT_OPENED_SCHEME: &str = "NOT OPENED: a link in the page is not an http or https address, so nothing was opened";

#[test]
fn a_links_destination_is_never_in_what_an_agent_reads() {
    use bevy_gooey::markdown::LinkKind;
    let root = project("design_marker");
    write(&root, LEVEL, "# Yard\n\n[where](../../../../MARKER_7f3a91_secret/x.md)\n");
    let mut app = editor(&root);
    press(&mut app, Action::WorkspaceDesign);
    click(&mut app, "m");
    link(&mut app, "../../../../MARKER_7f3a91_secret/x.md", LinkKind::Relative);
    assert_eq!(app.world().resource::<EditorState>().status.problem_text(), NOT_OPENED_LEAVES, "the refusal is a fixed sentence");
    link(&mut app, "MARKER_7f3a91_secret:prose that is not an address", LinkKind::Web);
    let state = state_text(&mut app);
    assert!(!state.contains("MARKER_7f3a91"), "a page's link destination reached grimoire_state");
}

#[test]
fn only_http_and_https_links_are_asked_about_and_nothing_else_is_launched() {
    use bevy_cauldron::app_verbs::LinksOff;
    use bevy_cauldron::confirm::{Asked, Confirm};
    use bevy_gooey::markdown::LinkKind;
    let root = project("design_schemes");
    let mut app = editor(&root);
    press(&mut app, Action::WorkspaceDesign);
    click(&mut app, "m");
    for dest in ["file:///Applications/Calculator.app", "smb://x/y", "javascript:alert(1)", "mailto:a@b.c", "x-custom-handler://run?cmd=1", "https:example.com", "http:/x"] {
        link(&mut app, dest, LinkKind::Web);
        assert!(!app.world().resource::<Confirm>().asking(Asked::OpenAddress), "`{dest}` was asked about");
        assert_eq!(app.world().resource::<EditorState>().status.problem_text(), NOT_OPENED_SCHEME, "`{dest}` is refused with a fixed sentence");
        assert!(app.world().resource::<LinksOff>().0.is_empty(), "`{dest}` was launched");
    }
    link(&mut app, "https://example.com/style", LinkKind::Web);
    assert!(app.world().resource::<Confirm>().asking(Asked::OpenAddress), "an https link still asks");
    assert!(app.world().resource::<LinksOff>().0.is_empty(), "and nothing opens before the person answers");
}

#[test]
fn an_uppercase_http_scheme_is_asked_about() {
    use bevy_cauldron::confirm::{Asked, Confirm};
    use bevy_gooey::markdown::LinkKind;
    let root = project("design_upper");
    let mut app = editor(&root);
    press(&mut app, Action::WorkspaceDesign);
    click(&mut app, "m");
    link(&mut app, "HTTP://example.com/x", LinkKind::Web);
    assert!(app.world().resource::<Confirm>().asking(Asked::OpenAddress), "a scheme is case-insensitive, so HTTP:// asks as http:// does");
}

#[test]
fn a_picture_with_more_pixels_than_the_cap_is_not_decoded() {
    let root = project("design_bomb");
    image::RgbaImage::from_pixel(4100, 4100, image::Rgba([1, 2, 3, 255])).save(root.join("big.png")).unwrap_or_else(|e| panic!("{e}"));
    let app = editor(&root);
    let pictures = app.world().resource::<bevy_cauldron::design_view::DesignPictures>().clone();
    let server = app.world().resource::<AssetServer>().clone();
    assert!(pictures.load(&server, &root, "big.png").is_none(), "a 4100 x 4100 picture is over the cap and is not decoded");
    assert!(pictures.load(&server, &root, FOG).is_some(), "a small one is");
}
#[test]
fn a_problem_is_shown_above_the_page_it_belongs_to() {
    let root = project("design_problem");
    write(&root, LEVEL, "# Yard\n\n![nothing here](board/nothing.png)\n");
    let mut app = editor(&root);
    press(&mut app, Action::WorkspaceDesign);
    click(&mut app, "m  (1 problem)");
    let problems = Tree::read(&root).problems;
    assert_eq!(problems.len(), 1, "{problems:?}");
    let line = problems[0].line();
    assert!(visible(&mut app, &line), "the page's problem is above it: `{line}`");
}

#[test]
fn a_dropped_picture_goes_on_the_selected_places_board_and_the_levels_board_is_left() {
    let root = project("design_drop");
    let mut app = editor(&root);
    press(&mut app, Action::WorkspaceDesign);
    assert_eq!(selected(&app).as_deref(), Some(GAME));
    picture(&root, "art/hall.png");
    app.world_mut().write_message(bevy::window::FileDragAndDrop::DroppedFile { window: Entity::PLACEHOLDER, path_buf: root.join("art/hall.png") });
    settle(&mut app);
    let game = Board::of_place(&root, "design").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(game.pictures.iter().map(|p| p.path.as_str()).collect::<Vec<_>>(), ["art/hall.png"], "the drop is on the game's board");
    let level = Board::of_place(&root, "design/levels/m").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(level.pictures.len(), 3, "the level's board is as it was");
}

#[test]
fn clicking_a_picture_lights_its_card_and_choosing_a_card_scrolls_the_page_to_the_picture() {
    let root = project("design_linked");
    // A long page, so the second picture is below the fold.
    let filler: String = (0..60).map(|n| format!("Line {n} of the level's feel, written long enough to take a row of its own.\n\n")).collect();
    write(&root, LEVEL, &format!("# Yard\n\n![the fog's colour, not its density](board/fog.png)\n\n{filler}![how the light leaves the roofs first](board/dusk.png)\n"));
    let mut app = editor(&root);
    press(&mut app, Action::WorkspaceDesign);
    click(&mut app, "m");
    // Picture 1 (0-based) is the dusk one: its click lights the second card.
    bevy_cauldron::design_view::pick_picture(app.world_mut(), 1);
    settle(&mut app);
    let lit: Vec<usize> = {
        let world = app.world_mut();
        world.query::<(&CardRef, Has<bevy::ui::Selected>)>().iter(world).filter(|(_, lit)| *lit).map(|(c, _)| c.index).collect()
    };
    assert_eq!(lit, [1], "the card of the second picture is lit");

    // Choosing the first card scrolls the page to the first picture; choosing the second scrolls to the one far down.
    let grid = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy_cauldron::design_view::DesignCards>>().single(world).unwrap_or_else(|e| panic!("{e}"))
    };
    app.world_mut().commands().trigger(bevy::ui_widgets::ValueChange { source: grid, value: 1usize, is_final: true });
    for _ in 0..30 {
        app.update();
    }
    let scrolled = {
        let world = app.world_mut();
        world.query_filtered::<&bevy::ui::ScrollPosition, With<bevy_gooey::MarkdownViewport>>().single(world).map(|s| s.0.y).unwrap_or_else(|e| panic!("{e}"))
    };
    assert!(scrolled > 0.0, "the page scrolled to the picture far down it (scroll {scrolled})");
}
