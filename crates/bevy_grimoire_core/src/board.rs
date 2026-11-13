//! **A dream board for every place** (SG-03, #402; the series #399): the pictures that show how a place should feel.
//!
//! A place of the design tree ([`crate::design`]) keeps its board in `<place>/board/board.yaml`, beside the
//! Markdown that says what the place is like. An [`Entry`] holds a project-relative picture `path`, a
//! `caption`, and optionally the `source` (a URL), `creator`, `licence` and `restricted` that Sanctuary's own
//! mood board manifest already records — so a game's existing pile maps across one entry per picture. **The
//! level's board is the open map's board**: it replaced `maps/<map>.references.yaml`, which nothing reads now.
//!
//! # The caption says what to take, not what is shown
//!
//! A caption is *what to take from the picture* — "the fog's colour, not its density", "how the roofs step down
//! the hill" — and never a description of it. Browsing galleries leads to design drift, and seeing too many
//! examples of one style leads to fixation (*GANSpiration*, 10.48550/arXiv.2203.03827); a curated board per place
//! with one stated reason per picture works against both. So every **writer** ([`Board::add`],
//! [`Board::recaption`]) refuses an empty caption. **The reader does not**: a board a person wrote by hand, or
//! one migrated from a reference that never had a caption, still reads, and [`Board::uncaptioned`] lists what
//! has none so the state can say so — a file that refused to read for a missing caption would hide every other
//! picture on the board.
//!
//! # A picture stays where it is
//!
//! A picture's `path` is relative to the project root and may point **anywhere in it**: a file already in the
//! project is referenced in place (Sanctuary's 1,385 mood board images are never copied), and one dropped from
//! outside is copied into the place's `board/` by the operation. A picture missing on this machine stays on the
//! board — [`Board::missing`] lists it and nothing removes it.
//!
//! # A restricted picture never ships
//!
//! `restricted: true` marks a picture the project may look at and may not ship (a store screenshot, a film
//! poster). **`assets/` is what ships**, so [`Board::add`] refuses a restricted entry whose path is under it, and
//! [`ships`] is the one test for that.
//!
//! Engine-free: `std` and `serde`.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The version `board.yaml` states.
pub const BOARD_VERSION: u32 = 1;

/// **The folder of a place's board**, inside the place.
pub const BOARD_DIR: &str = "board";

/// **The file that lists a board's pictures**, inside [`BOARD_DIR`].
pub const BOARD_FILE: &str = "board.yaml";

/// **The folder a project ships**, which a restricted picture may not be in.
pub const SHIPPED_DIR: &str = "assets";

/// The words a citation of a board picture starts with: `board picture design/levels/yard/board/court.png`.
pub const CITE_PREFIX: &str = "board picture ";

/// **The caption a dropped picture carries until a person says what to take from it** (SG-03, #402).
///
/// A drop has no caption and a writer refuses an empty one, so the picture goes on the board with this line, which
/// [`Board::uncaptioned`] lists and `grimoire_state.board` marks `uncaptioned: true`: the board is not pretending the
/// picture has been looked at. `board_set` with `action: "caption"` replaces it.
pub const PENDING_CAPTION: &str = "(caption needed: what to take from this picture)";

/// The extensions a board picture has: what [`crate::image_header`] reads.
const IMAGE_EXTS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];

/// **The deepest the design folder is walked for boards**, in folders: `design/regions/<r>/levels/<m>/areas/<a>`
/// is six below the root, and a board is one more. A walk with no bound would follow a symlink loop forever.
const WALK_DEPTH_MAX: usize = 8;

/// **One picture on a board.**
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Relative to the project root, `/`-separated, normalised.
    pub path: String,
    /// **What to take from the picture**, in a line: not what it shows.
    pub caption: String,
    /// Where the picture came from, a URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Who made it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    /// What it may be used under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licence: Option<String>,
    /// **A picture the project may look at and may not ship.** Shown with a badge, and never under `assets/`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub restricted: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Entry {
    /// Whether this picture still has no line saying what to take from it.
    pub fn is_uncaptioned(&self) -> bool {
        self.caption.trim().is_empty() || self.caption == PENDING_CAPTION
    }

    /// An entry with a path and a caption and nothing else.
    pub fn new(path: &str, caption: &str) -> Entry {
        Entry { path: path.to_owned(), caption: caption.to_owned(), source: None, creator: None, licence: None, restricted: false }
    }
}

/// **A place's board**, in the order its pictures are shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Board {
    pub version: u32,
    pub pictures: Vec<Entry>,
}

impl Default for Board {
    fn default() -> Self {
        Board { version: BOARD_VERSION, pictures: Vec::new() }
    }
}

/// **Whether a project-relative path is under the folder a project ships.**
pub fn ships(path: &str) -> bool {
    path.split('/').next() == Some(SHIPPED_DIR) && path.contains('/')
}

/// **A project-relative image path** with `.` removed; refuses an absolute path, `..` and a non-image.
pub fn normalise(path: &str) -> Result<String, String> {
    use std::path::Component;
    let mut parts = Vec::new();
    for c in Path::new(path).components() {
        match c {
            Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return Err(format!("board: {path:?} must be a path inside the project")),
        }
    }
    let Some(last) = parts.last() else {
        return Err("board: a picture needs a path".to_owned());
    };
    let ext = last.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    if !IMAGE_EXTS.contains(&ext.as_str()) {
        return Err(format!("board: {path:?} is not an image ({})", IMAGE_EXTS.join(", ")));
    }
    Ok(parts.join("/"))
}

/// **The board file of a place**, relative to the project root: `design/levels/yard/board/board.yaml`.
pub fn file_of(place: &str) -> String {
    format!("{place}/{BOARD_DIR}/{BOARD_FILE}")
}

/// **Whether `path` names a place of the design tree** by its shape: the game, a region, a level or an area.
///
/// The shape only: whether the folder exists, and whether a level's map does, is [`crate::design::Tree`]'s to say.
/// A board is written for a place that may have no folder yet.
pub fn is_place(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    let word = |s: &&str| !s.is_empty() && !s.starts_with('.') && !s.contains(['\\', ':']);
    if !parts.iter().all(word) {
        return false;
    }
    use crate::design::{AREAS_DIR, DESIGN_DIR, LEVELS_DIR, REGIONS_DIR};
    matches!(
        parts.as_slice(),
        [DESIGN_DIR]
            | [DESIGN_DIR, REGIONS_DIR, _]
            | [DESIGN_DIR, LEVELS_DIR, _]
            | [DESIGN_DIR, REGIONS_DIR, _, LEVELS_DIR, _]
            | [DESIGN_DIR, LEVELS_DIR, _, AREAS_DIR, _]
            | [DESIGN_DIR, REGIONS_DIR, _, LEVELS_DIR, _, AREAS_DIR, _]
    )
}

/// **The folder of the level a map is**, relative to the project root.
///
/// A level's folder is named after its map: under `design/levels/` or under a region's `levels/`. The one that
/// exists is the answer; a map with no folder yet is the level in no region, `design/levels/<map>`, which is where
/// its first board is written.
pub fn level_place(root: &Path, map: &str) -> String {
    use crate::design::{DESIGN_DIR, LEVELS_DIR, REGIONS_DIR};
    let none = format!("{DESIGN_DIR}/{LEVELS_DIR}/{map}");
    if root.join(&none).is_dir() {
        return none;
    }
    let regions = root.join(DESIGN_DIR).join(REGIONS_DIR);
    let mut names: Vec<String> = std::fs::read_dir(&regions)
        .map(|entries| entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    names.sort();
    for region in names {
        let path = format!("{DESIGN_DIR}/{REGIONS_DIR}/{region}/{LEVELS_DIR}/{map}");
        if root.join(&path).is_dir() {
            return path;
        }
    }
    none
}

/// **The places under `root` that have a board file**, as folders relative to the root, sorted.
///
/// A walk of the folders, not of the Markdown: [`crate::design::Tree::read`] reads every style file, and the
/// header check asks this once a second.
pub fn places_with_a_board(root: &Path) -> Vec<String> {
    fn walk(root: &Path, rel: &str, depth: usize, out: &mut Vec<String>) {
        if depth > WALK_DEPTH_MAX {
            return;
        }
        let Ok(entries) = std::fs::read_dir(root.join(rel)) else { return };
        let mut folders: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // A symlink is not followed: a board is a file of this project.
            let Ok(meta) = std::fs::symlink_metadata(entry.path()) else { continue };
            if name.starts_with('.') || !meta.is_dir() {
                continue;
            }
            folders.push(name);
        }
        folders.sort();
        for name in folders {
            if name == BOARD_DIR {
                if root.join(rel).join(BOARD_DIR).join(BOARD_FILE).is_file() {
                    out.push(rel.to_owned());
                }
                continue;
            }
            walk(root, &format!("{rel}/{name}"), depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(root, crate::design::DESIGN_DIR, 0, &mut out);
    out
}

impl Board {
    pub fn parse(text: &str) -> Result<Board, String> {
        let b: Board = crate::yaml::from_str(text, "board").map_err(|p| format!("board: parse: {}", p.line()))?;
        if b.version != BOARD_VERSION {
            return Err(format!("board: version {} is not the {BOARD_VERSION} this build reads", b.version));
        }
        Ok(b)
    }

    /// **The file as the one YAML writer writes it.**
    pub fn to_yaml(&self) -> Result<String, String> {
        crate::yaml::to_string(self).map_err(|p| format!("board: {}", p.line()))
    }

    /// A file that is not there is no board, not an error.
    pub fn load(path: &Path) -> Result<Board, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Board::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Board::default()),
            Err(e) => Err(format!("board: {}: {e}", path.display())),
        }
    }

    /// **The board of the place `place`** under `root`.
    pub fn of_place(root: &Path, place: &str) -> Result<Board, String> {
        Board::load(&root.join(file_of(place)))
    }

    /// Atomic; no pictures and no file writes nothing. The folder is made.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if self.pictures.is_empty() && !path.exists() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("board: {}: {e}", parent.display()))?;
        }
        crate::yaml::save(path, &self.to_yaml()?)
    }

    pub fn get(&self, path: &str) -> Option<&Entry> {
        self.pictures.iter().find(|p| p.path == path)
    }

    fn position(&self, path: &str) -> Result<usize, String> {
        let path = normalise(path)?;
        self.pictures.iter().position(|p| p.path == path).ok_or_else(|| format!("board: {path} is not on this board"))
    }

    /// **Adds a picture**, at the end or at `at`. The same path twice is one picture and the error names it; an
    /// empty caption is refused; a restricted picture under `assets/` is refused.
    pub fn add(&mut self, mut entry: Entry, at: Option<usize>) -> Result<(), String> {
        entry.path = normalise(&entry.path)?;
        entry.caption = entry.caption.trim().to_owned();
        if entry.caption.is_empty() {
            return Err(format!(
                "board: {} needs a caption saying what to take from it (\"the fog's colour, not its density\"), not what it shows",
                entry.path
            ));
        }
        if self.get(&entry.path).is_some() {
            return Err(format!("board: {} is already on this board", entry.path));
        }
        if entry.restricted && ships(&entry.path) {
            return Err(format!("board: {} is restricted and {SHIPPED_DIR}/ is what ships, so it cannot be put there", entry.path));
        }
        let at = at.unwrap_or(self.pictures.len());
        if at > self.pictures.len() {
            return Err(format!("board: place {at} is past the end of a board of {}", self.pictures.len()));
        }
        self.pictures.insert(at, entry);
        Ok(())
    }

    /// Takes a picture off the board, and says where it stood.
    pub fn remove(&mut self, path: &str) -> Result<(usize, Entry), String> {
        let i = self.position(path)?;
        Ok((i, self.pictures.remove(i)))
    }

    /// **Says again what to take from a picture.** An empty caption is refused.
    pub fn recaption(&mut self, path: &str, caption: &str) -> Result<(), String> {
        let i = self.position(path)?;
        let caption = caption.trim();
        if caption.is_empty() {
            return Err(format!("board: {} needs a caption saying what to take from it, not an empty one", self.pictures[i].path));
        }
        self.pictures[i].caption = caption.to_owned();
        Ok(())
    }

    /// Moves a picture to place `to` (counted from 0 in the order after it is taken out).
    pub fn move_to(&mut self, path: &str, to: usize) -> Result<(), String> {
        let i = self.position(path)?;
        if to >= self.pictures.len() {
            return Err(format!("board: place {to} is past the last of {} pictures", self.pictures.len()));
        }
        let entry = self.pictures.remove(i);
        self.pictures.insert(to, entry);
        Ok(())
    }

    /// The pictures whose file is not under `root`. **They stay on the board**: a project may name a file it is
    /// about to add, and a picture that is missing on this machine is not missing from the game's style.
    pub fn missing(&self, root: &Path) -> Vec<&Entry> {
        self.pictures.iter().filter(|p| !root.join(&p.path).is_file()).collect()
    }

    /// The pictures that do not yet say what to take from them: an empty caption (a hand edit, a migrated reference
    /// that had none) or the [`PENDING_CAPTION`] a drop leaves.
    pub fn uncaptioned(&self) -> Vec<&Entry> {
        self.pictures.iter().filter(|p| p.is_uncaptioned()).collect()
    }

    /// The restricted pictures that sit where the project ships.
    pub fn shipping_restricted(&self) -> Vec<&Entry> {
        self.pictures.iter().filter(|p| p.restricted && ships(&p.path)).collect()
    }

    /// **The line an agent writes into a note's summary or an assumption to say which picture it used.**
    pub fn cite(&self, path: &str) -> Result<String, String> {
        let e = self.get(path).ok_or_else(|| format!("board: {path} is not on this board"))?;
        Ok(format!("{CITE_PREFIX}{}: {}", e.path, e.caption))
    }
}

/// **The picture paths `texts` cite** as `board picture <path>`, in first-cited order, each once.
pub fn citations<'a>(texts: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for text in texts {
        let mut rest = text;
        while let Some(at) = rest.find(CITE_PREFIX) {
            rest = &rest[at + CITE_PREFIX.len()..];
            let word: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
            let path = word.trim_end_matches([':', ',', ';', '.', ')']).to_owned();
            if !path.is_empty() && !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_on_a_board_once_and_says_what_to_take() {
        let mut b = Board::default();
        b.add(Entry::new("art/fog.jpg", "the fog's colour, not its density"), None).unwrap();
        assert!(b.add(Entry::new("art/./fog.jpg", "again"), None).unwrap_err().contains("already"));
        assert!(b.add(Entry::new("art/x.png", "  "), None).unwrap_err().contains("caption"));
        for bad in ["/etc/x.png", "../x.png", "notes.txt", ""] {
            assert!(b.add(Entry::new(bad, "x"), None).is_err(), "{bad}");
        }
        assert_eq!(b.pictures.len(), 1);
    }

    #[test]
    fn a_citation_is_found_by_its_path() {
        let cited = citations(["matched board picture art/fog.jpg: the fog, and board picture a/b.png, too"]);
        assert_eq!(cited, ["art/fog.jpg", "a/b.png"]);
    }

    #[test]
    fn a_place_is_a_shape_of_the_design_tree() {
        for good in ["design", "design/regions/north", "design/levels/yard", "design/regions/n/levels/h", "design/levels/y/areas/docks"] {
            assert!(is_place(good), "{good}");
        }
        for bad in ["", "design/", "design/regions", "design/topics/x", "assets", "design/../x", "design/levels/.hidden"] {
            assert!(!is_place(bad), "{bad}");
        }
    }
}
