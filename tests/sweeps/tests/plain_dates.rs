//! Plain-date arithmetic through the host's own code (plain_dates.rs) against
//! ISO date arithmetic transcribed from the spec in oracle_dates.rs, which
//! computes on its own day numbers. Dates: every month end and its neighbours
//! across years that exercise the leap and century rules and both range limits,
//! and a date every 997 days from 1600 to 2400. Checks: add with constrain and
//! reject (weeks, days and time balancing into days included), until in every
//! date unit, rounded until, and IXDTF parsing and its refusals.
use temporal_rs::options::{DifferenceSettings, Overflow, RoundingIncrement, RoundingMode as M, Unit};
use temporal_rs::{Calendar, Duration};
use temporal_sweeps::oracle_dates::{add, day_number, days_in_month, from_day_number, until, within_limits, Iso};
use temporal_sweeps::oracle_plain_round::plain_until;
use temporal_sweeps::plain_dates;

const DAY_NS: i128 = 86_400_000_000_000;
const ALL: [M; 9] = [M::Ceil, M::Floor, M::Expand, M::Trunc, M::HalfCeil, M::HalfFloor, M::HalfExpand, M::HalfTrunc, M::HalfEven];
const SOME: [M; 3] = [M::Trunc, M::HalfExpand, M::Ceil];

fn dates() -> Vec<Iso> {
    let mut out = vec![];
    for y in [-271_821i64, -271_820, -1, 0, 1, 1582, 1899, 1900, 1970, 2000, 2023, 2024, 2100, 2400, 275_759, 275_760] {
        for m in 1..=12 {
            for d in [1, 15, 28, 29, 30, 31] {
                if d <= days_in_month(y, m) && within_limits(y, m, d) {
                    out.push((y as i32, m as u8, d as u8));
                }
            }
        }
    }
    let (from, to) = (day_number(1600, 1, 1), day_number(2400, 1, 1));
    out.extend((from..to).step_by(997).map(|n| { let (y, m, d) = from_day_number(n); (y as i32, m as u8, d as u8) }));
    out
}

fn shifted(d: Iso, days: i64) -> Option<Iso> {
    let (y, m, dd) = from_day_number(day_number(d.0.into(), d.1.into(), d.2.into()) + days);
    within_limits(y, m, dd).then_some((y as i32, m as u8, dd as u8))
}

fn iso_str(d: Iso) -> String {
    let year = if (0..=9999).contains(&d.0) { format!("{:04}", d.0) } else { format!("{}{:06}", if d.0 < 0 { '-' } else { '+' }, d.0.abs()) };
    format!("{year}-{:02}-{:02}", d.1, d.2)
}

#[test]
fn should_add_and_difference_dates_as_the_spec_does() {
    let durations: [(i64, i64, i64, i64, i64); 14] = [
        (0, 1, 0, 0, 0), (0, -1, 0, 0, 0), (1, 0, 0, 0, 0), (-1, 0, 0, 0, 0), (0, 13, 0, 0, 0), (0, 1, 0, 1, 0), (0, 0, 1, 0, 0),
        (0, 0, -3, -2, 0), (0, 0, 0, 0, 23), (0, 0, 0, 0, 48), (0, 0, 0, 0, -25), (-1, -1, 0, -1, 0), (0, 0, 0, 146_097, 0), (4, 0, 0, 0, 0),
    ];
    let spans = [1i64, 27, 28, 29, 30, 31, 59, 365, 366, 1461, -30, -400, -3653];
    let rounded: [(Unit, Unit, u32, &[M]); 7] = [
        (Unit::Year, Unit::Month, 1, &ALL), (Unit::Month, Unit::Week, 1, &ALL), (Unit::Year, Unit::Day, 5, &SOME),
        (Unit::Month, Unit::Day, 2, &SOME), (Unit::Week, Unit::Week, 2, &SOME), (Unit::Month, Unit::Month, 3, &SOME), (Unit::Day, Unit::Day, 7, &SOME),
    ];
    let (mut checked, mut wrong) = (0u64, vec![]);
    let mut check = |label: String, got: String, want: String| {
        checked += 1;
        if got != want && wrong.len() < 16 {
            wrong.push(format!("{label}: host {got}, oracle {want}"));
        }
    };
    let all = dates();
    for &a in &all {
        for (y, m, w, d, h) in durations {
            let dur = Duration::new(y, m, w, d, h, 0, 0, 0, 0, 0).unwrap();
            let days = ((i128::from(d) * DAY_NS + i128::from(h) * 3_600_000_000_000) / DAY_NS) as i64;
            for (overflow, reject) in [(Overflow::Constrain, false), (Overflow::Reject, true)] {
                let got = plain_dates::add(a, Calendar::ISO, &dur, overflow).ok();
                check(format!("{a:?} + P{y}Y{m}M{w}W{d}DT{h}H {overflow:?}"), format!("{got:?}"), format!("{:?}", add(a, y, m, w, days, reject)));
            }
        }
        let mut others: Vec<Iso> = spans.iter().filter_map(|s| shifted(a, *s)).collect();
        for months in [1i64, 2, 13] {
            let (y, m) = ((i64::from(a.0) * 12 + i64::from(a.1) - 1 + months).div_euclid(12), (i64::from(a.0) * 12 + i64::from(a.1) - 1 + months).rem_euclid(12) + 1);
            if within_limits(y, m, days_in_month(y, m)) {
                others.push((y as i32, m as u8, days_in_month(y, m) as u8));
            }
        }
        for b in others {
            for (unit, code) in [(Unit::Day, 0u8), (Unit::Week, 1), (Unit::Month, 2), (Unit::Year, 3)] {
                let mut s = DifferenceSettings::default();
                s.largest_unit = Some(unit);
                let got = plain_dates::until(a, b, Calendar::ISO, s).ok().map(|x| [x.years(), x.months(), x.weeks(), x.days()]);
                check(format!("{a:?} until {b:?} {unit:?}"), format!("{got:?}"), format!("{:?}", Some(until(a, b, code))));
            }
            for (largest, smallest, increment, modes) in rounded {
                for &mode in modes {
                    let mut s = DifferenceSettings::default();
                    s.largest_unit = Some(largest);
                    s.smallest_unit = Some(smallest);
                    s.rounding_mode = Some(mode);
                    s.increment = Some(RoundingIncrement::try_new(increment).unwrap());
                    let got = plain_dates::until(a, b, Calendar::ISO, s).ok().map(|x| [x.years(), x.months(), x.weeks(), x.days()]);
                    check(format!("{a:?} until {b:?} {largest:?}/{smallest:?}/{increment} {mode:?}"), format!("{got:?}"), format!("{:?}", plain_until(a, b, largest, smallest, increment.into(), mode)));
                }
            }
        }
        let text = iso_str(a);
        check(format!("parse {text}"), format!("{:?}", plain_dates::parse(&text).ok().map(|(d, c)| (d, c.identifier()))), format!("{:?}", Some((a, "iso8601"))));
        check(format!("parse {text}[u-ca=hebrew]"), format!("{:?}", plain_dates::parse(&format!("{text}[u-ca=hebrew]")).ok().map(|(d, c)| (d, c.identifier()))), format!("{:?}", Some((a, "hebrew"))));
        if (1..=9999).contains(&a.0) {
            check(format!("parse {text}T12:34:56.789"), format!("{:?}", plain_dates::parse(&format!("{text}T12:34:56.789")).ok().map(|(d, _)| d)), format!("{:?}", Some(a)));
            check(format!("parse {text}T12:00Z refused"), plain_dates::parse(&format!("{text}T12:00Z")).is_err().to_string(), "true".into());
        }
    }
    for bad in ["2026-02-30", "2025-02-29", "2026-13-01", "2026-00-10", "2026-04-31", "-271821-04-18", "+275760-09-14", "20260308x"] {
        check(format!("parse {bad} refused"), plain_dates::parse(bad).is_err().to_string(), "true".into());
    }
    eprintln!("plain dates: {checked} checked over {} dates", all.len());
    assert!(wrong.is_empty(), "plain-date arithmetic disagrees with the oracle:\n{}", wrong.join("\n"));
}
