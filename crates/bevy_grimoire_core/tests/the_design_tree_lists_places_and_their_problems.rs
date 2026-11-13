//! **The design tree: places, topics, and what is wrong with them** (SG-01, #400; the series #399).
//!
//! A game's style lives in `design/` at the project root, one folder per place: the whole game, each
//! region, each level (a folder named after its map) and each area of a level (a rectangle on that map).
//! `bevy_grimoire_core::design::Tree::read` walks it and says what is there and what is wrong, because a
//! tree nobody can check is a tree an agent reads half of and a person cannot see the fault in.
//!
//! The fixture is written on disk here rather than shipped, so this file is the whole of what a tree is:
//! a game with one region, two levels (one inside the region, one in no region) and an area — plus, in the
//! second half, one of every fault the reader reports.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::design::{self, PlaceKind, STYLE_CHAIN_MAX_BYTES, STYLE_FILE_MAX_BYTES};

/// A scratch project of this test's own, named for the test and the process.
struct Project(PathBuf);

impl Project {
    fn new(tag: &str) -> Project {
        let dir = std::env::temp_dir().join(format!("grimoire-design-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("design")).expect("the design folder is made");
        Project(dir)
    }

    fn write(&self, rel: &str, text: &str) -> PathBuf {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap_or(&self.0)).expect("the file's folder is made");
        std::fs::write(&path, text).expect("the file is written");
        path
    }

    fn map(&self, name: &str, bounds: (f32, f32)) -> PathBuf {
        let map = bevy_grimoire_core::map::Map {
            name: name.to_owned(),
            bounds: (bounds.0, 1.0, bounds.1),
            ..bevy_grimoire_core::map::Map::default()
        };
        let text = bevy_grimoire_core::yaml::to_string(&map).unwrap_or_else(|p| panic!("{}", p.line()));
        self.write(&format!("assets/grimoire/maps/{name}.map.yaml"), &text)
    }

    fn tree(&self) -> design::Tree {
        design::Tree::read(&self.0)
    }
}

/// **The fixture of #400's first acceptance item**: a game, one region, two levels and an area.
fn game(tag: &str) -> Project {
    let project = Project::new(tag);
    project.map("yard", (32.0, 32.0));
    project.map("harbour", (64.0, 48.0));
    // The whole game: its overall feel, and one topic.
    project.write("design/style.md", "# The game\n\nGrey concrete, sodium light.\n");
    project.write("design/dialogue.md", "Short sentences. Nobody explains themselves.\n");
    // A region, with a topic of its own beyond its feel.
    project.write("design/regions/north/style.md", "Salt on everything.\n");
    project.write("design/regions/north/dialogue.md", "They talk about the weather first.\n");
    project.write("design/regions/north/region.yaml", "title: The north shore\n");
    // A level inside the region: the folder is named after the map.
    project.write("design/regions/north/levels/harbour/style.md", "The harbour wall is the horizon.\n");
    project.write("design/regions/north/levels/harbour/dialogue.md", "Dock talk, clipped.\n");
    // Its area, with a rectangle in map metres.
    project.write("design/regions/north/levels/harbour/areas/docks/area.yaml", "version: 1\nrect: [-20.0, -10.0, -4.0, 8.0]\n");
    project.write("design/regions/north/levels/harbour/areas/docks/style.md", "Cranes, oil, rope.\n");
    // A level in no region.
    project.write("design/levels/yard/style.md", "A yard of gravel and puddles.\n");
    project
}

#[test]
fn the_tree_lists_every_place_with_its_topics_and_an_areas_rectangle_in_metres() {
    let project = game("fixture");
    let tree = project.tree();
    let paths: Vec<(PlaceKind, &str)> = tree.places.iter().map(|p| (p.kind, p.path.as_str())).collect();
    assert_eq!(
        paths,
        vec![
            (PlaceKind::Game, "design"),
            (PlaceKind::Region, "design/regions/north"),
            (PlaceKind::Level, "design/regions/north/levels/harbour"),
            (PlaceKind::Area, "design/regions/north/levels/harbour/areas/docks"),
            (PlaceKind::Level, "design/levels/yard"),
        ],
        "every place, the whole game first"
    );
    assert!(tree.problems.is_empty(), "the fixture is a sound tree: {:?}", tree.problems.iter().map(|p| p.line()).collect::<Vec<_>>());

    let game_place = tree.place("design").expect("the game is a place");
    let topics: Vec<(&str, usize)> = game_place.topics.iter().map(|t| (t.name.as_str(), t.bytes)).collect();
    assert_eq!(topics.len(), 2, "the game states two topics: {topics:?}");
    assert_eq!(topics[0].0, design::STYLE_TOPIC, "`style` comes first, because that is how the chain is written");
    assert_eq!(topics[1].0, "dialogue");
    assert_eq!(topics[0].1, "# The game\n\nGrey concrete, sodium light.\n".len(), "a topic carries its size");

    let area = tree.place("design/regions/north/levels/harbour/areas/docks").expect("the area is a place");
    assert_eq!(area.rect, Some([-20.0, -10.0, -4.0, 8.0]), "an area's rectangle is map metres, as `area.yaml` writes it");
    assert_eq!(area.map.as_deref(), Some("harbour"), "an area names its level's map");
    assert_eq!(area.topics.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), vec!["style"]);

    // **The chain one point is read with**: the game, its region, its level, and the area that holds it.
    let chain: Vec<&str> = tree.chain_to(Some("harbour"), Some((-10.0, 0.0))).iter().map(|p| p.path.as_str()).collect();
    assert_eq!(
        chain,
        vec![
            "design",
            "design/regions/north",
            "design/regions/north/levels/harbour",
            "design/regions/north/levels/harbour/areas/docks",
        ],
        "the most specific place comes last"
    );
    // A point outside the area is read with its level and no area.
    let outside: Vec<&str> = tree.chain_to(Some("harbour"), Some((10.0, 0.0))).iter().map(|p| p.path.as_str()).collect();
    assert_eq!(outside, vec!["design", "design/regions/north", "design/regions/north/levels/harbour"]);
    // A level in no region has a chain of two.
    let bare: Vec<&str> = tree.chain_to(Some("yard"), None).iter().map(|p| p.path.as_str()).collect();
    assert_eq!(bare, vec!["design", "design/levels/yard"]);
}

#[test]
fn every_fault_is_reported_once_with_its_path_and_nothing_is_truncated() {
    let project = game("faults");
    // A topic below the game with no game-level counterpart.
    project.write("design/regions/north/sound.md", "Gulls, and the wind through a fence.\n");
    // Two areas of one level that overlap, and an area outside its map.
    project.write("design/regions/north/levels/harbour/areas/quay/area.yaml", "version: 1\nrect: [-6.0, 0.0, 2.0, 20.0]\n");
    project.write("design/regions/north/levels/harbour/areas/far/area.yaml", "version: 1\nrect: [100.0, 0.0, 120.0, 20.0]\n");
    // A level folder whose map does not exist.
    project.write("design/levels/harbour/style.md", "A second folder for the harbour's map.\n");
    project.write("design/levels/nowhere/style.md", "No map is called this.\n");
    // A file one byte over its limit.
    let over = format!("{}\n", "x".repeat(STYLE_FILE_MAX_BYTES));
    assert_eq!(over.len(), STYLE_FILE_MAX_BYTES + 1, "the file is exactly one byte over the limit");
    project.write("design/levels/yard/ui.md", &over);
    // A picture that is not there, on a line of its own.
    project.write(
        "design/levels/yard/style.md",
        "A yard of gravel and puddles.\n\n![the fog's colour, not its density](board/fog.jpg)\n",
    );

    let tree = project.tree();
    let said: Vec<String> = tree.problems.iter().map(|p| p.line()).collect();
    let about = |needle: &str| -> Vec<&String> { said.iter().filter(|line| line.contains(needle)).collect() };

    // 1. A topic below the game with no game-level file: one problem, naming the missing file.
    let topic = about("design/sound.md");
    assert_eq!(topic.len(), 1, "one problem for the topic, naming the file the game is missing: {said:#?}");
    assert!(topic[0].contains("sound"), "the problem names the topic: {}", topic[0]);

    // 2. Two overlapping areas: one problem, naming both.
    let overlap = about("overlap");
    assert_eq!(overlap.len(), 1, "one problem for the pair: {said:#?}");
    assert!(overlap[0].contains("areas/docks") && overlap[0].contains("areas/quay"), "it names both areas: {}", overlap[0]);

    // 3. An area outside its map: one problem, naming the area.
    let outside = about("outside its map");
    assert_eq!(outside.len(), 1, "one problem for the area that leaves the map: {said:#?}");
    assert!(outside[0].contains("areas/far"), "it names the area: {}", outside[0]);

    // 4. A level folder with no map, and a map with a folder in two places: each named with both paths.
    let no_map = about("design/levels/nowhere");
    assert_eq!(no_map.len(), 1, "one problem for the level whose map does not exist: {said:#?}");
    assert!(no_map[0].contains("nowhere.map.yaml"), "it names the map file it looked for: {}", no_map[0]);
    let twice = about("design/levels/harbour");
    assert_eq!(twice.len(), 1, "one problem for the map with a folder both places: {said:#?}");
    assert!(
        twice[0].contains("design/regions/north/levels/harbour"),
        "it names the folder the map already has: {}",
        twice[0]
    );

    // 5. A file over `STYLE_FILE_MAX_BYTES`: named with its path and its size, and not truncated.
    let big = about(&format!("is {} bytes", STYLE_FILE_MAX_BYTES + 1));
    assert_eq!(big.len(), 1, "one problem for the file that is too large: {said:#?}");
    assert!(
        big[0].contains(&(STYLE_FILE_MAX_BYTES + 1).to_string()),
        "it names the size, because \"too large\" is not a number a person can act on: {}",
        big[0]
    );
    assert_eq!(tree.text("design/levels/yard/ui.md"), None, "a file over the limit is refused whole, never truncated");
    // The topic is still a row of the place, with the size it has, so the tree lists what is there.
    let yard = tree.place("design/levels/yard").expect("the yard is a place");
    let ui = yard.topic("ui").expect("the refused topic is still listed");
    assert_eq!(ui.bytes, STYLE_FILE_MAX_BYTES + 1);

    // 6. A picture that is not there: the file, the line and the path.
    let picture = about("board/fog.jpg");
    assert_eq!(picture.len(), 1, "one problem for the missing picture: {said:#?}");
    assert!(picture[0].contains("design/levels/yard/style.md"), "it names the file: {}", picture[0]);
    assert!(picture[0].contains("line 3"), "it names the line: {}", picture[0]);

    // **And each fault is reported once.** Five of them here besides the picture, and the tree is read once.
    assert_eq!(said.len(), 8, "one problem per fault and no repeats: {said:#?}");
    // **A refused file is still a topic the game does not state**, which is a second fault and its own
    // problem: the tree says both that `ui.md` is too large and that the game has no `design/ui.md`.
    assert_eq!(about("the topic `ui`").len(), 1, "one problem for the missing game topic: {said:#?}");
}

#[test]
fn a_pictures_path_is_relative_to_the_file_that_cites_it() {
    let project = Project::new("pictures");
    project.map("yard", (32.0, 32.0));
    project.write("design/regions/north/style.md", "Salt on everything.\n");
    project.write("design/style.md", "Grey concrete, sodium light.\n");
    project.write("design/regions/north/board/fog.png", "not really a png, and it does not matter");
    project.write("design/regions/north/levels/yard/board/fog.png", "not really a png, and it does not matter");
    project.write(
        "design/regions/north/levels/yard/style.md",
        "Fog.\n\n![its colour, not its density](board/fog.png)\n\nAnd a page: <https://example.com/a.png>\n",
    );
    let tree = project.tree();
    assert!(tree.problems.is_empty(), "{:?}", tree.problems.iter().map(|p| p.line()).collect::<Vec<_>>());
    let pictures = tree.pictures_of("design/regions/north/levels/yard/style.md");
    assert_eq!(pictures.len(), 1, "one picture, and a link is not a picture: {pictures:?}");
    assert_eq!(pictures[0].cited, "board/fog.png");
    assert_eq!(pictures[0].file.as_deref(), Some("design/regions/north/levels/yard/board/fog.png"));
    assert_eq!(pictures[0].caption, "its colour, not its density", "the alt text is the caption a person is given");
    assert_eq!(pictures[0].line, 3);
}

#[test]
fn a_file_that_is_not_utf8_is_refused_by_name_and_the_rest_of_the_tree_still_reads() {
    let project = game("utf8");
    let path = project.0.join("design/levels/yard/sound.md");
    std::fs::create_dir_all(path.parent().unwrap_or(&project.0)).expect("the folder is made");
    std::fs::write(&path, [0xff, 0xfe, 0x00, 0x41]).expect("the file is written");
    project.write("design/sound.md", "Gulls.\n");
    let tree = project.tree();
    let said: Vec<String> = tree.problems.iter().map(|p| p.line()).collect();
    assert_eq!(said.len(), 1, "one problem, for the one file that does not decode: {said:#?}");
    assert!(said[0].contains("design/levels/yard/sound.md") && said[0].contains("UTF-8"), "{}", said[0]);
    assert!(tree.place("design/levels/yard").is_some(), "a refused file does not cost the place it belongs to");
}

#[test]
fn a_topic_chain_over_the_limit_is_refused_with_its_sizes() {
    let project = Project::new("chain");
    // Three files of 30 KB each are each under `STYLE_FILE_MAX_BYTES` and add up past the chain's limit.
    let chunk = "y".repeat(24_000);
    project.write("design/style.md", &chunk);
    project.write("design/regions/north/style.md", &chunk);
    project.write("design/regions/north/levels/yard/style.md", &chunk);
    let tree = project.tree();
    let said: Vec<String> = tree.problems.iter().map(|p| p.line()).collect();
    assert!(
        said.iter().any(|line| line.contains(&STYLE_CHAIN_MAX_BYTES.to_string()) || line.contains("chain")),
        "the chain's limit is what is refused, not a file's: {said:#?}"
    );
    assert!(
        said.iter().filter(|line| line.contains("chain")).count() == 1,
        "one problem for the one chain: {said:#?}"
    );
    assert!(
        !said.iter().any(|line| line.contains(&STYLE_FILE_MAX_BYTES.to_string())),
        "no file is over the file limit, so no file is refused: {said:#?}"
    );
}

#[test]
fn a_project_with_no_design_folder_is_a_tree_of_the_game_alone_and_no_fault() {
    let dir = std::env::temp_dir().join(format!("grimoire-design-none-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch project");
    let tree = design::Tree::read(&dir);
    assert_eq!(tree.places.len(), 1, "the game is a place whether or not its folder exists");
    assert_eq!(tree.places[0].kind, PlaceKind::Game);
    assert!(tree.places[0].topics.is_empty());
    assert!(tree.problems.is_empty(), "most projects have no style yet, which is not a fault");
    // And a tree that is written is read again, so an edit needs no restart.
    let style = dir.join("design/style.md");
    std::fs::create_dir_all(dir.join("design")).expect("the design folder is made");
    std::fs::write(&style, "First words.\n").expect("the style is written");
    assert_eq!(design::Tree::read(&dir).text("design/style.md"), Some("First words.\n"));
    std::fs::write(&style, "Second words.\n").expect("the style is edited");
    assert_eq!(
        design::Tree::read(&dir).text("design/style.md"),
        Some("Second words.\n"),
        "the tree is read from disk on every call, so an edit is in the next answer"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The reader is a file kind of the reload contract, so "does a change to a style file reach anyone?"
/// has an answer for this folder as it has for every other.
#[test]
fn the_design_folder_is_one_row_of_the_reload_contract() {
    use bevy_grimoire_core::reload::{self, Base, Lifetime, ReloadClass};
    let kind = reload::kind_by_id("design").unwrap_or_else(|| panic!("`design` is no kind of reload::KINDS"));
    assert_eq!(kind.base, Base::Root, "`design/` hangs off the game's root, beside `assets/`");
    assert_eq!(kind.class, ReloadClass::Live, "nothing holds the tree, so an edit is live");
    assert_eq!(kind.lifetime, Lifetime::Global, "one tree per project, not one per map");
    assert!(!kind.other_writers.is_empty(), "a kind no operation writes names who does");
    // **The row names the game's own files**, and a nested place's file is the same kind of file read by the
    // same walk: `matches_pattern` matches one segment at a time and has no tail wildcard, so a `design/**`
    // row would be a pattern no path matches — and adding `**` as a literal segment makes the protected
    // pattern-uniqueness ratchet synthesise `**/library.yaml` and demand a kind claim it. So the row states
    // what it can, and `Tree::read` is what reaches the nested places.
    assert_eq!(reload::kind_of(Base::Root, "design/style.md").map(|k| k.id), Some("design"), "the game's own style is the row");
    assert_eq!(reload::kind_of(Base::Root, "design/dialogue.md").map(|k| k.id), Some("design"), "so is every other game topic");
    assert_eq!(reload::kind_of(Base::Root, "designx/style.md"), None, "a folder whose name merely starts that way is not it");
}

/// The limits are named constants with their unit, and the numbers a reader of the issue was given.
#[test]
fn the_limits_are_the_ones_the_issue_measured_against() {
    assert_eq!(STYLE_FILE_MAX_BYTES, 32_768, "32 KiB per file");
    assert_eq!(STYLE_CHAIN_MAX_BYTES, 65_536, "64 KiB for the longest chain");
    // **A file of exactly the limit is served and one byte more is refused**, which is what a limit means and
    // what an off-by-one in the reader would break. Sanctuary's `game-style.md` (74,733 bytes, measured
    // 2026-11-12) is over it by more than a byte, so it is refused whole and splits into its own topics.
    let project = Project::new("limit_edge");
    let at_the_limit = project.write("design/style.md", &"x".repeat(STYLE_FILE_MAX_BYTES));
    let tree = project.tree();
    assert!(tree.problems.is_empty(), "a file of exactly the limit is served: {:?}", tree.problems.iter().map(|p| p.line()).collect::<Vec<_>>());
    assert_eq!(tree.text("design/style.md").map(str::len), Some(STYLE_FILE_MAX_BYTES), "and served whole");
    std::fs::write(&at_the_limit, "x".repeat(STYLE_FILE_MAX_BYTES + 1)).expect("one byte more");
    let over = project.tree();
    assert_eq!(over.problems.len(), 1, "one byte more is refused: {:?}", over.problems.iter().map(|p| p.line()).collect::<Vec<_>>());
    assert_eq!(over.text("design/style.md"), None, "and not served at all");
}

/// A path helper, so a mistake in a fixture names the file it was writing.
#[allow(dead_code)]
fn shown(path: &Path) -> String {
    path.display().to_string()
}
