//! `since` with rounding, as the package computes it: TC39's
//! DifferenceTemporalPlainDate and DifferenceTemporalZonedDateTime measure from
//! the receiver either way, rounding a `since` with the mode negated
//! (GetNegatedRoundingMode, written out below) and negating the result. That
//! formula over the host's `until` is compared with temporal_rs's own `since`,
//! a separate path, and with the oracles' `until` put through the same formula,
//! for dates across leap and century years and zoned values near transitions.
use temporal_rs::options::{DifferenceSettings, RoundingIncrement, RoundingMode as M, Unit};
use temporal_rs::{Calendar, Duration, PlainDate, PlainTime, ZonedDateTime};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::{naive, transitions, Ymd, HOUR};
use temporal_sweeps::oracle_dates::{day_number, days_in_month, from_day_number, within_limits, Iso};
use temporal_sweeps::oracle_plain_round::plain_until;
use temporal_sweeps::oracle_round::until_rounded;
use temporal_sweeps::{plain_dates, ZONES};

const MINUTE: i128 = 60_000_000_000;
const DAY: i128 = 24 * HOUR;
const ALL: [M; 9] = [M::Ceil, M::Floor, M::Expand, M::Trunc, M::HalfCeil, M::HalfFloor, M::HalfExpand, M::HalfTrunc, M::HalfEven];

/// GetNegatedRoundingMode.
fn negated(mode: M) -> M {
    match mode {
        M::Ceil => M::Floor,
        M::Floor => M::Ceil,
        M::HalfCeil => M::HalfFloor,
        M::HalfFloor => M::HalfCeil,
        other => other,
    }
}

fn settings(largest: Unit, smallest: Unit, increment: u32, mode: M) -> DifferenceSettings {
    let mut s = DifferenceSettings::default();
    s.largest_unit = Some(largest);
    s.smallest_unit = Some(smallest);
    s.rounding_mode = Some(mode);
    s.increment = Some(RoundingIncrement::try_new(increment).unwrap());
    s
}

fn fields(d: &Duration) -> [i128; 10] {
    [d.years().into(), d.months().into(), d.weeks().into(), d.days().into(), d.hours().into(), d.minutes().into(),
     d.seconds().into(), d.milliseconds().into(), d.microseconds(), d.nanoseconds()]
}

fn minus<const N: usize>(f: [i128; N]) -> [i128; N] { f.map(|x| -x) }

#[test]
fn should_round_a_date_since_as_the_spec_does() {
    let cases: [(Unit, Unit, u32); 7] = [
        (Unit::Year, Unit::Month, 1), (Unit::Year, Unit::Day, 1), (Unit::Month, Unit::Week, 1), (Unit::Month, Unit::Day, 2),
        (Unit::Week, Unit::Week, 2), (Unit::Month, Unit::Month, 3), (Unit::Day, Unit::Day, 7),
    ];
    let mut dates: Vec<Iso> = vec![];
    for y in [-271_820i64, 1899, 1900, 2000, 2023, 2024, 2100, 275_759] {
        for m in 1..=12 {
            for d in [1, 15, 28, 29, 30, 31] {
                if d <= days_in_month(y, m) && within_limits(y, m, d) { dates.push((y as i32, m as u8, d as u8)); }
            }
        }
    }
    let (mut checked, mut wrong) = (0u64, vec![]);
    for &a in &dates {
        for span in [1i64, 17, 29, 30, 31, 45, 59, 365, 366, 400, 1461, -1, -17, -30, -45, -365, -1461] {
            let (y, m, d) = from_day_number(day_number(a.0.into(), a.1.into(), a.2.into()) + span);
            if !within_limits(y, m, d) { continue; }
            let b = (y as i32, m as u8, d as u8);
            let (pa, pb) = (PlainDate::try_new_iso(a.0, a.1, a.2).unwrap(), PlainDate::try_new_iso(b.0, b.1, b.2).unwrap());
            for (largest, smallest, increment) in cases {
                for mode in ALL {
                    checked += 1;
                    let formula = plain_dates::until(a, b, Calendar::ISO, settings(largest, smallest, increment, negated(mode)))
                        .ok().map(|x| minus(fields(&x)));
                    let upstream = pa.since(&pb, settings(largest, smallest, increment, mode)).ok().map(|x| fields(&x));
                    let oracle = plain_until(a, b, largest, smallest, increment.into(), negated(mode))
                        .map(|f| minus([f[0], f[1], f[2], f[3], 0, 0, 0, 0, 0, 0].map(i128::from)));
                    if (formula != upstream || formula != oracle) && wrong.len() < 12 {
                        wrong.push(format!("{a:?} since {b:?} {largest:?}/{smallest:?}/{increment} {mode:?}: formula {formula:?}, upstream {upstream:?}, oracle {oracle:?}"));
                    }
                }
            }
        }
    }
    eprintln!("date since rounded: {checked} checked over {} dates", dates.len());
    assert!(wrong.is_empty(), "since disagrees:\n{}", wrong.join("\n"));
}

#[test]
fn should_round_a_zoned_since_as_the_spec_does() {
    let cases: [(Unit, Unit, u32); 5] = [(Unit::Month, Unit::Day, 1), (Unit::Day, Unit::Hour, 1), (Unit::Year, Unit::Month, 1), (Unit::Month, Unit::Week, 1), (Unit::Hour, Unit::Minute, 15)];
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    let from = naive(Ymd { year: 2024, month: 1, day: 1 }, &midnight).unwrap();
    let to = naive(Ymd { year: 2027, month: 1, day: 1 }, &midnight).unwrap();
    let (mut checked, mut wrong) = (0u64, vec![]);
    for id in ZONES {
        let tz = temporal_rs::TimeZone::try_from_identifier_str(id).unwrap();
        for t in transitions(tz, from, to) {
            for k in -4..=4i128 {
                let a = t + k * 6 * HOUR + 17 * MINUTE;
                for span in [23 * HOUR, 25 * HOUR + 40 * MINUTE, 30 * DAY + 11 * HOUR, 400 * DAY] {
                    for b in [a + span, a - span] {
                        let za = ZonedDateTime::try_new_with_provider(a, tz, Calendar::ISO, &EXACT).unwrap();
                        let zb = ZonedDateTime::try_new_with_provider(b, tz, Calendar::ISO, &EXACT).unwrap();
                        for (largest, smallest, increment) in cases {
                            for mode in ALL {
                                checked += 1;
                                let formula = za.until_with_provider(&zb, settings(largest, smallest, increment, negated(mode)), &EXACT)
                                    .ok().map(|x| minus(fields(&x)));
                                let upstream = za.since_with_provider(&zb, settings(largest, smallest, increment, mode), &EXACT).ok().map(|x| fields(&x));
                                let oracle = until_rounded(a, b, tz, largest, smallest, increment.into(), negated(mode)).map(minus);
                                if (formula != upstream || formula != oracle) && wrong.len() < 12 {
                                    wrong.push(format!("{id} {a} since {b} {largest:?}/{smallest:?}/{increment} {mode:?}: formula {formula:?}, upstream {upstream:?}, oracle {oracle:?}"));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!("zoned since rounded: {checked} checked");
    assert!(wrong.is_empty(), "since disagrees:\n{}", wrong.join("\n"));
}
