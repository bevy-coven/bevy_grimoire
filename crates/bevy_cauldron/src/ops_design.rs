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
