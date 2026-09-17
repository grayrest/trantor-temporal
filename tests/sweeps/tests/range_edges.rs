//! Zoned operations within two days of both ends of the representable range,
//! in every zone and a fixed offset each side: adding days and hours, a day's
//! start and length, rounding to a day, printing and parsing back, and
//! differences in days. Where a step would leave the range the spec throws —
//! a candidate instant or a result outside ±8.64e21 ns, a date-time more than a
//! day beyond it — and the host must refuse exactly there.
use temporal_rs::options::{DifferenceSettings, Disambiguation, Overflow, RoundingIncrement, RoundingMode as M, RoundingOptions, Unit};
use temporal_rs::{Calendar, Duration, PlainTime, TimeZone, ZonedDateTime};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::{candidates, first_instant, local_date, naive, offset_at, resolve, zoned_difference, HOUR, LIMIT};
use temporal_sweeps::zoned_ops::{parse, to_str};
use temporal_sweeps::zones;

const DAY: i128 = 24 * HOUR;

fn crate_days(y: i32, m: u8, d: u8) -> i64 {
    temporal_sweeps::oracle_dates::day_number(y.into(), m.into(), d.into())
}

/// AddZonedDateTime, refusing where the spec throws for range: a candidate
/// instant for the stepped wall clock outside the limits, or the result.
fn want_add(i: i128, tz: TimeZone, days: i64, hours: i128) -> Option<i128> {
    let valid = |n: i128| (-LIMIT..=LIMIT).contains(&n);
    if days == 0 {
        return valid(i + hours * HOUR).then_some(i + hours * HOUR);
    }
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).ok()?;
    let d = local_date(i, tz)?;
    let tod = i + offset_at(i, tz)? - naive(d, &midnight)?;
    let wall = naive(d, &midnight)? + tod + i128::from(days) * DAY;
    if !(-LIMIT - DAY < wall && wall < LIMIT + DAY) {
        return None;
    }
    let c = candidates(wall, tz);
    if c.instants.iter().any(|n| !valid(*n)) {
        return None;
    }
    let r = resolve(wall, tz, Disambiguation::Compatible)? + hours * HOUR;
    valid(r).then_some(r)
}

#[test]
fn should_refuse_exactly_where_the_range_ends() {
    let offsets = [1i128, HOUR, 5 * HOUR, 11 * HOUR, 13 * HOUR, 15 * HOUR + 30 * 60_000_000_000, 17 * HOUR, 25 * HOUR, 49 * HOUR];
    let mut instants = vec![-LIMIT, LIMIT];
    for o in offsets {
        instants.extend([-LIMIT + o, LIMIT - o]);
    }
    let mut all: Vec<(String, TimeZone)> = zones().into_iter().map(|(id, tz)| (id.to_string(), tz)).collect();
    all.extend(["+14:00", "-12:00"].iter().map(|id| (id.to_string(), TimeZone::try_from_str(id).unwrap())));
    let (mut checked, mut wrong) = (0u64, vec![]);
    let mut check = |label: String, got: Option<String>, want: Option<String>| {
        checked += 1;
        if got != want && wrong.len() < 16 {
            wrong.push(format!("{label}: host {got:?}, oracle {want:?}"));
        }
    };
    let epoch = |r: Result<ZonedDateTime, temporal_rs::TemporalError>| r.ok().map(|x| x.epoch_nanoseconds().as_i128().to_string());
    for (id, tz) in &all {
        for &i in &instants {
            let Ok(z) = ZonedDateTime::try_new_with_provider(i, *tz, Calendar::ISO, &EXACT) else { continue };
            for (days, hours) in [(1i64, 0i128), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -25)] {
                let dur = Duration::new(0, 0, 0, days, hours as i64, 0, 0, 0, 0, 0).unwrap();
                check(format!("{id} {i} add {days}d{hours}h"), epoch(z.add_with_provider(&dur, Some(Overflow::Constrain), &EXACT)), want_add(i, *tz, days, hours).map(|n| n.to_string()));
            }
            let d = local_date(i, *tz).unwrap();
            let start = first_instant(d, *tz);
            check(format!("{id} {i} start of day"), epoch(z.start_of_day_with_provider(&EXACT)), start.map(|n| n.to_string()));
            let next = d.next().and_then(|n| first_instant(n, *tz));
            let length = start.zip(next).map(|(s, e)| ((e - s) as f64 / HOUR as f64).to_string());
            check(format!("{id} {i} hours in day"), z.hours_in_day_with_provider(&EXACT).ok().map(|h| h.to_string()), length);
            let mut by_day = RoundingOptions::default();
            by_day.smallest_unit = Some(Unit::Day);
            by_day.rounding_mode = Some(M::HalfExpand);
            by_day.increment = Some(RoundingIncrement::try_new(1).unwrap());
            let rounded = start.zip(next).filter(|(_, e)| *e > i).map(|(s, e)| if 2 * (i - s) >= e - s { e } else { s });
            check(format!("{id} {i} round to day"), epoch(z.round_with_provider(by_day, &EXACT)), rounded.map(|n| n.to_string()));
            // A string with an offset is refused when its local date is more than
            // 10^8 days from 1970 (CheckISODaysRange), which the first instants of
            // a zone behind UTC are; the same wall clock with no offset parses.
            let days_out = crate_days(d.year, d.month, d.day).abs() > 100_000_000;
            check(format!("{id} {i} print and parse back"), to_str(&z).ok().and_then(|s| epoch(parse(&s))), (!days_out).then(|| i.to_string()));
            for b in [i + 30 * HOUR, i - 30 * HOUR] {
                let Ok(zb) = ZonedDateTime::try_new_with_provider(b, *tz, Calendar::ISO, &EXACT) else { continue };
                let mut s = DifferenceSettings::default();
                s.largest_unit = Some(Unit::Day);
                check(format!("{id} {i} until {b}"), z.until_with_provider(&zb, s, &EXACT).ok().map(|x| format!("{:?}", [x.days(), x.hours()])), zoned_difference(i, b, *tz, Unit::Day).map(|f| format!("{:?}", [f[3] as i64, f[4] as i64])));
            }
        }
    }
    eprintln!("range edges: {checked} checked");
    assert!(wrong.is_empty(), "an operation at the ends of the range disagrees with the oracle:\n{}", wrong.join("\n"));
}
