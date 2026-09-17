//! Durations rounded, totalled and compared through the host's own code
//! (durations.rs) against the spec transcribed in oracle_durations.rs: measured
//! from month ends and their neighbours across leap and century years, and with
//! no date where the duration has no calendar units. A total is compared
//! exactly where the spec's quotient has a numerator and denominator a double
//! holds exactly, and to 15 significant digits otherwise.
use temporal_rs::options::{RoundingIncrement, RoundingMode as M, RoundingOptions, Unit};
use temporal_rs::{Calendar, Duration};
use temporal_sweeps::durations;
use temporal_sweeps::oracle_dates::{days_in_month, within_limits, Iso};
use temporal_sweeps::oracle_durations::{compare, round, total, Fields};
use temporal_sweeps::oracle_round::Reckoning;

const ALL: [M; 9] = [M::Ceil, M::Floor, M::Expand, M::Trunc, M::HalfCeil, M::HalfFloor, M::HalfExpand, M::HalfTrunc, M::HalfEven];
const SOME: [M; 3] = [M::Trunc, M::HalfExpand, M::Ceil];
const EXACT_IN_F64: i128 = 1 << 53;

fn duration(f: &Fields) -> Duration {
    Duration::new(f[0] as i64, f[1] as i64, f[2] as i64, f[3] as i64, f[4] as i64, f[5] as i64, f[6] as i64, f[7] as i64, f[8], f[9]).unwrap()
}

fn fields(d: &Duration) -> Fields {
    [d.years().into(), d.months().into(), d.weeks().into(), d.days().into(), d.hours().into(), d.minutes().into(),
     d.seconds().into(), d.milliseconds().into(), d.microseconds(), d.nanoseconds()]
}

#[test]
fn should_round_total_and_compare_durations_as_the_spec_does() {
    let samples: [Fields; 18] = [
        [0, 1, 0, 0, 0, 0, 0, 0, 0, 0], [0, 1, 0, 15, 0, 0, 0, 0, 0, 0], [0, 2, 0, 29, 12, 0, 0, 0, 0, 0],
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10], [0, 0, 0, 0, 36, 0, 0, 0, 0, 0], [0, 0, 0, 0, -49, -30, 0, 0, 0, 0],
        [0, -1, 0, -2, 0, 0, 0, 0, 0, 0], [0, 0, 0, 14, 0, 0, 0, 0, 0, 0], [0, 0, 3, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 1], [0, 0, 0, 400, 0, 0, 0, 0, 0, 0], [0, 0, 0, 0, 1000, 0, 0, 0, 0, 0],
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0], [-1, -6, 0, 0, 0, 0, 0, 0, 0, 0], [0, 0, 0, 0, 23, 59, 59, 999, 999, 999],
        [0, 0, 0, 29, 0, 0, 0, 0, 0, 0], [0, 0, 0, 30, 12, 0, 0, 0, 0, 0], [0, 0, 1, 1, 1, 0, 0, 0, 0, 0],
    ];
    // Smallest nanosecond: the balanced difference itself, unrounded, which is
    // where DifferenceISODateTime's sign adjustment shows.
    let rounds: [(Unit, Unit, u32, &[M]); 11] = [
        (Unit::Day, Unit::Nanosecond, 1, &[M::Trunc]), (Unit::Month, Unit::Nanosecond, 1, &[M::Trunc]),
        (Unit::Year, Unit::Month, 1, &ALL), (Unit::Day, Unit::Hour, 1, &ALL), (Unit::Month, Unit::Day, 1, &SOME),
        (Unit::Month, Unit::Week, 1, &SOME), (Unit::Week, Unit::Day, 2, &SOME), (Unit::Hour, Unit::Minute, 15, &SOME),
        (Unit::Year, Unit::Day, 1, &SOME), (Unit::Month, Unit::Month, 3, &SOME), (Unit::Day, Unit::Day, 1, &SOME),
    ];
    let units = [Unit::Year, Unit::Month, Unit::Week, Unit::Day, Unit::Hour, Unit::Minute, Unit::Second, Unit::Nanosecond];
    let mut anchors: Vec<Option<Iso>> = vec![None];
    for y in [1900i64, 2000, 2023, 2024, 2100] {
        for m in 1..=12 {
            for d in [1, 28, 29, 30, 31] {
                if d <= days_in_month(y, m) { anchors.push(Some((y as i32, m as u8, d as u8))); }
            }
        }
    }
    for edge in [(-271_821, 4, 20), (-271_821, 5, 31), (275_760, 8, 13), (275_760, 9, 12)] {
        if within_limits(edge.0, edge.1, edge.2) { anchors.push(Some((edge.0 as i32, edge.1 as u8, edge.2 as u8))); }
    }
    let (mut checked, mut approximate, mut wrong) = (0u64, 0u64, vec![]);
    let mut check = |label: String, got: String, want: String| {
        checked += 1;
        if got != want && wrong.len() < 16 {
            wrong.push(format!("{label}: host {got}, oracle {want}"));
        }
    };
    for from in &anchors {
        for (k, f) in samples.iter().enumerate() {
            let d = duration(f);
            for (largest, smallest, increment, modes) in rounds {
                for &mode in modes {
                    let mut o = RoundingOptions::default();
                    o.largest_unit = Some(largest);
                    o.smallest_unit = Some(smallest);
                    o.rounding_mode = Some(mode);
                    o.increment = Some(RoundingIncrement::try_new(increment).unwrap());
                    let got = durations::round(&d, o, *from, Calendar::ISO).ok().map(|r| fields(&r));
                    check(format!("{f:?} round {largest:?}/{smallest:?}/{increment} {mode:?} from {from:?}"), format!("{got:?}"), format!("{:?}", round(f, largest, smallest, increment.into(), mode, *from, Reckoning::Iso)));
                }
            }
            for unit in units {
                let got = durations::total(&d, unit, *from, Calendar::ISO).ok();
                let want = total(f, unit, *from, Reckoning::Iso);
                match (got, want) {
                    (Some(g), Some((n, q))) if n.abs() <= EXACT_IN_F64 && q <= EXACT_IN_F64 => {
                        check(format!("{f:?} total {unit:?} from {from:?}"), format!("{g:?}"), format!("{:?}", n as f64 / q as f64));
                    }
                    (Some(g), Some((n, q))) => {
                        approximate += 1;
                        let w = n as f64 / q as f64;
                        let close = (g - w).abs() <= w.abs() * 1e-15;
                        check(format!("{f:?} total {unit:?} from {from:?} to 15 digits ({g} vs {w})"), close.to_string(), "true".into());
                    }
                    (g, w) => check(format!("{f:?} total {unit:?} from {from:?}"), format!("{:?}", g.is_some()), format!("{:?}", w.is_some())),
                }
            }
            for other in [&samples[(k + 1) % samples.len()], &[0, 0, 0, 30, 0, 0, 0, 0, 0, 0], &[0, 1, 0, 0, 0, 0, 0, 0, 0, 0]] {
                let got = durations::compare(&d, &duration(other), *from, Calendar::ISO).ok().map(|o| o as i8);
                check(format!("{f:?} compare {other:?} from {from:?}"), format!("{got:?}"), format!("{:?}", compare(f, other, *from, Reckoning::Iso)));
            }
        }
    }
    eprintln!("durations: {checked} checked, {approximate} totals to 15 digits, over {} anchors", anchors.len());
    assert!(wrong.is_empty(), "duration rounding, totals or comparison disagree with the oracle:\n{}", wrong.join("\n"));
}
