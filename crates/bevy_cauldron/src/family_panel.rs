//! **A family crate's own settings, as a panel** (UI-57, #243).
//!
//! `docs/ui/vision.md` §4 and §8 make the panels gooey widgets and the family crates the editor's data,
//! and this is the pair meeting: the settings of each crate the project lists, drawn from that crate's
//! own file and edited in place.
//!
//! # The file is the crate's, and there is no second schema
//!
//! A project's `family.yaml` lists the family crates it uses; each one's settings are its own serde type
//! written as RON in `<project>/family/<crate>.yaml` — the file `family_set` writes (`ops_family.rs`) and
//! a running game's copy of the plugin reads. [`ElementsSettings`] is the one settings type this build
//! holds, and it is the editor's thin wrapper around the crate's own types (`WorldClock`, `Climate`,
//! `GeoLocation`): the panel's fields are that file's fields, at the dot-paths `family_set` already takes
//! (`clock.hour`, `climate.mean_temperature_c`).
//!
//! # Why a view, and not the crate's type in an inspector
//!
//! `bevy_gooey::inspector` edits a **reflected component on an entity**. `ElementsSettings` is a plain
//! serde type here and is not `#[reflect(Resource)]`, and even if it were, a reflected write straight
//! onto it would be a second path to the file — one that skips `family_set`'s RON surgery, its refusal of
//! a value the crate's type will not read, its atomic write, its log line and its inverse. So
//! [`FamilyView`] is a **reflected view of the file's editable fields**, spawned on an entity of its own;
//! the inspector edits *it*, and [`an_edit_is_the_operation`] turns the change into the write. That is the
//! shape `inspector_view.rs` uses for the selection's pose, for the same reason: the editor's own type,
//! and one writer per fact.
//!
//! # The hour is camera-style state, and everything else is an operation
//!
//! **Every setting but one is `family_set`**: one dispatch, one undo step, one log line, the file on disk.
//!
//! **The hour is a scrub** ([`ClockScrub`]), which is what the Environment panel's four hour buttons have
//! done since #98: turning the clock to look at dusk is not an edit to the project, so it writes no file
//! and leaves no undo step. It is also the one field whose *file* edits are walked rather than jumped
//! (#271): `bevy_elements` adapts the camera's exposure to the sky with a 1.5 s time constant
//! (`EXPOSURE_TAU_S`), so a file edit of the hour ramps there over a second and the sky is not at the new
//! hour in the next frame. A panel field that dispatched `family_set` would inherit that ramp, and the
//! acceptance for this issue is that the sky *follows within one frame*.
//!
//! **So the two doors are two facts that already existed** — the project's clock in the file, and the
//! preview's held hour in [`ClockScrub`] — not two paths to one fact. The alternative (dispatch the file
//! write and also set the scrub) would write the project when a person meant to look, and a third
//! (an instant file write that skips the ramp) would undo #271 for every door onto the file.
//!
//! **And the held hour is the hour this panel shows**, not the file's: while a scrub holds one, that is
//! the hour the sky is drawn at, so a field showing the file's noon beside a midnight sky would be the
//! panel disagreeing with the thing it edits ([`FamilyView::at_hour`]). It also has to be, or the panel
//! undoes its own edit: the hour is written by a scrub, the file never changes, and a rebuild that read
//! the hour out of the file would put the file's hour back on the next frame.
//!
//! # Read when something changed, never once per frame
//!
//! The panel is derived from three facts — the open [`Project`] (which `family.yaml` names, and the root
//! its settings live under), [`FamilySettings`] (the text the watch last saw good), and the preview's
//! held [`ClockScrub`] for the hour — so it re-reads on a change in one of those and on nothing else.
//!
//! **The first cut did not.** It loaded `family.yaml`, read every `family/<crate>.yaml` and parsed its RON
//! every frame, and it read from disk even when [`FamilySettings`] already held the text: sixty file
//! reads a second to be told what the panel was already showing (`#243` review). A settled panel now
//! costs one comparison, and the disk is the *fallback* for the first frames before the watch has polled
//! and for a file the crate's own type refused.
//!
//! # The read-only block follows its file
//!
//! A crate this build has no fields for is drawn as its settings text, which is one `Text` — and the
//! first cut wrote it once, at spawn, so a `family_set` line left the block showing the settings it had
//! before the edit. It carries its crate ([`FamilyFile`]) and is rewritten when the text changes, which
//! is the same rule the fields follow, one widget over.
//!
//! # A crate this build has no type for
//!
//! It gets a block that shows its file as RON and no fields: the editor will not invent a schema for
//! another crate's settings, and it will not draw an empty panel as if the crate had none. That is the
//! two-way street's own answer — a settings type that wants fields here needs `Reflect` in its own crate,
//! filed there (`consumer:bevy_grimoire`) rather than worked around here.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy::prelude::*;
use bevy::reflect::Reflect;

use crate::editor::{EditorState, MapControlsPane};
use crate::environment::{ClockScrub, ElementsSettings};
use crate::family_host::FamilySettings;
use crate::project::Project;

/// The crate whose settings this panel can build fields from: the one settings type this build holds.
pub const CRATE: &str = crate::environment::CRATE;

/// **The fields of [`FamilyView`] that are written to the crate's settings file**, as
/// `(the view's field, the dot-path `family_set` takes)`.
///
/// `hour` is deliberately absent: it is the one field that is camera-style state, handled by
/// [`an_edit_is_the_operation`] through [`ClockScrub`] (see the module note).
pub const FILE_FIELDS: &[(&str, &str)] = &[
    ("time_scale", "clock.time_scale"),
    ("paused", "clock.paused"),
    ("month", "clock.date.month"),
    ("day", "clock.date.day"),
    ("weather_year", "clock.date.year"),
    ("latitude_deg", "location.latitude_deg"),
    ("longitude_deg", "location.longitude_deg"),
    ("utc_offset_hours", "location.utc_offset_hours"),
    ("mean_temperature_c", "climate.mean_temperature_c"),
    ("annual_amplitude_c", "climate.annual_amplitude_c"),
    ("mean_humidity", "climate.mean_humidity"),
    ("mean_wind_ms", "climate.mean_wind_ms"),
];

/// **Which family crates this project draws a block for**, and the settings text the last read saw.
///
/// A crate is offered when the project's `family.yaml` lists it, this build was compiled with it, and its
/// settings file is on disk. The list is what a person sees as blocks, and a test can read it without
/// hunting the screen.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct FamilyPanel {
    /// The crates with a block, in `family.yaml`'s order.
    crates: Vec<String>,
    /// The crates whose fields this build can draw; the others show their file as RON.
    fields: Vec<String>,
    /// The project root the list was read for, so another project re-reads.
    root: Option<PathBuf>,
    /// Each crate's settings text as last read, so a settled panel costs one comparison.
    text: BTreeMap<String, String>,
    /// **How many times this panel has gone to its files**: one per frame in which one of its inputs
    /// moved, and none in the frames between.
    ///
    /// A count of reads rather than of frames, on purpose. The first cut read `family.yaml` and every
    /// `family/<crate>.yaml` sixty times a second to be told what it already showed (`#243` review), and
    /// a number that only goes up on a read is the cheapest thing that can hold it to *"a settled panel
    /// reads nothing"* — `a_settled_panel_reads_nothing` is the test.
    reads: u64,
}

impl FamilyPanel {
    /// The crates the panel draws, in the project's own order.
    pub fn crates(&self) -> impl Iterator<Item = &str> {
        self.crates.iter().map(String::as_str)
    }

    /// Whether the panel draws `krate`'s fields (rather than its file as RON).
    pub fn has_fields(&self, krate: &str) -> bool {
        self.fields.iter().any(|c| c == krate)
    }

    /// The settings text the panel last read for `krate`, if it draws that crate.
    pub fn text_of(&self, krate: &str) -> Option<&str> {
        self.text.get(krate).map(String::as_str)
    }

    /// How many times the panel has read its files (see the field's note).
    pub fn reads(&self) -> u64 {
        self.reads
    }
}

/// **`bevy_elements`' settings as the reflect inspector edits them.**
///
/// One entity carries it, spawned with [`FamilyPanelPlugin`]. It is written from the settings file
/// whenever the file changes and read *back* only when gooey reports an edit — so these fields are the
/// inspector's state, never the editor's truth (the file is, and `family_set` is what writes it).
#[derive(Component, Reflect, Debug, Clone, PartialEq)]
#[reflect(Component)]
pub struct FamilyView {
    /// The crate this view is of, as `family.yaml` names it.
    pub krate: String,
    /// Time of day, hours in `[0, 24)`. The one field that is a scrub, not an operation.
    #[reflect(@HOURS)]
    #[reflect(@bevy_gooey::InspectStep(STEP_HOURS))]
    pub hour: f64,
    /// Game seconds per real second.
    pub time_scale: f32,
    /// Whether the world clock stands still.
    pub paused: bool,
    /// Month of the file's date, 1–12.
    #[reflect(@MONTHS)]
    pub month: u8,
    /// Day of the month, 1–31.
    #[reflect(@DAYS)]
    pub day: u8,
    /// Year of the date.
    pub weather_year: i32,
    /// The observer's latitude, degrees north.
    #[reflect(@LATITUDE)]
    #[reflect(@bevy_gooey::InspectStep(STEP_DEGREES))]
    pub latitude_deg: f64,
    /// The observer's longitude, degrees east.
    #[reflect(@LONGITUDE)]
    #[reflect(@bevy_gooey::InspectStep(STEP_DEGREES))]
    pub longitude_deg: f64,
    /// Local civil time minus UTC, hours.
    #[reflect(@UTC_OFFSET)]
    #[reflect(@bevy_gooey::InspectStep(STEP_DEGREES))]
    pub utc_offset_hours: f64,
    /// The climate's annual mean air temperature, °C.
    #[reflect(@bevy_gooey::InspectStep(STEP_CLIMATE))]
    pub mean_temperature_c: f32,
    /// Half the warmest-to-coldest swing of daily means over a year, °C.
    #[reflect(@bevy_gooey::InspectStep(STEP_CLIMATE))]
    pub annual_amplitude_c: f32,
    /// Mean relative humidity, 0–1.
    #[reflect(@HUMIDITY)]
    #[reflect(@bevy_gooey::InspectStep(STEP_HUMIDITY))]
    pub mean_humidity: f32,
    /// Mean wind speed at 10 m, m/s.
    #[reflect(@bevy_gooey::InspectStep(STEP_CLIMATE))]
    pub mean_wind_ms: f32,
}

impl FamilyView {
    /// The view of `krate`'s settings, or `None` for a crate this build has no type to draw fields from.
    pub fn of(krate: &str, settings: &ElementsSettings) -> Option<FamilyView> {
        if krate != CRATE {
            return None;
        }
        Some(FamilyView {
            krate: krate.to_owned(),
            hour: settings.clock.hour.rem_euclid(24.0),
            time_scale: settings.clock.time_scale,
            paused: settings.clock.paused,
            month: settings.clock.date.month,
            day: settings.clock.date.day,
            weather_year: settings.clock.date.year,
            latitude_deg: settings.location.latitude_deg,
            longitude_deg: settings.location.longitude_deg,
            utc_offset_hours: settings.location.utc_offset_hours,
            mean_temperature_c: settings.climate.mean_temperature_c,
            annual_amplitude_c: settings.climate.annual_amplitude_c,
            mean_humidity: settings.climate.mean_humidity,
            mean_wind_ms: settings.climate.mean_wind_ms,
        })
    }

    /// **This view with the hour a [`ClockScrub`] holds**, or unchanged when nothing is held.
    ///
    /// The panel's hour is the preview's, not the file's, for as long as a scrub holds one — see the
    /// module note. A held hour that is not finite is not an hour, so it is left to the file.
    #[must_use]
    pub fn at_hour(mut self, held: Option<f64>) -> FamilyView {
        if let Some(hour) = held.filter(|h| h.is_finite()) {
            self.hour = hour.rem_euclid(24.0);
        }
        self
    }

    /// **The RON value `family_set` is given for a field of this view**, kept inside what the crate's own
    /// type accepts: a typed month of 0 or 40 lands on January or December rather than in the file.
    ///
    /// A float is written with its point (`0.0`, never `0`), so the file keeps the field's type.
    pub fn ron_value(&self, field: &str) -> Option<String> {
        /// A whole number kept inside its range.
        fn inside(v: f64, least: f64, most: f64) -> String {
            format!("{}", v.round().clamp(least, most) as i64)
        }
        Some(match field {
            "time_scale" => format!("{:?}", self.time_scale),
            "paused" => self.paused.to_string(),
            "month" => inside(f64::from(self.month), *MONTHS.start(), *MONTHS.end()),
            "day" => inside(f64::from(self.day), *DAYS.start(), *DAYS.end()),
            "weather_year" => self.weather_year.to_string(),
            "latitude_deg" => format!("{:?}", self.latitude_deg),
            "longitude_deg" => format!("{:?}", self.longitude_deg),
            "utc_offset_hours" => format!("{:?}", self.utc_offset_hours),
            "mean_temperature_c" => format!("{:?}", self.mean_temperature_c),
            "annual_amplitude_c" => format!("{:?}", self.annual_amplitude_c),
            "mean_humidity" => format!("{:?}", self.mean_humidity),
            "mean_wind_ms" => format!("{:?}", self.mean_wind_ms),
            _ => return None,
        })
    }
}

/// The months a year has.
const MONTHS: std::ops::RangeInclusive<f64> = 1.0..=12.0;
/// The days a month may have.
const DAYS: std::ops::RangeInclusive<f64> = 1.0..=31.0;

/// **The ranges the inspector keeps a person inside**, as the numbers they are, so the reflected
/// attributes below and [`FamilyView::ron_value`] cannot disagree about them.
///
/// A `#[reflect(@…)]` attribute is an expression in the crate's own source, so a constant is as usable
/// there as a literal — and it is the only form that keeps the *range a person is shown* and the *range
/// the file is written inside* one fact.
const HOURS: std::ops::RangeInclusive<f64> = 0.0..=24.0;
/// Degrees of latitude: the poles.
const LATITUDE: std::ops::RangeInclusive<f64> = -90.0..=90.0;
/// Degrees of longitude: the antimeridian.
const LONGITUDE: std::ops::RangeInclusive<f64> = -180.0..=180.0;
/// Hours of civil time offset from UTC, as a place on Earth can have (`+14:00` at Kiritimati).
const UTC_OFFSET: std::ops::RangeInclusive<f64> = -12.0..=14.0;
/// A relative humidity, as a fraction of saturation.
const HUMIDITY: std::ops::RangeInclusive<f32> = 0.0..=1.0;

/// **How far one press of an arrow moves a field**, in the field's own unit.
///
/// A quarter hour is a clock step a person recognises; half a degree of place is finer than anyone sets
/// a location to; a hundredth is the resolution humidity is worth; and half a degree of temperature, wind
/// or swing is the finest a setting of this kind is meaningful at. Named, because the reflected attribute
/// and the doc comment above each field are the same number and drifted apart once already.
const STEP_HOURS: f32 = 0.25;
/// See [`STEP_HOURS`]: degrees of place.
const STEP_DEGREES: f32 = 0.5;
/// See [`STEP_HOURS`]: a fraction of saturation.
const STEP_HUMIDITY: f32 = 0.01;
/// See [`STEP_HOURS`]: degrees Celsius, and metres per second.
const STEP_CLIMATE: f32 = 0.5;

/// The entity each crate's view is on, so a system can find it without a second resource.
#[derive(Resource, Default)]
pub struct FamilyViews(pub BTreeMap<String, Entity>);

/// **The block in the INSPECTOR panel**: a heading and, under it, the crate's fields or its file as RON.
#[derive(Component)]
pub struct FamilyBlock {
    /// The crate the block draws.
    pub krate: String,
}

/// **gooey's reflect inspector over a view.** An edit made in it arrives as an `InspectorChange` on this
/// entity.
#[derive(Component)]
pub struct FamilyWidget;

/// **The settings file of a crate this build has no fields for, as it is shown**: one `Text` under that
/// crate's heading, carrying the crate whose text it is so [`mirror_the_blocks`] can keep it current.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct FamilyFile {
    /// The crate the text is read from.
    pub krate: String,
}

pub struct FamilyPanelPlugin;

impl Plugin for FamilyPanelPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<FamilyView>()
            .init_resource::<FamilyPanel>()
            .init_resource::<FamilyViews>()
            .add_systems(Update, (read_the_files, mirror_the_blocks).chain())
            .add_observer(an_edit_is_the_operation);
    }
}

/// **`family.yaml`'s size and modification time.**
///
/// **A `stat` a frame, not a parse a frame** — the shape `project_config::refresh` uses for this same
/// file. A crate listed or taken out is a write to `family.yaml`, and the project configuration's
/// switches write it without touching [`Project`], so the panel has to watch the file itself or a crate
/// switched on would have no block until the next restart.
type Stamp = (std::time::SystemTime, u64);

/// **What the panel last saw of `family.yaml`.**
///
/// The inner `Option` is the file's own answer (`None` for a project that lists no crate at all, which is
/// an ordinary project rather than an error); the outer one is *"the panel has not looked yet"*, which has
/// to be a read even when the file is absent, or a project without the file would be read every frame.
type SeenListed = Option<Option<Stamp>>;

fn listed_stamp(root: &std::path::Path) -> Option<Stamp> {
    let meta = std::fs::metadata(bevy_grimoire_core::family::path_for_project(root)).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// **Which crates get a block, and what their views show**, read from the project's files.
///
/// # Read when an input moved, and never otherwise
///
/// The panel is derived from four facts: the [`Project`] (which `family.yaml` names, and the root the
/// settings live under), `family.yaml` itself (the switches write it without touching the project),
/// [`FamilySettings`] (the text the watch last took as good), and the preview's held [`ClockScrub`]
/// (the hour). A frame in which none of them moved returns here on one `stat`, two change ticks and a
/// look at the view entities — the first cut loaded `family.yaml`, read every `family/<crate>.yaml` and
/// parsed its RON on **every** frame, sixty times a second, to be told what the panel already showed.
///
/// **The view entities are part of the gate** because a door change sweeps them without touching these
/// resources: a slot naming an entity the world no longer has is exactly the frame the panel must read
/// again, or the second door would draw no block at all.
///
/// # The text, and where it comes from
///
/// [`FamilySettings`] first — that is the watch's job, so the panel and the preview read one text — and
/// the file **only when the host holds nothing for that crate** (`or_else`, which reads the disk lazily;
/// the first cut was `or`, which read every file every frame whether the host had the text or not). The
/// fallback is the first frames before the watch has polled, and a file the crate's own type refused at
/// launch — and a refused file stays refused, so the last text the crate would read is what is shown,
/// which is what a game would read too.
///
/// # The hour is the preview's, not the file's
///
/// While a [`ClockScrub`] holds an hour that is the hour the sky is drawn at, so it is the hour this
/// field shows ([`FamilyView::at_hour`]) — and it has to be, or the panel would undo its own edit one
/// frame later by rebuilding the view from a file the hour never reaches.
#[allow(clippy::too_many_arguments)]
fn read_the_files(
    project: Option<Res<Project>>,
    held: Option<Res<FamilySettings>>,
    scrub: Option<Res<ClockScrub>>,
    mut panel: ResMut<FamilyPanel>,
    mut views: Query<&mut FamilyView>,
    mut slots: ResMut<FamilyViews>,
    mut commands: Commands,
    mut seen: Local<SeenListed>,
) {
    let Some(project) = project else {
        if !panel.crates.is_empty() {
            *panel = FamilyPanel::default();
            slots.0.clear();
        }
        return;
    };
    let stamp = listed_stamp(&project.root);
    let moved = seen.as_ref() != Some(&stamp)
        || panel.root.as_deref() != Some(project.root.as_path())
        || held.as_ref().is_some_and(|h| h.is_changed())
        || scrub.as_ref().is_some_and(|s| s.is_changed())
        || slots.0.values().any(|entity| views.get(*entity).is_err());
    if !moved {
        return;
    }
    *seen = Some(stamp);
    panel.reads = panel.reads.saturating_add(1);

    let built = crate::family_host::built();
    let listed = bevy_grimoire_core::family::Family::load(&project.root)
        .map(|f| f.active(&built).into_iter().map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_default();
    let settings_dir = bevy_grimoire_core::family::settings_dir(&project.root);
    let hour = scrub.as_ref().and_then(|s| s.0);
    let (mut crates, mut fields, mut text) = (Vec::new(), Vec::new(), BTreeMap::new());
    let mut parsed = Vec::new();
    for krate in listed {
        let from_host = held.as_ref().and_then(|h| h.text.get(&krate).cloned());
        let Some(settings_text) = from_host.or_else(|| std::fs::read_to_string(settings_dir.join(format!("{krate}.yaml"))).ok()) else {
            continue;
        };
        if krate == CRATE
            && let Ok(settings) = ElementsSettings::from_yaml(&settings_text)
        {
            parsed.push((krate.clone(), settings));
            fields.push(krate.clone());
        }
        crates.push(krate.clone());
        text.insert(krate, settings_text);
    }
    // A view exists per crate this build can draw fields of; the others are listed, so a person sees the
    // crate is there and can read its file.
    for (krate, settings) in parsed {
        let want = FamilyView::of(&krate, &settings).map(|view| view.at_hour(hour));
        match (slots.0.get(&krate).copied(), want) {
            (Some(entity), Some(want)) => {
                if let Ok(mut view) = views.get_mut(entity)
                    && *view != want
                {
                    *view = want;
                }
            }
            (None, Some(view)) => {
                let entity = commands.spawn((Name::new(format!("FamilyView {krate}")), view)).id();
                slots.0.insert(krate, entity);
            }
            (Some(entity), None) => {
                commands.entity(entity).despawn();
                slots.0.remove(&krate);
            }
            (None, None) => {}
        }
    }
    // A crate that left the project takes its view with it.
    for krate in slots.0.keys().filter(|k| !fields.contains(k)).cloned().collect::<Vec<_>>() {
        if let Some(entity) = slots.0.remove(&krate) {
            commands.entity(entity).despawn();
        }
    }
    if panel.root.as_deref() == Some(project.root.as_path()) && panel.crates == crates && panel.text == text {
        return;
    }
    panel.root = Some(project.root.clone());
    panel.crates = crates;
    panel.fields = fields;
    panel.text = text;
}

/// **A block per crate, in the Map door's INSPECTOR panel**, and out again when the crate goes.
///
/// Spawned on demand and never kept hidden: a gooey inspector is a set of entities, and its number rows
/// carry `TextHint`, which is how this editor's screens find "the one search box" (`outliner.rs` says the
/// same of its tree and its search field).
///
/// **A block whose crate has no fields shows the settings text, and is rewritten when that text moves.**
/// It is one `Text` ([`FamilyFile`]) rather than a spawn-and-despawn, because the text under a heading
/// that stays put is what a person is watching: the first cut wrote it once at spawn, so a `family_set`
/// line left the block showing the settings from before the edit. The write is compared first, which is
/// `no_system_writes_every_frame.rs`'s rule and the reason a settled panel costs nothing.
fn mirror_the_blocks(
    panel: Res<FamilyPanel>,
    slots: Res<FamilyViews>,
    panes: Query<Entity, With<MapControlsPane>>,
    blocks: Query<(Entity, &FamilyBlock)>,
    files: Query<(Entity, &FamilyFile)>,
    mut texts: Query<&mut Text>,
    mut commands: Commands,
) {
    let Ok(pane) = panes.single() else { return };
    // The crates the panel draws, in its own order: one with a view always, one without when its file is
    // there to read.
    let wanted: Vec<(&str, Option<Entity>)> = panel
        .crates()
        .filter(|k| slots.0.contains_key(*k) || !panel.has_fields(k))
        .map(|k| (k, slots.0.get(k).copied()))
        .collect();
    for (block, drawn) in &blocks {
        if !wanted.iter().any(|(krate, _)| *krate == drawn.krate) {
            commands.entity(block).despawn();
        }
    }
    // **The text of a crate with no fields follows its file.** Its block stays where it is; only the
    // settings under the heading change. Compared before written, so a settled panel writes nothing.
    for (entity, file) in &files {
        let Some(want) = panel.text_of(&file.krate) else { continue };
        if let Ok(mut text) = texts.get_mut(entity)
            && text.0 != want
        {
            text.0 = want.to_owned();
        }
    }
    for (krate, view) in &wanted {
        let name = (*krate).to_owned();
        if blocks.iter().any(|(_, b)| b.krate == *krate) {
            continue;
        }
        let ron = if view.is_none() { panel.text_of(krate).map(str::to_owned) } else { None };
        let block = commands
            .spawn((
                FamilyBlock { krate: name.clone() },
                Name::new(format!("family block {name}")),
                bevy_gooey::column(crate::chrome::GAP_TIGHT_STEPS),
            ))
            .with_children(|block| {
                crate::chrome::list_heading(block, &name.to_uppercase());
                match view {
                    Some(view) => {
                        block.spawn((
                            FamilyWidget,
                            Name::new(format!("family inspector {name}")),
                            bevy_gooey::inspector(bevy_gooey::InspectTarget::component::<FamilyView>(*view)),
                        ));
                    }
                    // **A crate whose settings type is not this build's is read, not guessed at.** The text
                    // is its own file, so what a person sees is what a game will read.
                    None => {
                        block.spawn((
                            FamilyFile { krate: name.clone() },
                            Name::new(format!("family file {name}")),
                            bevy_gooey::label(ron.unwrap_or_default()),
                        ));
                    }
                }
            })
            .id();
        // At the end of the panel, not above what is selected: the pose inspector and the outliner are
        // about the piece in hand, which is what a person looked at first.
        commands.entity(pane).add_child(block);
    }
}

/// **An edit of a view is the write that owns that field**, exactly as the typed `family_set` line is.
///
/// The hour is the one field that is camera-style state ([`ClockScrub`]); every other field is
/// `family_set`, whose outcome is one undo step and one log line. The settings text the write produced is
/// put into [`FamilySettings`] at once, so the preview is drawn at the new value in this frame rather than
/// waiting out the watch's poll — the same text the watch would have delivered, only sooner.
#[allow(clippy::too_many_arguments)]
fn an_edit_is_the_operation(
    change: On<bevy_gooey::InspectorChange>,
    views: Res<FamilyViews>,
    inspectors: Query<&bevy_gooey::Inspector>,
    drawn: Query<&FamilyView>,
    project: Option<Res<Project>>,
    mut settings: Option<ResMut<FamilySettings>>,
    mut scrub: Option<ResMut<ClockScrub>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let Some(state) = state.as_deref_mut() else { return };
    // **The change names the view, or the inspector over it.** gooey triggers it on the inspector's own
    // root, whose `target` is the component it edits, so a widget answers with the view it draws.
    let view_entity = if views.0.values().any(|e| *e == change.entity) {
        Some(change.entity)
    } else {
        match inspectors.get(change.entity).map(|i| i.target) {
            Ok(bevy_gooey::InspectTarget::Component { entity, .. }) => Some(entity),
            _ => None,
        }
    };
    let Some(view) = view_entity.and_then(|e| drawn.get(e).ok()) else { return };
    let path = change.path.as_str();
    if path == "hour" {
        if let Some(scrub) = scrub.as_mut() {
            scrub.set(view.hour);
            state.status.note(format!("the preview is at {:.2}:00", view.hour));
        }
        return;
    }
    let Some((field, family_path)) = FILE_FIELDS.iter().find(|(field, _)| *field == path) else {
        state.status.problem(format!("`{path}` is not a setting this panel writes, so nothing was written."));
        return;
    };
    let Some(value) = view.ron_value(field) else {
        state.status.problem(format!("`{path}` has no value to write, so nothing was written."));
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: project.as_deref().cloned(),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    let krate = view.krate.clone();
    let params = serde_json::json!({ "crate": krate, "field": family_path, "value": value });
    match crate::ops::dispatch(&mut ctx, "family_set", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record_logged(operation_id, outcome.summary, false);
            // **The preview is drawn at the new value this frame.** The file is the truth and the watch
            // delivers it within a tenth of a second; the panel does not make a person wait for that.
            if let Some(settings) = settings.as_mut()
                && let Some(root) = ctx.project.as_ref().map(|p| p.root.clone())
                && let Ok(text) = std::fs::read_to_string(bevy_grimoire_core::family::settings_dir(&root).join(format!("{krate}.yaml")))
            {
                settings.text.insert(krate, text);
            }
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A month or a day a person typed outside the calendar is kept inside it, a float keeps its point,
    /// and the hour is never a written field.
    #[test]
    fn a_typed_month_is_kept_inside_the_year() {
        let mut view = FamilyView::of(CRATE, &ElementsSettings::defaults()).expect("the crate this build holds");
        view.month = 0;
        assert_eq!(view.ron_value("month").as_deref(), Some("1"));
        view.month = 40;
        assert_eq!(view.ron_value("month").as_deref(), Some("12"));
        view.day = 0;
        assert_eq!(view.ron_value("day").as_deref(), Some("1"));
        view.time_scale = 60.0;
        assert_eq!(view.ron_value("time_scale").as_deref(), Some("60.0"), "the file keeps the field's type");
        assert_eq!(view.ron_value("hour"), None, "the hour is a scrub, never a written field");
        assert_eq!(*MONTHS.start(), 1.0);
        assert_eq!(*DAYS.end(), 31.0);
    }

    /// A crate with no settings type in this build has no view, which is what sends its block to the RON
    /// text instead of an empty panel.
    #[test]
    fn a_crate_this_build_has_no_type_for_has_no_view() {
        assert!(FamilyView::of("bevy_siren", &ElementsSettings::defaults()).is_none());
        assert!(FamilyView::of(CRATE, &ElementsSettings::defaults()).is_some());
    }
}
