//! **The game's style: a tree of places, and what is wrong with it** (SG-01, #400; the series #399).
//!
//! A game's look and feel is written down in Markdown, one folder per place, so an agent can look up what
//! a place should feel like before it shapes that place. `design/` at the project root is that tree, and
//! every folder in it is a **place**:
//!
//! ```text
//! design/
//!   style.md                          the whole game, its overall feel
//!   dialogue.md                       a topic of the whole game
//!   regions/<region>/                 a region
//!     style.md  <topic>.md
//!     levels/<map>/                   a level: its folder is named after its map
//!       style.md  <topic>.md
//!       areas/<area>/                 an area: a rectangle on that level's map
//!         area.yaml  style.md  <topic>.md
//!   levels/<map>/                     a level in no region
//! ```
//!
//! **A place's files are its topics.** `style.md` is its overall feel; every other `<topic>.md` is one
//! topic — `dialogue.md`, `interaction.md`, `sound.md`, `ui.md`, `names.md`. A topic below the game must
//! have a file of the same name at the game level, so a region cannot grow a topic the rest of the game
//! never mentions.
//!
//! # What this module answers, and what it refuses
//!
//! [`Tree::read`] answers the tree: every place, its topics and their sizes, and an area's rectangle in
//! map metres. [`Tree::problems`] is what is wrong with it, each reported once with the path it is about:
//!
//! - a level's folder whose map does not exist, and a map with a folder both under a region and under
//!   `design/levels/`;
//! - a topic below the game with no game-level counterpart;
//! - two areas of one level that overlap, and an area outside its map's bounds;
//! - a file over [`STYLE_FILE_MAX_BYTES`], a topic whose longest chain is over
//!   [`STYLE_CHAIN_MAX_BYTES`], and a file that is not UTF-8;
//! - a picture the text cites that is not there — named by the file, the **line**, and the path.
//!
//! **A file over its limit is refused whole, never truncated.** Agent context files in the wild are long
//! and hard to read (*Agent READMEs*, 10.1145/3840295) and a model loses what sits in the middle of a long
//! context (*Lost in the Middle*, 10.48550/arXiv.2307.03172) — so the limits are the point, and a file an
//! agent half-reads is worse than one it is told is missing, because it cannot tell which half it has.
//!
//! # Why the tree is read from disk on every call
//!
//! There is no parsed state to hold and no reload path to write: [`Tree::read`] walks the folder and
//! returns what is there now, so an edit to a file on disk is in the next answer with no restart. That is
//! why the reload table's `design` row is [`crate::reload::ReloadClass::Live`].
//!
//! Engine-free: `std`, `serde` and a CommonMark parser.
//!
//! # The Markdown reader
//!
//! Pictures and links are read with `pulldown-cmark` (MIT), with the extensions a page of this kind uses:
//! tables, strikethrough and task lists.
//!
//! **It is the one parser, not a duplicate of another.** The editor has no Markdown renderer to share it
//! with: GD-11's Markdown support is a *tokenizer* that colours syntax and recognises neither an image nor a
//! link, and `bevy_gooey` at the pinned revision ships no Markdown module (measured 2026-11-12). A second
//! implementation written by hand would be a CommonMark parser with tables — and a picture the text cites is
//! exactly the construct a regex gets wrong, which is the check this module exists for. When a view lands
//! (SG-05, #404), the test that the two list the same pictures for every fixture page is what keeps them
//! one answer.
//!
//! **Owner exception (2026-11-12):** this adds `pulldown-cmark` to `bevy_grimoire_core`, where GD-11's
//! "no new dependency" note governed the editor's *syntax colours* only; those stay on their tokenizer.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::problem::{Detail, Location, Problem};

/// **The folder, under a project's root, the tree lives in.** Outside `assets/`, so nothing in it ships.
pub const DESIGN_DIR: &str = "design";

/// The folder of a region's levels, inside the region.
pub const REGIONS_DIR: &str = "regions";

/// The folder of a region's own levels, and of the levels in no region.
pub const LEVELS_DIR: &str = "levels";

/// The folder of a level's areas.
pub const AREAS_DIR: &str = "areas";

/// The topic `style.md` states, without its extension.
pub const STYLE_TOPIC: &str = "style";

/// **An area's own file**: its rectangle on its level's map.
pub const AREA_FILE: &str = "area.yaml";

/// The extension a topic file has.
pub const MARKDOWN_EXT: &str = "md";

/// **The largest a single style file may be, in bytes.** 32 KiB is about five thousand words — more prose
/// than one place's feel takes. Past it the file is refused rather than served, because an agent's context
/// is the thing this limit protects. Sanctuary's 74,733-byte `game-style.md` is refused whole by this and
/// splits into its own topics.
pub const STYLE_FILE_MAX_BYTES: usize = 32_768;

/// **The largest chain the tree may serve, in bytes**, counted over the longest chain one topic can reach:
/// the game's file for it, its region's, its level's and its area's. 64 KiB is the guide's own limit.
pub const STYLE_CHAIN_MAX_BYTES: usize = 65_536;

/// The version `area.yaml` states.
pub const AREA_VERSION: u32 = 1;

/// Where the grimoire's data lives under a project root, for the maps an area is measured against.
///
/// **The one definition in this crate**: [`crate::reload_check::GRIMOIRE_DIR`] states it for the reload
/// checks, and a second literal here would be a second thing to keep in step with the folder the editor writes.
pub const GRIMOIRE_DIR: &str = crate::reload_check::GRIMOIRE_DIR;

/// **An area's rectangle on its level's map**: `[x0, z0, x1, z1]` in map metres, the box
/// [`crate::map::Map::floor_rect`] is in, which is the same box the editor's `Shown::Map` names.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Area {
    pub version: u32,
    /// `[x0, z0, x1, z1]`, map metres, with `x0 <= x1` and `z0 <= z1`.
    pub rect: [f32; 4],
}

impl Area {
    /// **Whether two rectangles share a cell.** Rects that merely touch share a line and no cell, so the
    /// comparison is strict on both axes: this is the overlap that makes a cell's chain ambiguous.
    pub fn overlaps(&self, other: &[f32; 4]) -> bool {
        let [x0, z0, x1, z1] = self.rect;
        let [ox0, oz0, ox1, oz1] = *other;
        x0 < ox1 && ox0 < x1 && z0 < oz1 && oz0 < z1
    }

    /// The rectangle as the problem sentences state it.
    pub fn said(&self) -> String {
        let [x0, z0, x1, z1] = self.rect;
        format!("[{x0}, {z0}, {x1}, {z1}]")
    }
}

/// **Which kind of place a folder is.** Ordered the way a chain is read: the whole game first, then the
/// region, then the level, then the area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlaceKind {
    Game,
    Region,
    Level,
    Area,
}

impl PlaceKind {
    /// The word the tree, `grimoire_state.design` and a refusal use.
    pub fn word(self) -> &'static str {
        match self {
            PlaceKind::Game => "game",
            PlaceKind::Region => "region",
            PlaceKind::Level => "level",
            PlaceKind::Area => "area",
        }
    }
}

/// **One topic file of a place**, as it is on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Topic {
    /// The topic's name, without `.md`: `style`, `dialogue`.
    pub name: String,
    /// The file, relative to the project root, `/`-separated.
    pub file: String,
    /// Its size in bytes, whether it was read or refused.
    pub bytes: usize,
}

/// **One place**: the whole game, a region, a level, or an area of a level.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub kind: PlaceKind,
    /// The folder, relative to the project root, `/`-separated: `design`, `design/regions/harbour`,
    /// `design/levels/yard`, `design/regions/harbour/levels/harbour/areas/docks`.
    pub path: String,
    /// **The map a level names** — its folder's name, because a level's folder is named after its map.
    /// `Some` for a level and for an area, which names its level's map.
    pub map: Option<String>,
    /// An area's rectangle in map metres, `[x0, z0, x1, z1]`. `None` for every other kind of place.
    pub rect: Option<[f32; 4]>,
    /// Its topics, `style` first and the rest by name.
    pub topics: Vec<Topic>,
}

impl Place {
    /// The topic with this name, when the place has it.
    pub fn topic(&self, name: &str) -> Option<&Topic> {
        self.topics.iter().find(|t| t.name == name)
    }
}

/// **One chain that is over its limit**: its size, the topic it was measured for, and its files with the
/// bytes each one adds.
type Overrun = (usize, String, Vec<(String, usize)>);

/// **One picture a style text cites**: the destination as written, the line it is written on, and the path
/// it resolves to relative to the project root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    /// What the Markdown says: `board/harbour-fog.jpg`.
    pub cited: String,
    /// The line, counted from 1 as an editor shows it.
    pub line: usize,
    /// The path relative to the project root, `/`-separated, when it is a path inside the project.
    pub file: Option<String>,
    /// **The caption: the alt text when the picture has one, and its title otherwise.** Never both glued
    /// together — they say different things, and `The fog fog` says neither.
    pub caption: String,
    /// The alt text as written, before the caption chose between it and the title.
    alt: String,
    /// The title as written, `""` when the picture gives none.
    title: String,
}

/// **The tree of places, and what is wrong with it.**
#[derive(Clone, Debug, PartialEq)]
pub struct Tree {
    /// The project root the tree was read from.
    pub root: PathBuf,
    /// Every place: the game, then each region with its levels and their areas, then the levels in no
    /// region. Always at least the game, which is a place whether or not its folder exists.
    pub places: Vec<Place>,
    /// What is wrong, each once, with the path it is about.
    pub problems: Vec<Problem>,
    /// **The maps the project has, by name**, each with its floor rectangle `(x0, z0, x1, z1)` in map metres, or `None`
    /// when its file does not read (which `problems` says). What `grimoire_style` checks a map and a point against.
    pub maps: BTreeMap<String, Option<(f32, f32, f32, f32)>>,
    /// **The text of every topic file that read**, by its path relative to the project root. A file that was
    /// refused (too large, not UTF-8, not a regular file) is absent, and its problem says why.
    texts: BTreeMap<String, String>,
}

/// A topic file as it read.
struct Read {
    text: String,
    bytes: usize,
}

impl Tree {
    /// **The tree under `root`.** A project with no `design/` folder is a tree of the game alone, with no
    /// problems: most projects have no style yet, which is not a fault.
    pub fn read(root: &Path) -> Tree {
        let mut problems = Vec::new();
        let mut texts = BTreeMap::new();
        let maps = maps_of(root, &mut problems);
        let design = root.join(DESIGN_DIR);
        let mut places = vec![Place {
            kind: PlaceKind::Game,
            path: DESIGN_DIR.to_owned(),
            map: None,
            rect: None,
            topics: Vec::new(),
        }];
        read_topics(&design, DESIGN_DIR, &mut places[0], &mut problems, &mut texts);
        if design.is_dir() {
            read_regions(root, &design, &maps, &mut places, &mut problems, &mut texts);
            read_levels_in_no_region(root, &design, &maps, &mut places, &mut problems, &mut texts);
        }
        let mut tree = Tree { root: root.to_path_buf(), places, problems, maps, texts };
        tree.check_a_map_has_one_folder();
        tree.check_topics_are_the_games();
        tree.check_chains();
        tree.check_pictures();
        tree
    }

    /// The place with this folder.
    pub fn place(&self, path: &str) -> Option<&Place> {
        self.places.iter().find(|p| p.path == path)
    }

    /// **The text of a topic file that read**, by its path relative to the project root.
    pub fn text(&self, file: &str) -> Option<&str> {
        self.texts.get(file).map(String::as_str)
    }

    /// **The pictures `file`'s text cites**, in the order it cites them.
    pub fn pictures_of(&self, file: &str) -> Vec<Picture> {
        match self.texts.get(file) {
            Some(text) => pictures(text, file),
            None => Vec::new(),
        }
    }

    /// **Every place after this one in a chain that contains it**: the game, then its region, then its level,
    /// then the area at a point. The order the chain is read in, most general first.
    pub fn chain_to(&self, map: Option<&str>, at: Option<(f32, f32)>) -> Vec<&Place> {
        let mut chain: Vec<&Place> = self.places.iter().filter(|p| p.kind == PlaceKind::Game).collect();
        let Some(map) = map else { return chain };
        // **The region's place, then the level's, then the area that holds `at`.** A map with a folder in no
        // region has no region layer, which is a chain of three rather than four.
        if let Some(region) = self.places.iter().find(|p| p.kind == PlaceKind::Region && self.holds_level(p, map)) {
            chain.push(region);
        }
        let levels: Vec<&Place> = self
            .places
            .iter()
            .filter(|p| p.kind == PlaceKind::Level && p.map.as_deref() == Some(map))
            .collect();
        chain.extend(levels.iter().copied());
        let Some(level) = levels.first() else { return chain };
        if let Some((x, z)) = at {
            let level_path = format!("{}/{}", level.path, AREAS_DIR);
            let mut areas: Vec<&Place> = self
                .places
                .iter()
                .filter(|p| p.kind == PlaceKind::Area && p.path.starts_with(&level_path))
                .collect();
            areas.sort_by(|a, b| a.path.cmp(&b.path));
            // **The smallest area that holds the point wins**, so an area inside another is the one a point
            // in it reaches: the more specific place is the one that says more about it.
            let mut holding: Vec<&&Place> = areas.iter().filter(|p| p.rect.is_some_and(|r| inside(r, (x, z)))).collect();
            holding.sort_by(|a, b| area_size(a.rect).total_cmp(&area_size(b.rect)));
            chain.extend(holding.first().map(|p| **p));
        }
        chain
    }

    /// Whether `region` holds a level named after `map`.
    fn holds_level(&self, region: &Place, map: &str) -> bool {
        let prefix = format!("{}/{}/{}", region.path, LEVELS_DIR, map);
        self.places.iter().any(|p| p.kind == PlaceKind::Level && p.path == prefix)
    }

    /// **A map has one folder.** Two folders for one map — one under a region and one under `design/levels/` —
    /// are two answers to "what is this level's style?", and which one an agent read would be an accident of
    /// the walk.
    fn check_a_map_has_one_folder(&mut self) {
        let mut by_map: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for place in self.places.iter().filter(|p| p.kind == PlaceKind::Level) {
            if let Some(map) = place.map.as_deref() {
                by_map.entry(map).or_default().push(place.path.as_str());
            }
        }
        for (map, folders) in by_map {
            if folders.len() < 2 {
                continue;
            }
            let named: Vec<String> = folders.iter().map(|f| format!("`{f}`")).collect();
            self.problems.push(Problem::with(
                format!("the map `{map}` has two level folders"),
                Detail::default()
                    .at(Location::file(folders[1].to_owned()))
                    .because(format!("{} both name it, and a level has one folder", named.join(" and ")))
                    .fix("keep one of the folders and move the other's files into it, or rename one to another map"),
            ));
        }
    }

    /// **Every topic a place below the game uses is a topic the game states**, because a topic is tied to the
    /// whole game. A region that grows one nothing else mentions is a rule the rest of the game never agreed
    /// to.
    fn check_topics_are_the_games(&mut self) {
        let game_topics: BTreeSet<String> = match self.places.iter().find(|p| p.kind == PlaceKind::Game) {
            Some(game) => game.topics.iter().map(|t| t.name.clone()).collect(),
            None => return,
        };
        // Once per topic, naming the missing game file and the first place that used it.
        let mut first_use: BTreeMap<&str, &str> = BTreeMap::new();
        for place in self.places.iter().filter(|p| p.kind != PlaceKind::Game) {
            for topic in &place.topics {
                if !game_topics.contains(&topic.name) {
                    first_use.entry(topic.name.as_str()).or_insert(topic.file.as_str());
                }
            }
        }
        for (topic, used_by) in first_use {
            let missing = format!("{DESIGN_DIR}/{topic}.{MARKDOWN_EXT}");
            self.problems.push(Problem::with(
                format!("the topic `{topic}` is used below the game and the game does not state it"),
                Detail::default()
                    .at(Location::file(missing.clone()))
                    .because(format!(
                        "{used_by} is one of the files that uses it, and a topic below the game has to be a \
                         topic of the whole game or the region grows a rule nothing else mentions"
                    ))
                    .fix(format!("write {missing}, or rename the topic below the game to one the game has")),
            ));
        }
    }

    /// **Every chain the tree can serve is within [`STYLE_CHAIN_MAX_BYTES`].**
    ///
    /// **A chain is the files one point is read with** — the game's, its region's, its level's and its
    /// area's — so the limit is measured over each chain this tree can actually hand out, **not** over a
    /// topic's total across the whole tree. Twelve regions of 6,144 bytes are 73,728 bytes all told and
    /// every one of their chains is two files of 12,288; refusing that tree would be refusing a project
    /// every one of whose answers fits, which is a limit that punishes a project for having regions.
    ///
    /// **The chains are enumerated, not sampled through [`Self::chain_to`].** A chain holds at most one
    /// area, and [`Self::chain_to`] takes the smallest that holds a point — so sampling points would measure
    /// whichever area won each one, and an area it never picked would go unmeasured. (Two areas of one level
    /// that overlap are a problem of their own, so a legitimate tree has no nested areas to sample: the
    /// enumeration is what removes the dependence on that rule either way.) The shapes are: the game alone,
    /// and for each level the game with its region and that level — and, for each area of it, that same chain
    /// with the area's own file. Each is measured once per topic the chain carries, because a chain is served
    /// for the topic asked for, and two topics over the same files are one problem: a person fixes files.
    fn check_chains(&mut self) {
        let topics: BTreeSet<String> =
            self.places.iter().flat_map(|p| p.topics.iter().map(|t| t.name.clone())).collect();
        // **The chain of one level**: the game, its region when it has one, and the level itself.
        let mut level_chains: Vec<Vec<&Place>> = Vec::new();
        for level in self.places.iter().filter(|p| p.kind == PlaceKind::Level) {
            let game: Vec<&Place> = self.places.iter().filter(|p| p.kind == PlaceKind::Game).collect();
            let region = self.region_of(level);
            let mut chain = game;
            chain.extend(region);
            chain.push(level);
            level_chains.push(chain);
        }
        // **Every chain, by the files it adds up for a topic.** An area's chain is its level's plus the area.
        let mut chains: BTreeMap<Vec<String>, String> = BTreeMap::new();
        for topic in &topics {
            let game: Vec<String> = self
                .places
                .iter()
                .filter(|p| p.kind == PlaceKind::Game)
                .filter_map(|p| p.topic(topic))
                .map(|t| t.file.clone())
                .collect();
            if !game.is_empty() {
                chains.entry(game.clone()).or_insert_with(|| topic.clone());
            }
            for chain in &level_chains {
                let files: Vec<String> = chain.iter().filter_map(|p| p.topic(topic)).map(|t| t.file.clone()).collect();
                chains.entry(files.clone()).or_insert_with(|| topic.clone());
                let level = chain.last().copied();
                let Some(level) = level else { continue };
                for area in self.areas_of(level) {
                    let mut with_area = files.clone();
                    if let Some(row) = area.topic(topic) {
                        with_area.push(row.file.clone());
                    }
                    chains.entry(with_area).or_insert_with(|| topic.clone());
                }
            }
        }
        // **Each chain's bytes, off the places' own rows**, so a refusal and the tree cannot disagree.
        let bytes_of: BTreeMap<&str, usize> =
            self.places.iter().flat_map(|p| p.topics.iter()).map(|t| (t.file.as_str(), t.bytes)).collect();
        let mut over: Vec<Overrun> = Vec::new();
        for (files, topic) in chains {
            let total: usize = files.iter().map(|f| bytes_of.get(f.as_str()).copied().unwrap_or(0)).sum();
            if total <= STYLE_CHAIN_MAX_BYTES {
                continue;
            }
            let mut files: Vec<(String, usize)> = files
                .into_iter()
                .map(|f| {
                    let n = bytes_of.get(f.as_str()).copied().unwrap_or(0);
                    (f, n)
                })
                .collect();
            // **The largest file first**: it is the one a person has to cut.
            files.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            over.push((total, topic, files));
        }
        over.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (total, topic, files) in over {
            let named: Vec<String> = files.iter().map(|(file, bytes)| format!("{file} ({bytes} bytes)")).collect();
            let first = files.first().map(|(f, _)| f.clone()).unwrap_or_else(|| DESIGN_DIR.to_owned());
            self.problems.push(Problem::with(
                format!("the `{topic}` chain is {total} bytes, over the {STYLE_CHAIN_MAX_BYTES} an agent is served"),
                Detail::default()
                    .at(Location::file(first))
                    .because(format!("the chain adds up {}", named.join(", ")))
                    .fix("shorten the largest file of the chain, or move part of it into a topic of its own"),
            ));
        }
    }

    /// **The region a level is in**, when it is in one.
    fn region_of(&self, level: &Place) -> Option<&Place> {
        // **The folder two above the level**, which is the region's own path: a level is
        // `design/regions/<region>/levels/<map>`, so the region is `design/regions/<region>`. Splitting once
        // gives `design/regions/<region>/levels`, which is the folder the levels live in and no region at all.
        let (levels, _map) = level.path.rsplit_once('/')?;
        let (region, folder) = levels.rsplit_once('/')?;
        if folder != LEVELS_DIR {
            return None;
        }
        self.places.iter().find(|p| p.kind == PlaceKind::Region && p.path == region)
    }

    /// **The areas of a level**, in path order.
    fn areas_of(&self, level: &Place) -> Vec<&Place> {
        let prefix = format!("{}/{AREAS_DIR}/", level.path);
        let mut areas: Vec<&Place> = self
            .places
            .iter()
            .filter(|p| p.kind == PlaceKind::Area && p.path.starts_with(&prefix))
            .collect();
        areas.sort_by(|a, b| a.path.cmp(&b.path));
        areas
    }

    /// **A picture the text cites is there.** A picture is a plain Markdown path relative to its own file.
    fn check_pictures(&mut self) {
        let mut missing = Vec::new();
        for (file, text) in &self.texts {
            for picture in pictures(text, file) {
                let Some(rel) = &picture.file else {
                    // **A URL is not this project's picture to check**: it is not on this machine, and it is
                    // not a file the editor draws.
                    continue;
                };
                if !self.root.join(rel).is_file() {
                    missing.push((file.clone(), picture));
                }
            }
        }
        for (file, picture) in missing {
            let rel = picture.file.clone().unwrap_or_default();
            let caption =
                if picture.caption.is_empty() { String::new() } else { format!(", whose caption says '{}'", picture.caption) };
            self.problems.push(Problem::with(
                format!("`{file}` cites a picture that is not there, on line {}", picture.line),
                Detail::default()
                    // **The line is carried as a number as well as said**, so a caller that jumps to it reads
                    // it here instead of scraping it out of the sentence.
                    .at(Location::file(file.clone()).line(picture.line))
                    .because(format!("line {} cites `{}`{caption}, and {rel} is not there", picture.line, picture.cited))
                    .fix(format!("put the picture at {rel}, or change the line to name one that is there")),
            ));
        }
    }

    /// **The tree as `grimoire_state.design` hands it to an agent**: every place with its topics, an area with
    /// its rectangle, and what is wrong.
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "root": self.root.to_string_lossy(),
            "places": self.places.iter().map(|p| serde_json::json!({
                "kind": p.kind.word(),
                "path": p.path,
                "map": p.map,
                "rect": p.rect,
                "topics": p.topics.iter().map(|t| serde_json::json!({
                    "name": t.name,
                    "file": t.file,
                    "bytes": t.bytes,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "problems": self.problems.iter().map(Problem::to_json).collect::<Vec<_>>(),
        })
    }
}

/// The area of a rectangle, for the smallest-area choice a chain makes.
fn area_size(rect: Option<[f32; 4]>) -> f32 {
    match rect {
        Some([x0, z0, x1, z1]) => (x1 - x0).max(0.0) * (z1 - z0).max(0.0),
        None => f32::MAX,
    }
}

/// **Whether a point is in a rectangle**, the line of the rectangle included.
pub fn inside(rect: [f32; 4], (x, z): (f32, f32)) -> bool {
    let [x0, z0, x1, z1] = rect;
    x >= x0 && x <= x1 && z >= z0 && z <= z1
}

/// **The maps a project has**, by name, with the floor rectangle an area is measured against.
///
/// **A map file that does not read is reported, not skipped.** Skipping it would say "there is no such map"
/// about a map that is right there — a wrong reason, and a person would go looking for a missing file — and
/// the areas of its level would never be bounds-checked. So an unreadable map is a problem of its own, and
/// it is still a name the tree knows: its level is a level, and only the bounds check is skipped for it.
fn maps_of(root: &Path, problems: &mut Vec<Problem>) -> BTreeMap<String, Option<(f32, f32, f32, f32)>> {
    let dir = root.join(GRIMOIRE_DIR).join(crate::project_data::MAPS_DIR);
    let mut out: BTreeMap<String, Option<(f32, f32, f32, f32)>> = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(&dir) else { return out };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(crate::map::MAP_SUFFIX) else { continue };
        let file = format!("{GRIMOIRE_DIR}/{}/{name}", crate::project_data::MAPS_DIR);
        let map = match std::fs::read_to_string(entry.path()) {
            Ok(text) => match crate::map::Map::parse(&text) {
                Ok(map) => map,
                Err(why) => {
                    problems.push(Problem::with(
                        format!("the map `{stem}` does not read, so its level's areas cannot be measured"),
                        Detail::default()
                            .at(Location::file(file.clone()))
                            .because(why)
                            .fix("fix the map file; until it reads, its level is listed and its areas are not measured against it"),
                    ));
                    out.insert(stem.to_owned(), None);
                    continue;
                }
            },
            Err(e) => {
                problems.push(Problem::with(
                    format!("the map `{stem}` cannot be read, so its level's areas cannot be measured"),
                    Detail::default()
                        .at(Location::file(file.clone()))
                        .because(e.to_string())
                        .fix("make the file readable; until it is, its level is listed and its areas are not measured against it"),
                ));
                out.insert(stem.to_owned(), None);
                continue;
            }
        };
        // **The map's own name wins over its file's**, because the editor writes them together and a file
        // renamed by hand would otherwise be two maps with one name.
        out.insert(map.name.clone(), Some(map.floor_rect()));
        out.entry(stem.to_owned()).or_insert_with(|| Some(map.floor_rect()));
    }
    out
}

/// **The regions, each with its levels and their areas.**
fn read_regions(
    root: &Path,
    design: &Path,
    maps: &BTreeMap<String, Option<(f32, f32, f32, f32)>>,
    places: &mut Vec<Place>,
    problems: &mut Vec<Problem>,
    texts: &mut BTreeMap<String, String>,
) {
    for (name, dir) in folders(&design.join(REGIONS_DIR)) {
        let path = format!("{DESIGN_DIR}/{REGIONS_DIR}/{name}");
        let mut region = Place { kind: PlaceKind::Region, path: path.clone(), map: None, rect: None, topics: Vec::new() };
        read_topics(&dir, &path, &mut region, problems, texts);
        places.push(region);
        let levels = format!("{path}/{LEVELS_DIR}");
        read_levels(root, &dir.join(LEVELS_DIR), &levels, maps, places, problems, texts);
    }
}

/// **The levels in no region.**
fn read_levels_in_no_region(
    root: &Path,
    design: &Path,
    maps: &BTreeMap<String, Option<(f32, f32, f32, f32)>>,
    places: &mut Vec<Place>,
    problems: &mut Vec<Problem>,
    texts: &mut BTreeMap<String, String>,
) {
    let dir = design.join(LEVELS_DIR);
    let levels = format!("{DESIGN_DIR}/{LEVELS_DIR}");
    read_levels(root, &dir, &levels, maps, places, problems, texts);
}

/// **The levels under `dir`**, each named after its map, with its areas.
fn read_levels(
    _root: &Path,
    dir: &Path,
    levels_path: &str,
    maps: &BTreeMap<String, Option<(f32, f32, f32, f32)>>,
    places: &mut Vec<Place>,
    problems: &mut Vec<Problem>,
    texts: &mut BTreeMap<String, String>,
) {
    for (map, level_dir) in folders(dir) {
        let path = format!("{levels_path}/{map}");
        let mut level = Place { kind: PlaceKind::Level, path: path.clone(), map: Some(map.clone()), rect: None, topics: Vec::new() };
        read_topics(&level_dir, &path, &mut level, problems, texts);
        // **A level's folder is named after its map, so no file binds them.** A folder whose map does not
        // exist is a level nothing will ever open, and the problem names both paths.
        let bounds = match maps.get(&map) {
            // **A map that is there but does not read has its own problem already**; the level is a level and
            // only its areas go unmeasured, so nothing is said about it twice.
            Some(None) => None,
            Some(Some(bounds)) => Some(*bounds),
            None => {
                let map_file = format!("{GRIMOIRE_DIR}/{}/{map}{}", crate::project_data::MAPS_DIR, crate::map::MAP_SUFFIX);
                problems.push(Problem::with(
                    format!("the level `{map}` has a folder and the project has no such map"),
                    Detail::default()
                        .at(Location::file(path.clone()))
                        .because(format!("a level's folder is named after its map, and {map_file} is not there"))
                        .fix("create the map, or rename the folder to the name of a map the project has"),
                ));
                None
            }
        };
        places.push(level);
        read_areas(&level_dir, &path, &map, bounds, places, problems, texts);
    }
}

/// **The areas of one level**, each a rectangle on that level's map.
fn read_areas(
    level_dir: &Path,
    level_path: &str,
    map: &str,
    bounds: Option<(f32, f32, f32, f32)>,
    places: &mut Vec<Place>,
    problems: &mut Vec<Problem>,
    texts: &mut BTreeMap<String, String>,
) {
    let areas_path = format!("{level_path}/{AREAS_DIR}");
    let mut read: Vec<(String, Option<Area>)> = Vec::new();
    let areas = folders(&level_dir.join(AREAS_DIR));
    for (name, dir) in areas {
        let path = format!("{areas_path}/{name}");
        let file = format!("{path}/{AREA_FILE}");
        let area = match std::fs::read_to_string(dir.join(AREA_FILE)) {
            Ok(text) => match crate::yaml::from_str::<Area>(&text, &file) {
                Ok(area) if area.version == AREA_VERSION => {
                    let [x0, z0, x1, z1] = area.rect;
                    if !(x0.is_finite() && z0.is_finite() && x1.is_finite() && z1.is_finite()) {
                        problems.push(area_problem(&file, "a corner is not a number"));
                        None
                    } else if x0 > x1 || z0 > z1 {
                        problems.push(area_problem(&file, &format!("{x0} is greater than {x1} or {z0} is greater than {z1}")));
                        None
                    } else {
                        Some(area)
                    }
                }
                Ok(area) => {
                    problems.push(area_problem(&file, &format!("its version is {}, and this build reads {AREA_VERSION}", area.version)));
                    None
                }
                Err(problem) => {
                    problems.push(Problem::with(
                        format!("`{file}` does not read"),
                        problem.detail().at(Location::file(file.clone())),
                    ));
                    None
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // **An area's folder with no `area.yaml` is an area with no rectangle**, which nothing can
                // place a point in: it is reported rather than guessed at.
                problems.push(area_problem(&file, "there is no such file, and an area is its rectangle"));
                None
            }
            Err(e) => {
                problems.push(area_problem(&file, &e.to_string()));
                None
            }
        };
        let mut place = Place {
            kind: PlaceKind::Area,
            path,
            map: Some(map.to_owned()),
            rect: area.map(|a| a.rect),
            topics: Vec::new(),
        };
        read_topics(&dir, &place.path.clone(), &mut place, problems, texts);
        if let (Some(rect), Some(bounds)) = (place.rect, bounds) {
            let (min_x, min_z, max_x, max_z) = bounds;
            if rect[0] < min_x || rect[2] > max_x || rect[1] < min_z || rect[3] > max_z {
                problems.push(Problem::with(
                    format!("the area at {} reaches outside its map `{map}`", place.path),
                    Detail::default()
                        .at(Location::file(format!("{}/{AREA_FILE}", place.path)))
                        .because(format!(
                            "the map's floor is x {min_x}..{max_x} and z {min_z}..{max_z} in map metres, and every \
                             corner of an area is on it"
                        ))
                        .fix("move the area's corners onto the map, or make the map bigger"),
                ));
            }
        }
        read.push((place.path.clone(), area));
        places.push(place);
    }
    // **Two areas of one level that overlap make one cell belong to two places**, so the chain a cell is read
    // with would depend on which of them was found first.
    let rects: Vec<(&str, [f32; 4])> = read
        .iter()
        .filter_map(|(path, area)| area.as_ref().map(|a| (path.as_str(), a.rect)))
        .collect();
    for (i, (a_path, a_rect)) in rects.iter().enumerate() {
        for (b_path, b_rect) in rects.iter().skip(i + 1) {
            let area = Area { version: AREA_VERSION, rect: *a_rect };
            if area.overlaps(b_rect) {
                problems.push(Problem::with(
                    format!("two areas of one level overlap: {a_path} and {b_path}"),
                    Detail::default()
                        .at(Location::file(format!("{b_path}/{AREA_FILE}")))
                        .because(format!(
                            "{} and {} share a cell, and a cell belongs to one area or its chain is ambiguous",
                            area.said(),
                            Area { version: AREA_VERSION, rect: *b_rect }.said()
                        ))
                        .fix("move one area's corner so the two do not overlap"),
                ));
            }
        }
    }
}

fn area_problem(file: &str, why: &str) -> Problem {
    Problem::with(
        format!("`{file}` is not a usable area"),
        Detail::default()
            .at(Location::file(file))
            .because(why.to_owned())
            .fix("write `rect: [x0, z0, x1, z1]` in map metres, inside the map"),
    )
}

/// **The folders directly under `dir`**, by name, sorted, so a tree reads the same twice.
fn folders(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            out.push((name, path));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// **A place's topics, read into it**, with every file that is refused named in `problems`.
fn read_topics(dir: &Path, place_path: &str, place: &mut Place, problems: &mut Vec<Problem>, texts: &mut BTreeMap<String, String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut topics = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Some(stem) = name.strip_suffix(&format!(".{MARKDOWN_EXT}")) else { continue };
        if stem.is_empty() {
            continue;
        }
        let file = format!("{place_path}/{name}");
        let path = entry.path();
        match read_topic(&path, &file, problems) {
            Some(read) => {
                texts.insert(file.clone(), read.text);
                topics.push(Topic { name: stem.to_owned(), file, bytes: read.bytes });
            }
            None => {
                // **A refused file is still a topic of the place**, with the size it has: the tree lists what
                // is there, and the problem says why it is not served.
                let bytes = entry.metadata().map(|m| m.len() as usize).unwrap_or(0);
                topics.push(Topic { name: stem.to_owned(), file, bytes });
            }
        }
    }
    // **`style` first**, then the rest by name, because that is how the chain is written and how a person
    // reads it.
    topics.sort_by(|a, b| {
        let key = |t: &Topic| (u8::from(t.name != STYLE_TOPIC), t.name.clone());
        key(a).cmp(&key(b))
    });
    place.topics = topics;
}

/// **One topic file, read whole or refused**, never truncated.
///
/// # Why this does not call `std::fs::read`
///
/// **A style path arrives with a cloned project**, and its text is read while an agent waits. `std::fs::read`
/// on a FIFO blocks forever and on `/dev/zero` grows until memory is gone, and the limit would be checked
/// only after the damage. So the path is `stat`ed first and **anything that is not a regular file is refused
/// without being opened**, and what is regular is read through `.take(STYLE_FILE_MAX_BYTES + 1)` so a file
/// that grew between the two is still bounded.
fn read_topic(path: &Path, file: &str, problems: &mut Vec<Problem>) -> Option<Read> {
    use std::io::Read as _;
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) => {
            problems.push(refused(file, format!("it cannot be read: {e}")));
            return None;
        }
    };
    if !meta.is_file() {
        problems.push(refused(file, "it is not a regular file".to_owned()));
        return None;
    }
    let open = match std::fs::File::open(path) {
        Ok(open) => open,
        Err(e) => {
            problems.push(refused(file, format!("it cannot be read: {e}")));
            return None;
        }
    };
    let mut bytes = Vec::new();
    if let Err(e) = open.take(STYLE_FILE_MAX_BYTES as u64 + 1).read_to_end(&mut bytes) {
        problems.push(refused(file, format!("it cannot be read: {e}")));
        return None;
    }
    // **The file's real size, from its own metadata.** `bytes.len()` is capped at
    // `STYLE_FILE_MAX_BYTES + 1` by the `take` above, so a 100 KB file would report itself as 32,769 bytes
    // while its topic row carried the true size — two numbers for one file, and the one a person reads in
    // the refusal would be the wrong one.
    let size = usize::try_from(meta.len()).unwrap_or(usize::MAX);
    if bytes.len() > STYLE_FILE_MAX_BYTES {
        // **The size, not the head.** A file that said "32 KB" and then stopped would be a truncation.
        problems.push(Problem::with(
            format!("`{file}` is {size} bytes, and a style file is at most {STYLE_FILE_MAX_BYTES}"),
            Detail::default()
                .at(Location::file(file))
                .because("it is served whole or not at all: a file an agent half-reads is a file that cannot tell which half it has")
                .fix("move part of it into a topic of its own, or cut what this place does not need to say"),
        ));
        return None;
    }
    let text_len = bytes.len();
    match String::from_utf8(bytes) {
        // **A served file's size is the bytes read**, which is its whole content because it is at or under
        // the limit; `size` above is the same number for one, and is only larger for a refused one.
        Ok(text) => Some(Read { text, bytes: text_len }),
        Err(_) => {
            problems.push(refused(file, "it is not UTF-8".to_owned()));
            None
        }
    }
}

fn refused(file: &str, why: String) -> Problem {
    Problem::with(
        format!("`{file}` was not served"),
        Detail::default().at(Location::file(file)).because(why).fix("save it as a UTF-8 Markdown file"),
    )
}

/// **The CommonMark extensions a style page uses**: tables, strikethrough and task lists. One set, so the
/// reader here and the view SG-05 (#404) draws cannot disagree about what a page contains.
pub fn options() -> pulldown_cmark::Options {
    use pulldown_cmark::Options;
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

/// **The pictures `text` cites, in the order it cites them**, each with the line it is written on and the
/// alt text a reader is given in place of the picture.
///
/// `file` is the text's path relative to the project root; a destination is resolved against the text's own
/// folder, which is what "relative to its file" means in Markdown.
pub fn pictures(text: &str, file: &str) -> Vec<Picture> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let mut out: Vec<Picture> = Vec::new();
    let mut inside = 0usize;
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                inside += 1;
                let cited = dest_url.to_string();
                let line = text[..range.start.min(text.len())].matches('\n').count() + 1;
                // **The alt text is the caption; a title is only where there is none.** They arrive as
                // separate events, so appending the alt text onto the title spelled `The fog fog` for
                // `![fog](f.png "The fog")` — two captions glued into one that says neither.
                out.push(Picture {
                    file: resolve(file, &cited),
                    cited,
                    line,
                    caption: String::new(),
                    alt: String::new(),
                    title: title.to_string(),
                });
            }
            Event::End(TagEnd::Image) => inside = inside.saturating_sub(1),
            // **The alt text is what a person who cannot see the picture is given in its place**, so it is
            // gathered where a Markdown reader puts it: between the image's start and its end.
            Event::Text(chunk) if inside > 0 => {
                if let Some(picture) = out.last_mut() {
                    if !picture.alt.is_empty() {
                        picture.alt.push(' ');
                    }
                    picture.alt.push_str(&chunk);
                }
            }
            _ => {}
        }
    }
    // **The alt text is the caption when it has one**, and the title otherwise. An alt text of nothing but
    // space (`![ ](f.png "The fog")`) is no alt text: a picture that names itself only by its title would
    // otherwise be captioned `" "`, which says less than the title does.
    for picture in &mut out {
        picture.caption = if picture.alt.trim().is_empty() {
            std::mem::take(&mut picture.title)
        } else {
            std::mem::take(&mut picture.alt)
        };
    }
    out
}

/// **The project-relative path a cited destination names**, or `None` when it names no file of this project:
/// an absolute URL, a `data:` payload, a path climbing out of the project.
///
fn resolve(file: &str, cited: &str) -> Option<String> {
    if cited.is_empty() || cited.contains("://") || cited.starts_with('/') || cited.starts_with('#') {
        return None;
    }
    let mut parts: Vec<&str> = file.split('/').collect();
    parts.pop();
    for segment in cited.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}
