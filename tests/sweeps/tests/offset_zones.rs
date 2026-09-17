//! Fixed-offset zones (`+05:30`), which temporal_rs resolves without asking a
//! tzdb provider, so nothing else here reaches them. Identifiers normalise as
//! TC39 says (`+0530` is `+05:30`, `-00:00` is `+00:00`) and a sub-minute or
//! 24-hour offset is not one. Across the representable range and densely
//! around now: a wall clock is `wall - offset` in every mode, printing and
//! parsing back agree and a wrong offset is refused, add, until (rounded and
//! not), a day's start, length and rounding follow the oracle, and there are
//! no transitions.
use temporal_rs::options::{DifferenceSettings, Disambiguation, Overflow, RoundingIncrement, RoundingMode as M, RoundingOptions, Unit};
use temporal_rs::provider::TransitionDirection;
use temporal_rs::{Calendar, Duration, PlainTime, TimeZone, ZonedDateTime};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::{local_date, naive, zoned_difference, Ymd, HOUR, LIMIT, SECOND};
use temporal_sweeps::oracle_round::until_rounded;
use temporal_sweeps::transition::transition;
use temporal_sweeps::zoned_ops::{from_wall_clock, parse, to_str};

const DAY: i128 = 24 * HOUR;
const MINUTE: i128 = 60 * SECOND;

fn offset_str(ns: i128) -> String {
    let sign = if ns < 0 { '-' } else { '+' };
    format!("{sign}{:02}:{:02}", ns.abs() / HOUR, ns.abs() % HOUR / MINUTE)
}

fn wall_str(d: Ymd, tod: i128) -> String {
    let (h, m, s, frac) = (tod / HOUR, tod % HOUR / MINUTE, tod % MINUTE / SECOND, tod % SECOND);
    let fraction = if frac == 0 { String::new() } else { format!(".{frac:09}").trim_end_matches('0').to_string() };
    let year = if (0..=9999).contains(&d.year) { format!("{:04}", d.year) } else { format!("{}{:06}", if d.year < 0 { '-' } else { '+' }, d.year.abs()) };
    format!("{year}-{:02}-{:02}T{h:02}:{m:02}:{s:02}{fraction}", d.month, d.day)
}

/// An offset one minute nearer zero (or +00:01 for zero): still a valid zone.
fn nudged(ns: i128) -> i128 {
    if ns > 0 { ns - MINUTE } else if ns < 0 { ns + MINUTE } else { MINUTE }
}

fn fields(d: &Duration) -> [i128; 10] {
    [d.years().into(), d.months().into(), d.weeks().into(), d.days().into(), d.hours().into(), d.minutes().into(),
     d.seconds().into(), d.milliseconds().into(), d.microseconds(), d.nanoseconds()]
}

#[test]
fn should_treat_a_fixed_offset_as_a_zone_with_one_offset_forever() {
    let zones: [(&str, &str, i128); 8] = [
        ("+05:30", "+05:30", 5 * HOUR + 30 * MINUTE), ("-00:00", "+00:00", 0), ("-09:30", "-09:30", -(9 * HOUR + 30 * MINUTE)),
        ("+14:00", "+14:00", 14 * HOUR), ("-23:59", "-23:59", -(23 * HOUR + 59 * MINUTE)), ("+23:59", "+23:59", 23 * HOUR + 59 * MINUTE),
        ("+0530", "+05:30", 5 * HOUR + 30 * MINUTE), ("+05", "+05:00", 5 * HOUR),
    ];
    let (mut checked, mut wrong) = (0u64, vec![]);
    let mut check = |label: String, got: String, want: String| {
        checked += 1;
        if got != want && wrong.len() < 16 {
            wrong.push(format!("{label}: host {got}, oracle {want}"));
        }
    };
    for bad in ["+24:00", "+05:30:15", "+5:30", "05:30", "+05:60"] {
        check(format!("{bad} is not a zone"), TimeZone::try_from_str(bad).is_err().to_string(), "true".into());
    }
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    let spread = (2 * LIMIT - 8 * DAY) / 3000;
    let now = naive(Ymd { year: 2024, month: 1, day: 1 }, &midnight).unwrap();
    let mut instants: Vec<i128> = (0..=3000).map(|k| -LIMIT + 4 * DAY + k * spread + k * 7_919 * SECOND).collect();
    instants.extend((0..3000).map(|k| now + k * (7 * HOUR + 37 * MINUTE) + k * 1_000_003));
    for (id, canonical, offset) in zones {
        let tz = TimeZone::try_from_str(id).unwrap();
        check(format!("{id} identifier"), format!("{:?}", tz.identifier()), format!("{:?}", Ok::<String, ()>(canonical.to_string())));
        for &i in &instants {
            let label = |what: &str| format!("{id} {i} {what}");
            let Some(d) = local_date(i, tz) else { continue };
            let day_start = naive(d, &midnight).unwrap() - offset;
            let tod = i - day_start;
            let z = ZonedDateTime::try_new_with_provider(i, tz, Calendar::ISO, &EXACT).unwrap();
            let time = z.to_plain_time();
            for dis in [Disambiguation::Compatible, Disambiguation::Earlier, Disambiguation::Later, Disambiguation::Reject] {
                let got = from_wall_clock((d.year, d.month, d.day), &time, tz, Calendar::ISO, dis).map(|r| r.epoch_nanoseconds().as_i128());
                check(label(&format!("wall clock {dis:?}")), format!("{:?}", got.ok()), format!("{:?}", Some(i)));
            }
            let printed = format!("{}{}[{canonical}]", wall_str(d, tod), offset_str(offset));
            check(label("to_str"), format!("{:?}", to_str(&z).ok()), format!("{:?}", Some(printed.clone())));
            check(label("parse back"), format!("{:?}", parse(&printed).ok().map(|p| p.epoch_nanoseconds().as_i128())), format!("{:?}", Some(i)));
            let wrong_offset = format!("{}{}[{canonical}]", wall_str(d, tod), offset_str(nudged(offset)));
            check(label("parse wrong offset"), parse(&wrong_offset).is_err().to_string(), "true".into());
            check(label("start of day"), format!("{:?}", z.start_of_day_with_provider(&EXACT).ok().map(|s| s.epoch_nanoseconds().as_i128())), format!("{:?}", Some(day_start)));
            check(label("hours in day"), format!("{:?}", z.hours_in_day_with_provider(&EXACT).ok()), format!("{:?}", Some(24.0)));
            let mut by_day = RoundingOptions::default();
            by_day.smallest_unit = Some(Unit::Day);
            by_day.rounding_mode = Some(M::HalfExpand);
            by_day.increment = Some(RoundingIncrement::try_new(1).unwrap());
            let rounded = if 2 * tod >= DAY { day_start + DAY } else { day_start };
            check(label("round to day"), format!("{:?}", z.round_with_provider(by_day, &EXACT).ok().map(|r| r.epoch_nanoseconds().as_i128())), format!("{:?}", Some(rounded)));
            for d in [TransitionDirection::Next, TransitionDirection::Previous] {
                check(label(&format!("transition {d:?}")), format!("{:?}", transition(&z, d).ok().map(|t| t.is_some())), format!("{:?}", Some(false)));
            }
            for (years, months, days, ns) in [(0i64, 0i64, 1i64, 0i128), (0, -1, 0, 0), (1, 0, 0, 0), (0, 0, 0, 5 * HOUR), (0, 0, 1, 1)] {
                let dur = Duration::new(years, months, 0, days, 0, 0, 0, 0, 0, ns).unwrap();
                let got = z.add_with_provider(&dur, Some(Overflow::Constrain), &EXACT).ok().map(|r| r.epoch_nanoseconds().as_i128());
                // The spec refuses where a step leaves the range: the stepped date
                // (CalendarDateAdd), the instant its wall clock names, or the result.
                let in_range = |n: i128| (-LIMIT..=LIMIT).contains(&n);
                let want = if (years, months, days) == (0, 0, 0) {
                    Some(i + ns).filter(|n| in_range(*n))
                } else {
                    z.to_plain_date().add(&Duration::new(years, months, 0, days, 0, 0, 0, 0, 0, 0).unwrap(), Some(Overflow::Constrain)).ok()
                        .and_then(|stepped| naive(Ymd::of(&stepped), &midnight))
                        .map(|n| n + tod - offset)
                        .filter(|n| in_range(*n))
                        .map(|n| n + ns)
                        .filter(|n| in_range(*n))
                };
                check(label(&format!("add {years}y{months}m{days}d+{ns}ns")), format!("{got:?}"), format!("{want:?}"));
            }
            for b in [i + 40 * DAY + 5 * HOUR, i - 400 * DAY] {
                let Ok(zb) = ZonedDateTime::try_new_with_provider(b, tz, Calendar::ISO, &EXACT) else { continue };
                for unit in [Unit::Day, Unit::Month] {
                    let mut s = DifferenceSettings::default();
                    s.largest_unit = Some(unit);
                    check(label(&format!("until {b} {unit:?}")), format!("{:?}", z.until_with_provider(&zb, s, &EXACT).ok().map(|x| fields(&x))), format!("{:?}", zoned_difference(i, b, tz, unit)));
                }
                for (largest, smallest, mode) in [(Unit::Month, Unit::Day, M::HalfExpand), (Unit::Day, Unit::Hour, M::Ceil)] {
                    let mut s = DifferenceSettings::default();
                    s.largest_unit = Some(largest);
                    s.smallest_unit = Some(smallest);
                    s.rounding_mode = Some(mode);
                    s.increment = Some(RoundingIncrement::try_new(1).unwrap());
                    check(label(&format!("until rounded {b} {largest:?}/{smallest:?}")), format!("{:?}", z.until_with_provider(&zb, s, &EXACT).ok().map(|x| fields(&x))), format!("{:?}", until_rounded(i, b, tz, largest, smallest, 1, mode)));
                }
            }
            let same = ZonedDateTime::try_new_with_provider(i, TimeZone::try_from_str(canonical).unwrap(), Calendar::ISO, &EXACT).unwrap();
            let shifted = ZonedDateTime::try_new_with_provider(i, TimeZone::try_from_str(&offset_str(nudged(offset))).unwrap(), Calendar::ISO, &EXACT).unwrap();
            check(label("equals itself spelled canonically"), format!("{:?}", z.equals_with_provider(&same, &EXACT).ok()), "Some(true)".into());
            check(label("not equal a minute off"), format!("{:?}", z.equals_with_provider(&shifted, &EXACT).ok()), "Some(false)".into());
        }
    }
    eprintln!("offset zones: {checked} checked");
    assert!(wrong.is_empty(), "a fixed-offset zone disagrees with the oracle:\n{}", wrong.join("\n"));
}
