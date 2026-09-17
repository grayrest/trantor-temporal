//! Zoned arithmetic where the other sweeps use whole units: durations with
//! sub-second parts, rounding to milliseconds, microseconds and nanoseconds,
//! differences a nanosecond off a whole span, and `add` with `reject` from the
//! last days of a month into a shorter one — in every zone near five eras of
//! transitions, against the oracle.
use temporal_rs::options::{DifferenceSettings, Disambiguation, Overflow, RoundingIncrement, RoundingMode as M, RoundingOptions, Unit};
use temporal_rs::{Calendar, Duration, PlainTime, ZonedDateTime};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::{local_date, naive, offset_at, resolve, zoned_difference, Ymd, HOUR, SECOND};
use temporal_sweeps::oracle_dates;
use temporal_sweeps::rounding::round_to_increment;
use temporal_sweeps::zones;

const DAY: i128 = 24 * HOUR;

fn fields(d: &Duration) -> [i128; 10] {
    [d.years().into(), d.months().into(), d.weeks().into(), d.days().into(), d.hours().into(), d.minutes().into(),
     d.seconds().into(), d.milliseconds().into(), d.microseconds(), d.nanoseconds()]
}

/// AddZonedDateTime with ISO date arithmetic from oracle_dates.
fn want_add(i: i128, tz: temporal_rs::TimeZone, years: i64, months: i64, days: i64, time: i128, reject: bool) -> Option<i128> {
    if (years, months, days) == (0, 0, 0) {
        return Some(i + time);
    }
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).ok()?;
    let d = local_date(i, tz)?;
    let tod = i + offset_at(i, tz)? - naive(d, &midnight)?;
    let (y, m, dd) = oracle_dates::add((d.year, d.month, d.day), years, months, 0, days, reject)?;
    Some(resolve(naive(Ymd { year: y, month: m, day: dd }, &midnight)? + tod, tz, Disambiguation::Compatible)? + time)
}

/// Rounding to a sub-second unit: the wall clock rounds, the offset is kept
/// where it still reads back, else `compatible`.
fn want_round(i: i128, tz: temporal_rs::TimeZone, step: i128, mode: M) -> Option<i128> {
    let off = offset_at(i, tz)?;
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).ok()?;
    let day0 = naive(local_date(i, tz)?, &midnight)?;
    let rounded = day0 + round_to_increment(i + off - day0, step, mode);
    if offset_at(rounded - off, tz) == Some(off) {
        return Some(rounded - off);
    }
    resolve(rounded, tz, Disambiguation::Compatible)
}

#[test]
fn should_handle_sub_second_parts_and_reject_near_transitions() {
    let durations: [(i64, i64, i64, i128); 6] = [(0, 0, 0, 1), (0, 0, 0, -SECOND / 2), (0, 0, 1, 1_000), (0, 0, 0, DAY - 1), (0, 1, 0, 999_999_999), (0, 0, -1, -1)];
    let rounds: [(Unit, u32, i128, M); 4] = [(Unit::Millisecond, 1, 1_000_000, M::HalfExpand), (Unit::Microsecond, 250, 250_000, M::Floor), (Unit::Nanosecond, 5, 5, M::HalfEven), (Unit::Second, 15, 15 * SECOND, M::Ceil)];
    let (mut checked, mut wrong) = (0u64, vec![]);
    let mut check = |label: String, got: Option<String>, want: Option<String>| {
        checked += 1;
        if got != want && wrong.len() < 16 {
            wrong.push(format!("{label}: host {got:?}, oracle {want:?}"));
        }
    };
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    for (id, tz) in zones() {
        for (k, i) in temporal_sweeps::near_era_sample(tz, 4).into_iter().enumerate() {
            // Off the second by a varying amount, so nanosecond fields are live.
            let i = i + (k as i128 * 7_919) % SECOND;
            let Ok(z) = ZonedDateTime::try_new_with_provider(i, tz, Calendar::ISO, &EXACT) else { continue };
            let epoch = |r: Result<ZonedDateTime, temporal_rs::TemporalError>| r.ok().map(|x| x.epoch_nanoseconds().as_i128().to_string());
            for (years, months, days, time) in durations {
                let dur = Duration::new(years, months, 0, days, 0, 0, 0, 0, 0, time).unwrap();
                check(format!("{id} {i} add {years}y{months}m{days}d{time}ns"), epoch(z.add_with_provider(&dur, Some(Overflow::Constrain), &EXACT)), want_add(i, tz, years, months, days, time, false).map(|n| n.to_string()));
            }
            for (unit, inc, step, mode) in rounds {
                let mut o = RoundingOptions::default();
                o.smallest_unit = Some(unit);
                o.rounding_mode = Some(mode);
                o.increment = Some(RoundingIncrement::try_new(inc).unwrap());
                check(format!("{id} {i} round {unit:?}/{inc} {mode:?}"), epoch(z.round_with_provider(o, &EXACT)), want_round(i, tz, step, mode).map(|n| n.to_string()));
            }
            for b in [i + DAY + 1, i - 31 * DAY - 1, i + 1] {
                let Ok(zb) = ZonedDateTime::try_new_with_provider(b, tz, Calendar::ISO, &EXACT) else { continue };
                for unit in [Unit::Day, Unit::Month] {
                    let mut s = DifferenceSettings::default();
                    s.largest_unit = Some(unit);
                    check(format!("{id} {i} until {b} {unit:?}"), z.until_with_provider(&zb, s, &EXACT).ok().map(|d| format!("{:?}", fields(&d))), zoned_difference(i, b, tz, unit).map(|f| format!("{f:?}")));
                }
            }
            // The same wall clock on the month's last days, a month on under reject and constrain.
            let Some(d) = local_date(i, tz) else { continue };
            let tod = i + offset_at(i, tz).unwrap() - naive(d, &midnight).unwrap();
            let length = oracle_dates::days_in_month(d.year.into(), d.month.into()) as u8;
            for day in (length - 2)..=length {
                let Some(base) = resolve(naive(Ymd { year: d.year, month: d.month, day }, &midnight).unwrap() + tod, tz, Disambiguation::Compatible) else { continue };
                let Ok(zb) = ZonedDateTime::try_new_with_provider(base, tz, Calendar::ISO, &EXACT) else { continue };
                for months in [1i64, -1] {
                    let dur = Duration::new(0, months, 0, 0, 0, 0, 0, 0, 0, 0).unwrap();
                    for (overflow, reject) in [(Overflow::Reject, true), (Overflow::Constrain, false)] {
                        check(format!("{id} {base} add {months}m {overflow:?}"), epoch(zb.add_with_provider(&dur, Some(overflow), &EXACT)), want_add(base, tz, 0, months, 0, 0, reject).map(|n| n.to_string()));
                    }
                }
            }
        }
    }
    eprintln!("zoned fine: {checked} checked");
    assert!(wrong.is_empty(), "zoned arithmetic with sub-second parts or reject disagrees with the oracle:\n{}", wrong.join("\n"));
}
