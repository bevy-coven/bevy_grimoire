//! **The tree editor, as one registered operation** (#102): `tree_edit`.
//!
//! A tree's parameters are `bevy_flora`'s own `TreeParams`, written as `<grimoire_dir>/trees/<name>.tree.ron`,
//! the format the crate's asset loader reads. `preset` starts the file from one of the crate's presets;
//! `set` changes fields by dotted path (`trunk.radius`), and a path the crate does not know is refused
//! with the crate's own message, so a typo is never written. The game and the editor's preview load the
//! same file as an asset, so every tree sharing it regrows after an edit with nothing to recompile.
//!
//! The inverse is the same operation carrying the file as it was (`put_back`), which only an undo may
//! send. Without the `bevy_flora` feature the operation is registered and refuses, naming the feature.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// The tree file's name, without `.tree.ron`.
    pub name: String,
    /// Start (or restart) the file from this preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// Dotted field path to its new value.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub set: serde_json::Map<String, serde_json::Value>,
    /// The file as it was (`None` inside: there was none). Only an undo sends this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutBack {
    pub text: Option<String>,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["name"],
            "properties": {
                "name": {"type": "string", "description": "The tree file's name: trees/<name>.tree.ron in the project."},
                "preset": {"type": "string", "description": "Start the file from one of bevy_flora's presets (oak, pine, maple, ...). Without it the file must already exist."},
                "set": {"type": "object", "description": "Dotted field path to value, e.g. {\"trunk.radius\": 0.6, \"seed\": 9}. An unknown field is refused."},
                "put_back": {"type": "object", "description": "An undo's put-back. Only an undo sends this."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        let p: Params = serde_json::from_value(raw.clone())
            .map_err(|e| Refusal::new(format!("this is not a tree edit: {e}. The fields are `name`, `preset` and `set`.")))?;
        if p.name.is_empty() || p.name.contains(['/', '\\', '.']) {
            return Err(Refusal::new(format!("`{}` is not a tree name. Nothing was written.", p.name)));
        }
        Ok(p)
    }
}

fn tree_path(ctx: &Ctx, name: &str) -> Result<std::path::PathBuf, Refusal> {
    let Some(project) = ctx.project.as_ref() else {
        return Err(Refusal::new("no project is open, so there is no tree file to write."));
    };
    Ok(project.grimoire_dir.join("trees").join(format!("{name}.tree.ron")))
}

#[cfg(feature = "bevy_flora")]
/// **Set `path` (dotted) inside `root` to `value`**; a path that is not already a field is refused.
fn set_path(root: &mut serde_json::Value, path: &str, value: serde_json::Value) -> Result<(), Refusal> {
    let mut at = root;
    for part in path.split('.') {
        at = at
            .get_mut(part)
            .ok_or_else(|| Refusal::new(format!("a tree has no field `{path}` (`{part}` is not one). Nothing was written.")))?;
    }
    *at = value;
    Ok(())
}

/// **The tree file's text after this edit**, for the file as it is now.
#[cfg(feature = "bevy_flora")]
fn edited(existing: Option<&str>, p: &Params) -> Result<String, Refusal> {
    use bevy_flora::{TreeParams, TreePreset};
    let base: TreeParams = match (&p.preset, existing) {
        (Some(name), _) => TreePreset::ALL
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| {
                let all: Vec<_> = TreePreset::ALL.iter().map(|t| t.name()).collect();
                Refusal::new(format!("`{name}` is not a preset. They are {}. Nothing was written.", all.join(", ")))
            })?
            .params(),
        (None, Some(text)) => TreeParams::from_ron(text).map_err(|e| Refusal::new(format!("{e}. Nothing was written.")))?,
        (None, None) => return Err(Refusal::new(format!("trees/{}.tree.ron does not exist: name a `preset` to start it.", p.name))),
    };
    let mut value = serde_json::to_value(&base).map_err(|e| Refusal::new(format!("the tree could not be read: {e}")))?;
    for (path, v) in &p.set {
        set_path(&mut value, path, v.clone())?;
    }
    let tree: TreeParams = serde_json::from_value(value).map_err(|e| Refusal::new(format!("that is not a tree: {e}. Nothing was written.")))?;
    Ok(tree.to_ron())
}

/// **Write or edit a tree file.**
pub struct TreeEdit;

impl TreeEdit {
    #[cfg(feature = "bevy_flora")]
    fn apply(ctx: &Ctx, params: &serde_json::Value, write: bool) -> Result<Outcome, Refusal> {
        let p = Params::parse(params)?;
        let path = tree_path(ctx, &p.name)?;
        let before = std::fs::read_to_string(&path).ok();
        let text = match &p.put_back {
            Some(back) => back.text.clone(),
            None => Some(edited(before.as_deref(), &p)?),
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
            summary: format!("tree {}: {} field(s) set", p.name, p.set.len()),
            affected: p.set.len().max(1),
            saved: write,
            inverse: Some(json!({"name": p.name, "put_back": {"text": before}})),
            after: Some(json!({"name": p.name, "text": text})),
        })
    }

    #[cfg(not(feature = "bevy_flora"))]
    fn apply(_ctx: &Ctx, params: &serde_json::Value, _write: bool) -> Result<Outcome, Refusal> {
        Params::parse(params)?;
        Err(Refusal::new("this build has no trees: rebuild the editor with `--features bevy_flora`. Nothing was changed."))
    }
}

impl Operation for TreeEdit {
    fn name(&self) -> &'static str {
        "tree_edit"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["trees"]
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
        let name = after.get("name")?.as_str()?;
        Some(json!({"name": name, "text": std::fs::read_to_string(tree_path(ctx, name).ok()?).ok()}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let o = Self::apply(ctx, params, false)?;
        Ok(json!({"name": Params::parse(params)?.name, "fields": o.affected}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Self::apply(ctx, params, true)
    }
}
