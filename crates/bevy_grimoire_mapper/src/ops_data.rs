//! **A game's tuning files, as one registered operation** (#86): `data_set`.
//!
//! The files are the ones `<grimoire_dir>/data.ron` registers (`bevy_grimoire_core::data_files`). The
//! operation sets one scalar leaf and writes at once: data files have no dirty state. Its inverse is
//! this operation with the old value and the old token's exact text (`restore`, which only an undo may
//! send), so one undo leaves the file byte-identical even where `1.50` was rewritten as `1.5`.

use bevy_grimoire_core::data_files::{self, DataFiles};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// The registry id of the file, as listed in `data.ron`.
    pub file: String,
    /// Dotted leaf path inside the file.
    pub path: String,
    /// The new scalar. A number stays a number, a string a string.
    pub value: serde_json::Value,
    /// **The undo's own form**: the old token's exact text, put back verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<String>,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["file", "path", "value"],
            "properties": {
                "file": {"type": "string", "description": "The registry id of the data file, as listed in data.ron (grimoire_state project.data_files)."},
                "path": {"type": "string", "description": "Dotted path of one scalar leaf, e.g. mob.speed or rooms[2].weight."},
                "value": {"description": "The new scalar as JSON: a number, string or bool of the kind the leaf already holds."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("these are not a data edit: {e}. The fields are `file`, `path` and `value`, and nothing else."))
        })
    }
}

fn registry(ctx: &Ctx) -> Result<DataFiles, Refusal> {
    let Some(project) = ctx.project.as_ref() else {
        return Err(Refusal::new("no project is open, so there are no data files to change."));
    };
    let dir = &project.grimoire_dir;
    DataFiles::load(dir).map_err(Refusal::new)?.ok_or_else(|| {
        Refusal::new(format!("{} has no data.ron, so no data file is registered. Nothing was changed.", dir.display()))
    })
}

fn path_of(ctx: &Ctx, file: &str) -> Result<(std::path::PathBuf, data_files::DataFile), Refusal> {
    let reg = registry(ctx)?;
    let f = reg.get(file).cloned().ok_or_else(|| {
        let ids: Vec<&str> = reg.files.iter().map(|f| f.id.as_str()).collect();
        Refusal::new(format!("`{file}` is not a registered data file; the registered ones are {}.", ids.join(", ")))
    })?;
    Ok((reg.resolve(&f), f))
}

fn read(ctx: &Ctx, file: &str) -> Option<String> {
    std::fs::read_to_string(path_of(ctx, file).ok()?.0).ok()
}

/// **Set one leaf of a registered data file.**
pub struct DataSet;

impl DataSet {
    fn apply(ctx: &Ctx, params: &serde_json::Value, write: bool) -> Result<Outcome, Refusal> {
        let p = Params::parse(params)?;
        let (path, f) = path_of(ctx, &p.file)?;
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Refusal::new(format!("{} cannot be read, so nothing was changed: {e}", path.display())))?;
        let shown = ctx.project.as_ref().map_or_else(|| path.display().to_string(), |p| crate::map_files::file_name(p, &path));
        let old = data_files::read(&shown, &text)
            .map_err(|r| Refusal::new(format!("{}. The file is left as it is: fix it, then try again. Nothing was changed.", r.sentence())))?
            .into_iter()
            .find(|(l, _)| *l == p.path)
            .map(|(_, t)| t);
        let new = match &p.restore {
            Some(token) => data_files::restore_leaf(&text, &p.path, token),
            None => data_files::set_leaf(&text, &p.path, &p.value, f.bounds.get(&p.path)),
        }
        .map_err(Refusal::new)?;
        let old = old.expect("set_leaf succeeded, so the leaf exists");
        if write {
            bevy_grimoire_core::atomic_file::write(&path, &new)
                .map_err(|e| Refusal::new(format!("{} refused the change, so nothing was written: {e}", path.display())))?;
        }
        Ok(Outcome {
            summary: format!("{}: {} = {}", p.file, p.path, p.value),
            affected: 1,
            saved: write,
            inverse: Some(json!({
                "file": p.file,
                "path": p.path,
                "value": data_files::leaf_json(&old).unwrap_or(serde_json::Value::Null),
                "restore": old,
            })),
            after: Some(json!({"file": p.file, "text": new})),
        })
    }
}

impl Operation for DataSet {
    fn name(&self) -> &'static str {
        "data_set"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["game_data"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        Params::parse(params).map(|p| p.restore.is_some()).unwrap_or(false)
    }
    fn schema(&self) -> serde_json::Value {
        Params::schema()
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let file = after.get("file")?.as_str()?;
        Some(json!({"file": file, "text": read(ctx, file)?}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = Params::parse(params)?;
        let o = Self::apply(ctx, params, false)?;
        let from = o.inverse.as_ref().and_then(|i| i.get("value")).cloned().unwrap_or_default();
        Ok(json!({"file": p.file, "path": p.path, "from": from, "to": p.value}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Self::apply(ctx, params, true)
    }
}

/// One leaf of a registered data file, as the Data panel lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafRow {
    pub file: String,
    pub path: String,
    /// The leaf's current token, exactly as written in the file.
    pub value: String,
}

/// **Every leaf of every registered file**, in registry then file order. No `data.ron` is an empty
/// list; a file that cannot be read or parsed contributes no rows (`data_set` names why on use).
pub fn listing(grimoire_dir: &std::path::Path) -> Vec<LeafRow> {
    let Ok(Some(reg)) = DataFiles::load(grimoire_dir) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for f in &reg.files {
        let Ok(text) = std::fs::read_to_string(reg.resolve(f)) else { continue };
        let Ok(leaves) = data_files::leaves(&text) else { continue };
        rows.extend(leaves.into_iter().map(|(path, value)| LeafRow { file: f.id.clone(), path, value }));
    }
    rows
}
