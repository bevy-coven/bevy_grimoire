//! **Carving and adding volume, as one registered operation** (#101): `volume_edit`.
//!
//! The volume is `<grimoire_dir>/volume/<map>.ron`, the engine-free `bevy_grimoire_core::volume::Volume`
//! (its edit list, not voxels) as RON, saved beside the map. One call adds or carves one primitive,
//! or clears the list; either is one undo. The inverse is the same operation carrying the file as it was.

use bevy_grimoire_core::volume::{Edit, Volume};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

/// An undo's put-back: the file as it was (`None`: there was none).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutBack {
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub map: String,
    /// One edit: `{"shape": {"Sphere": {center, radius_m}}, "mode": "Carve", "smooth_m": 0.5}`; shapes are
    /// `Sphere {center, radius_m}`, `Box {center, half_m}` and `Capsule {a, b, radius_m}`; modes `Add` and `Carve`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit: Option<Edit>,
    /// Remove every edit of the map.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear: bool,
    /// An undo's put-back. Not a thing a caller sends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["map"],
            "properties": {
                "map": {"type": "string", "description": "The map whose volume is edited."},
                "edit": {"type": "object", "description": "One edit: {\"shape\": {\"Sphere\": {center:[x,y,z], radius_m}} | {\"Box\": {center, half_m:[x,y,z]}} | {\"Capsule\": {a, b, radius_m}}, \"mode\": \"Add\"|\"Carve\", \"smooth_m\": blend metres, default 0}."},
                "clear": {"type": "boolean", "description": "Remove every edit of the map."},
                "put_back": {"type": "object", "description": "An undo's put-back. Only an undo sends this."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        let p: Params = serde_json::from_value(raw.clone())
            .map_err(|e| Refusal::new(format!("this is not a volume change: {e}. The fields are `map`, `edit` and `clear`.")))?;
        if p.edit.is_none() && !p.clear && p.put_back.is_none() {
            return Err(Refusal::new("a volume change needs `edit` or `clear`. Nothing was changed."));
        }
        Ok(p)
    }
}

fn path_of(ctx: &Ctx, map: &str) -> Result<std::path::PathBuf, Refusal> {
    let Some(project) = ctx.project.as_ref() else {
        return Err(Refusal::new("no project is open, so there is no volume to edit."));
    };
    if map.is_empty() || map.contains(['/', '\\', '.']) {
        return Err(Refusal::new(format!("`{map}` is not a map name. Nothing was written.")));
    }
    Ok(project.grimoire_dir.join("volume").join(format!("{map}.ron")))
}

fn read(ctx: &Ctx, map: &str) -> Option<String> {
    std::fs::read_to_string(path_of(ctx, map).ok()?).ok()
}

/// **The saved volume of the open map**, or `None` when it has none.
pub fn volume_of(project: &crate::project::Project, open: &crate::project::OpenMap) -> Option<Volume> {
    let name = open.map_path.file_name()?.to_str()?.strip_suffix(".map.ron")?;
    ron::from_str(&std::fs::read_to_string(project.grimoire_dir.join("volume").join(format!("{name}.ron"))).ok()?).ok()
}

/// **Add or carve one primitive, or clear the map's volume.**
pub struct VolumeEdit;

impl VolumeEdit {
    fn apply(ctx: &Ctx, params: &serde_json::Value, write: bool) -> Result<Outcome, Refusal> {
        let p = Params::parse(params)?;
        let path = path_of(ctx, &p.map)?;
        let before = std::fs::read_to_string(&path).ok();
        let inverse = json!({"map": p.map, "put_back": {"text": before}});
        let (text, affected) = if let Some(back) = &p.put_back {
            (back.text.clone(), 0)
        } else if p.clear {
            (None, 0)
        } else {
            let mut volume: Volume = match &before {
                Some(t) => ron::from_str(t).map_err(|e| Refusal::new(format!("{} is not a volume: {e}", path.display())))?,
                None => Volume::default(),
            };
            volume.push(p.edit.expect("parse needs an edit, a clear or a put_back")).map_err(|e| Refusal::new(format!("{e}. Nothing was changed.")))?;
            let t = ron::ser::to_string_pretty(&volume, ron::ser::PrettyConfig::default())
                .map_err(|e| Refusal::new(format!("the volume could not be written: {e}")))?;
            (Some(t), 1)
        };
        if write {
            match &text {
                Some(t) => {
                    if let Some(dir) = path.parent() {
                        std::fs::create_dir_all(dir).map_err(|e| Refusal::new(format!("{} cannot be made: {e}", dir.display())))?;
                    }
                    bevy_grimoire_core::atomic_file::write(&path, t)
                        .map_err(|e| Refusal::new(format!("{} refused the change, so nothing was written: {e}", path.display())))?;
                }
                None => {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        Ok(Outcome {
            summary: format!("volume {}: {}", p.map, if affected == 1 { "edited" } else if p.clear { "cleared" } else { "put back" }),
            affected,
            saved: write,
            inverse: Some(inverse),
            after: Some(json!({"map": p.map, "text": text})),
        })
    }
}

impl Operation for VolumeEdit {
    fn name(&self) -> &'static str {
        "volume_edit"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["volume"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        Params::parse(params).map(|p| p.put_back.is_some()).unwrap_or(false)
    }
    fn schema(&self) -> serde_json::Value {
        Params::schema()
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let map = after.get("map")?.as_str()?;
        Some(json!({"map": map, "text": read(ctx, map)}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let o = Self::apply(ctx, params, false)?;
        Ok(json!({"map": Params::parse(params)?.map, "edited": o.affected}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Self::apply(ctx, params, true)
    }
}
