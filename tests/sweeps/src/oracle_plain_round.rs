//! DifferenceTemporalPlainDate with rounding (spec/plaindate.html): the date
//! difference, then RoundRelativeDuration from the first date's midnight in
//! UTC, in ISO over `oracle_dates` or on another calendar over the proposal's
//! arithmetic and ICU4C's calendars (`oracle_calendars`).
use temporal_rs::options::{RoundingMode as M, Unit};

use crate::oracle::{Ymd, HOUR};
use crate::oracle_calendars::Model;
use crate::oracle_dates::{day_number, from_day_number, until, Iso};
use crate::oracle_round::{round_relative, Date, Origin, Reckoning};

const DAY: i128 = 24 * HOUR;

fn code(u: Unit) -> u8 {
    match u { Unit::Year => 3, Unit::Month => 2, Unit::Week => 1, _ => 0 }
}

/// Rounds a date difference from `one` to `two`, both day numbers.
#[allow(clippy::too_many_arguments)]
fn rounded(date: Date, one: i64, two: i64, reckoning: Reckoning, largest: Unit, smallest: Unit, increment: i128, mode: M) -> Option<[i64; 4]> {
    let (y, m, d) = from_day_number(one);
    let origin = Origin { ns: i128::from(one) * DAY, date: Ymd { year: y as i32, month: m as u8, day: d as u8 }, time: 0, tz: None, reckoning };
    let (date, time) = round_relative(date, 0, &origin, i128::from(two) * DAY, largest, increment, smallest, mode)?;
    // TemporalDurationFromInternal with `day`: any whole days of time carry.
    Some([date[0], date[1], date[2], date[3] + (time / DAY) as i64])
}

/// `until` for ISO dates.
pub fn plain_until(one: Iso, two: Iso, largest: Unit, smallest: Unit, increment: i128, mode: M) -> Option<[i64; 4]> {
    let date = until(one, two, code(largest));
    if smallest == Unit::Day && increment == 1 {
        return Some(date);
    }
    let number = |d: Iso| day_number(d.0.into(), d.1.into(), d.2.into());
    rounded(date, number(one), number(two), Reckoning::Iso, largest, smallest, increment, mode)
}

/// `until` for dates on the model's calendar, as day numbers. `None` is a
/// refusal or, when `model.take_declined()` says so, a year it does not know.
pub fn plain_until_in(model: &Model, one: i64, two: i64, largest: Unit, smallest: Unit, increment: i128, mode: M) -> Option<[i64; 4]> {
    let date = model.until(one, two, code(largest)).ok()?;
    if smallest == Unit::Day && increment == 1 {
        return Some(date);
    }
    rounded(date, one, two, Reckoning::Model(model), largest, smallest, increment, mode)
}
