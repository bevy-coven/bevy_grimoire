//! **An area of a level, as a registered operation** (SG-06, #405): `design_area_set`.
//!
//! An area is a rectangle on its level's map (`bevy_grimoire_core::design`), and it is the **design's data, not the
//! map's**: its `area.yaml` lives in the design tree beside the area's own `style.md` and board, so a map file is
//! unchanged by having areas and a game that reads its map reads nothing new. This operation is the only way the editor
//! changes one, so a person's typed corners and an agent's call are one change with one log line and one undo.
//!
//! `{map, name, rect}` creates the area, or moves or resizes it when it is there; with no `rect` it is removed. `map` is the
//! open map when it is left out.
//!
//! **Overlap is refused at the door**, naming both areas, so [`bevy_grimoire_core::design::Tree::problems`]'s overlap
//! problem only ever comes from a hand edit. A rectangle outside its map's floor is refused too, for the same reason.
//!
//! **Removing an area that has anything but its `area.yaml` never deletes it**: the folder, with its `style.md` and its
//! board, is moved to `.dark_factory/removed-areas/<name>/` (a numeral is added to the name when that is taken), and the
//! undo moves it back. An area that is only its rectangle is simply taken away.
//!
//! Its inverse is the same operation sent a parameter only an undo may send (`put_back`: the rectangle as it was, or
//! none, and the folder to move back).

use std::path::Path;

use bevy_grimoire_core::design::{self, AREA_FILE, AreaOnMap};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Shown, Undo};

/// **How many folders of one name `removed-areas/` is searched through** before a removal gives up finding a free one.
const REMOVED_NAME_TRIES_MAX: u32 = 1000;

/// **An area as it stood**, for an undo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutBack {
    pub map: String,
    pub name: String,
    /// The rectangle it had; `None` is "there was no such area", which an undo puts back by taking it away.
    pub rect: Option<[f32; 4]>,
    /// The folder, relative to the project root, a removal moved the area to: an undo moves it back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved_to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// The map whose level the area is on. The open map when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<String>,
    /// The area's name: letters, digits, `_` and `-`. It is the area's folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `[x0, z0, x1, z1]` in map metres. Left out, the area is removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<[f32; 4]>,
    /// Undo only: the area as it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["name"],
            "properties": {
                "map": {"type": "string", "description": "The map whose level the area is on. The open map when omitted."},
                "name": {"type": "string", "description": "The area's name, which is its folder under the level's `areas/`: letters, digits, `_` and `-`."},
                "rect": {
                    "type": "array", "items": {"type": "number"}, "minItems": 4, "maxItems": 4,
                    "description": "[x0, z0, x1, z1] in map metres, x0 < x1 and z0 < z1, on the map's floor and overlapping no other area of the level. Creates the area, or moves or resizes it when it is there. Omit it to remove the area: one with a page or a board is moved to `.dark_factory/removed-areas/<name>/`, never deleted."
                }
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        let p: Params = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("these are not an area change: {e}. The fields are `map`, `name` and `rect` (`[x0, z0, x1, z1]`, left out to remove the area), and nothing else."))
        })?;
        match (&p.name, &p.put_back) {
            (Some(_), Some(_)) => Err(Refusal::new("an area change names an area, or is an undo, and not both. Nothing was changed.")),
            (None, None) => Err(Refusal::new("an area change gives the area's `name`. Nothing was changed.")),
            _ => Ok(p),
        }
    }

    fn undo_only(&self) -> bool {
        self.put_back.is_some()
    }
}

/// **The level folder a call is about**, and the map it names.
fn level_of(ctx: &Ctx, root: &Path, map: &Option<String>) -> Result<(String, String), Refusal> {
    let map = match map.clone().or_else(|| ctx.open.as_ref().map(|o| o.map.name.clone())) {
        Some(map) => map,
        None => return Err(Refusal::new("no map is open, so there is no level whose area this is: name a `map`. Nothing was changed.")),
    };
    Ok((bevy_grimoire_core::board::level_place(root, &map), map))
}

/// **The floor of the map an area is on**: the open map's when it is that one, else the saved file's.
fn floor_of(ctx: &Ctx, root: &Path, map: &str) -> Result<[f32; 4], Refusal> {
    if let Some(open) = ctx.open.as_ref().filter(|o| o.map.name == map) {
        let (x0, z0, x1, z1) = open.map.floor_rect();
        return Ok([x0, z0, x1, z1]);
    }
    design::floor_of_map(root, map).ok_or_else(|| Refusal::new(format!("the project has no map `{map}` that reads, so there is no level to draw an area on. Nothing was changed.")))
}

/// **Every area of every level**, by folder, which is the state an undo checks.
fn areas_by_folder(root: &Path) -> std::collections::BTreeMap<String, [f32; 4]> {
    design::levels_of(root).iter().flat_map(|level| design::areas_of_level(root, level)).map(|a| (a.path, a.rect)).collect()
}

fn state_of_project(ctx: &Ctx) -> Option<serde_json::Value> {
    let root = ctx.root_or_project()?;
    serde_json::to_value(areas_by_folder(&root)).ok()
}

fn valid_rect(rect: [f32; 4]) -> Result<(), Refusal> {
    let [x0, z0, x1, z1] = rect;
    if !rect.iter().all(|v| v.is_finite()) {
        return Err(Refusal::new("a corner of the area is not a number. Nothing was changed."));
    }
    if x0 >= x1 || z0 >= z1 {
        return Err(Refusal::new(format!(
            "the rectangle [{x0}, {z0}, {x1}, {z1}] is empty or the wrong way round: an area is `[x0, z0, x1, z1]` with x0 < x1 and z0 < z1, because a rectangle with no floor holds no cell. Nothing was changed."
        )));
    }
    Ok(())
}

/// **A free folder under `removed-areas/`**, relative to the root: `<name>`, then `<name>-2`, `<name>-3` …
fn free_removed_folder(root: &Path, name: &str) -> Result<String, Refusal> {
    for n in 1..=REMOVED_NAME_TRIES_MAX {
        let leaf = if n == 1 { name.to_owned() } else { format!("{name}-{n}") };
        let rel = format!("{}/{leaf}", design::REMOVED_AREAS_DIR);
        if !root.join(&rel).exists() {
            return Ok(rel);
        }
    }
    Err(Refusal::new(format!("`{}` already holds {REMOVED_NAME_TRIES_MAX} folders named after `{name}`, so this one could not be kept there. Nothing was changed.", design::REMOVED_AREAS_DIR)))
}

/// **Whether an area's folder holds anything besides its `area.yaml`.**
fn has_more_than_its_rectangle(folder: &Path) -> bool {
    std::fs::read_dir(folder).map(|entries| entries.flatten().any(|e| e.file_name() != AREA_FILE)).unwrap_or(false)
}

fn write_rect(folder: &Path, rect: [f32; 4]) -> Result<(), Refusal> {
    std::fs::create_dir_all(folder).map_err(|e| Refusal::new(format!("{} cannot be made ({e}), so nothing was written.", folder.display())))?;
    let text = design::area_yaml(rect).map_err(Refusal::new)?;
    bevy_grimoire_core::yaml::save(&folder.join(AREA_FILE), &text).map_err(|e| Refusal::new(format!("the area file refused the change, so nothing was written: {e}")))
}

/// **Take an area's rectangle away**: its `area.yaml`, and the folder when that leaves it empty.
fn take_rect_away(folder: &Path) -> Result<(), Refusal> {
    match std::fs::remove_file(folder.join(AREA_FILE)) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Refusal::new(format!("{} could not be removed ({e}), so nothing was changed.", folder.join(AREA_FILE).display()))),
    }
    // Empty and nothing more: a folder that still holds a page stays, and `remove_dir` refuses a folder that is not empty.
    let _ = std::fs::remove_dir(folder);
    Ok(())
}

/// **Take an area away without losing a file**: a folder that holds anything besides its `area.yaml` is moved to a free folder under
/// `removed-areas/` and that folder is returned; one that is only its rectangle is taken away, and `None` is returned. The one rule a
/// removal and an undo of a creation share, so a page written beside a new area is never left folder-less or deleted.
fn set_aside(root: &Path, folder: &Path, folder_rel: &str, name: &str, write: bool) -> Result<Option<String>, Refusal> {
    if has_more_than_its_rectangle(folder) {
        let to = free_removed_folder(root, name)?;
        if write {
            bevy_grimoire_core::atomic_file::move_folder(folder, &root.join(&to))
                .map_err(|e| Refusal::new(format!("{folder_rel} could not be moved to {to} ({e}), so nothing was changed.")))?;
        }
        return Ok(Some(to));
    }
    if write {
        take_rect_away(folder)?;
    }
    Ok(None)
}

/// What changed.
struct Change {
    summary: String,
    inverse: PutBack,
}

fn say(rect: [f32; 4]) -> String {
    let [x0, z0, x1, z1] = rect;
    format!("[{x0}, {z0}, {x1}, {z1}]")
}

fn apply(ctx: &mut Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
    let p = Params::parse(params)?;
    let Some(root) = ctx.root_or_project() else {
        return Err(Refusal::new("no project is open, so there is no area to change. Nothing was changed."));
    };
    let change = if let Some(back) = &p.put_back {
        let (level, _) = level_of(ctx, &root, &Some(back.map.clone()))?;
        let folder_rel = design::area_folder(&level, &back.name);
        let folder = root.join(&folder_rel);
        let before = design::areas_of_level(&root, &level).into_iter().find(|a| a.name == back.name).map(|a| a.rect);
        // **Putting back "no area" is a removal**, with a removal's care: a page written beside the area since is moved aside, not left
        // without its rectangle or deleted. Its own inverse then moves that folder back, so an undo of an undo loses nothing.
        let mut moved_aside = None;
        match (&back.rect, &back.moved_to) {
            (Some(rect), Some(moved)) => {
                if folder.exists() {
                    return Err(Refusal::new(format!("{folder_rel} is there again, so the removed area was not moved back over it. Nothing was changed.")));
                }
                if write {
                    bevy_grimoire_core::atomic_file::move_folder(&root.join(moved), &folder)
                        .map_err(|e| Refusal::new(format!("{moved} could not be moved back to {folder_rel} ({e}), so nothing was put back.")))?;
                    write_rect(&folder, *rect)?;
                }
            }
            (Some(rect), None) => {
                if write {
                    write_rect(&folder, *rect)?;
                }
            }
            (None, _) => moved_aside = set_aside(&root, &folder, &folder_rel, &back.name, write)?,
        }
        Change {
            summary: format!("put back the area `{}` of {}", back.name, back.map),
            inverse: PutBack { map: back.map.clone(), name: back.name.clone(), rect: before, moved_to: moved_aside },
        }
    } else {
        let name = p.name.clone().unwrap_or_default();
        if !design::is_area_name(&name) {
            return Err(Refusal::new(format!(
                "`{name}` is not a name an area can have: letters, digits, `_` and `-`, at most {} of them, because it is a folder name and a label drawn on the map. Nothing was changed.",
                design::AREA_NAME_MAX_CHARS
            )));
        }
        let (level, map) = level_of(ctx, &root, &p.map)?;
        let folder_rel = design::area_folder(&level, &name);
        let folder = root.join(&folder_rel);
        let areas = design::areas_of_level(&root, &level);
        let standing: Option<&AreaOnMap> = areas.iter().find(|a| a.name == name);
        // A folder with an `area.yaml` that does not read is the person's: it is never written over.
        if standing.is_none() && folder.join(AREA_FILE).is_file() {
            return Err(Refusal::new(format!("{folder_rel}/{AREA_FILE} is there and does not read, so it was not written over. Fix or remove that file first. Nothing was changed.")));
        }
        match p.rect {
            Some(rect) => {
                valid_rect(rect)?;
                let floor = floor_of(ctx, &root, &map)?;
                let [fx0, fz0, fx1, fz1] = floor;
                if rect[0] < fx0 || rect[1] < fz0 || rect[2] > fx1 || rect[3] > fz1 {
                    return Err(Refusal::new(format!(
                        "the rectangle {} reaches outside the map `{map}`, whose floor is {}: every corner of an area is on the map. Nothing was changed.",
                        say(rect),
                        say(floor)
                    )));
                }
                let candidate = design::Area { version: design::AREA_VERSION, rect };
                if let Some(other) = areas.iter().find(|a| a.name != name && candidate.overlaps(&a.rect)) {
                    return Err(Refusal::new(format!(
                        "the area `{name}` at {} would overlap the area `{}` at {}: a cell belongs to one area, or its chain of style is ambiguous. Move one of them. Nothing was changed.",
                        say(rect),
                        other.name,
                        say(other.rect)
                    )));
                }
                let summary = match standing {
                    None => format!("created the area `{name}` on {map} at {}", say(rect)),
                    Some(s) if s.rect == rect => {
                        return Err(Refusal::new(format!("the area `{name}` is already at {}. Nothing was changed.", say(rect))));
                    }
                    Some(s) => {
                        let size = |r: [f32; 4]| (r[2] - r[0], r[3] - r[1]);
                        let verb = if size(s.rect) == size(rect) { "moved" } else { "resized" };
                        format!("{verb} the area `{name}` on {map} from {} to {}", say(s.rect), say(rect))
                    }
                };
                if write {
                    write_rect(&folder, rect)?;
                }
                Change { summary, inverse: PutBack { map, name, rect: standing.map(|s| s.rect), moved_to: None } }
            }
            None => {
                let Some(standing) = standing else {
                    return Err(Refusal::new(format!("the level of `{map}` has no area `{name}` to remove. Nothing was changed.")));
                };
                let moved_to = set_aside(&root, &folder, &folder_rel, &name, write)?;
                let summary = match &moved_to {
                    Some(to) => format!("removed the area `{name}` from {map}; its page and board are kept in {to}"),
                    None => format!("removed the area `{name}` from {map}"),
                };
                Change { summary, inverse: PutBack { map, name, rect: Some(standing.rect), moved_to } }
            }
        }
    };
    if !write {
        return Ok(None);
    }
    Ok(Some(Outcome { summary: change.summary, affected: 1, saved: true, inverse: Some(json!({"put_back": change.inverse})), after: state_of_project(ctx) }))
}

/// **Create, move, resize or remove an area of a level.**
pub struct DesignAreaSet;

impl Operation for DesignAreaSet {
    fn name(&self) -> &'static str {
        "design_area_set"
    }
    fn shows(&self, ctx: &Ctx, params: &serde_json::Value) -> Shown {
        // **An area is a box on the map**, the one `Shown::Map` frames (the follow and `grimoire_style`'s `at` share it). A
        // removal names where the area stood; a call with no rectangle to name shows the whole map.
        let rect = Params::parse(params).ok().and_then(|p| {
            p.rect.or(p.put_back.as_ref().and_then(|b| b.rect)).or_else(|| {
                let root = ctx.root_or_project()?;
                let (level, _) = level_of(ctx, &root, &p.map).ok()?;
                design::areas_of_level(&root, &level).into_iter().find(|a| Some(&a.name) == p.name.as_ref()).map(|a| a.rect)
            })
        });
        match rect {
            Some(rect) if valid_rect(rect).is_ok() => Shown::Map { rect, placements: Vec::new() },
            _ => Shown::MapWhole,
        }
    }
    fn writes(&self) -> &'static [&'static str] {
        &["design"]
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
        let mut scratch = Ctx { open: ctx.open.clone(), project: ctx.project.clone(), root: ctx.root.clone(), ..Ctx::default() };
        apply(&mut scratch, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        apply(ctx, params, true)?.ok_or_else(|| Refusal::new("the area change was planned and not written, so nothing was changed."))
    }
}

// ── style_edit ───────────────────────────────────────────────────────────────

/// **One small edit of a style page**: the exact span, and what replaces it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextEdit {
    /// **The exact text to replace**, which must be in the page exactly once.
    pub find: String,
    /// What takes its place (it may be empty, which takes the span out).
    pub replace: String,
}

/// **The page as it was**, for an undo: its whole old text, or none when there was no such page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StylePutBack {
    /// The old text, put back verbatim; `null` is "there was no page", which an undo puts back by taking the file away.
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleParams {
    /// The page, relative to the project root: `design/levels/<map>/style.md`, `design/levels/<map>/areas/<a>/dialogue.md`.
    pub path: String,
    /// Small edits of a page that is there, in order, each seeing the text the one before it left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<Vec<TextEdit>>,
    /// The whole text of a page that is not there yet (a new topic file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create: Option<String>,
    /// Undo only: the page as it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<StylePutBack>,
}

impl StyleParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["path"],
            "properties": {
                "path": {"type": "string", "description": "The page, relative to the project root: a Markdown file of a level or an area of the design tree, such as `design/levels/yard/style.md` or `design/levels/yard/areas/docks/dialogue.md`. A game or region page is changed only by a proposal (`grimoire_style_propose`); a board is changed only by `board_set` and an area's rectangle only by `design_area_set`."},
                "edits": {
                    "type": "array", "minItems": 1,
                    "items": {
                        "type": "object", "additionalProperties": false, "required": ["find", "replace"],
                        "properties": {
                            "find": {"type": "string", "description": "The exact text to replace. It must be in the page exactly once; a span that matches twice is refused with the count, and you make it longer until it matches once."},
                            "replace": {"type": "string", "description": "What takes its place; empty takes the span out."}
                        }
                    },
                    "description": "Small edits of a page that is there, applied in order. Not whole files: an exact span is what a person can review as a diff and what an undo can put back."
                },
                "create": {"type": "string", "description": "The whole text of a page that is not there yet (a new topic file). A topic below the game must be one the game already has. Give `edits` or `create`, not both."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<StyleParams, Refusal> {
        let p: StyleParams = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("these are not a style edit: {e}. The fields are `path`, and `edits` (`[{{find, replace}}]`) or `create` (the text of a new page), and nothing else."))
        })?;
        match (&p.edits, &p.create, &p.put_back) {
            (Some(_), None, None) | (None, Some(_), None) | (None, None, Some(_)) => Ok(p),
            (None, None, None) => Err(Refusal::new("a style edit gives `edits` (small edits of a page that is there) or `create` (the text of a new page). Nothing was changed.")),
            _ => Err(Refusal::new("a style edit gives `edits`, or `create`, or is an undo: only one. Nothing was changed.")),
        }
    }
}

/// **What a style edit leaves**: the page as it was (`None` when there was none) and as it will be (`None` when it is taken away).
struct Edited {
    root: std::path::PathBuf,
    old: Option<String>,
    new: Option<String>,
}

/// **The folder of a page**, `design/levels/m` for `design/levels/m/style.md`.
fn folder_of_page(path: &str) -> &str {
    path.rsplit_once('/').map_or(design::DESIGN_DIR, |(folder, _)| folder)
}

/// **The pages a path may not be, with the sentence that says what changes it**: a board and an area's rectangle have an operation of their own.
fn refuse_a_file_that_is_not_a_page(path: &str) -> Result<(), Refusal> {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name == bevy_grimoire_core::board::BOARD_FILE {
        return Err(Refusal::new(format!("`{path}` is a board, which is changed only by `board_set`. Nothing was changed.")));
    }
    if name == AREA_FILE {
        return Err(Refusal::new(format!("`{path}` is an area's rectangle, which is changed only by `design_area_set`. Nothing was changed.")));
    }
    if !crate::design_view::is_page(path) {
        return Err(Refusal::new(format!("`{path}` is not a page of the game's style: a Markdown file under `design/`, inside the project. Nothing was changed.")));
    }
    Ok(())
}

/// **Whether the person has allowed this page as it stands**, from their own store: an agent may not change what it has not been shown.
fn allowed(ctx: &Ctx, root: &Path, path: &str, text: &str) -> Result<bool, Refusal> {
    let store = crate::trusted::load_at(None, ctx.trust_file.as_deref()).map_err(|e| Refusal::new(format!("the person's trust store would not read ({e}), so `{path}` could not be checked. Nothing was changed.")))?;
    let decision = store.text_decision(root, path, text).map_err(|e| Refusal::new(format!("`{path}` could not be checked against what the person has allowed ({e}). Nothing was changed.")))?;
    Ok(decision != bevy_grimoire_core::trust::Decision::Untrusted)
}

/// **Apply `edits` in order**, each `find` exactly once in the text the one before left.
fn apply_edits(text: &str, edits: &[TextEdit]) -> Result<String, Refusal> {
    let mut now = text.to_owned();
    for (n, edit) in edits.iter().enumerate() {
        let which = format!("edit {}", n + 1);
        if edit.find.is_empty() {
            return Err(Refusal::new(format!("{which}: `find` is empty, and an empty span is everywhere. Nothing was changed.")));
        }
        let count = now.matches(edit.find.as_str()).count();
        if count != 1 {
            return Err(Refusal::new(format!(
                "{which}: `find` is in the page {count} times, and it must be there exactly once (0 means the page does not say that, and more than one means the span is not exact enough: make it longer). Nothing was changed."
            )));
        }
        now = now.replacen(edit.find.as_str(), &edit.replace, 1);
    }
    Ok(now)
}

/// **A page's pictures that are not in the project**, by the path the page cites.
fn missing_pictures(root: &Path, path: &str, text: &str) -> Vec<String> {
    design::pictures(text, path).into_iter().filter_map(|p| p.file).filter(|file| !root.join(file).is_file()).collect()
}

/// **The longest chain this page would be part of once it said `new_len` bytes**, over the topics the game states and the overall feel.
fn longest_chain_with(tree: &design::Tree, path: &str, new_len: usize) -> usize {
    let place = folder_of_page(path);
    let chain: Vec<&design::Place> = tree.chain_of(place).or_else(|| tree.chain_of(folder_of_page(place))).unwrap_or_default();
    let mut topics: Vec<String> = vec![design::STYLE_TOPIC.to_owned()];
    topics.extend(tree.place(design::DESIGN_DIR).map(|g| g.topics.iter().map(|t| t.name.clone()).filter(|n| n != design::STYLE_TOPIC).collect::<Vec<_>>()).unwrap_or_default());
    let edited_topic = path.rsplit('/').next().and_then(|n| n.strip_suffix(".md")).unwrap_or_default().to_owned();
    topics
        .iter()
        .map(|topic| {
            let mut total = 0usize;
            let mut saw_edited = false;
            for place in &chain {
                for name in [Some(design::STYLE_TOPIC), (topic != design::STYLE_TOPIC).then_some(topic.as_str())].into_iter().flatten() {
                    if let Some(file) = place.topic(name) {
                        if file.file == path {
                            total += new_len;
                            saw_edited = true;
                        } else {
                            total += file.bytes;
                        }
                    }
                }
            }
            // A page being made is in no place of the tree yet: it joins the chains of its own topic and of the overall feel.
            if !saw_edited && (edited_topic == *topic || edited_topic == design::STYLE_TOPIC) {
                total += new_len;
            }
            total
        })
        .max()
        .unwrap_or(new_len)
}

impl StyleEdit {
    /// **What the call would leave, or the reason it is refused** — pure, so the plan, the place and the commit agree.
    fn resolve(ctx: &Ctx, p: &StyleParams) -> Result<Edited, Refusal> {
        let Some(root) = ctx.root_or_project() else {
            return Err(Refusal::new("no project is open, so there is no style page to change. Nothing was changed."));
        };
        refuse_a_file_that_is_not_a_page(&p.path)?;
        let full = crate::files::project_path(&root, &p.path).map_err(|e| Refusal::new(format!("{e}. Nothing was changed.")))?;
        let old = std::fs::read_to_string(&full).ok();
        if let Some(back) = &p.put_back {
            return Ok(Edited { root, old, new: back.text.clone() });
        }
        let place = folder_of_page(&p.path);
        let kind = design::kind_of_place_path(place).ok_or_else(|| Refusal::new(format!("`{place}` is not a place of the design tree. Nothing was changed.")))?;
        // **Which files take which path is the place's decision, never the agent's.**
        if matches!(kind, design::PlaceKind::Game | design::PlaceKind::Region) {
            return Err(Refusal::new(format!(
                "`{}` is a {} page, and the whole game rests on it: an agent changes it only by proposing the change (`grimoire_style_propose`), which a person keeps or discards. `style_edit` changes the pages of a level or an area. Nothing was changed.",
                p.path,
                kind.word()
            )));
        }
        if crate::files::person_is_editing(&root, &p.path) {
            return Err(Refusal::new(format!("the person is editing {}; wait or ask. Nothing was changed.", p.path)));
        }
        let new = match (&p.edits, &p.create) {
            (Some(edits), _) => {
                let Some(old_text) = old.as_deref() else {
                    return Err(Refusal::new(format!("`{}` is not there, so there is nothing to edit; to make a new page pass `create` with its text. Nothing was changed.", p.path)));
                };
                // **An agent cannot change what it has not been shown**: a page the person has not allowed is refused.
                if !allowed(ctx, &root, &p.path, old_text)? {
                    return Err(Refusal::new(format!("`{}` is waiting for the person to allow it, and an agent cannot change what it has not been shown. Ask them to allow it in the editor. Nothing was changed.", p.path)));
                }
                apply_edits(old_text, edits)?
            }
            (None, Some(text)) => {
                if old.is_some() {
                    return Err(Refusal::new(format!("`{}` is there already, so it was not written over: change it with `edits`. Nothing was changed.", p.path)));
                }
                let topic = p.path.rsplit('/').next().and_then(|n| n.strip_suffix(".md")).unwrap_or_default();
                if !design::is_topic_name(topic) {
                    return Err(Refusal::new(format!("`{topic}` is not a name a page can have: letters, digits, `_` and `-`. Nothing was changed.")));
                }
                // A topic below the game is a topic the game states.
                if topic != design::STYLE_TOPIC && !root.join(design::DESIGN_DIR).join(format!("{topic}.md")).is_file() {
                    return Err(Refusal::new(format!("the game has no `{topic}` topic, and a place below it can only take up the game's topics. Nothing was changed.")));
                }
                text.clone()
            }
            (None, None) => return Err(Refusal::new("a style edit gives `edits` or `create`. Nothing was changed.")),
        };
        if old.as_deref() == Some(new.as_str()) {
            return Err(Refusal::new(format!("those edits leave `{}` as it is. Nothing was changed.", p.path)));
        }
        // **The result must still read**: inside the limits the tree serves a page by, and citing no picture that is not there.
        if new.len() > design::STYLE_FILE_MAX_BYTES {
            return Err(Refusal::new(format!("`{}` would be {} bytes, over the {} a style page may be, and a page is served whole or not at all: shorten it or move part of it into a topic of its own. Nothing was changed.", p.path, new.len(), design::STYLE_FILE_MAX_BYTES)));
        }
        let tree = design::Tree::read(&root);
        let chain = longest_chain_with(&tree, &p.path, new.len());
        if chain > design::STYLE_CHAIN_MAX_BYTES {
            return Err(Refusal::new(format!("with this change the longest chain of style that reaches `{}` would be {chain} bytes, over the {} an agent is served, and a chain is never cut. Nothing was changed.", p.path, design::STYLE_CHAIN_MAX_BYTES)));
        }
        let before: std::collections::BTreeSet<String> = old.as_deref().map(|t| missing_pictures(&root, &p.path, t)).unwrap_or_default().into_iter().collect();
        let now_missing: Vec<String> = missing_pictures(&root, &p.path, &new).into_iter().filter(|f| !before.contains(f)).collect();
        if !now_missing.is_empty() {
            return Err(Refusal::new(format!("`{}` would cite {}, which {} not in the project. Put the picture on the board first (`board_set`), or cite one that is there. Nothing was changed.", p.path, now_missing.iter().map(|f| format!("`{f}`")).collect::<Vec<_>>().join(", "), if now_missing.len() == 1 { "is" } else { "are" })));
        }
        Ok(Edited { root, old, new: Some(new) })
    }
}

/// **The text of a page as it is now**, for an undo's check.
fn page_now(root: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join(rel)).ok()
}

/// **Change a level's or an area's style page by small edits, or make a new topic page.**
pub struct StyleEdit;

impl Operation for StyleEdit {
    fn name(&self) -> &'static str {
        "style_edit"
    }
    fn shows(&self, ctx: &Ctx, params: &serde_json::Value) -> Shown {
        let Ok(p) = StyleParams::parse(params) else { return Shown::File { path: design::DESIGN_DIR.to_owned(), lines: None } };
        // **The changed lines**, from the page as it is and as it will be, so the view goes to the Design page at the lines that moved.
        let lines = Self::resolve(ctx, &p).ok().and_then(|e| match (e.old, e.new) {
            (Some(old), Some(new)) => crate::files::changed_lines(&old, &new),
            (None, Some(_)) => Some([1, 1]),
            _ => None,
        });
        Shown::File { path: p.path, lines }
    }
    fn writes(&self) -> &'static [&'static str] {
        &["design"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        StyleParams::parse(params).is_ok_and(|p| p.put_back.is_some())
    }
    fn schema(&self) -> serde_json::Value {
        StyleParams::schema()
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = StyleParams::parse(params)?;
        let e = Self::resolve(ctx, &p)?;
        Ok(json!({"summary": format!("change {}", p.path), "path": p.path, "lines": e.old.as_deref().zip(e.new.as_deref()).and_then(|(o, n)| crate::files::changed_lines(o, n))}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = StyleParams::parse(params)?;
        let Edited { root, old, new } = Self::resolve(ctx, &p)?;
        let full = root.join(&p.path);
        match &new {
            Some(text) => {
                // **The person's own editor is where this was made, in front of them, and it can be undone**, so the new text is allowed:
                // granted before it is written, so a store that will not take the grant leaves the page as it was.
                let store_file = crate::trusted::resolve(None, ctx.trust_file.as_deref()).map_err(Refusal::new)?;
                let mut store = bevy_grimoire_core::trust::TrustStore::load(&store_file).map_err(|e| Refusal::new(format!("the person's trust store would not read ({e}), so nothing was written.")))?;
                store.trust_text(&root, &p.path, text).map_err(Refusal::new)?;
                store.save(&store_file).map_err(|e| Refusal::new(format!("the person's trust store would not take the grant ({e}), so nothing was written.")))?;
                if let Some(folder) = full.parent() {
                    std::fs::create_dir_all(folder).map_err(|e| Refusal::new(format!("{} cannot be made ({e}), so nothing was written.", folder.display())))?;
                }
                bevy_grimoire_core::atomic_file::write(&full, text).map_err(|e| Refusal::new(format!("{} was not written ({e}). Nothing was changed.", p.path)))?;
            }
            None => {
                std::fs::remove_file(&full).map_err(|e| Refusal::new(format!("{} could not be taken away ({e}). Nothing was changed.", p.path)))?;
                // An empty folder the page was the last of goes with it, and `remove_dir` refuses one that holds anything.
                if let Some(folder) = full.parent() {
                    let _ = std::fs::remove_dir(folder);
                }
            }
        }
        if ctx.source == crate::ops::Source::Agent {
            crate::files::note_agent_edit(&root, &p.path);
        }
        let summary = match (&old, &new, p.put_back.is_some()) {
            (_, _, true) => format!("put back {}", p.path),
            (None, Some(_), _) => format!("created {}", p.path),
            _ => format!("changed {}", p.path),
        };
        Ok(Outcome {
            summary,
            affected: 1,
            saved: true,
            inverse: Some(json!({"path": p.path, "put_back": {"text": old}})),
            after: Some(json!({"path": p.path, "text": new})),
        })
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let root = ctx.root_or_project()?;
        let path = after.get("path")?.as_str()?;
        Some(json!({"path": path, "text": page_now(&root, path)}))
    }
}
