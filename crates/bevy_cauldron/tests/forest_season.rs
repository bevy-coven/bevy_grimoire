//! **The trees wear the map's date** (#102).
//! `cargo test -p bevy_cauldron --features "bevy_flora bevy_elements" --test editor forest_season::`

#![cfg(all(feature = "bevy_flora", feature = "bevy_elements"))]

use bevy::prelude::*;
use bevy_elements::{CivilDate, GeoLocation, WorldClock};
use bevy_cauldron::forest_season::{follow_date, season_for};
use bevy_flora::FloraSeason;

fn on(month: u8, day: u8, latitude: f64) -> FloraSeason {
    season_for(&WorldClock::new(CivilDate::new(2026, month, day)), &GeoLocation { latitude_deg: latitude, ..GeoLocation::default() })
}

#[test]
fn the_year_runs_blossom_green_autumn_snow_in_the_north_and_half_a_year_apart_in_the_south() {
    assert!(on(3, 5, 45.0).blossom > 0.5, "early March blossoms");
    let summer = on(7, 15, 45.0);
    assert_eq!((summer.autumn, summer.leaf_fall, summer.snow, summer.blossom), (0.0, 0.0, 0.0, 0.0));
    assert!(on(11, 25, 45.0).autumn > 0.9, "late November is autumn");
    assert!(on(2, 5, 45.0).snow > 0.9, "early February holds snow");
    assert!(on(5, 27, -35.0).autumn > 0.9, "the south turns in May");
    assert!(on(10, 25, -35.0).autumn == 0.0, "and is green in October");
    assert!(on(9, 3, -35.0).blossom > 0.5, "the south blossoms in early September");
}

#[test]
fn setting_the_clocks_month_changes_the_resource_the_trees_draw_from() {
    let mut app = App::new();
    app.insert_resource(WorldClock::new(CivilDate::new(2026, 7, 15)))
        .insert_resource(GeoLocation::default())
        .init_resource::<FloraSeason>()
        .add_systems(Update, follow_date);
    app.update();
    assert_eq!(app.world().resource::<FloraSeason>().autumn, 0.0);
    app.world_mut().resource_mut::<WorldClock>().date = CivilDate::new(2026, 11, 25);
    app.update();
    assert!(app.world().resource::<FloraSeason>().autumn > 0.9);
}
