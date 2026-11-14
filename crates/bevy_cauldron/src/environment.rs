//! **A map's environment: climate, date, clock, place and forced weather** (#98).
//!
//! The data is `bevy_elements`' own serde types, written as YAML in `<project>/family/bevy_elements.yaml`
//! (the file `family_set` edits and the game's copy of the plugin reads). [`ElementsSettings`] only
//! lists which of the crate's resources the file holds; it defines no field of its own except which
//! weather preset is forced, because the crate keeps that inside a private director.
//!
//! [`apply_environment`] copies a changed file into the running resources, so the editor's preview
//! and the game see the same values. Forcing rain is `WeatherDirector::force`; clearing the field
//! releases the weather back to the climate's own schedule.

use std::time::Duration;

use bevy::prelude::*;
use bevy_elements::{
    Climate, DustSettings, GeoLocation, RockSettings, SnowSettings, SurfaceCover, WeatherDirector, WeatherPreset, WeatheredExtension,
    WeatheredMaterial, WetnessSettings, WorldClock,
};
use serde::{Deserialize, Serialize};

use crate::family_host::FamilySettings;
use crate::terrain_view::TerrainView;

/// The crate this settings file belongs to, as `family.yaml` names it.
pub const CRATE: &str = "bevy_elements";

/// Game time a forced or released weather takes to blend, so the sky does not jump.
const BLEND: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElementsSettings {
    pub clock: WorldClock,
    #[serde(default)]
    pub location: GeoLocation,
    pub climate: Climate,
    /// `Some(Rain)` forces rain; `None` (the default) lets the climate's scheduler choose.
    #[serde(default)]
    pub forced_weather: Option<WeatherPreset>,
    /// How snow settles, melts and drifts (#100). The crate's own type; a file without it takes the crate's defaults.
    #[serde(default)]
    pub snow: SnowSettings,
    /// How the ground wets, dries and puddles.
    #[serde(default)]
    pub wetness: WetnessSettings,
    /// How many rocks lie on the ground (`talus_cover`, `meadow_cover`, `range_scale` 0 for none).
    #[serde(default)]
    pub rocks: RockSettings,
    /// Blown sand and dust.
    #[serde(default)]
    pub dust: DustSettings,
    /// The snow and wetness the carved ground wears whatever the weather is (#101): the crate's own
    /// `SurfaceCover`, put on the volume's material by [`dress_volume`]. `None` (the default) lets the
    /// scene's weather decide, as it does for a game's own weathered material.
    #[serde(default)]
    pub volume_cover: Option<SurfaceCover>,
}

impl ElementsSettings {
    pub fn from_yaml(text: &str) -> Result<Self, String> {
        bevy_grimoire_core::yaml::from_str(text, "family/bevy_elements.yaml").map_err(|p| format!("bevy_elements settings: {}", p.line()))
    }

    /// The settings a new map starts with: the crate's own defaults.
    pub fn defaults() -> Self {
        ElementsSettings {
            clock: WorldClock::default(),
            location: GeoLocation::default(),
            climate: Climate::default(),
            forced_weather: None,
            snow: SnowSettings::default(),
            wetness: WetnessSettings::default(),
            rocks: RockSettings::default(),
            dust: DustSettings::default(),
            volume_cover: None,
        }
    }

    pub fn to_yaml(&self) -> String {
        bevy_grimoire_core::yaml::to_string(self).expect("settings serialise")
    }
}

/// Puts `settings` into the running resources. `applied_force` is the forced preset this was last
/// called with, so an unchanged field is not forced again (which would restart the blend).
pub fn apply(
    settings: &ElementsSettings,
    clock: &mut WorldClock,
    location: &mut GeoLocation,
    climate: &mut Climate,
    director: &mut WeatherDirector,
    applied_force: &mut Option<Option<WeatherPreset>>,
) {
    // **The hour is walked, not jumped** (#271). `bevy_elements` adapts the camera's exposure to the
    // sky with a 1.5 s time constant (`EXPOSURE_TAU_S`), so a step from noon to 20:00 shows the night
    // scene at noon's exposure for about a second and a half — measured as 12 frames of a viewport
    // more than 45% white on big, 2026-11-02, which is why the clock demos could not be recorded.
    // The crate is right to smooth; the step is the editor's. When the settings differ in the hour
    // and nothing else, [`ramp_clock`] takes it there over a second, and the exposure follows a slope
    // instead of a cliff. Any other change is applied whole, as before: a date, a climate or a pause
    // is not a transition, and walking one would be a lie about what the file says.
    if same_except_hour(clock, &settings.clock) && clock.hour != settings.clock.hour {
        // A ramp is already in flight for this text if one was started; the hour it walks to is the
        // file's, so the walk simply continues and the text's own change is never re-applied.
        *clock = WorldClock { hour: clock.hour, ..settings.clock.clone() };
    } else {
        *clock = settings.clock.clone();
    }
    *location = settings.location.clone();
    *climate = settings.climate.clone();
    if *applied_force != Some(settings.forced_weather) {
        match settings.forced_weather {
            Some(preset) => director.force(preset, BLEND),
            None if director.is_forced() => director.release(BLEND),
            None => {}
        }
        *applied_force = Some(settings.forced_weather);
    }
}

/// Puts the ground-cover settings into the preview's resources, only where the plugin made one and the value differs.
pub fn apply_cover(settings: &ElementsSettings, preview: &mut Preview) {
    fn put<T: bevy::prelude::Resource<Mutability = bevy::ecs::component::Mutable> + Clone + PartialEq>(slot: &mut Option<ResMut<T>>, want: &T) {
        if let Some(r) = slot.as_mut()
            && **r != *want
        {
            **r = want.clone();
        }
    }
    put(&mut preview.snow, &settings.snow);
    put(&mut preview.wetness, &settings.wetness);
    put(&mut preview.rocks, &settings.rocks);
    put(&mut preview.dust, &settings.dust);
}

/// The preview's resources the settings file drives; absent until `bevy_elements`' plugins are added.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Preview<'w> {
    clock: Option<ResMut<'w, WorldClock>>,
    location: Option<ResMut<'w, GeoLocation>>,
    climate: Option<ResMut<'w, Climate>>,
    director: Option<ResMut<'w, WeatherDirector>>,
    snow: Option<ResMut<'w, SnowSettings>>,
    wetness: Option<ResMut<'w, WetnessSettings>>,
    rocks: Option<ResMut<'w, RockSettings>>,
    dust: Option<ResMut<'w, DustSettings>>,
}

/// Reads a reloaded `family/bevy_elements.yaml` into the preview's resources. A file that does not
/// fit the crate's types is a problem on the status line and the previous values stay.
pub fn apply_environment(
    settings: Res<FamilySettings>,
    mut state: ResMut<crate::editor::EditorState>,
    mut preview: Preview,
    mut ramp: Option<ResMut<ClockRamp>>,
    mut applied_force: Local<Option<Option<WeatherPreset>>>,
    mut seen: Local<Option<String>>,
) {
    let Some(text) = settings.text.get(CRATE) else {
        *seen = None;
        return;
    };
    if seen.as_ref() == Some(text) {
        return;
    }
    *seen = Some(text.clone());
    let parsed = match ElementsSettings::from_yaml(text) {
        Ok(p) => p,
        Err(e) => {
            state.status.problem(format!("{e}; the previous environment stays"));
            return;
        }
    };
    if let (Some(clock), Some(location), Some(climate), Some(director)) =
        (preview.clock.as_mut(), preview.location.as_mut(), preview.climate.as_mut(), preview.director.as_mut())
    {
        // **The ramp starts here**, where the file's hour and the clock's are both in hand. `apply`
        // then leaves the hour to the walk and writes every other field whole.
        if let Some(ramp) = ramp.as_mut()
            && same_except_hour(clock, &parsed.clock)
            && let Some(walk) = ClockRamp::between(clock.hour, parsed.clock.hour)
        {
            **ramp = walk;
        }
        apply(&parsed, clock, location, climate, director, &mut applied_force);
    }
    apply_cover(&parsed, &mut preview);
}

/// What [`dress_volume`] keeps between frames: the shared material, the settings text it last read,
/// and the cover the material wears (none, as `WeatheredExtension::default()` starts).
#[derive(Default)]
pub struct Dressing {
    handle: Option<Handle<WeatheredMaterial>>,
    seen: Option<String>,
    worn: Option<SurfaceCover>,
}

/// **The carved ground wears the game's weathered material** (#101). While the project has a
/// `family/bevy_elements.yaml` and `bevy_elements`' plugins are in the app, every chunk of the carved
/// ground ([`TerrainView::Volume`](crate::terrain_view::TerrainView::Volume)) is drawn with one
/// `WeatheredMaterial`, so the preview's snow and wetness are what the game draws. The file's
/// [`ElementsSettings::volume_cover`] is put on that material with `set_cover` when the file changes,
/// so an edit shows in the next frame without remeshing; a file that does not parse keeps the previous
/// cover (`apply_environment` reports it). Without the file the chunks go back to the plain material.
pub fn dress_volume(
    settings: Res<FamilySettings>,
    weathered: Option<ResMut<Assets<WeatheredMaterial>>>,
    mut plain_materials: ResMut<Assets<StandardMaterial>>,
    plain: Query<(Entity, &TerrainView), With<MeshMaterial3d<StandardMaterial>>>,
    dressed: Query<(Entity, &TerrainView), With<MeshMaterial3d<WeatheredMaterial>>>,
    mut commands: Commands,
    mut state: Local<Dressing>,
) {
    let Some(mut weathered) = weathered else { return };
    let Dressing { handle, seen, worn } = &mut *state;
    let Some(text) = settings.text.get(CRATE) else {
        let mut back = None;
        for (e, view) in &dressed {
            if *view == TerrainView::Volume {
                let back = back.get_or_insert_with(|| plain_materials.add(crate::terrain_view::volume_base())).clone();
                commands.entity(e).remove::<MeshMaterial3d<WeatheredMaterial>>().insert(MeshMaterial3d(back));
            }
        }
        *seen = None;
        return;
    };
    let handle = handle
        .get_or_insert_with(|| weathered.add(WeatheredMaterial { base: crate::terrain_view::volume_base(), extension: WeatheredExtension::default() }))
        .clone();
    if seen.as_ref() != Some(text) {
        *seen = Some(text.clone());
        // Only a changed cover touches the material: `get_mut` re-prepares it, and a clock edit should not.
        if let Ok(parsed) = ElementsSettings::from_yaml(text)
            && parsed.volume_cover != *worn
            && let Some(mut material) = weathered.get_mut(&handle)
        {
            material.extension.set_cover(parsed.volume_cover);
            *worn = parsed.volume_cover;
        }
    }
    for (e, view) in &plain {
        if *view == TerrainView::Volume {
            commands.entity(e).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(handle.clone()));
        }
    }
}

/// **Keep the weathered carved-ground pipeline compiled** (#268), as
/// [`crate::terrain_view::warm_volume_pipeline`] does the plain one: the first carve of a project with
/// a `family/bevy_elements.yaml` drew nothing for 0.5 s while `WeatheredMaterial`'s pipeline compiled.
/// Does nothing without `bevy_elements`' material store.
pub fn warm_weathered_pipeline(
    mut commands: Commands,
    weathered: Option<ResMut<Assets<WeatheredMaterial>>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(mut weathered) = weathered else { return };
    commands.spawn((
        crate::terrain_view::PipelineWarm,
        bevy::camera::visibility::NoFrustumCulling,
        Mesh3d(meshes.add(crate::terrain_view::warm_mesh())),
        MeshMaterial3d(weathered.add(WeatheredMaterial { base: crate::terrain_view::volume_base(), extension: WeatheredExtension::default() })),
        Transform::from_translation(crate::terrain_view::WARM_AT),
    ));
}

/// **The clock scrubber** (#98): camera-style state, not an operation. `Some(hour)` holds the
/// preview's clock at that hour of day without touching `family/bevy_elements.yaml`, so scrubbing
/// writes no file and leaves no undo step; `None` lets the file's clock run.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct ClockScrub(pub Option<f64>);

impl ClockScrub {
    /// Hold the clock at `hour`, wrapped into `[0, 24)`. A non-finite hour is ignored.
    pub fn set(&mut self, hour: f64) {
        if hour.is_finite() {
            self.0 = Some(hour.rem_euclid(24.0));
        }
    }

    /// Give the clock back to the settings file.
    pub fn release(&mut self) {
        self.0 = None;
    }
}

/// **A settings edit that changes only the hour, on its way there** (#271).
///
/// `bevy_elements` adapts the camera's exposure with a 1.5 s time constant (`EXPOSURE_TAU_S`), so a
/// clock that jumps from noon to 20:00 shows the night scene at noon's exposure for about a second and
/// a half: measured on big, 2026-11-02, as 12 frames with more than 45 % of the viewport white. The crate
/// smooths the right thing; the step was the editor's. This walks the hour there instead.
///
/// **Only the hour.** A date, a climate or a pause is not a transition — walking one would be a lie about
/// what the file says — and [`same_except_hour`] is the test that keeps this to the one edit it is for.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct ClockRamp {
    /// The hour the file asks for, in `[0, 24)`.
    pub to: f64,
    /// Hours still to walk, signed: positive forward through the day.
    pub left: f64,
}

impl ClockRamp {
    /// **Hours a settings edit walks over.** A quarter of the crate's exposure time constant would
    /// overshoot; twice it is long enough that the adaptation never sees a step, and short enough that
    /// a person does not wait for their own edit.
    pub const WALK: f64 = 3.0;

    /// The ramp an edit from `from` to `to` starts, or `None` when the two are the same hour.
    ///
    /// **Forward through the day, the short way.** Noon to 20:00 is eight hours forward; noon to 06:00 is
    /// six hours *back*, because a person turning the clock down expects the sun to set, not to leap.
    /// Choosing by the smaller arc would send noon to 06:00 forward through dusk and midnight, which is
    /// the opposite of what the button says.
    pub fn between(from: f64, to: f64) -> Option<ClockRamp> {
        if from == to || !from.is_finite() || !to.is_finite() {
            return None;
        }
        let mut d = (to - from).rem_euclid(24.0);
        if d > 12.0 {
            d -= 24.0;
        }
        Some(ClockRamp { to: to.rem_euclid(24.0), left: d })
    }

    /// The hour `clock` should be at after `dt` seconds, and whether the ramp is done.
    pub fn step(&mut self, clock: f64, dt: f32) -> (f64, bool) {
        if self.left == 0.0 {
            return (clock, true);
        }
        let per_second = Self::WALK / 1.0; // the walk takes a second
        let want = (per_second * f64::from(dt.max(0.0))).min(self.left.abs()) * self.left.signum();
        self.left -= want;
        let hour = (clock + want).rem_euclid(24.0);
        // **Land exactly on the file's hour.** Floating-point accumulation over a second of frames
        // would otherwise leave the clock a thousandth of an hour short, and the next reload would see
        // that as a difference and start another ramp.
        if self.left.abs() < 1e-9 || (hour - self.to).abs() < 1e-9 {
            self.left = 0.0;
            return (self.to, true);
        }
        (hour, false)
    }
}

/// **Whether two clocks differ only in the hour**, the one edit a ramp may smooth.
pub fn differs_only_in_hour(a: &WorldClock, b: &WorldClock) -> bool {
    same_except_hour(a, b)
}

fn same_except_hour(a: &WorldClock, b: &WorldClock) -> bool {
    a.date == b.date && a.time_scale == b.time_scale && a.paused == b.paused
}

/// **Walk a ramped hour to where the file says, every frame** (#271).
///
/// A ramp that is in flight owns the clock's hour; the file's own text is unchanged, so the settings
/// reader sees the same text it already applied and does not restart the walk.
pub fn walk_the_clock(ramp: Option<ResMut<ClockRamp>>, clock: Option<ResMut<WorldClock>>, time: Res<Time>) {
    let (Some(mut ramp), Some(mut clock)) = (ramp, clock) else {
        return;
    };
    if ramp.left == 0.0 {
        return;
    }
    let (hour, done) = ramp.step(clock.hour, time.delta_secs());
    clock.hour = hour;
    if done {
        ramp.left = 0.0;
    }
}

/// Holds the preview's clock at the scrubbed hour, every frame after the file was applied.
pub fn scrub_clock(scrub: Res<ClockScrub>, clock: Option<ResMut<WorldClock>>) {
    if let (Some(hour), Some(mut clock)) = (scrub.0, clock)
        && (clock.hour != hour || !clock.paused)
    {
        clock.hour = hour;
        clock.paused = true;
    }
}

/// **One button of the Environment panel** (#98). The clock buttons are camera-style state
/// ([`ClockScrub`]); the weather buttons are `family_set` operations, one undo step each.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum EnvAction {
    /// Hold the preview's clock at this hour of day.
    Scrub(f64),
    /// Let the settings file's clock run again.
    ReleaseClock,
    /// Force this weather preset (`forced_weather`).
    Force(WeatherPreset),
    /// Return the weather to the climate's own schedule.
    ReleaseWeather,
    /// Set the date's month (`clock.date.month`), the season.
    Month(u8),
    /// Replace the climate with one of the crate's own presets.
    Climate(ClimatePreset),
}

/// The crate's climate constructors the panel offers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClimatePreset {
    Temperate,
    Alpine,
    Desert,
    Tropical,
    Tundra,
}

impl ClimatePreset {
    pub const ALL: [(ClimatePreset, &'static str); 5] = [
        (ClimatePreset::Temperate, "Temperate"),
        (ClimatePreset::Alpine, "Alpine"),
        (ClimatePreset::Desert, "Desert"),
        (ClimatePreset::Tropical, "Tropical"),
        (ClimatePreset::Tundra, "Tundra"),
    ];

    pub fn climate(self) -> Climate {
        match self {
            ClimatePreset::Temperate => Climate::temperate(),
            ClimatePreset::Alpine => Climate::alpine(),
            ClimatePreset::Desert => Climate::desert(),
            ClimatePreset::Tropical => Climate::tropical(),
            ClimatePreset::Tundra => Climate::tundra(),
        }
    }
}

impl EnvAction {
    /// Hours the scrubber offers: dawn, noon, dusk and night.
    pub const HOURS: [(f64, &'static str); 4] = [(6.0, "6:00"), (12.0, "12:00"), (18.5, "18:30"), (23.0, "23:00")];

    /// Months the season row offers: the four solstice/equinox months.
    pub const SEASONS: [(u8, &'static str); 4] = [(3, "Spring"), (6, "Summer"), (9, "Autumn"), (12, "Winter")];

    /// The `family_set` params this action writes, or `None` for the camera-style clock buttons.
    pub fn family_params(self) -> Option<serde_json::Value> {
        let (field, value) = match self {
            EnvAction::Force(p) => ("forced_weather", format!("{p:?}")),
            EnvAction::ReleaseWeather => ("forced_weather", "null".to_owned()),
            EnvAction::Month(m) => ("clock.date.month", m.to_string()),
            EnvAction::Climate(c) => (
                "climate",
                bevy_grimoire_core::yaml::to_string(&c.climate()).expect("climate serialises"),
            ),
            EnvAction::Scrub(_) | EnvAction::ReleaseClock => return None,
        };
        Some(serde_json::json!({"crate": CRATE, "field": field, "value": value}))
    }
}

/// The ENVIRONMENT section of the controls panel.
pub fn spawn_section(s: &mut ChildSpawnerCommands) {
    use crate::chrome::{GAP_TIGHT_STEPS, Severity, button, section};
    section(s, "ENVIRONMENT");
    let row = |s: &mut ChildSpawnerCommands, items: &mut dyn FnMut(&mut ChildSpawnerCommands)| {
        crate::chrome::wrapping_row(s, GAP_TIGHT_STEPS, crate::chrome::NO_MARGIN, AlignItems::Stretch).with_children(|r| items(r));
    };
    row(s, &mut |r| {
        for (hour, label) in EnvAction::HOURS {
            button(r, EnvAction::Scrub(hour), label, Severity::Plain);
        }
        button(r, EnvAction::ReleaseClock, "Run", Severity::Plain);
    });
    row(s, &mut |r| {
        for (preset, label) in [(WeatherPreset::Clear, "Clear"), (WeatherPreset::Fog, "Fog"), (WeatherPreset::Rain, "Rain")] {
            button(r, EnvAction::Force(preset), label, Severity::Plain);
        }
        button(r, EnvAction::ReleaseWeather, "Release", Severity::Plain);
    });
    row(s, &mut |r| {
        for (month, label) in EnvAction::SEASONS {
            button(r, EnvAction::Month(month), label, Severity::Plain);
        }
    });
    row(s, &mut |r| {
        for (preset, label) in ClimatePreset::ALL {
            button(r, EnvAction::Climate(preset), label, Severity::Plain);
        }
    });
}

type WorldCamera = (Entity, Has<bevy_elements::ElementsCamera>, &'static mut Camera);

/// **The preview's look** (#98): a project with a `family/bevy_elements.yaml` gets `bevy_elements'`
/// atmosphere, fog, clouds and sun on the editor camera, which is what the game draws; a project
/// without one keeps the editor's own light. Follows the file's presence, so adding or deleting it
/// switches the preview without a restart.
///
/// **The surface is recomposed with it (#118).** `ElementsCamera` requires `Hdr`, and an HDR camera
/// renders into its own texture, not the one the ground and interface cameras share. Each camera then
/// writes its own result into the image (`Camera::output_mode`), and with the default
/// `blend_state: None` the later write replaces the earlier: the interface camera, which had drawn
/// onto the shared texture holding only the ground colour, wiped the world out of every frame. So
/// while dressed the interface camera clears that texture to transparent and both it and the world
/// are written over what is below them with premultiplied alpha.
///
/// **And the interface does not copy the ground back (#193, #194).** That clear was not honoured: the
/// interface shares the ground camera's texture, and with MSAA on `MsaaWriteback::Auto` copies what
/// the lower camera wrote (the opaque ground colour) into the interface's sampled texture instead of
/// clearing it, so the interface's write put the ground back over the world. While dressed the
/// interface's writeback is `Off`; with `Off` set live over BRP the viewport went from one colour to
/// the sky, the box and the grid.
pub fn dress_camera(
    settings: Res<FamilySettings>,
    mut camera: Query<WorldCamera, (With<crate::view::MainCamera>, Without<crate::surface::SurfaceCamera>)>,
    mut interface: Query<&mut Camera, With<crate::surface::SurfaceCamera>>,
    mut commands: Commands,
) {
    let wanted = settings.text.contains_key(CRATE);
    for (entity, has, mut world) in &mut camera {
        if wanted == has {
            continue;
        }
        if wanted {
            commands.entity(entity).insert(bevy_elements::ElementsCamera::default());
        } else {
            commands.entity(entity).remove::<bevy_elements::ElementsCamera>();
            // **`Hdr` is ours to take back** (#193 step 3). `ElementsCamera` arrives with
            // `#[require(Hdr)]`, so the crate cannot tell whether this camera had `Hdr` before it
            // dressed it — bevy_elements#55 says plainly that this is "left to the caller
            // (bevy_grimoire removes it itself)", and until now nothing here did. A camera left HDR
            // after the file goes is the state in which the viewport draws one flat colour for the
            // rest of the session: measured on big 2026-11-03, the viewport patch fell from
            // std 99.4 (the plain view) to std 1.7 (one colour), and stayed there.
            commands.entity(entity).remove::<bevy::camera::Hdr>();
        }
        composite_surface(wanted, &mut world, interface.iter_mut());
    }
}

/// **The editor's own key light steps aside while the camera wears the crate's sky** (#103).
///
/// `bevy_elements` meters the camera's exposure for the sky's physical light: a moonlit night is a gain of
/// about ten stops. The editor's key light is a fixed 6,000 lux, so at that gain it lit every piece to
/// white and the viewport at 22:00 was one white sheet (measured on big with `clock.hour = 22.0`). While
/// the camera is dressed the sun and the moon are the key light, so this one is hidden; it comes back when
/// the file goes.
pub fn key_light_follows_dressing(
    dressed: Query<(), (With<crate::view::MainCamera>, With<bevy_elements::ElementsCamera>)>,
    mut lights: Query<&mut Visibility, With<crate::view::EditorKeyLight>>,
) {
    let want = if dressed.is_empty() { Visibility::Inherited } else { Visibility::Hidden };
    for mut visibility in &mut lights {
        if *visibility != want {
            *visibility = want;
        }
    }
}

/// **Sets the surface's cameras to compose an HDR world (`dressed`) or the shared texture (not).**
pub fn composite_surface<'a>(
    dressed: bool,
    world: &mut Camera,
    interface: impl Iterator<Item = Mut<'a, Camera>>,
) {
    use bevy::camera::{CameraOutputMode, ClearColorConfig, MsaaWriteback};
    use bevy::render::render_resource::BlendState;
    let over = |dressed| {
        if dressed {
            CameraOutputMode::Write {
                blend_state: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            }
        } else {
            CameraOutputMode::default()
        }
    };
    let clear = if dressed { ClearColorConfig::Custom(Color::NONE) } else { ClearColorConfig::None };
    world.output_mode = over(dressed);
    world.clear_color = clear;
    for mut ui in interface {
        ui.output_mode = over(dressed);
        ui.clear_color = clear;
        ui.msaa_writeback = if dressed { MsaaWriteback::Off } else { MsaaWriteback::default() };
    }
}
