//! Rounding a zoned value, against TC39's definition built from the oracle:
//! a day rounds by the day's real length between the oracle's first instants,
//! a time unit rounds the wall clock and keeps the offset where it still reads
//! back, else resolves `Compatible`. Sampled every 15 minutes through the two
//! days either side of every transition in 2021-2026 (off the quarter-hour
//! within six hours of it), plus every 25 hours of those years, in the sweep's
//! zones.
use temporal_rs::options::{Disambiguation, RoundingIncrement, RoundingMode as M, RoundingOptions, Unit};
use temporal_rs::{Calendar, ZonedDateTime};
use temporal_sweeps::oracle::{first_instant, local_date, naive, offset_at, resolve, transitions, HOUR};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::Ymd;
use temporal_sweeps::ZONES;

const MINUTE: i128 = 60_000_000_000;
const MODES: [M; 9] = [M::Ceil, M::Floor, M::Expand, M::Trunc, M::HalfCeil, M::HalfFloor, M::HalfExpand, M::HalfTrunc, M::HalfEven];

/// The multiple of `step` a mode picks for non-negative `x`, by comparing the
/// distances to the two neighbours rather than by quotient and remainder.
fn pick(x: i128, step: i128, mode: M) -> i128 {
    let down = x - x.rem_euclid(step);
    let up = if down == x { x } else { down + step };
    let (to_down, to_up) = (x - down, up - x);
    let down_is_even = (down / step) % 2 == 0;
    match mode {
        M::Ceil | M::Expand => up,
        M::Floor | M::Trunc => down,
        _ if to_down < to_up => down,
        _ if to_up < to_down => up,
        M::HalfCeil | M::HalfExpand => up,
        M::HalfFloor | M::HalfTrunc => down,
        M::HalfEven => if down_is_even { down } else { up },
    }
}

fn want_day(i: i128, tz: temporal_rs::TimeZone, mode: M) -> Option<i128> {
    let d = local_date(i, tz)?;
    let (start, end) = (first_instant(d, tz)?, first_instant(d.next()?, tz)?);
    Some(start + pick(i - start, end - start, mode))
}

fn want_time(i: i128, tz: temporal_rs::TimeZone, step: i128, mode: M) -> Option<i128> {
    let off = offset_at(i, tz)?;
    let d = local_date(i, tz)?;
    let day0 = naive(d, &temporal_rs::PlainTime::try_new(0, 0, 0, 0, 0, 0).ok()?)?;
    let rounded = day0 + pick(i + off - day0, step, mode);
    if offset_at(rounded - off, tz) == Some(off) {
        return Some(rounded - off);
    }
    resolve(rounded, tz, Disambiguation::Compatible)
}

#[test]
fn should_round_by_the_real_day_and_the_wall_clock() {
    let midnight = temporal_rs::PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    let from = naive(Ymd { year: 2021, month: 1, day: 1 }, &midnight).unwrap();
    let to = naive(Ymd { year: 2027, month: 1, day: 1 }, &midnight).unwrap();
    let cases: [(Unit, u32, i128, &[M]); 5] = [
        (Unit::Day, 1, 0, &MODES),
        (Unit::Hour, 1, HOUR, &MODES),
        (Unit::Hour, 6, 6 * HOUR, &[M::HalfExpand, M::Floor]),
        (Unit::Minute, 15, 15 * MINUTE, &[M::HalfEven, M::Ceil]),
        (Unit::Second, 30, 30_000_000_000, &[M::HalfTrunc]),
    ];
    let (mut checked, mut wrong) = (0u64, vec![]);
    for id in ZONES {
        let tz = temporal_rs::TimeZone::try_from_identifier_str(id).unwrap();
        // Every 25 hours, so the hour of day walks round; then each transition's
        // two days either side by quarter-hour, and its six hours off the quarter.
        let mut instants: Vec<i128> = (0..).map(|k| from + k * 25 * HOUR).take_while(|i| *i < to).collect();
        for t in transitions(tz, from, to) {
            instants.extend((-192..=192i128).map(|k| t + k * 15 * MINUTE));
            instants.extend((-24..=24i128).map(|k| t + k * 15 * MINUTE + 7 * MINUTE + 29_500_000_000));
        }
        for i in instants {
            let base = ZonedDateTime::try_new_with_provider(i, tz, Calendar::ISO, &EXACT).unwrap();
            for (unit, inc, step, modes) in cases {
                for &mode in modes {
                    checked += 1;
                    let want = if unit == Unit::Day { want_day(i, tz, mode) } else { want_time(i, tz, step, mode) };
                    let mut options = RoundingOptions::default();
                    options.smallest_unit = Some(unit);
                    options.rounding_mode = Some(mode);
                    options.increment = Some(RoundingIncrement::try_new(inc).unwrap());
                    let got = base.round_with_provider(options, &EXACT).ok().map(|z| z.epoch_nanoseconds().as_i128());
                    if got != want && wrong.len() < 12 {
                        wrong.push(format!("{id} {i} {unit:?}/{inc} {mode:?}: host {got:?}, oracle {want:?}"));
                    }
                }
            }
        }
    }
    eprintln!("round: {checked} roundings checked");
    assert!(wrong.is_empty(), "round disagrees with the oracle:\n{}", wrong.join("\n"));
}

/// Near every transition of five eras, in every zone, every 4 hours: a day in
/// four modes, an hour in two, 15 minutes half-even. TC39 asserts an instant
/// lies before the start of the next date, and where a zone breaks that — the
/// clock reads the next date briefly, then returns to this one (a transition
/// at 00:01 local: Creston 1943-44, Newfoundland, Moncton, Goose Bay 1995-2000) — the spec gives no answer, so those instants are counted and
/// skipped rather than compared.
#[test]
fn should_round_near_transitions_in_every_zone() {
    let cases: [(Unit, u32, i128, &[M]); 3] = [
        (Unit::Day, 1, 0, &[M::Ceil, M::Floor, M::HalfExpand, M::HalfEven]),
        (Unit::Hour, 1, HOUR, &[M::HalfExpand, M::Floor]),
        (Unit::Minute, 15, 15 * MINUTE, &[M::HalfEven]),
    ];
    let (mut checked, mut undefined, mut wrong) = (0u64, 0u64, vec![]);
    for (id, tz) in temporal_sweeps::zones() {
        for i in temporal_sweeps::near_era_sample(tz, 2) {
            let Ok(base) = ZonedDateTime::try_new_with_provider(i, tz, Calendar::ISO, &EXACT) else { continue };
            let next_day_starts_after = local_date(i, tz).and_then(|d| first_instant(d.next()?, tz)).is_some_and(|end| end > i);
            for (unit, inc, step, modes) in cases {
                if unit == Unit::Day && !next_day_starts_after {
                    undefined += 1;
                    continue;
                }
                for &mode in modes {
                    checked += 1;
                    let want = if unit == Unit::Day { want_day(i, tz, mode) } else { want_time(i, tz, step, mode) };
                    let mut options = RoundingOptions::default();
                    options.smallest_unit = Some(unit);
                    options.rounding_mode = Some(mode);
                    options.increment = Some(RoundingIncrement::try_new(inc).unwrap());
                    let got = base.round_with_provider(options, &EXACT).ok().map(|z| z.epoch_nanoseconds().as_i128());
                    if got != want && wrong.len() < 12 {
                        wrong.push(format!("{id} {i} {unit:?}/{inc} {mode:?}: host {got:?}, oracle {want:?}"));
                    }
                }
            }
        }
    }
    eprintln!("round, every zone: {checked} roundings checked, {undefined} instants outside the spec's day precondition");
    assert!(wrong.is_empty(), "round disagrees with the oracle:\n{}", wrong.join("\n"));
}
