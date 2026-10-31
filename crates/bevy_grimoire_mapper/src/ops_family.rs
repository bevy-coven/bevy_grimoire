//! **A family crate's settings, as one registered operation** (#96): `family_set`.
//!
//! The settings are the crate's own serde types written as RON in `<project>/family/<crate>.ron`.
//! The editor holds no schema for them: this operation rewrites one dot-pathed field's value text
//! (`bevy_grimoire_core::family::set_field`) and keeps the rest of the file as the author left it. Its
//! inverse is the same operation with the old value, so a person's Enter and an agent's `apply` are
//! one change with one log line and one undo.

use bevy_grimoire_core::family;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// The family crate whose settings file is written, as named in `family.ron`.
    #[serde(rename = "crate")]
    pub krate: String,
    /// Dot-separated field names inside the settings, `sun.hour`.
    pub field: String,
    /// One RON value: `6.5`, `"dusk"`, `(1.0, 2.0)`.
    pub value: String,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["crate", "field", "value"],
            "properties": {
                "crate": {"type": "string", "description": "The family crate whose settings are written, as listed in family.ron."},
                "field": {"type": "string", "description": "Dot-separated field names inside its settings file, e.g. sun.hour."},
                "value": {"type": "string", "description": "The new value as one RON value, e.g. 6.5 or \"dusk\"."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("these are not a family setting: {e}. The fields are `crate`, `field` and `value`, and nothing else."))
        })
    }
}

pub(crate) fn settings_path(ctx: &Ctx, krate: &str) -> Result<std::path::PathBuf, Refusal> {
    let Some(root) = ctx.root_or_project() else {
        return Err(Refusal::new("no project is open, so there are no family settings to change."));
    };
    // A crate name is one path segment: it is a file name, never a route out of `family/`.
    if krate.is_empty() || krate.contains(['/', '\\', '.']) {
        return Err(Refusal::new(format!("`{krate}` is not a family crate name. Nothing was written.")));
    }
    let listed = family::Family::load(&root).map_err(Refusal::new)?;
    if !listed.crates.iter().any(|c| c == krate) {
        return Err(Refusal::new(format!("{krate} is not listed in family.ron, so its settings were not changed.")));
    }
    Ok(root.join("family").join(format!("{krate}.ron")))
}

fn read(ctx: &Ctx, krate: &str) -> Option<String> {
    std::fs::read_to_string(settings_path(ctx, krate).ok()?).ok()
}

/// **Set one field of a family crate's settings file.**
pub struct FamilySet;

impl FamilySet {
    fn apply(ctx: &Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p = Params::parse(params)?;
        let path = settings_path(ctx, &p.krate)?;
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Refusal::new(format!("{} cannot be read, so nothing was changed: {e}", path.display())))?;
        let (new, old) = family::set_field(&text, &p.field, &p.value).map_err(Refusal::new)?;
        check_script(&p.krate, &new)?;
        check_society(&p.krate, &new)?;
        if !write {
            return Ok(None);
        }
        bevy_grimoire_core::atomic_file::write(&path, &new)
            .map_err(|e| Refusal::new(format!("{} refused the change, so nothing was written: {e}", path.display())))?;
        Ok(Some(Outcome {
            summary: format!("{}: {} = {}", p.krate, p.field, p.value.trim()),
            affected: 1,
            saved: true,
            inverse: Some(json!({"crate": p.krate, "field": p.field, "value": old})),
            after: Some(json!({"crate": p.krate, "file": new})),
        }))
    }
}

impl Operation for FamilySet {
    fn name(&self) -> &'static str {
        "family_set"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["family"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn schema(&self) -> serde_json::Value {
        Params::schema()
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let krate = after.get("crate")?.as_str()?;
        Some(json!({"crate": krate, "file": read(ctx, krate)?}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        Self::apply(ctx, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Ok(Self::apply(ctx, params, true)?.expect("a write returns its outcome"))
    }
}

// ---------------------------------------------------------------- placement_set_family

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PieceParams {
    /// The placement's id, as the map file names it.
    pub id: String,
    /// The family crate whose settings this piece carries, as listed in `family.ron`.
    #[serde(rename = "crate")]
    pub krate: String,
    /// The crate's own settings type for this piece as one RON value, `(base: 900.0, phase: 1.3, depth: 0.15)`.
    /// Absent takes the settings off the piece.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl PieceParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "crate"],
            "properties": {
                "id": {"type": "string", "description": "The placement's id, as the map file names it."},
                "crate": {"type": "string", "description": "The family crate whose settings the piece carries, as listed in family.ron, e.g. bevy_lights."},
                "value": {"type": "string", "description": "The crate's own settings type as one RON value, e.g. (base: 900.0, phase: 1.3, depth: 0.15) for a bevy_lights Flicker. Leave it out to take the settings off the piece."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<PieceParams, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("these are not a piece's family settings: {e}. The fields are `id`, `crate` and `value`, and nothing else."))
        })
    }
}

/// **Whether `text` is a value the crate's own type reads**: a crate this build knows is parsed as
/// that type, so a typo in a field name is refused here and not discovered by the game; any other
/// listed crate is held to being well-formed RON.
fn reads_as(krate: &str, text: &str) -> Result<(), String> {
    #[cfg(feature = "bevy_lights")]
    if krate == "bevy_lights" {
        return ron::from_str::<bevy_lights::Flicker>(text).map(|_| ()).map_err(|e| format!("`{text}` is not a bevy_lights Flicker: {e}"));
    }
    let _ = krate;
    ron::from_str::<ron::Value>(text).map(|_| ()).map_err(|e| format!("`{text}` is not RON: {e}"))
}

fn family_of(open: &crate::project::OpenMap, id: &str, krate: &str) -> serde_json::Value {
    let value = open.map.placements.iter().find(|p| p.id == id).and_then(|p| p.family.get(krate));
    json!({"map": open.map.name, "id": id, "crate": krate, "value": value})
}

/// **Put a family crate's settings on one placement, or take them off.**
pub struct PlacementSetFamily;

impl PlacementSetFamily {
    fn check(ctx: &Ctx, p: &PieceParams) -> Result<(), Refusal> {
        let Some(open) = ctx.open.as_ref() else {
            return Err(Refusal::new("no map is open, so there are no placements to set."));
        };
        if !open.map.placements.iter().any(|q| q.id == p.id) {
            return Err(Refusal::new(format!("no placement is called `{}` on this map, so nothing was set.", p.id)));
        }
        // Reuse the settings-file checks: the name is one path segment and the project lists it.
        settings_path(ctx, &p.krate)?;
        if let Some(v) = &p.value {
            reads_as(&p.krate, v).map_err(|e| Refusal::new(format!("{e}. Nothing was set.")))?;
        }
        check_conversation(&settings_path(ctx, &p.krate)?, p)?;
        Ok(())
    }
}

/// **What a piece carries for `bevy_speech_bubbles`** (#106): a conversation's name (a RON string), which the
/// project's `DialogueScript` must define, or a list of the crate's own `Bark`s (a RON list), which it must read.
#[cfg(feature = "bevy_speech_bubbles")]
fn check_conversation(script: &std::path::Path, p: &PieceParams) -> Result<(), Refusal> {
    let (true, Some(v)) = (p.krate == "bevy_speech_bubbles", p.value.as_deref()) else {
        return Ok(());
    };
    if v.trim_start().starts_with('[') {
        return ron::from_str::<Vec<bevy_speech_bubbles::Bark>>(v)
            .map(|_| ())
            .map_err(|e| Refusal::new(format!("`{v}` is not a list of barks (speaker, kind, emotion, text): {e}. Nothing was set.")));
    }
    let name: String = ron::from_str(v)
        .map_err(|e| Refusal::new(format!("`{v}` is not a conversation name (a quoted string such as \"hello\"): {e}. Nothing was set.")))?;
    let text = std::fs::read_to_string(script)
        .map_err(|e| Refusal::new(format!("{} cannot be read, so no conversation can be attached: {e}", script.display())))?;
    let defined: bevy_speech_bubbles::DialogueScript =
        ron::from_str(&text).map_err(|e| Refusal::new(format!("the project's dialogue script is not readable: {e}")))?;
    if defined.conversations.contains_key(&name) {
        return Ok(());
    }
    let known: Vec<&str> = defined.conversations.keys().map(String::as_str).collect();
    Err(Refusal::new(format!("the dialogue script has no conversation `{name}` (it has: {}). Nothing was set.", known.join(", "))))
}

#[cfg(not(feature = "bevy_speech_bubbles"))]
fn check_conversation(_script: &std::path::Path, _p: &PieceParams) -> Result<(), Refusal> {
    Ok(())
}

impl Operation for PlacementSetFamily {
    fn name(&self) -> &'static str {
        "placement_set_family"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["map"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn schema(&self) -> serde_json::Value {
        PieceParams::schema()
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let p = PieceParams::parse(&json!({"id": after.get("id")?, "crate": after.get("crate")?})).ok()?;
        Some(family_of(ctx.open.as_ref()?, &p.id, &p.krate))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = PieceParams::parse(params)?;
        Self::check(ctx, &p)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = PieceParams::parse(params)?;
        Self::check(ctx, &p)?;
        let mut open = ctx.open.take().expect("check found the map");
        let index = open.map.placements.iter().position(|q| q.id == p.id).expect("check found the piece");
        let old = open.map.placements[index].family.get(&p.krate).cloned();
        let put = |open: &mut crate::project::OpenMap, v: &Option<String>| match v {
            Some(v) => {
                open.map.placements[index].family.insert(p.krate.clone(), v.clone());
            }
            None => {
                open.map.placements[index].family.remove(&p.krate);
            }
        };
        put(&mut open, &p.value);
        let Some(project) = ctx.project.clone() else {
            put(&mut open, &old);
            ctx.open = Some(open);
            return Err(Refusal::new("no project is open."));
        };
        if let Err(e) = open.save(&project) {
            put(&mut open, &old);
            ctx.open = Some(open);
            return Err(Refusal::new(format!("the file refused the change, so nothing was written: {e}")));
        }
        let summary = match &p.value {
            Some(v) => format!("{}: {} = {}", p.id, p.krate, v.trim()),
            None => format!("{}: took {} off", p.id, p.krate),
        };
        let after = Some(family_of(&open, &p.id, &p.krate));
        ctx.open = Some(open);
        Ok(Outcome {
            summary,
            affected: 1,
            saved: true,
            inverse: Some(serde_json::to_value(PieceParams { id: p.id, krate: p.krate, value: old }).expect("params serialise")),
            after,
        })
    }
}

/// **A society definition that `SocietyDefs` refuses to compile is never written** (#104): the saved file is
/// the crate's own `DefsFile`, so the crate's own compile step decides, and its message is the refusal.
#[cfg(feature = "bevy_society")]
fn check_society(krate: &str, text: &str) -> Result<(), Refusal> {
    if krate != "bevy_society" {
        return Ok(());
    }
    bevy_society::SocietyDefs::from_ron(text).map(|_| ()).map_err(|e| Refusal::new(format!("{e}. Nothing was written.")))
}

#[cfg(not(feature = "bevy_society"))]
fn check_society(_krate: &str, _text: &str) -> Result<(), Refusal> {
    Ok(())
}

/// **A dialogue script that `validate_script` refuses is never written** (#106): the saved file is the
/// crate's own `DialogueScript`, so the crate's own validation decides, and its message is the refusal.
#[cfg(feature = "bevy_speech_bubbles")]
fn check_script(krate: &str, text: &str) -> Result<(), Refusal> {
    if krate != "bevy_speech_bubbles" {
        return Ok(());
    }
    let script: bevy_speech_bubbles::DialogueScript =
        ron::from_str(text).map_err(|e| Refusal::new(format!("that is not a dialogue script, so nothing was written: {e}")))?;
    bevy_speech_bubbles::validate_script(&script).map_err(|e| Refusal::new(format!("{e}. Nothing was written.")))
}

#[cfg(not(feature = "bevy_speech_bubbles"))]
fn check_script(_krate: &str, _text: &str) -> Result<(), Refusal> {
    Ok(())
}
