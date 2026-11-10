//! **What a `bevy_lights` placement carries** (#103): the crate's own `Flicker` (a bare light that breathes)
//! or its own `Fire` (a whole fire: flame, pooled light, sparks, smoke), as one RON value under the family
//! key `bevy_lights`. The editor never defines a second schema: [`Fixture::read`] only decides which of the
//! crate's two types a value is, then lets the crate's own serde read it.
//!
//! Both types are `serde(default)` and neither denies unknown fields, so `(kind: Campfir)` read as a
//! `Flicker` would be a default flicker with the typo ignored. A value is a `Fire` when it names one of
//! `Fire`'s own fields ([`FIRE_FIELDS`]); `phase` is shared, so it decides nothing.

use bevy_lights::{FIRE_EDGE_LUX, Fire, Flicker, flame_light_offset};

/// The family key a placement's fixture is saved under, as listed in `family.yaml`.
pub const KEY: &str = "bevy_lights";

/// The fields only a `Fire` has. A value that names any of them is a `Fire`.
pub const FIRE_FIELDS: [&str; 7] = ["kind", "lumens", "color", "embers", "smoke", "tongues", "bed"];

/// A placement's light: a bare breathing light or a whole fire.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fixture {
    Flicker(Flicker),
    Fire(Fire),
}

impl Fixture {
    /// Read `text` as the crate's own type for the value it names. The error says which type it read it as.
    pub fn read(text: &str) -> Result<Fixture, String> {
        let names_a_fire = match bevy_grimoire_core::yaml::to_json(text, "bevy_lights") {
            Ok(serde_json::Value::Object(fields)) => fields.keys().any(|k| FIRE_FIELDS.contains(&k.as_str())),
            _ => false,
        };
        if names_a_fire {
            bevy_grimoire_core::yaml::from_str::<Fire>(text, "bevy_lights").map(Fixture::Fire).map_err(|p| format!("`{text}` is not a bevy_lights Fire (kind: Candle, Lantern, Torch, Brazier, Campfire or Bonfire): {}", p.line()))
        } else {
            bevy_grimoire_core::yaml::from_str::<Flicker>(text, "bevy_lights").map(Fixture::Flicker).map_err(|p| format!("`{text}` is not a bevy_lights Flicker (and names no field only a Fire has, such as `kind`): {}", p.line()))
        }
    }

    /// The light's mean lumens, the number the light map and the pool budget read: a `Flicker`'s `base`, and for a `Fire` its
    /// own `lumens` or, with none, the crate's size for its kind (the lumens that put [`FIRE_EDGE_LUX`] on the ground at the
    /// edge of the kind's reach, with the light at the flame's middle), the same figure `FirePlugin` hands the pool.
    pub fn lumens(&self) -> f32 {
        match self {
            Fixture::Flicker(f) => f.base,
            Fixture::Fire(fire) => fire.lumens.unwrap_or_else(|| fire.kind.lumens(FIRE_EDGE_LUX, flame_light_offset(fire.kind.height())[1])),
        }
    }
}
