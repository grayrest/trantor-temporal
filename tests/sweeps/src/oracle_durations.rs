//! Durations rounded, totalled and compared, transcribed from the spec
//! (spec/duration.html Temporal.Duration.prototype.round and .total,
//! Temporal.Duration.compare, DateDurationDays, TemporalDurationFromInternal,
//! TotalRelativeDuration; spec/plaindatetime.html DifferenceISODateTime and
//! DifferencePlainDateTimeWith{Rounding,Total}) over oracle_round's
//! RoundRelativeDuration, with CalendarDateAdd and CalendarDateUntil from the
//! reckoning: oracle_dates' ISO arithmetic, or the proposal's over the ICU4C
//! model for any other calendar.
use temporal_rs::options::{RoundingMode as M, Unit};

use crate::oracle::{Ymd, HOUR};
use crate::oracle_dates::{add, day_number, from_day_number, until, Iso};
use crate::oracle_round::{nudge_total, round_relative, Date, Origin, Reckoning};
use crate::rounding::round_to_increment;

const DAY: i128 = 24 * HOUR;
/// Years, months, weeks, days, hours, minutes, seconds, ms, µs, ns.
pub type Fields = [i128; 10];

const LENGTHS: [i128; 6] = [HOUR, 60_000_000_000, 1_000_000_000, 1_000_000, 1_000, 1];

fn is_calendar(u: Unit) -> bool {
    matches!(u, Unit::Year | Unit::Month | Unit::Week)
}

fn is_date(u: Unit) -> bool {
    is_calendar(u) || u == Unit::Day
}

fn unit_ns(u: Unit) -> i128 {
    match u {
        Unit::Day => DAY,
        Unit::Hour => LENGTHS[0],
        Unit::Minute => LENGTHS[1],
        Unit::Second => LENGTHS[2],
        Unit::Millisecond => LENGTHS[3],
        Unit::Microsecond => LENGTHS[4],
        _ => 1,
    }
}

/// DefaultTemporalLargestUnit.
pub fn largest_of(f: &Fields) -> Unit {
    const ORDER: [Unit; 10] = [Unit::Year, Unit::Month, Unit::Week, Unit::Day, Unit::Hour, Unit::Minute, Unit::Second, Unit::Millisecond, Unit::Microsecond, Unit::Nanosecond];
    ORDER.iter().zip(f.iter()).find(|(_, v)| **v != 0).map(|(u, _)| *u).unwrap_or(Unit::Nanosecond)
}

fn time_of(f: &Fields) -> i128 {
    f[4..].iter().zip(LENGTHS.iter()).map(|(v, l)| v * l).sum()
}

/// TemporalDurationFromInternal.
fn from_internal(date: Date, time: i128, largest: Unit) -> Fields {
    let (sign, mut n) = (time.signum(), time.abs());
    let mut out = [i128::from(date[0]), i128::from(date[1]), i128::from(date[2]), i128::from(date[3]), 0, 0, 0, 0, 0, 0];
    if is_date(largest) {
        out[3] += sign * (n / DAY);
        n %= DAY;
    }
    let top = if is_date(largest) { HOUR } else { unit_ns(largest) };
    for (k, len) in LENGTHS.iter().enumerate() {
        if *len > top { continue; }
        out[4 + k] = sign * (n / len);
        n %= len;
    }
    out
}

/// CalendarDateAdd with `constrain`.
fn calendar_add(reckoning: Reckoning, from: Iso, date: [i64; 4]) -> Option<Iso> {
    match reckoning {
        Reckoning::Iso => add(from, date[0], date[1], date[2], date[3], false),
        Reckoning::Model(model) => {
            let (y, m, d) = from_day_number(model.add_constrained(day_number(from.0.into(), from.1.into(), from.2.into()), date)?);
            Some((y as i32, m as u8, d as u8))
        }
    }
}

/// CalendarDateUntil.
fn calendar_until(reckoning: Reckoning, from: Iso, to: Iso, largest: u8) -> Option<Date> {
    match reckoning {
        Reckoning::Iso => Some(until(from, to, largest)),
        Reckoning::Model(model) => {
            let day = |d: Iso| day_number(d.0.into(), d.1.into(), d.2.into());
            model.until(day(from), day(to), largest).ok()
        }
    }
}

fn epoch(d: Iso) -> i128 {
    i128::from(day_number(d.0.into(), d.1.into(), d.2.into())) * DAY
}

/// ISODateTimeWithinLimits, strictly inside a day beyond the instant limits.
fn datetime_in_limits(ns: i128) -> bool {
    const LIMIT: i128 = 8_640_000_000_000_000_000_000;
    -LIMIT - DAY < ns && ns < LIMIT + DAY
}

/// The target a duration reaches from `from`'s midnight (the plain-relativeTo
/// branch of round and total): its date, time of day and epoch nanoseconds.
fn target(f: &Fields, from: Iso, reckoning: Reckoning) -> Option<(Iso, i128)> {
    let time = time_of(f) + f[3] * DAY;
    let (days, tod) = (time.div_euclid(DAY), time.rem_euclid(DAY));
    let date = calendar_add(reckoning, from, [f[0] as i64, f[1] as i64, f[2] as i64, days as i64])?;
    Some((date, tod))
}

fn code(u: Unit) -> u8 {
    match u { Unit::Year => 3, Unit::Month => 2, Unit::Week => 1, _ => 0 }
}

/// DifferenceISODateTime from `from`'s midnight to (`to`, `tod`).
fn difference(from: Iso, to: Iso, tod: i128, largest: Unit, reckoning: Reckoning) -> Option<(Date, i128)> {
    let date_sign = match from.cmp(&to) { core::cmp::Ordering::Greater => 1, core::cmp::Ordering::Less => -1, _ => 0 };
    let (mut adjusted, mut time) = (to, tod);
    if tod.signum() == date_sign && date_sign != 0 {
        adjusted = add(to, 0, 0, 0, date_sign as i64, false).unwrap_or(to);
        time -= date_sign * DAY;
    }
    let date_largest = if is_date(largest) { largest } else { Unit::Day };
    let mut dd = calendar_until(reckoning, from, adjusted, code(date_largest))?;
    if !is_date(largest) {
        time += i128::from(dd[3]) * DAY;
        dd[3] = 0;
    }
    Some((dd, time))
}

fn origin(from: Iso, reckoning: Reckoning) -> Origin {
    Origin { ns: epoch(from), date: Ymd { year: from.0, month: from.1, day: from.2 }, time: 0, tz: None, reckoning }
}

/// Temporal.Duration.prototype.round with explicit largest and smallest units.
#[allow(clippy::too_many_arguments)]
pub fn round(f: &Fields, largest: Unit, smallest: Unit, increment: i128, mode: M, from: Option<Iso>, reckoning: Reckoning) -> Option<Fields> {
    // The option checks: an increment above 1 on a date unit only rounds that
    // same unit, and a time unit's increment divides its maximum.
    if increment > 1 && largest != smallest && is_date(smallest) { return None; }
    let maximum = match smallest { Unit::Hour => Some(24), Unit::Minute | Unit::Second => Some(60), Unit::Millisecond | Unit::Microsecond | Unit::Nanosecond => Some(1000), _ => None };
    if maximum.is_some_and(|max| increment >= max || max % increment != 0) { return None; }
    let Some(from) = from else {
        if is_calendar(largest_of(f)) || is_calendar(largest) { return None; }
        let time = time_of(f) + f[3] * DAY;
        return Some(if smallest == Unit::Day {
            from_internal([0, 0, 0, (round_to_increment(time, DAY * increment, mode) / DAY) as i64], 0, largest)
        } else {
            from_internal([0; 4], round_to_increment(time, unit_ns(smallest) * increment, mode), largest)
        });
    };
    let (to, tod) = target(f, from, reckoning)?;
    if (to, tod) == (from, 0) {
        return Some(from_internal([0; 4], 0, largest));
    }
    if !datetime_in_limits(epoch(from)) || !datetime_in_limits(epoch(to) + tod) { return None; }
    let (date, time) = difference(from, to, tod, largest, reckoning)?;
    let (date, time) = if smallest == Unit::Nanosecond && increment == 1 {
        (date, time)
    } else {
        round_relative(date, time, &origin(from, reckoning), epoch(to) + tod, largest, increment, smallest, mode)?
    };
    Some(from_internal(date, time, largest))
}

/// Temporal.Duration.prototype.total, as a numerator over a positive denominator.
pub fn total(f: &Fields, unit: Unit, from: Option<Iso>, reckoning: Reckoning) -> Option<(i128, i128)> {
    let Some(from) = from else {
        if is_calendar(largest_of(f)) || is_calendar(unit) { return None; }
        return Some((time_of(f) + f[3] * DAY, unit_ns(unit)));
    };
    let (to, tod) = target(f, from, reckoning)?;
    if (to, tod) == (from, 0) { return Some((0, 1)); }
    if !datetime_in_limits(epoch(from)) || !datetime_in_limits(epoch(to) + tod) { return None; }
    let (date, time) = difference(from, to, tod, unit, reckoning)?;
    if unit == Unit::Nanosecond { return Some((time, 1)); }
    if is_calendar(unit) {
        let sign = if date.iter().find(|v| **v != 0).map(|v| *v < 0).unwrap_or(time < 0) { -1 } else { 1 };
        return nudge_total(sign, date, &origin(from, reckoning), epoch(to) + tod, unit);
    }
    Some((time + i128::from(date[3]) * DAY, unit_ns(unit)))
}

/// DateDurationDays.
fn date_days(f: &Fields, from: Iso, reckoning: Reckoning) -> Option<i128> {
    if f[0] == 0 && f[1] == 0 && f[2] == 0 { return Some(f[3]); }
    let later = calendar_add(reckoning, from, [f[0] as i64, f[1] as i64, f[2] as i64, 0])?;
    Some(f[3] + (epoch(later) - epoch(from)) / DAY)
}

/// Temporal.Duration.compare: -1, 0 or 1.
pub fn compare(a: &Fields, b: &Fields, from: Option<Iso>, reckoning: Reckoning) -> Option<i8> {
    if a == b { return Some(0); }
    let (da, db) = if is_calendar(largest_of(a)) || is_calendar(largest_of(b)) {
        let from = from?;
        (date_days(a, from, reckoning)?, date_days(b, from, reckoning)?)
    } else {
        (a[3], b[3])
    };
    Some((time_of(a) + da * DAY).cmp(&(time_of(b) + db * DAY)) as i8)
}
