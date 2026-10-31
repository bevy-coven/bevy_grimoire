//! **People on a map, as one registered operation** (#104): `person_set`.
//!
//! A person is a marker: a name, where they stand, a faction, and the buildings that are their home and
//! their workplace. Their behaviour is `bevy_society`'s, never the editor's; the marker is only what the
//! game needs to spawn them (`Home(entity)`, a faction membership). The file is
//! `<grimoire_dir>/people/<map>.ron`, beside the map, so the game reads it and the editor reloads it.
//!
//! A home or workplace names a placement that is on the map, and **a placement that someone lives or works
//! in cannot be removed** (`tenant_of`, called by `placement_remove`): the refusal names the person.
//! With the `bevy_society` feature a faction must be one the project's own society definitions declare.
//!
//! The inverse is the same operation carrying the file as it was (`put_back`), which only an undo may send.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

/// One person on the map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Person {
    pub name: String,
    /// World `(x, z)` where they start the day.
    pub at: (f32, f32),
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction: Option<String>,
    /// The id of the placement they live in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
    /// The id of the placement they work in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work: Option<String>,
    /// Ties this person starts with, to other people on the map (`bevy_society`'s `TieSpec`, by name).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ties: Vec<Tie>,
    /// The name of the conversation they hold when talked to: one the project's `bevy_speech_bubbles`
    /// `DialogueScript` defines (#106).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation: Option<String>,
}

/// A directed starting tie from one person to another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tie {
    /// The other person's name.
    pub with: String,
    /// Liking in `[-1, 1]`.
    pub affinity: f32,
    /// Reliance in `[0, 1]`.
    pub trust: f32,
}

/// A map's people: `<grimoire_dir>/people/<map>.ron`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct People {
    pub people: Vec<Person>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutBack {
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TieParams {
    /// The other person's name; they must be on the map.
    pub with: String,
    /// Liking in `[-1, 1]`; 0 when left out on a new tie, unchanged on a standing one.
    #[serde(default)]
    pub affinity: Option<f32>,
    /// Reliance in `[0, 1]`; 0.5 when left out on a new tie, unchanged on a standing one.
    #[serde(default)]
    pub trust: Option<f32>,
    /// End the tie.
    #[serde(default)]
    pub remove: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub map: String,
    /// The person's name. A new name adds a person (and needs `at`); a standing one is changed.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub at: Option<(f32, f32)>,
    /// A faction id; `""` leaves the faction.
    #[serde(default)]
    pub faction: Option<String>,
    /// A placement id; `""` clears it.
    #[serde(default)]
    pub home: Option<String>,
    #[serde(default)]
    pub work: Option<String>,
    /// A conversation name from the project's dialogue script; `""` clears it (#106).
    #[serde(default)]
    pub conversation: Option<String>,
    /// Take the person off the map.
    #[serde(default)]
    pub remove: bool,
    /// Set the tie from this person to another person, or end it with `remove`.
    #[serde(default)]
    pub tie: Option<TieParams>,
    #[serde(default)]
    pub put_back: Option<PutBack>,
}

fn path_of(project: &crate::project::Project, map: &str) -> Result<std::path::PathBuf, Refusal> {
    if map.is_empty() || map.contains(['/', '\\', '.']) {
        return Err(Refusal::new(format!("`{map}` is not a map name. Nothing was written.")));
    }
    Ok(project.grimoire_dir.join("people").join(format!("{map}.ron")))
}

/// **The saved people of a map**, nobody when there is no file.
pub fn people_of(project: &crate::project::Project, map: &str) -> People {
    path_of(project, map)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| ron::from_str(&t).ok())
        .unwrap_or_default()
}

/// **Who lives or works in one of `ids`**: `(person, "home" | "work", placement)`, the first found.
pub fn tenant_of(project: &crate::project::Project, map: &str, ids: &[String]) -> Option<(String, &'static str, String)> {
    people_of(project, map).people.into_iter().find_map(|p| {
        if let Some(h) = p.home.as_ref().filter(|h| ids.contains(h)) {
            return Some((p.name.clone(), "home", h.clone()));
        }
        p.work.as_ref().filter(|w| ids.contains(w)).map(|w| (p.name.clone(), "work", w.clone()))
    })
}

#[cfg(feature = "bevy_society")]
fn check_faction(ctx: &Ctx, faction: &str) -> Result<(), Refusal> {
    let Ok(path) = crate::ops_family::settings_path(ctx, "bevy_society") else { return Ok(()) };
    let Ok(text) = std::fs::read_to_string(&path) else { return Ok(()) };
    let Ok(file) = ron::from_str::<bevy_society::DefsFile>(&text) else { return Ok(()) };
    if file.factions.iter().any(|f| f.id == faction) {
        return Ok(());
    }
    let known: Vec<&str> = file.factions.iter().map(|f| f.id.as_str()).collect();
    Err(Refusal::new(format!("the society definitions declare no faction `{faction}` (they declare: {}). Nothing was changed.", known.join(", "))))
}

#[cfg(not(feature = "bevy_society"))]
fn check_faction(_ctx: &Ctx, _faction: &str) -> Result<(), Refusal> {
    Ok(())
}

/// **A conversation a person holds must be one the dialogue script defines** (#106). A project without the
/// script, or a build without the crate, has nothing to check it against.
#[cfg(feature = "bevy_speech_bubbles")]
fn check_conversation(ctx: &Ctx, name: &str) -> Result<(), Refusal> {
    let path = crate::ops_family::settings_path(ctx, "bevy_speech_bubbles")
        .map_err(|_| Refusal::new("this project does not use bevy_speech_bubbles, so there is no dialogue script to hold a conversation. Nothing was changed."))?;
    let text = std::fs::read_to_string(&path)
        .map_err(|e| Refusal::new(format!("{} cannot be read, so no conversation can be attached: {e}. Nothing was changed.", path.display())))?;
    let script: bevy_speech_bubbles::DialogueScript =
        ron::from_str(&text).map_err(|e| Refusal::new(format!("the project's dialogue script is not readable: {e}. Nothing was changed.")))?;
    if script.conversations.contains_key(name) {
        return Ok(());
    }
    let known: Vec<&str> = script.conversations.keys().map(String::as_str).collect();
    Err(Refusal::new(format!("the dialogue script has no conversation `{name}` (it has: {}). Nothing was changed.", known.join(", "))))
}

#[cfg(not(feature = "bevy_speech_bubbles"))]
fn check_conversation(_ctx: &Ctx, _name: &str) -> Result<(), Refusal> {
    Ok(())
}

fn building(ctx: &Ctx, what: &str, id: &str) -> Result<(), Refusal> {
    let Some(open) = ctx.open.as_ref() else {
        return Err(Refusal::new("no map is open, so there is no building to name."));
    };
    if open.map.placements.iter().any(|p| p.id == id) {
        Ok(())
    } else {
        Err(Refusal::new(format!("no placement is called `{id}` on this map, so it cannot be a {what}. Nothing was changed.")))
    }
}

/// **Author a person.**
pub struct PersonSet;

impl PersonSet {
    fn apply(ctx: &Ctx, raw: &serde_json::Value, write: bool) -> Result<Outcome, Refusal> {
        let p: Params = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("this is not a person: {e}. The fields are `map`, `name`, `at`, `faction`, `home`, `work`, `tie` and `remove`."))
        })?;
        let Some(project) = ctx.project.as_ref() else {
            return Err(Refusal::new("no project is open, so there is nobody to place."));
        };
        let path = path_of(project, &p.map)?;
        let before = std::fs::read_to_string(&path).ok();
        let inverse = json!({"map": p.map, "put_back": {"text": before}});

        let (text, summary) = if let Some(back) = &p.put_back {
            (back.text.clone(), format!("people {}: put back", p.map))
        } else {
            let Some(name) = p.name.as_deref().filter(|n| !n.trim().is_empty()) else {
                return Err(Refusal::new("a person needs a `name`. Nothing was changed."));
            };
            let mut people = people_of(project, &p.map);
            let at_row = people.people.iter().position(|q| q.name == name);
            let summary = if p.remove {
                let Some(i) = at_row else {
                    return Err(Refusal::new(format!("nobody on `{}` is called `{name}`. Nothing was changed.", p.map)));
                };
                people.people.remove(i);
                for q in &mut people.people {
                    q.ties.retain(|t| t.with != name);
                }
                format!("{name} left {}", p.map)
            } else {
                let mut person = match at_row {
                    Some(i) => people.people[i].clone(),
                    None => match p.at {
                        Some(at) => Person { name: name.to_owned(), at, faction: None, home: None, work: None, ties: Vec::new(), conversation: None },
                        None => return Err(Refusal::new(format!("`{name}` is new, so they need an `at` (x, z). Nothing was changed."))),
                    },
                };
                if let Some(at) = p.at {
                    person.at = at;
                }
                if let Some(f) = &p.faction {
                    if f.is_empty() {
                        person.faction = None;
                    } else {
                        check_faction(ctx, f)?;
                        person.faction = Some(f.clone());
                    }
                }
                match p.conversation.as_deref() {
                    Some("") => person.conversation = None,
                    Some(c) => {
                        check_conversation(ctx, c)?;
                        person.conversation = Some(c.to_owned());
                    }
                    None => {}
                }
                for (slot, what, value) in [(&mut person.home, "home", &p.home), (&mut person.work, "workplace", &p.work)] {
                    match value.as_deref() {
                        Some("") => *slot = None,
                        Some(id) => {
                            building(ctx, what, id)?;
                            *slot = Some(id.to_owned());
                        }
                        None => {}
                    }
                }
                if let Some(t) = &p.tie {
                    if t.with == name {
                        return Err(Refusal::new(format!("{name} cannot have a tie to themselves. Nothing was changed.")));
                    }
                    if !people.people.iter().any(|q| q.name == t.with) {
                        return Err(Refusal::new(format!("nobody on `{}` is called `{}`, so {name} cannot have a tie to them. Nothing was changed.", p.map, t.with)));
                    }
                    for (v, lo, hi, what) in [(t.affinity, -1.0, 1.0, "affinity"), (t.trust, 0.0, 1.0, "trust")] {
                        if let Some(v) = v
                            && !(v.is_finite() && (lo..=hi).contains(&v))
                        {
                            return Err(Refusal::new(format!("the tie's {what} is {v}; it is between {lo} and {hi}. Nothing was changed.")));
                        }
                    }
                    let row = person.ties.iter().position(|x| x.with == t.with);
                    if t.remove {
                        let Some(i) = row else {
                            return Err(Refusal::new(format!("{name} has no tie to {}. Nothing was changed.", t.with)));
                        };
                        person.ties.remove(i);
                    } else {
                        match row {
                            Some(i) => {
                                person.ties[i].affinity = t.affinity.unwrap_or(person.ties[i].affinity);
                                person.ties[i].trust = t.trust.unwrap_or(person.ties[i].trust);
                            }
                            None => person.ties.push(Tie { with: t.with.clone(), affinity: t.affinity.unwrap_or(0.0), trust: t.trust.unwrap_or(0.5) }),
                        }
                    }
                }
                let summary = format!("{} {name}", if at_row.is_some() { "changed" } else { "added" });
                match at_row {
                    Some(i) => people.people[i] = person,
                    None => people.people.push(person),
                }
                summary
            };
            let text = ron::ser::to_string_pretty(&people, ron::ser::PrettyConfig::default())
                .map_err(|e| Refusal::new(format!("the people could not be written: {e}")))?;
            (Some(text), summary)
        };

        if write {
            match &text {
                Some(t) => {
                    if let Some(dir) = path.parent() {
                        std::fs::create_dir_all(dir).map_err(|e| Refusal::new(format!("{} cannot be made: {e}", dir.display())))?;
                    }
                    std::fs::write(&path, t)
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

impl Operation for PersonSet {
    fn name(&self) -> &'static str {
        "person_set"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["people"]
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
                "map": {"type": "string", "description": "The map the person lives on."},
                "name": {"type": "string", "description": "The person's name. A new name adds a person and needs `at`; a standing one is changed."},
                "at": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2, "description": "World [x, z] where they start the day."},
                "faction": {"type": "string", "description": "A faction the project's society definitions declare; \"\" leaves it."},
                "home": {"type": "string", "description": "The id of the placement they live in; \"\" clears it. That placement cannot be removed while they live there."},
                "work": {"type": "string", "description": "The id of the placement they work in; \"\" clears it."},
                "tie": {"type": "object", "additionalProperties": false, "required": ["with"], "description": "A starting tie from this person to another person on the map; the run begins with it. `remove` ends it.", "properties": {
                    "with": {"type": "string"},
                    "affinity": {"type": "number", "minimum": -1, "maximum": 1},
                    "trust": {"type": "number", "minimum": 0, "maximum": 1},
                    "remove": {"type": "boolean"}
                }},
                "remove": {"type": "boolean", "description": "Take the named person off the map."},
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

/// **Where one person stood after the run, and how many ties they had formed.**
#[cfg(feature = "bevy_society")]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stood {
    pub name: String,
    pub at: (f32, f32),
    pub ties: usize,
    /// Each tie as `(other person, affinity, trust)` after the run.
    pub bonds: Vec<(String, f32, f32)>,
}

/// What a run's receipt adds about scripts: each trigger that fired with the people it fired for, then any script
/// that failed. Empty when neither happened, so a society without triggers reads as before.
#[cfg(feature = "bevy_society")]
fn trigger_receipt(r: &RunReport) -> String {
    let mut by_trigger: Vec<(&str, Vec<&str>)> = Vec::new();
    for f in &r.fired {
        let i = match by_trigger.iter().position(|(t, _)| *t == f.trigger) {
            Some(i) => i,
            None => {
                by_trigger.push((&f.trigger, Vec::new()));
                by_trigger.len() - 1
            }
        };
        if !f.person.is_empty() && !by_trigger[i].1.contains(&f.person.as_str()) {
            by_trigger[i].1.push(&f.person);
        }
    }
    let mut out = String::new();
    if !by_trigger.is_empty() {
        let list: Vec<String> = by_trigger
            .iter()
            .map(|(t, who)| if who.is_empty() { (*t).to_owned() } else { format!("{t} ({})", who.join(", ")) })
            .collect();
        out.push_str(&format!("; fired: {}", list.join(", ")));
    }
    if !r.script_errors.is_empty() {
        out.push_str(&format!("; script errors: {}", r.script_errors.join(" | ")));
    }
    out
}

/// **The society after `days` game-days**: who stands where and how many ties each has, with one hash of
/// every position and tie so two runs can be compared exactly.
#[cfg(feature = "bevy_society")]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunReport {
    pub seed: u64,
    pub days: f32,
    pub ticks: usize,
    pub people: Vec<Stood>,
    /// Every time a saved trigger ran for someone, in the order they ran (#105).
    pub fired: Vec<Fired>,
    /// Scripts that failed while the society ran: `owner: message`.
    pub script_errors: Vec<String>,
    pub hash: u64,
}

/// One trigger running, for one person.
#[cfg(feature = "bevy_society")]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fired {
    pub trigger: String,
    /// The person the event was about, `""` for an event about nobody (a tick, an hour).
    pub person: String,
    pub tick: usize,
}

/// Game-day length in seconds the preview runs at; one society tick is 1/64 s, so a day is 4096 ticks.
#[cfg(feature = "bevy_society")]
const SECONDS_PER_DAY: f32 = 64.0;
#[cfg(feature = "bevy_society")]
const TICK_HZ: f32 = 64.0;

/// **Run a map's people forward `days` game-days** in `bevy_society`'s own headless app, never touching
/// the project: each person is spawned at their marker with their faction, their home and their workplace
/// bound as `Home` and the `"work"` place, and the crate's own systems decide where they go. The editor adds
/// no behaviour. Same `defs`, people, placements, seed and days give the same report, bit for bit.
#[cfg(feature = "bevy_society")]
pub fn run_forward(
    defs: bevy_society::SocietyDefs,
    placements: &[(String, (f32, f32))],
    people: &People,
    seed: u64,
    days: f32,
) -> Result<RunReport, Refusal> {
    use bevy::prelude::*;
    use bevy::time::TimeUpdateStrategy;
    use bevy_society::{Autonomous, BoundPlaces, Home, Person, PersonIndex, Place, PlaceTags, SocietyPlugins, SocietySettings, Ties};

    if !(days.is_finite() && days > 0.0 && days <= 7.0) {
        return Err(Refusal::new(format!("`days` is {days}; a run is more than 0 and at most 7 days.")));
    }
    let settings = SocietySettings {
        seed,
        tick_secs: 1.0 / TICK_HZ,
        seconds_per_day: SECONDS_PER_DAY,
        start_hour: 6.0,
        ..SocietySettings::default()
    };
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::transform::TransformPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / TICK_HZ)))
        .insert_resource(settings)
        .insert_resource(defs.clone())
        .add_plugins(SocietyPlugins);
    app.finish();
    app.cleanup();

    let mut places: std::collections::BTreeMap<String, Entity> = std::collections::BTreeMap::new();
    for p in &people.people {
        for id in [&p.home, &p.work].into_iter().flatten() {
            if places.contains_key(id) {
                continue;
            }
            let Some((_, (x, z))) = placements.iter().find(|(pid, _)| pid == id) else {
                return Err(Refusal::new(format!("{} names `{id}`, which is not on the map.", p.name)));
            };
            let e = app
                .world_mut()
                .spawn((
                    Name::new(id.clone()),
                    Place { capacity: 8 },
                    PlaceTags(vec!["gathering".to_owned(), "food".to_owned(), "work".to_owned()]),
                    Transform::from_xyz(*x, 0.0, *z),
                ))
                .id();
            places.insert(id.clone(), e);
        }
    }
    for p in &people.people {
        let mut e = app.world_mut().spawn((
            Person,
            Autonomous,
            Name::new(p.name.clone()),
            Transform::from_xyz(p.at.0, 0.0, p.at.1),
        ));
        if let Some(h) = p.home.as_ref().and_then(|h| places.get(h)) {
            e.insert(Home(*h));
        }
        if let Some(w) = p.work.as_ref().and_then(|w| places.get(w)) {
            e.insert(BoundPlaces(vec![("work".to_owned(), *w)]));
        }
        if let Some(f) = &p.faction {
            let m = defs.memberships([f.as_str()]).map_err(|e| Refusal::new(format!("{}: {e}", p.name)))?;
            e.insert(m);
        }
    }

    let named: std::collections::BTreeMap<String, Entity> = {
        let mut q = app.world_mut().query_filtered::<(Entity, &Name), With<Person>>();
        q.iter(app.world()).map(|(e, n)| (n.as_str().to_owned(), e)).collect()
    };
    let by_name = |n: &str| named.get(n).copied();
    for p in people.people.iter().filter(|p| !p.ties.is_empty()) {
        let owner = by_name(&p.name).expect("spawned above");
        let mut ties = Ties::default();
        for t in &p.ties {
            let Some(target) = by_name(&t.with) else {
                return Err(Refusal::new(format!("{} has a tie to `{}`, who is not on the map.", p.name, t.with)));
            };
            ties.entry_with(target, t.affinity).trust = t.trust;
        }
        app.world_mut().entity_mut(owner).insert(ties);
    }

    let ticks = (days * SECONDS_PER_DAY * TICK_HZ) as usize + 1;
    let mut fired = Vec::new();
    let mut script_errors = Vec::new();
    let mut events = bevy::ecs::message::MessageCursor::<bevy_society::ScriptEvent>::default();
    let mut errors = bevy::ecs::message::MessageCursor::<bevy_society::ScriptError>::default();
    for tick in 0..ticks {
        app.update();
        let w = app.world();
        for e in events.read(w.resource::<bevy::ecs::message::Messages<bevy_society::ScriptEvent>>()) {
            if e.name != crate::ops_scripts::FIRED {
                continue;
            }
            let trigger = e.data.iter().find_map(|(k, v)| match (k.as_str(), v) {
                ("trigger", bevy_society::ScriptValue::Text(t)) => Some(t.clone()),
                _ => None,
            });
            let person = e.actor.and_then(|a| w.get::<Name>(a)).map(|n| n.as_str().to_owned()).unwrap_or_default();
            fired.push(Fired { trigger: trigger.unwrap_or_default(), person, tick });
        }
        for e in errors.read(w.resource::<bevy::ecs::message::Messages<bevy_society::ScriptError>>()) {
            script_errors.push(format!("{}: {}", e.owner, e.message));
        }
    }

    let ids: Vec<(bevy_society::PersonId, Entity)> = app.world().resource::<PersonIndex>().sorted.clone();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: u64| {
        for b in v.to_le_bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let mut out = Vec::new();
    for (id, e) in ids {
        let w = app.world();
        let t = w.get::<Transform>(e).map(|t| t.translation).unwrap_or_default();
        let ties = w.get::<Ties>(e).map(|t| t.0.len()).unwrap_or(0);
        mix(u64::from(id.0));
        mix(u64::from(t.x.to_bits()));
        mix(u64::from(t.z.to_bits()));
        mix(ties as u64);
        let bonds = w
            .get::<Ties>(e)
            .map(|ts| {
                ts.0.iter()
                    .map(|t| (w.get::<Name>(t.target).map(|n| n.as_str().to_owned()).unwrap_or_default(), t.affinity, t.trust))
                    .collect()
            })
            .unwrap_or_default();
        out.push(Stood { name: w.get::<Name>(e).map(|n| n.as_str().to_owned()).unwrap_or_default(), at: (t.x, t.z), ties, bonds });
    }
    Ok(RunReport { seed, days, ticks, people: out, fired, script_errors, hash })
}

#[cfg(feature = "bevy_society")]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunParams {
    map: String,
    #[serde(default)]
    seed: Option<u64>,
    #[serde(default)]
    days: Option<f32>,
}

/// **Run a map's people forward and say where they went** (#104). Reads; changes nothing.
pub struct PeopleRun;

impl PeopleRun {
    #[cfg(feature = "bevy_society")]
    fn report(ctx: &Ctx, raw: &serde_json::Value) -> Result<(RunReport, usize), Refusal> {
        let p: RunParams = serde_json::from_value(raw.clone())
            .map_err(|e| Refusal::new(format!("this is not a people run: {e}. The fields are `map`, `seed` and `days`.")))?;
        let Some(project) = ctx.project.as_ref() else {
            return Err(Refusal::new("no project is open, so there is nobody to run."));
        };
        let Some(open) = ctx.open.as_ref().filter(|o| o.map.name == p.map) else {
            return Err(Refusal::new(format!("`{}` is not the open map, so its placements cannot be read.", p.map)));
        };
        let people = people_of(project, &p.map);
        if people.people.is_empty() {
            return Err(Refusal::new(format!("nobody lives on `{}`; author someone with person_set first.", p.map)));
        }
        let defs = crate::ops_scripts::defs_with_triggers(ctx, &p.map, &people)?;
        let at: Vec<(String, (f32, f32))> = open.map.placements.iter().map(|q| (q.id.clone(), q.at)).collect();
        let n = people.people.len();
        run_forward(defs, &at, &people, p.seed.unwrap_or(0), p.days.unwrap_or(1.0)).map(|r| (r, n))
    }
    #[cfg(not(feature = "bevy_society"))]
    fn report(_ctx: &Ctx, _raw: &serde_json::Value) -> Result<((), usize), Refusal> {
        Err(Refusal::new("this build has no `bevy_society` feature, so a society cannot be run."))
    }
}

impl Operation for PeopleRun {
    fn name(&self) -> &'static str {
        "people_run"
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["map"],
            "properties": {
                "map": {"type": "string", "description": "The open map whose people are run."},
                "seed": {"type": "integer", "description": "The society's seed. Same seed, same day. Default 0."},
                "days": {"type": "number", "description": "Game-days to run, more than 0 and at most 7. Default 1."}
            }
        })
    }
    fn state_of(&self, _ctx: &Ctx, _after: &serde_json::Value) -> Option<serde_json::Value> {
        None
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        #[cfg(feature = "bevy_society")]
        {
            let (r, _) = Self::report(ctx, params)?;
            Ok(serde_json::to_value(r).expect("report serialises"))
        }
        #[cfg(not(feature = "bevy_society"))]
        {
            Self::report(ctx, params).map(|_| json!({}))
        }
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        #[cfg(feature = "bevy_society")]
        {
            let (r, n) = Self::report(ctx, params)?;
            let mut summary = format!("ran {n} people for {} day(s), seed {}: hash {:016x}", r.days, r.seed, r.hash);
            summary.push_str(&trigger_receipt(&r));
            Ok(Outcome { summary, affected: 0, saved: false, inverse: None, after: Some(serde_json::to_value(r).expect("report serialises")) })
        }
        #[cfg(not(feature = "bevy_society"))]
        {
            Self::report(ctx, params).map(|_| unreachable!("a build without bevy_society always refuses"))
        }
    }
}
