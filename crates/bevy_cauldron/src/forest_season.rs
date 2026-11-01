//! **The trees wear the map's date** (#102): the preview's `bevy_flora::FloraSeason` follows
//! `bevy_elements`' `WorldClock`, so setting the month in the environment panel (#98) turns a painted
//! forest from blossom to leaf to autumn to snow, in the editor and in the game that reads the same file.
//!
//! `FloraSeason::of_year` starts its year in early spring (blossom at 0, autumn at 0.55-0.75, snow near
//! 0.9), so the year's progress here counts from 1 March in the northern hemisphere and from 1 September
//! in the southern, where the seasons are half a year apart.

use bevy::prelude::*;
use bevy_elements::{GeoLocation, WorldClock};
use bevy_flora::FloraSeason;

/// Day of the year (1-based) on which the flora's year begins in the northern hemisphere: 1 March, in a non-leap year.
const SPRING_START_DAY: f64 = 60.0;

/// The season the trees wear on `clock`'s date at `location`.
pub fn season_for(clock: &WorldClock, location: &GeoLocation) -> FloraSeason {
    let days_in_year = if clock.date.is_leap_year() { 366.0 } else { 365.0 };
    let shift = if location.is_southern() { days_in_year / 2.0 } else { 0.0 };
    let t = (clock.day_of_year() as f64 - SPRING_START_DAY - shift) / days_in_year;
    FloraSeason::of_year(t as f32)
}

/// Keep the preview's `FloraSeason` on the clock's date. Does nothing until both resources exist.
pub fn follow_date(clock: Option<Res<WorldClock>>, location: Option<Res<GeoLocation>>, season: Option<ResMut<FloraSeason>>) {
    let (Some(clock), Some(location), Some(mut season)) = (clock, location, season) else { return };
    let wanted = season_for(&clock, &location);
    if *season != wanted {
        *season = wanted;
    }
}
