//! **A map's environment is saved in `bevy_elements`' own types and drives the preview** (#98).
#![cfg(feature = "bevy_elements")]

use bevy_elements::{Climate, GeoLocation, WeatherDirector, WeatherPreset, WorldClock};
use bevy_cauldron::environment::{apply, ElementsSettings};
use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

fn project(tag: &str) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_elements\"])").unwrap();
    let file = root.join("family/bevy_elements.ron");
    std::fs::write(&file, ElementsSettings::defaults().to_ron()).unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    (Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() }, file)
}

fn set(ctx: &mut Ctx, field: &str, value: &str) {
    let r = ops::dispatch(ctx, "family_set", &json!({"crate": "bevy_elements", "field": field, "value": value}));
    assert!(matches!(r, Ok(Applied::Done { .. })), "{field} = {value}");
}

#[test]
fn a_chosen_date_and_climate_are_in_the_file_and_read_back_after_a_restart() {
    let (mut ctx, file) = project("restart");
    set(&mut ctx, "clock.date.month", "12");
    set(&mut ctx, "clock.hour", "18.5");
    set(&mut ctx, "climate.mean_temperature_c", "-4.0");
    // "Restart": nothing but the file is read.
    let s = ElementsSettings::from_ron(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(s.clock.date.month, 12);
    assert_eq!(s.clock.hour, 18.5);
    assert_eq!(s.climate.mean_temperature_c, -4.0);
    ops::undo(&mut ctx, None).unwrap();
    let s = ElementsSettings::from_ron(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(s.climate.mean_temperature_c, Climate::default().mean_temperature_c, "one undo per change");
    assert_eq!(s.clock.hour, 18.5);
}

#[test]
fn forcing_rain_holds_until_it_is_released_back_to_the_climate() {
    let (mut ctx, file) = project("rain");
    let (mut clock, mut loc, mut climate, mut director) =
        (WorldClock::default(), GeoLocation::default(), Climate::default(), WeatherDirector::default());
    let mut applied = None;
    let read = |file: &std::path::Path| ElementsSettings::from_ron(&std::fs::read_to_string(file).unwrap()).unwrap();

    set(&mut ctx, "forced_weather", "Some(Rain)");
    apply(&read(&file), &mut clock, &mut loc, &mut climate, &mut director, &mut applied);
    assert!(director.is_forced());
    assert_eq!(director.current(), WeatherPreset::Rain);

    // The same file again does not restart the blend.
    let before = director.clone();
    apply(&read(&file), &mut clock, &mut loc, &mut climate, &mut director, &mut applied);
    assert_eq!(director, before);

    set(&mut ctx, "forced_weather", "None");
    apply(&read(&file), &mut clock, &mut loc, &mut climate, &mut director, &mut applied);
    assert!(!director.is_forced(), "None releases the weather to the climate's schedule");
}

#[test]
fn scrubbing_the_clock_moves_the_preview_clock_and_writes_nothing() {
    use bevy::prelude::*;
    use bevy_cauldron::environment::{scrub_clock, ClockScrub};
    let (mut ctx, file) = project("scrub");
    let before = std::fs::read_to_string(&file).unwrap();
    let mut app = App::new();
    app.init_resource::<ClockScrub>().insert_resource(WorldClock::default()).add_systems(Update, scrub_clock);
    app.update();
    let untouched = app.world().resource::<WorldClock>().hour;

    app.world_mut().resource_mut::<ClockScrub>().set(18.5);
    app.update();
    assert_eq!(app.world().resource::<WorldClock>().hour, 18.5);
    app.world_mut().resource_mut::<ClockScrub>().set(-1.0);
    app.update();
    assert_eq!(app.world().resource::<WorldClock>().hour, 23.0, "wraps into the day");

    assert_eq!(std::fs::read_to_string(&file).unwrap(), before, "scrubbing writes no file");
    assert!(bevy_cauldron::ops::undo(&mut ctx, None).is_err(), "scrubbing is not an undo step");

    app.world_mut().resource_mut::<ClockScrub>().release();
    app.world_mut().resource_mut::<WorldClock>().hour = untouched;
    app.update();
    assert_eq!(app.world().resource::<WorldClock>().hour, untouched, "released: the file's clock runs");
}

#[test]
fn the_panels_weather_buttons_write_the_forced_weather_and_the_clock_buttons_write_nothing() {
    use bevy_cauldron::environment::EnvAction;
    let (mut ctx, file) = project("buttons");
    let read = |file: &std::path::Path| ElementsSettings::from_ron(&std::fs::read_to_string(file).unwrap()).unwrap();
    let dispatch = |ctx: &mut Ctx, a: EnvAction| {
        let r = ops::dispatch(ctx, "family_set", &a.family_params().unwrap());
        assert!(matches!(r, Ok(Applied::Done { .. })), "{a:?}");
    };
    for preset in [WeatherPreset::Rain, WeatherPreset::Fog, WeatherPreset::Clear] {
        dispatch(&mut ctx, EnvAction::Force(preset));
        assert_eq!(read(&file).forced_weather, Some(preset));
    }
    dispatch(&mut ctx, EnvAction::ReleaseWeather);
    assert_eq!(read(&file).forced_weather, None);
    assert!(EnvAction::Scrub(6.0).family_params().is_none() && EnvAction::ReleaseClock.family_params().is_none());
}

#[test]
fn the_panels_season_and_climate_buttons_are_one_undo_step_each() {
    use bevy_cauldron::environment::{ClimatePreset, EnvAction};
    let (mut ctx, file) = project("season_climate");
    let read = |file: &std::path::Path| ElementsSettings::from_ron(&std::fs::read_to_string(file).unwrap()).unwrap();
    let dispatch = |ctx: &mut Ctx, a: EnvAction| {
        let r = ops::dispatch(ctx, "family_set", &a.family_params().unwrap());
        assert!(matches!(r, Ok(Applied::Done { .. })), "{a:?}: {r:?}");
    };
    for (month, _) in EnvAction::SEASONS {
        dispatch(&mut ctx, EnvAction::Month(month));
        assert_eq!(read(&file).clock.date.month, month);
    }
    for (preset, _) in ClimatePreset::ALL {
        dispatch(&mut ctx, EnvAction::Climate(preset));
        assert_eq!(ron::to_string(&read(&file).climate).unwrap(), ron::to_string(&preset.climate()).unwrap());
    }
    ops::undo(&mut ctx, None).unwrap();
    let now = ron::to_string(&read(&file).climate).unwrap();
    assert_eq!(now, ron::to_string(&ClimatePreset::Tropical.climate()).unwrap(), "one undo steps back one climate");
}

#[test]
fn snow_rocks_and_sand_are_set_in_the_crates_own_types_and_an_old_file_still_loads() {
    let (mut ctx, file) = project("cover");
    set(&mut ctx, "snow.drift", "false");
    set(&mut ctx, "rocks.talus_cover", "0.5");
    set(&mut ctx, "wetness.wet_per_mm", "0.9");
    let s = ElementsSettings::from_ron(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert!(!s.snow.drift);
    assert_eq!((s.rocks.talus_cover, s.wetness.wet_per_mm), (0.5, 0.9));
    ops::undo(&mut ctx, None).unwrap();
    let s = ElementsSettings::from_ron(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(s.wetness.wet_per_mm, bevy_elements::WetnessSettings::default().wet_per_mm, "one undo per change");
    // A file written before these fields existed takes the crate's defaults.
    let old = ElementsSettings::defaults().to_ron();
    let start = old.find("snow:").unwrap();
    let trimmed = format!("{})", &old[..start]);
    let s = ElementsSettings::from_ron(&trimmed).unwrap();
    assert_eq!(s.snow, bevy_elements::SnowSettings::default());
}
