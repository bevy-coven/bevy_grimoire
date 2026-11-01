//! **Triggers on a map, as one registered operation** (#105): `trigger_set`.
//!
//! A trigger is `bevy_society`'s own: an event it listens to (`on`, the crate's `TriggerOn` in RON), an
//! optional Rhai `if`, and Rhai `then`, written against the crate's `ctx` reader and `fx` writer. The editor
//! adds no scripting language. What it adds is where the trigger is kept and who it is for: the file is
//! `<grimoire_dir>/scripts/<map>.ron`, beside the map and the people, and a trigger may be **bound** to named
//! people, to a faction, or to an area of the map. A binding is folded into the Rhai condition when the
//! trigger is compiled (`ctx.name(actor)` and `ctx.member(actor, ..)`), so the crate sees nothing but
//! its own trigger type.
//!
//! An area binds the people whose **marker** (`Person::at`) lies inside the rectangle when the trigger is
//! compiled; where a person walks later does not change it.
//!
//! **A script that does not compile is refused with the crate's own message (which carries the Rhai line and
//! position), and nothing is written.** The check is the crate's `SocietyDefs::compile` over the project's
//! definitions plus the one trigger, so an unknown need, deed or faction is refused the same way.
//!
//! The inverse is the same operation carrying the file as it was (`put_back`), which only an undo may send.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};
use crate::ops_people::People;

/// Who a trigger is for. Everyone, when all three are empty.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bind {
    /// Only these people, by name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub people: Vec<String>,
    /// Only members of this faction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction: Option<String>,
    /// Only people whose marker is inside this world rectangle `[x0, z0, x1, z1]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<[f32; 4]>,
    /// A request, never saved: the id of a note on the open map whose drawn rectangle becomes `area`.
    #[serde(default, skip_serializing)]
    pub note: Option<String>,
}

/// One trigger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub id: String,
    /// `bevy_society`'s `TriggerOn`, as RON: `Activity("work")`, `NeedBelow(need: "food", below: 0.4)`.
    pub on: String,
    /// A Rhai boolean that must hold.
    #[serde(default, rename = "if", skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// Rhai statements to run.
    pub then: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub once: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cooldown_hours: f32,
    #[serde(default, skip_serializing_if = "is_default_bind")]
    pub bind: Bind,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
fn is_default_bind(b: &Bind) -> bool {
    *b == Bind::default()
}

/// A map's triggers: `<grimoire_dir>/scripts/<map>.ron`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scripts {
    pub triggers: Vec<Trigger>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutBack {
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub map: String,
    /// The trigger's id. A new id adds a trigger (and needs `on` and `then`); a standing one is changed.
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub on: Option<String>,
    /// `""` removes the condition.
    #[serde(default, rename = "if")]
    pub condition: Option<String>,
    #[serde(default)]
    pub then: Option<String>,
    #[serde(default)]
    pub once: Option<bool>,
    #[serde(default)]
    pub cooldown_hours: Option<f32>,
    /// Replaces the binding; an empty object makes the trigger for everyone.
    #[serde(default)]
    pub bind: Option<Bind>,
    #[serde(default)]
    pub remove: bool,
    #[serde(default)]
    pub put_back: Option<PutBack>,
}

fn path_of(project: &crate::project::Project, map: &str) -> Result<std::path::PathBuf, Refusal> {
    if map.is_empty() || map.contains(['/', '\\', '.']) {
        return Err(Refusal::new(format!("`{map}` is not a map name. Nothing was written.")));
    }
    Ok(project.grimoire_dir.join("scripts").join(format!("{map}.ron")))
}

/// **The saved triggers of a map**, none when there is no file.
pub fn scripts_of(project: &crate::project::Project, map: &str) -> Scripts {
    path_of(project, map)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| ron::from_str(&t).ok())
        .unwrap_or_default()
}

/// **The saved triggers of a map for an operation that rewrites the file**: none when there is no file,
/// and a refusal naming the file and the field when there is one that does not read (#200).
pub fn try_scripts_of(project: &crate::project::Project, map: &str) -> Result<Scripts, Refusal> {
    crate::map_files::read_or_empty("scripts", project, &path_of(project, map)?)
}

/// **A trigger as the terrain line writes it**: `trigger <id> | <on> | <if> | <then> [| people=..] [| faction=..] [| area=..]`,
/// the form `set_trigger` parses, so a Scripts row can open the line to edit the trigger it shows.
pub fn trigger_line(t: &Trigger) -> String {
    let mut line = format!("trigger {} | {} | {} | {}", t.id, t.on, t.condition.as_deref().unwrap_or(""), t.then);
    if !t.bind.people.is_empty() {
        line.push_str(&format!(" | people={}", t.bind.people.join(",")));
    }
    if let Some(f) = &t.bind.faction {
        line.push_str(&format!(" | faction={f}"));
    }
    if let Some([a, b, c, d]) = t.bind.area {
        line.push_str(&format!(" | area={a},{b},{c},{d}"));
    }
    line
}

/// **The Rhai condition a trigger compiles with**: its own `if` and its binding, joined with `&&`.
/// `None` when nothing restricts it.
pub fn effective_condition(t: &Trigger, people: &People) -> Option<String> {
    let lit = |s: &str| serde_json::to_string(s).expect("a string serialises");
    let names = |names: Vec<&str>| format!("[{}].contains(ctx.name(actor))", names.iter().map(|n| lit(n)).collect::<Vec<_>>().join(", "));
    let mut parts: Vec<String> = Vec::new();
    if let Some(c) = t.condition.as_deref().filter(|c| !c.trim().is_empty()) {
        parts.push(format!("({c})"));
    }
    if !t.bind.people.is_empty() {
        parts.push(names(t.bind.people.iter().map(String::as_str).collect()));
    }
    if let Some(f) = &t.bind.faction {
        parts.push(format!("ctx.member(actor, {})", lit(f)));
    }
    if let Some([x0, z0, x1, z1]) = t.bind.area {
        let (xl, xh, zl, zh) = (x0.min(x1), x0.max(x1), z0.min(z1), z0.max(z1));
        let inside: Vec<&str> = people
            .people
            .iter()
            .filter(|p| (xl..=xh).contains(&p.at.0) && (zl..=zh).contains(&p.at.1))
            .map(|p| p.name.as_str())
            .collect();
        parts.push(if inside.is_empty() { "false".to_owned() } else { names(inside) });
    }
    (!parts.is_empty()).then(|| parts.join(" && "))
}

/// **The name a test run's `then` emits when a trigger fires**, so a run can say which fired for whom.
#[cfg(feature = "bevy_society")]
pub const FIRED: &str = "grimoire_fired";

#[cfg(feature = "bevy_society")]
fn def_of(t: &Trigger, people: &People, watch: bool) -> Result<bevy_society::TriggerDef, Refusal> {
    let on: bevy_society::TriggerOn = ron::from_str(&t.on).map_err(|e| {
        Refusal::new(format!("trigger `{}`: `on` is not a bevy_society event ({e}). Write it as RON, for example `Activity(\"work\")`. Nothing was saved.", t.id))
    })?;
    let then = if watch {
        format!("fx.emit({}, #{{trigger: {}}});\n{}", serde_json::to_string(FIRED).expect("str"), serde_json::to_string(&t.id).expect("str"), t.then)
    } else {
        t.then.clone()
    };
    Ok(bevy_society::TriggerDef {
        id: t.id.clone(),
        on,
        condition: effective_condition(t, people),
        then,
        once: t.once,
        cooldown_hours: t.cooldown_hours,
    })
}

/// The project's society definitions as authored, or the starter pack.
#[cfg(feature = "bevy_society")]
fn base_file(ctx: &Ctx) -> Result<bevy_society::DefsFile, Refusal> {
    match crate::ops_family::settings_path(ctx, "bevy_society").ok().and_then(|f| std::fs::read_to_string(f).ok()) {
        Some(text) => bevy_society::DefsFile::from_ron(&text).map_err(|e| Refusal::new(format!("the society definitions do not parse: {e}"))),
        None => bevy_society::DefsFile::starter().map_err(|e| Refusal::new(format!("the starter pack does not parse: {e}"))),
    }
}

/// **The compiled definitions a run uses**: the project's definitions (or the starter pack) with every saved
/// trigger of `map` added, each emitting [`FIRED`] when it runs.
#[cfg(feature = "bevy_society")]
pub fn defs_with_triggers(ctx: &Ctx, map: &str, people: &People) -> Result<bevy_society::SocietyDefs, Refusal> {
    let Some(project) = ctx.project.as_ref() else {
        return Err(Refusal::new("no project is open."));
    };
    let mut file = base_file(ctx)?;
    for t in try_scripts_of(project, map)?.triggers {
        file.triggers.push(def_of(&t, people, true)?);
    }
    bevy_society::SocietyDefs::compile(file).map_err(|e| Refusal::new(format!("the society definitions do not compile: {e}")))
}

/// **The rectangle of a note on the open map**, in metres: where the person drew it.
fn note_rect(ctx: &Ctx, map: &str, id: &str) -> Result<[f32; 4], Refusal> {
    let open = ctx.open.as_ref().filter(|o| o.map.name == map).ok_or_else(|| {
        Refusal::new(format!("`{map}` is not the open map, so its notes cannot be read to bind a trigger. Nothing was changed."))
    })?;
    let notes = bevy_grimoire_core::notes::Notes::load(&bevy_grimoire_core::notes::path_for_map(&open.map_path)).unwrap_or_default();
    notes
        .get(id)
        .and_then(|n| n.frame_rect())
        .ok_or_else(|| Refusal::new(format!("`{map}` has no note `{id}` with an area, so a trigger cannot be bound to it. Nothing was changed.")))
}

/// **Set, change or remove one trigger.**
pub struct TriggerSet;

impl TriggerSet {
    fn apply(ctx: &Ctx, raw: &serde_json::Value, write: bool) -> Result<Outcome, Refusal> {
        let p: Params = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("this is not a trigger: {e}. The fields are `map`, `id`, `on`, `if`, `then`, `once`, `cooldown_hours`, `bind` and `remove`."))
        })?;
        let Some(project) = ctx.project.as_ref() else {
            return Err(Refusal::new("no project is open, so there is nowhere to keep a trigger."));
        };
        let path = path_of(project, &p.map)?;
        let before = std::fs::read_to_string(&path).ok();
        let inverse = json!({"map": p.map, "put_back": {"text": before}});

        let (text, summary) = if let Some(back) = &p.put_back {
            (back.text.clone(), format!("triggers {}: put back", p.map))
        } else {
            let Some(id) = p.id.as_deref().filter(|n| !n.trim().is_empty()) else {
                return Err(Refusal::new("a trigger needs an `id`. Nothing was changed."));
            };
            let mut scripts = try_scripts_of(project, &p.map)?;
            let at_row = scripts.triggers.iter().position(|t| t.id == id);
            let summary = if p.remove {
                let Some(i) = at_row else {
                    return Err(Refusal::new(format!("`{}` has no trigger `{id}`. Nothing was changed.", p.map)));
                };
                scripts.triggers.remove(i);
                format!("removed trigger {id}")
            } else {
                let mut t = match at_row {
                    Some(i) => scripts.triggers[i].clone(),
                    None => match (&p.on, &p.then) {
                        (Some(on), Some(then)) => Trigger {
                            id: id.to_owned(),
                            on: on.clone(),
                            condition: None,
                            then: then.clone(),
                            once: false,
                            cooldown_hours: 0.0,
                            bind: Bind::default(),
                        },
                        _ => return Err(Refusal::new(format!("`{id}` is new, so it needs `on` and `then`. Nothing was changed."))),
                    },
                };
                if let Some(on) = &p.on {
                    t.on = on.clone();
                }
                if let Some(c) = &p.condition {
                    t.condition = (!c.is_empty()).then(|| c.clone());
                }
                if let Some(then) = &p.then {
                    t.then = then.clone();
                }
                if let Some(v) = p.once {
                    t.once = v;
                }
                if let Some(h) = p.cooldown_hours {
                    if !(h.is_finite() && h >= 0.0) {
                        return Err(Refusal::new(format!("`cooldown_hours` is {h}; it is 0 or more. Nothing was changed.")));
                    }
                    t.cooldown_hours = h;
                }
                if let Some(b) = &p.bind {
                    if let Some([a, b2, c, d]) = b.area
                        && ![a, b2, c, d].iter().all(|v| v.is_finite())
                    {
                        return Err(Refusal::new("the area is not four finite numbers. Nothing was changed."));
                    }
                    t.bind = b.clone();
                    if let Some(id) = t.bind.note.take() {
                        t.bind.area = Some(note_rect(ctx, &p.map, &id)?);
                    }
                }
                let people = crate::ops_people::people_of(project, &p.map);
                for n in &t.bind.people {
                    if !people.people.iter().any(|q| &q.name == n) {
                        return Err(Refusal::new(format!("nobody on `{}` is called `{n}`, so a trigger cannot be bound to them. Nothing was changed.", p.map)));
                    }
                }
                check(ctx, &t, &people, &scripts)?;
                let summary = format!("{} trigger {id}", if at_row.is_some() { "changed" } else { "added" });
                match at_row {
                    Some(i) => scripts.triggers[i] = t,
                    None => scripts.triggers.push(t),
                }
                summary
            };
            let text = if scripts.triggers.is_empty() {
                None
            } else {
                Some(
                    ron::ser::to_string_pretty(&scripts, ron::ser::PrettyConfig::default())
                        .map_err(|e| Refusal::new(format!("the triggers could not be written: {e}")))?,
                )
            };
            (text, summary)
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
        Ok(Outcome { summary, affected: 1, saved: write, inverse: Some(inverse), after: Some(json!({"map": p.map, "text": text})) })
    }
}

/// Compile the project's definitions with this trigger in place of its saved self.
#[cfg(feature = "bevy_society")]
fn check(ctx: &Ctx, t: &Trigger, people: &People, saved: &Scripts) -> Result<(), Refusal> {
    let mut file = base_file(ctx)?;
    for other in saved.triggers.iter().filter(|o| o.id != t.id) {
        file.triggers.push(def_of(other, people, false)?);
    }
    file.triggers.push(def_of(t, people, false)?);
    bevy_society::SocietyDefs::compile(file)
        .map(|_| ())
        .map_err(|e| Refusal::new(format!("trigger `{}` was refused: {e}. Nothing was saved.", t.id)))
}

#[cfg(not(feature = "bevy_society"))]
fn check(_ctx: &Ctx, _t: &Trigger, _people: &People, _saved: &Scripts) -> Result<(), Refusal> {
    Err(Refusal::new("this build has no `bevy_society` feature, so a script cannot be checked and nothing was saved."))
}

impl Operation for TriggerSet {
    fn name(&self) -> &'static str {
        "trigger_set"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["scripts"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        params.get("put_back").is_some()
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["map"],
            "properties": {
                "map": {"type": "string", "description": "The map the trigger belongs to."},
                "id": {"type": "string", "description": "The trigger's id. A new id adds a trigger and needs `on` and `then`; a standing one is changed."},
                "on": {"type": "string", "description": "A bevy_society event as RON, e.g. `Activity(\"work\")` or `NeedBelow(need: \"food\", below: 0.4)`."},
                "if": {"type": "string", "description": "A Rhai boolean written against `ctx` (`ctx.trust(actor, target) < 0.2`); \"\" removes it."},
                "then": {"type": "string", "description": "Rhai statements written against `fx` (`fx.bark(actor, \"Hello\")`). A script that does not compile is refused with its line, and nothing is saved."},
                "once": {"type": "boolean", "description": "Fire at most once per person."},
                "cooldown_hours": {"type": "number", "minimum": 0, "description": "Game-hours between firings per person."},
                "bind": {"type": "object", "additionalProperties": false, "description": "Who the trigger is for; replaces the old binding. Nothing bound means everyone.", "properties": {
                    "people": {"type": "array", "items": {"type": "string"}, "description": "Only these people, by name."},
                    "faction": {"type": "string", "description": "Only members of this faction."},
                    "area": {"type": "array", "items": {"type": "number"}, "minItems": 4, "maxItems": 4, "description": "Only people whose marker is inside this world rectangle [x0, z0, x1, z1]."},
                    "note": {"type": "string", "description": "The id of a note on the open map: its drawn rectangle becomes `area`."}
                }},
                "remove": {"type": "boolean", "description": "Remove the trigger."},
                "put_back": {"type": "object", "description": "An undo's put-back. Only an undo sends this."}
            }
        })
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let map = after.get("map")?.as_str()?;
        let path = path_of(ctx.project.as_ref()?, map).ok()?;
        Some(json!({"map": map, "text": std::fs::read_to_string(path).ok()}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let o = Self::apply(ctx, params, false)?;
        Ok(json!({"change": o.summary}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Self::apply(ctx, params, true)
    }
}
