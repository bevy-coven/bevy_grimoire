//! **A place's dream board as a registered operation** (SG-03, #402): `board_set`.
//!
//! A board is the pictures that show how a place should feel (`bevy_grimoire_core::board`): `board/board.yaml`
//! inside the place, each picture with a caption saying what to take from it. This operation is the only way the
//! editor changes one, so a person's drop and an agent's call are one change with one log line and one undo. It
//! replaced `reference_set` and the `maps/<map>.references.yaml` beside a map: the level's board is the open map's.
//!
//! Four actions: `add`, `remove`, `caption` (says again what to take from a picture) and `move` (reorders). The
//! place is `place`, or the level of the open map when it is left out.
//!
//! **A picture stays where it is.** `add` with a path inside the project references it where it is; an absolute path to a
//! file outside it (a drop) is copied into the place's `board/` first, and a different file already holding that name
//! is refused rather than overwritten. **Only a person's drop copies**: an agent's call (`Ctx::source` is not `Human`) naming a
//! path outside the project is refused, naming the path, because it would let a prompt pull any readable file into a repository.
//! **A restricted picture never ships**: one under `assets/` is refused, and a restricted one is never copied anywhere a project ships.
//!
//! Its inverse is the same operation sent a parameter only an undo may send (`put_back`: the board as it was).
//! Undo does not delete a picture file that an `add` copied in: it is the person's file now.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy_grimoire_core::board::{self, Board, Entry};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Shown, Source, Undo};

/// **A board as it stood**, for an undo. `None` is "there was no board file", which an undo puts back by removing it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutBack {
    pub place: String,
    pub board: Option<Board>,
}

/// **What the call does.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Add,
    Remove,
    Caption,
    Move,
}

impl Action {
    fn word(self) -> &'static str {
        match self {
            Action::Add => "add",
            Action::Remove => "remove",
            Action::Caption => "caption",
            Action::Move => "move",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// What to do. Left out when this is an undo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
    /// The place whose board it is: `design`, `design/levels/<map>`, `design/regions/<r>`, an area. The open map's level when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// The picture: a project-relative path, or (for a person's drop, by `add`) an absolute path to a file outside the project, which is copied in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// What to take from the picture, in a line (`add`, `caption`). Not what it shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restricted: Option<bool>,
    /// `add`: where it goes (0 is first); the end when left out. `move`: where it moves to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<usize>,
    /// Undo only: the board as it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "action": {"type": "string", "enum": ["add", "remove", "caption", "move"], "description": "add a picture, remove one, say again what to take from one (caption), or reorder (move)."},
                "place": {"type": "string", "description": "The place whose board: `design` (the game), `design/levels/<map>`, `design/regions/<region>`, or an area. Omit it for the level of the open map."},
                "path": {"type": "string", "description": "The picture: its path inside the project (png, jpg, jpeg, webp or gif), referenced where it is. An absolute path to a file outside the project is refused when an agent sends it: only a person's drop copies one into the place's board/ folder."},
                "caption": {"type": "string", "description": "What to take from the picture, in a line (\"the fog's colour, not its density\"): not what it shows. Required for add and caption; empty is refused."},
                "source": {"type": "string", "description": "add: where the picture came from, a URL."},
                "creator": {"type": "string", "description": "add: who made it."},
                "licence": {"type": "string", "description": "add: what it may be used under."},
                "restricted": {"type": "boolean", "description": "add: a picture the project may look at and may not ship (a store screenshot, a film poster). A restricted picture under assets/ is refused."},
                "at": {"type": "integer", "minimum": 0, "description": "add: the place in the order (0 is first), the end when omitted. move: the place it moves to."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        let p: Params = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!(
                "these are not a board change: {e}. The fields are `action` (add, remove, caption or move), `place`, `path`, `caption`, `source`, `creator`, `licence`, `restricted` and `at`, and nothing else."
            ))
        })?;
        match (&p.action, &p.put_back) {
            (Some(_), Some(_)) => Err(Refusal::new("a board change gives an `action`, or is an undo, and not both. Nothing was changed.")),
            (None, None) => Err(Refusal::new("a board change gives an `action`: add, remove, caption or move. Nothing was changed.")),
            _ => Ok(p),
        }
    }

    fn undo_only(&self) -> bool {
        self.put_back.is_some()
    }
}

/// **Where a board's file is**, for a place.
fn path_of(root: &Path, place: &str) -> PathBuf {
    root.join(board::file_of(place))
}

/// **The place a call is about**: the one it names, or the level of the open map.
fn place_of(ctx: &Ctx, p: &Params) -> Result<String, Refusal> {
    if let Some(place) = &p.place {
        let place = place.trim_end_matches('/');
        return if board::is_place(place) {
            Ok(place.to_owned())
        } else {
            Err(Refusal::new(format!(
                "`{place}` is not a place of the design tree (`design`, `design/levels/<map>`, `design/regions/<r>`, `design/levels/<map>/areas/<a>`). Nothing was changed."
            )))
        };
    }
    let (Some(open), Some(root)) = (ctx.open.as_ref(), ctx.root_or_project()) else {
        return Err(Refusal::new("no map is open, so there is no level whose board this is: name a `place`. Nothing was changed."));
    };
    Ok(board::level_place(&root, &open.map.name))
}

/// **Every board of the project, by place**, which is the state an undo checks.
fn boards_of(root: &Path) -> BTreeMap<String, Board> {
    board::places_with_a_board(root).into_iter().filter_map(|place| Some((place.clone(), Board::of_place(root, &place).ok()?))).collect()
}

fn state_of_project(ctx: &Ctx) -> Option<serde_json::Value> {
    let root = ctx.root_or_project()?;
    serde_json::to_value(boards_of(&root)).ok()
}

/// **One picture as an agent reads it**: its fields, whether its file is on this machine, and whether it still needs a caption.
fn entry_json(root: &Path, e: &Entry) -> serde_json::Value {
    let mut v = serde_json::to_value(e).unwrap_or_else(|_| json!({"path": e.path}));
    v["missing"] = (!root.join(&e.path).is_file()).into();
    v["uncaptioned"] = e.is_uncaptioned().into();
    v
}

/// **What an agent reads in `grimoire_state.board`**: the level of the open map, and every place that has a board with
/// its pictures in order, each with whether its file is here.
pub fn state_json(open: Option<&crate::project::OpenMap>, root: &Path) -> serde_json::Value {
    let level = open.map(|o| board::level_place(root, &o.map.name));
    let mut places = board::places_with_a_board(root);
    if let Some(level) = &level
        && !places.contains(level)
    {
        places.push(level.clone());
        places.sort();
    }
    let places: Vec<serde_json::Value> = places
        .iter()
        .map(|place| match Board::of_place(root, place) {
            Ok(b) => json!({"place": place, "file": board::file_of(place), "pictures": b.pictures.iter().map(|e| entry_json(root, e)).collect::<Vec<_>>()}),
            Err(e) => json!({"place": place, "file": board::file_of(place), "problem": e}),
        })
        .collect();
    json!({
        "level": level,
        "places": places,
        "cite_as": "name a picture by its path in a note's summary or assumptions, e.g. `board picture design/levels/yard/board/court.png: how the roofs step down the hill`",
    })
}

/// **A dropped file**: where it is, the project-relative path the board names it by, and whether it must be copied there.
struct Dropped {
    from: PathBuf,
    rel: String,
    copy: bool,
}

/// **Where a dropped file goes in the project**, or `None` when `path` is a path in the project to be referenced where it is.
fn drop_target(root: &Path, place: &str, path: &str, source: Source) -> Result<Option<Dropped>, Refusal> {
    let given = Path::new(path);
    if !given.is_absolute() {
        return Ok(None);
    }
    let shown = given.display();
    let outside = || {
        Refusal::new(format!(
            "{shown} is outside the project, and only a person's drop copies a file in from there: name a picture inside the project, or ask the person to drop it. Nothing was changed."
        ))
    };
    // **Only a person's drop pulls a file in from outside the project** (#419): an agent that could would let a prompt copy any
    // readable image under the user's home into a repository. **And it is refused before the disk is asked about the path**,
    // so the refusal is the same sentence whether the file is there or not and an agent cannot probe the host for files.
    // A `..` is outside by this check (it could climb out); a link inside the project that leads out is refused below with the same words.
    let human = source == Source::Human;
    if !human {
        let climbs = given.components().any(|c| matches!(c, std::path::Component::ParentDir));
        let under_root = given.starts_with(root) || root.canonicalize().is_ok_and(|canon_root| given.starts_with(canon_root));
        if climbs || !under_root {
            return Err(outside());
        }
    }
    let canon = given.canonicalize().map_err(|e| Refusal::new(format!("{shown} cannot be read ({e}), so it is not on the board. Nothing was changed.")))?;
    let canon_root = root.canonicalize().map_err(|e| Refusal::new(format!("the project folder {} cannot be read ({e}), so {shown} is not on the board. Nothing was changed.", root.display())))?;
    let relative = |p: &Path| p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
    if let Ok(inside) = canon.strip_prefix(&canon_root) {
        return Ok(Some(Dropped { rel: relative(inside), from: canon, copy: false }));
    }
    if !human {
        return Err(outside());
    }
    let name = canon.file_name().ok_or_else(|| Refusal::new(format!("{shown} has no file name, so it is not on the board. Nothing was changed.")))?;
    let into = Path::new(place).join(board::BOARD_DIR).join(name);
    Ok(Some(Dropped { rel: relative(&into), from: canon, copy: true }))
}

/// **Copy a dropped file into the project**, unless it is already there. A different file holding the name is refused.
fn copy_in(root: &Path, from: &Path, rel: &str) -> Result<(), Refusal> {
    let dest = root.join(rel);
    let bytes = std::fs::read(from).map_err(|e| Refusal::new(format!("{} cannot be read ({e}), so it is not on the board. Nothing was changed.", from.display())))?;
    match std::fs::read(&dest) {
        Ok(there) if there == bytes => Ok(()),
        Ok(_) => Err(Refusal::new(format!("{rel} already holds a different picture, so {} was not copied over it. Nothing was changed.", from.display()))),
        Err(_) => {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| Refusal::new(format!("{} cannot be made ({e}), so {} is not on the board. Nothing was changed.", parent.display(), from.display())))?;
            }
            bevy_grimoire_core::atomic_file::write(&dest, &bytes)
                .map_err(|e| Refusal::new(format!("{} cannot be copied into {rel} ({e}), so it is not on the board. Nothing was changed.", from.display())))
        }
    }
}

fn need<'a>(v: &'a Option<String>, what: &str, action: Action) -> Result<&'a str, Refusal> {
    v.as_deref().ok_or_else(|| Refusal::new(format!("`{}` needs `{what}`. Nothing was changed.", action.word())))
}

/// What changed, and the file to look at.
struct Change {
    summary: String,
    place: String,
    before: Option<Board>,
}

fn apply(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
    let p = Params::parse(params)?;
    let Some(root) = ctx.root_or_project() else {
        return Err(Refusal::new("no project is open, so there is no board to change. Nothing was changed."));
    };
    let change = if let Some(pb) = &p.put_back {
        let file = path_of(&root, &pb.place);
        let before = file.is_file().then(|| Board::load(&file)).transpose().map_err(Refusal::new)?;
        if write {
            match &pb.board {
                Some(b) => b.save(&file).map_err(|e| Refusal::new(format!("the board file refused the change, so nothing was written: {e}")))?,
                None => match std::fs::remove_file(&file) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(Refusal::new(format!("{} could not be removed ({e}), so nothing was put back.", file.display()))),
                },
            }
        }
        Change { summary: format!("put back the board of {}", pb.place), place: pb.place.clone(), before }
    } else {
        let action = p.action.ok_or_else(|| Refusal::new("a board change gives an `action`. Nothing was changed."))?;
        let place = place_of(ctx, &p)?;
        let file = path_of(&root, &place);
        let exists = file.is_file();
        let mut board = Board::load(&file).map_err(Refusal::new)?;
        let before = exists.then(|| board.clone());
        let mut copy: Option<(PathBuf, String)> = None;
        let summary = match action {
            Action::Add => {
                let given = need(&p.path, "path", action)?;
                let caption = need(&p.caption, "caption", action)?;
                let rel = match drop_target(&root, &place, given, ctx.source)? {
                    Some(dropped) => {
                        // The destination is checked as a board path before anything is copied.
                        board::normalise(&dropped.rel).map_err(Refusal::new)?;
                        if dropped.copy {
                            copy = Some((dropped.from, dropped.rel.clone()));
                        }
                        dropped.rel
                    }
                    None => given.to_owned(),
                };
                let entry = Entry {
                    path: rel,
                    caption: caption.to_owned(),
                    source: p.source.clone(),
                    creator: p.creator.clone(),
                    licence: p.licence.clone(),
                    restricted: p.restricted.unwrap_or(false),
                };
                let added = board::normalise(&entry.path).map_err(Refusal::new)?;
                board.add(entry, p.at).map_err(Refusal::new)?;
                format!("added {added} to the board of {place}")
            }
            Action::Remove => {
                let (_, entry) = board.remove(need(&p.path, "path", action)?).map_err(Refusal::new)?;
                format!("removed {} from the board of {place}", entry.path)
            }
            Action::Caption => {
                let path = need(&p.path, "path", action)?;
                board.recaption(path, need(&p.caption, "caption", action)?).map_err(Refusal::new)?;
                format!("re-captioned {path} on the board of {place}")
            }
            Action::Move => {
                let path = need(&p.path, "path", action)?;
                let to = p.at.ok_or_else(|| Refusal::new("`move` needs `at`, the place it moves to. Nothing was changed."))?;
                board.move_to(path, to).map_err(Refusal::new)?;
                format!("moved {path} to place {to} on the board of {place}")
            }
        };
        if write {
            if let Some((from, rel)) = &copy {
                copy_in(&root, from, rel)?;
            }
            board.save(&file).map_err(|e| Refusal::new(format!("the board file refused the change, so nothing was written: {e}")))?;
        }
        Change { summary, place, before }
    };
    if !write {
        return Ok(None);
    }
    let inverse = json!({"put_back": PutBack { place: change.place.clone(), board: change.before }});
    Ok(Some(Outcome { summary: change.summary, affected: 1, saved: true, inverse: Some(inverse), after: state_of_project(ctx) }))
}

/// **Add a picture to a place's board, remove one, say again what to take from one, or reorder.**
pub struct BoardSet;

impl Operation for BoardSet {
    fn name(&self) -> &'static str {
        "board_set"
    }
    fn shows(&self, ctx: &Ctx, params: &serde_json::Value) -> Shown {
        // **A board is a file of the design tree**, so that is where a person sees it until the board is drawn (SG-05).
        // An empty or refused call still has a place a person looks: the open map's level, or the game's own board.
        let named = Params::parse(params).ok().and_then(|p| p.put_back.as_ref().map(|b| b.place.clone()).or_else(|| place_of(ctx, &p).ok()));
        let place = named
            .or_else(|| Some(board::level_place(&ctx.root_or_project()?, &ctx.open.as_ref()?.map.name)))
            .unwrap_or_else(|| bevy_grimoire_core::design::DESIGN_DIR.to_owned());
        Shown::File { path: board::file_of(&place), lines: None }
    }
    fn writes(&self) -> &'static [&'static str] {
        // The board file, and the picture a drop copies in beside it.
        &["board", "reference_images"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        Params::parse(params).is_ok_and(|p| p.undo_only())
    }
    fn schema(&self) -> serde_json::Value {
        Params::schema()
    }
    fn state(&self, ctx: &Ctx) -> Option<serde_json::Value> {
        state_of_project(ctx)
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let mut scratch = Ctx { open: ctx.open.clone(), project: ctx.project.clone(), root: ctx.root.clone(), source: ctx.source, ..Ctx::default() };
        apply(&mut scratch, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        apply(ctx, params, true)?.ok_or_else(|| Refusal::new("the board change was planned and not written, so nothing was changed."))
    }
}
