//! A rounded zoned difference, transcribed step by step from the TC39 spec
//! (proposal-temporal main, spec/zoneddatetime.html
//! DifferenceZonedDateTimeWithRounding, spec/duration.html RoundRelativeDuration
//! and the nudge operations it calls) — not from temporal_rs. Integer
//! arithmetic throughout: a progress is kept as a fraction. Wall clocks resolve
//! with the oracle's own `resolve`; ISO date arithmetic is `oracle_dates`', and
//! any other calendar's is the proposal's over the ICU4C model (`Reckoning`) —
//! no date arithmetic comes from temporal_rs.
use temporal_rs::options::{Disambiguation, RoundingMode as M, Unit};
use temporal_rs::{PlainTime, TimeZone};

use crate::oracle_calendars::Model;
use crate::oracle_dates::{day_number, from_day_number};
use crate::rounding::{apply, round_to_increment, unsigned};
use crate::oracle::{local_date, naive, offset_at, resolve, zoned_difference_in, Ymd, HOUR};

pub type Date = [i64; 4];

fn unit_ns(u: Unit) -> Option<i128> {
    Some(match u {
        Unit::Hour => HOUR,
        Unit::Minute => 60_000_000_000,
        Unit::Second => 1_000_000_000,
        Unit::Millisecond => 1_000_000,
        Unit::Microsecond => 1_000,
        Unit::Nanosecond => 1,
        _ => return None,
    })
}

fn sign_of(date: Date, time: i128) -> i128 {
    date.iter().find(|v| **v != 0).map(|v| i128::from(v.signum())).unwrap_or(time.signum())
}

/// Whose calendar arithmetic CalendarDateAdd and CalendarDateUntil use: ISO's
/// from `oracle_dates`, or the proposal's over ICU4C's calendars
/// (`oracle_calendars`).
#[derive(Clone, Copy)]
pub enum Reckoning<'a> { Iso, Model(&'a Model) }

fn day_of(d: Ymd) -> i64 { day_number(d.year.into(), d.month.into(), d.day.into()) }

fn ymd_of(n: i64) -> Ymd {
    let (year, month, day) = from_day_number(n);
    Ymd { year: year as i32, month: month as u8, day: day as u8 }
}

/// CalendarDateAdd with `constrain`, returning the ISO date.
fn calendar_date_add(d: Ymd, dur: Date, reckoning: Reckoning) -> Option<Ymd> {
    match reckoning {
        Reckoning::Iso => date_add(d, dur),
        Reckoning::Model(model) => model.add_constrained(day_of(d), dur).map(ymd_of),
    }
}

/// CalendarDateUntil's weeks, largest unit `week`.
fn weeks_between(a: Ymd, b: Ymd, reckoning: Reckoning) -> Option<i64> {
    match reckoning {
        Reckoning::Iso => Some(crate::oracle_dates::until((a.year, a.month, a.day), (b.year, b.month, b.day), 1)[2]),
        Reckoning::Model(model) => model.until(day_of(a), day_of(b), 1).ok().map(|d| d[2]),
    }
}

/// CalendarDateAdd for ISO with `constrain`, on oracle_dates' day numbers.
fn date_add(d: Ymd, dur: Date) -> Option<Ymd> {
    crate::oracle_dates::add((d.year, d.month, d.day), dur[0], dur[1], dur[2], dur[3], false).map(|(year, month, day)| Ymd { year, month, day })
}

/// GetEpochNanosecondsFor(tz, date + time of day, compatible), or with no
/// zone GetUTCEpochNanoseconds.
fn epoch_for(d: Ymd, time_of_day: i128, tz: Option<TimeZone>) -> Option<i128> {
    let wall = i128::from(crate::oracle_dates::day_number(d.year.into(), d.month.into(), d.day.into())) * 24 * HOUR + time_of_day;
    match tz {
        Some(tz) => resolve(wall, tz, Disambiguation::Compatible),
        None => Some(wall),
    }
}

struct Nudge { date: Date, time: i128, nudged: i128, expanded: bool }

/// The starting point a duration is measured from: its instant, ISO date and
/// time of day, the zone (none for a plain date-time), and whose calendar
/// arithmetic applies.
pub struct Origin<'a> { pub ns: i128, pub date: Ymd, pub time: i128, pub tz: Option<TimeZone>, pub reckoning: Reckoning<'a> }

/// ComputeNudgeWindow: (r1, r2, startEpochNs, endEpochNs, startDuration, endDuration).
fn window(sign: i128, date: Date, o: &Origin, increment: i128, unit: Unit, shift: bool) -> Option<(i128, i128, i128, i128, Date, Date)> {
    let trunc = |v: i64| round_to_increment(i128::from(v), increment, M::Trunc);
    let shifted = |v: i128| if shift { v + increment * sign } else { v };
    let (r1, r2, start, end): (i128, i128, Date, Date) = match unit {
        Unit::Year => {
            let r1 = shifted(trunc(date[0]));
            let r2 = r1 + increment * sign;
            (r1, r2, [r1 as i64, 0, 0, 0], [r2 as i64, 0, 0, 0])
        }
        Unit::Month => {
            let r1 = shifted(trunc(date[1]));
            let r2 = r1 + increment * sign;
            (r1, r2, [date[0], r1 as i64, 0, 0], [date[0], r2 as i64, 0, 0])
        }
        Unit::Week => {
            let weeks_start = calendar_date_add(o.date, [date[0], date[1], 0, 0], o.reckoning)?;
            let weeks_end = date_add(weeks_start, [0, 0, 0, date[3]])?;
            let until_weeks = weeks_between(weeks_start, weeks_end, o.reckoning)?;
            let r1 = round_to_increment(i128::from(date[2] + until_weeks), increment, M::Trunc);
            let r2 = r1 + increment * sign;
            (r1, r2, [date[0], date[1], r1 as i64, 0], [date[0], date[1], r2 as i64, 0])
        }
        _ => {
            let r1 = trunc(date[3]);
            let r2 = r1 + increment * sign;
            (r1, r2, [date[0], date[1], date[2], r1 as i64], [date[0], date[1], date[2], r2 as i64])
        }
    };
    // The spec says "If r1 = 0, startEpochNs is originEpochNs". Read literally
    // that is wrong whenever a larger unit is still in the start duration: from
    // 2024-03-08 02:17 back to 02-06 15:17 in New York is -1 month -1 day -11
    // hours, and to the week the window would run from March 8 rather than
    // February 8, measuring 1.5 days against 36 and rounding to -P1M1W. The
    // origin is the start only when the whole start duration is zero, which is
    // what temporal_rs checks; this follows that reading (D-T2-17).
    let start_ns = if start == [0; 4] { o.ns } else { epoch_for(calendar_date_add(o.date, start, o.reckoning)?, o.time, o.tz)? };
    let end_ns = epoch_for(calendar_date_add(o.date, end, o.reckoning)?, o.time, o.tz)?;
    Some((r1, r2, start_ns, end_ns, start, end))
}

/// NudgeToCalendarUnit's [[Total]] with increment 1 and `trunc`, as a
/// numerator over a positive denominator.
pub fn nudge_total(sign: i128, date: Date, o: &Origin, dest: i128, unit: Unit) -> Option<(i128, i128)> {
    nudge_calendar_with_total(sign, date, o, dest, 1, unit, M::Trunc).map(|(_, total)| total)
}

/// NudgeToCalendarUnit.
fn nudge_calendar(sign: i128, date: Date, o: &Origin, dest: i128, increment: i128, unit: Unit, mode: M) -> Option<Nudge> {
    nudge_calendar_with_total(sign, date, o, dest, increment, unit, mode).map(|(nudge, _)| nudge)
}

#[allow(clippy::too_many_arguments)]
fn nudge_calendar_with_total(sign: i128, date: Date, o: &Origin, dest: i128, increment: i128, unit: Unit, mode: M) -> Option<(Nudge, (i128, i128))> {
    let mut expanded = false;
    let mut w = window(sign, date, o, increment, unit, false)?;
    let inside = |w: &(i128, i128, i128, i128, Date, Date)| if sign == 1 { w.2 <= dest && dest <= w.3 } else { w.3 <= dest && dest <= w.2 };
    if !inside(&w) {
        w = window(sign, date, o, increment, unit, true)?;
        if !inside(&w) { return None; }
        expanded = true;
    }
    let (r1, r2, start_ns, end_ns, start, end) = w;
    // total = r1 + progress × increment × sign, as a fraction over the window.
    let den = end_ns - start_ns;
    let num = r1 * den + (dest - start_ns) * increment * sign;
    let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
    let rounded = if dest == end_ns { r2.abs() } else { apply(num.abs(), den, r1.abs(), r2.abs(), unsigned(mode, sign < 0)) };
    let nudge = if rounded == r2.abs() {
        Nudge { date: end, time: 0, nudged: end_ns, expanded: true }
    } else {
        Nudge { date: start, time: 0, nudged: start_ns, expanded }
    };
    Some((nudge, (num, den)))
}

/// NudgeToZonedTime.
fn nudge_time(sign: i128, date: Date, time: i128, o: &Origin, increment: i128, unit: Unit, mode: M) -> Option<Nudge> {
    let start = calendar_date_add(o.date, date, o.reckoning)?;
    let end = date_add(start, [0, 0, 0, sign as i64])?;
    let (start_ns, end_ns) = (epoch_for(start, o.time, o.tz)?, epoch_for(end, o.time, o.tz)?);
    let step = increment * unit_ns(unit)?;
    let rounded = round_to_increment(time, step, mode);
    let beyond = rounded - (end_ns - start_ns);
    Some(if beyond.signum() != -sign {
        let rounded = round_to_increment(beyond, step, mode);
        Nudge { date: [date[0], date[1], date[2], date[3] + sign as i64], time: rounded, nudged: rounded + end_ns, expanded: true }
    } else {
        Nudge { date, time: rounded, nudged: rounded + start_ns, expanded: false }
    })
}

/// NudgeToDayOrTime, for a smallest unit of a day or less with no zone.
fn nudge_day_or_time(date: Date, time: i128, dest: i128, largest: Unit, increment: i128, smallest: Unit, mode: M) -> Option<Nudge> {
    const DAY: i128 = 24 * HOUR;
    let time_duration = time + i128::from(date[3]) * DAY;
    let unit = if smallest == Unit::Day { DAY } else { unit_ns(smallest)? };
    let rounded = round_to_increment(time_duration, unit * increment, mode);
    let (whole, rounded_whole) = (time_duration / DAY, rounded / DAY);
    let delta = rounded_whole - whole;
    let expanded = delta.signum() == time_duration.signum();
    let nudged = rounded - time_duration + dest;
    let (days, remainder) = if matches!(largest, Unit::Year | Unit::Month | Unit::Week | Unit::Day) {
        (rounded_whole, rounded - rounded_whole * DAY)
    } else {
        (0, rounded)
    };
    Some(Nudge { date: [date[0], date[1], date[2], days as i64], time: remainder, nudged, expanded })
}

fn index(u: Unit) -> usize {
    [Unit::Year, Unit::Month, Unit::Week, Unit::Day].iter().position(|x| *x == u).unwrap_or(4)
}

/// BubbleRelativeDuration.
fn bubble(sign: i128, mut date: Date, mut time: i128, nudged: i128, o: &Origin, largest: Unit, smallest: Unit) -> Option<(Date, i128)> {
    if smallest == largest { return Some((date, time)); }
    let (largest_i, smallest_i) = (index(largest), index(smallest));
    let mut unit_i = smallest_i as i64 - 1;
    while unit_i >= largest_i as i64 {
        let unit = [Unit::Year, Unit::Month, Unit::Week][unit_i as usize];
        if unit != Unit::Week || largest == Unit::Week {
            let end: Date = match unit {
                Unit::Year => [date[0] + sign as i64, 0, 0, 0],
                Unit::Month => [date[0], date[1] + sign as i64, 0, 0],
                _ => [date[0], date[1], date[2] + sign as i64, 0],
            };
            let end_ns = epoch_for(calendar_date_add(o.date, end, o.reckoning)?, o.time, o.tz)?;
            if (nudged - end_ns).signum() != -sign {
                date = end;
                time = 0;
            } else {
                break;
            }
        }
        unit_i -= 1;
    }
    Some((date, time))
}

/// DifferenceZonedDateTimeWithRounding then TemporalDurationFromInternal, as
/// the ten fields of the duration `a.until(b)` answers.
pub fn until_rounded(a: i128, b: i128, tz: TimeZone, largest: Unit, smallest: Unit, increment: i128, mode: M) -> Option<[i128; 10]> {
    until_rounded_in(a, b, tz, Reckoning::Iso, largest, smallest, increment, mode)
}

/// `until_rounded` on a calendar's reckoning.
#[allow(clippy::too_many_arguments)]
pub fn until_rounded_in(a: i128, b: i128, tz: TimeZone, reckoning: Reckoning, largest: Unit, smallest: Unit, increment: i128, mode: M) -> Option<[i128; 10]> {
    if let Some(largest_ns) = unit_ns(largest) {
        // DifferenceInstant, then TemporalDurationFromInternal balancing the
        // time down from the largest unit.
        let rounded = round_to_increment(b - a, increment * unit_ns(smallest)?, mode);
        let (sign, n) = (rounded.signum(), rounded.abs());
        let lengths = [HOUR, 60_000_000_000, 1_000_000_000, 1_000_000, 1_000, 1];
        let mut fields = [0i128; 10];
        let mut rest = n;
        for (k, len) in lengths.iter().enumerate() {
            if *len > largest_ns { continue; }
            fields[4 + k] = sign * (rest / len);
            rest %= len;
        }
        return Some(fields);
    }
    let (date, time) = zoned_difference_in(a, b, tz, reckoning, largest)?;
    if smallest == Unit::Nanosecond && increment == 1 {
        return Some(crate::oracle::duration_fields(date, time));
    }
    let d = local_date(a, tz)?;
    let o = Origin { ns: a, date: d, time: a + offset_at(a, tz)? - naive(d, &PlainTime::try_new(0, 0, 0, 0, 0, 0).ok()?)?, tz: Some(tz), reckoning };
    let (date, time) = round_relative(date, time, &o, b, largest, increment, smallest, mode)?;
    Some(crate::oracle::duration_fields(date, time))
}

/// RoundRelativeDuration.
#[allow(clippy::too_many_arguments)]
pub fn round_relative(date: Date, time: i128, o: &Origin, dest: i128, largest: Unit, increment: i128, smallest: Unit, mode: M) -> Option<(Date, i128)> {
    let irregular = matches!(smallest, Unit::Year | Unit::Month | Unit::Week) || (o.tz.is_some() && smallest == Unit::Day);
    let sign = if sign_of(date, time) < 0 { -1 } else { 1 };
    let n = if irregular {
        nudge_calendar(sign, date, o, dest, increment, smallest, mode)?
    } else if o.tz.is_some() {
        nudge_time(sign, date, time, o, increment, smallest, mode)?
    } else {
        nudge_day_or_time(date, time, dest, largest, increment, smallest, mode)?
    };
    if n.expanded && smallest != Unit::Week {
        bubble(sign, n.date, n.time, n.nudged, o, largest, smallest.max(Unit::Day))
    } else {
        Some((n.date, n.time))
    }
}
