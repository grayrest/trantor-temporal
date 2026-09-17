//! Rounded zoned differences through the host's provider, against the spec's
//! RoundRelativeDuration transcribed in `oracle_round.rs`. Pairs start every
//! six hours through the two days either side of each transition of 2024-2026
//! in the sweep's zones, and end 23 hours to 400 days away in both directions.
use temporal_rs::options::{DifferenceSettings, RoundingIncrement, RoundingMode as M, Unit};
use temporal_rs::{Calendar, Duration, PlainTime, ZonedDateTime};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::{naive, transitions, Ymd, HOUR};
use temporal_sweeps::oracle_round::until_rounded;
use temporal_sweeps::ZONES;

const MINUTE: i128 = 60_000_000_000;
const DAY: i128 = 24 * HOUR;
const ALL: [M; 9] = [M::Ceil, M::Floor, M::Expand, M::Trunc, M::HalfCeil, M::HalfFloor, M::HalfExpand, M::HalfTrunc, M::HalfEven];
const SOME: [M; 3] = [M::Trunc, M::HalfExpand, M::Ceil];

fn fields(d: &Duration) -> [i128; 10] {
    [d.years().into(), d.months().into(), d.weeks().into(), d.days().into(), d.hours().into(), d.minutes().into(),
     d.seconds().into(), d.milliseconds().into(), d.microseconds(), d.nanoseconds()]
}

#[test]
fn should_round_a_zoned_difference_as_the_spec_does() {
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    let from = naive(Ymd { year: 2024, month: 1, day: 1 }, &midnight).unwrap();
    let to = naive(Ymd { year: 2027, month: 1, day: 1 }, &midnight).unwrap();
    // (largest, smallest, increment, modes)
    let cases: [(Unit, Unit, u32, &[M]); 11] = [
        (Unit::Month, Unit::Day, 1, &ALL),
        (Unit::Day, Unit::Hour, 1, &ALL),
        (Unit::Year, Unit::Month, 1, &SOME),
        (Unit::Year, Unit::Day, 1, &SOME),
        (Unit::Month, Unit::Week, 1, &SOME),
        (Unit::Week, Unit::Day, 1, &SOME),
        (Unit::Day, Unit::Day, 2, &SOME),
        (Unit::Month, Unit::Month, 3, &SOME),
        (Unit::Month, Unit::Hour, 1, &SOME),
        (Unit::Day, Unit::Minute, 15, &SOME),
        (Unit::Hour, Unit::Minute, 15, &SOME),
    ];
    let spans = [23 * HOUR, 25 * HOUR + 40 * MINUTE, 3 * DAY + 7 * HOUR, 30 * DAY + 11 * HOUR, 45 * DAY + 13 * HOUR, 400 * DAY];
    let (mut checked, mut wrong) = (0u64, vec![]);
    let mut by_case = std::collections::BTreeMap::new();
    for id in ZONES {
        let tz = temporal_rs::TimeZone::try_from_identifier_str(id).unwrap();
        for t in transitions(tz, from, to) {
            for k in -8..=8i128 {
                let a = t + k * 6 * HOUR + 17 * MINUTE;
                for span in spans {
                    for b in [a + span, a - span] {
                        let (Ok(za), Ok(zb)) = (
                            ZonedDateTime::try_new_with_provider(a, tz, Calendar::ISO, &EXACT),
                            ZonedDateTime::try_new_with_provider(b, tz, Calendar::ISO, &EXACT),
                        ) else { continue };
                        for (largest, smallest, increment, modes) in cases {
                            for &mode in modes {
                                checked += 1;
                                let mut settings = DifferenceSettings::default();
                                settings.largest_unit = Some(largest);
                                settings.smallest_unit = Some(smallest);
                                settings.rounding_mode = Some(mode);
                                settings.increment = Some(RoundingIncrement::try_new(increment).unwrap());
                                let got = za.until_with_provider(&zb, settings, &EXACT).ok().map(|d| fields(&d));
                                let want = until_rounded(a, b, tz, largest, smallest, increment.into(), mode);
                                if got != want {
                                    *by_case.entry(format!("{largest:?}/{smallest:?}/{increment} {mode:?}")).or_insert(0u64) += 1;
                                }
                                if got != want && wrong.len() < 12 {
                                    wrong.push(format!("{id} {a} until {b} {largest:?}/{smallest:?}/{increment} {mode:?}: host {got:?}, oracle {want:?}"));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!("until rounded: {checked} differences checked");
    for (case, n) in &by_case { eprintln!("  {case}: {n} wrong"); }
    assert!(wrong.is_empty(), "the rounded difference disagrees with the oracle:\n{}", wrong.join("\n"));
}

/// The same oracle in every zone, every 16 hours near each transition of five
/// eras, a day, a month and a year away, in four settings.
#[test]
fn should_round_a_zoned_difference_in_every_zone() {
    let cases: [(Unit, Unit, u32, M); 4] = [(Unit::Month, Unit::Day, 1, M::HalfExpand), (Unit::Day, Unit::Hour, 1, M::Floor), (Unit::Year, Unit::Month, 1, M::Ceil), (Unit::Month, Unit::Week, 1, M::HalfEven)];
    let (mut checked, mut wrong) = (0u64, vec![]);
    for (id, tz) in temporal_sweeps::zones() {
        for a in temporal_sweeps::near_era_sample(tz, 8) {
            for b in [a + 25 * HOUR, a - 31 * DAY - 7 * HOUR, a + 366 * DAY] {
                let (Ok(za), Ok(zb)) = (
                    ZonedDateTime::try_new_with_provider(a, tz, Calendar::ISO, &EXACT),
                    ZonedDateTime::try_new_with_provider(b, tz, Calendar::ISO, &EXACT),
                ) else { continue };
                for (largest, smallest, increment, mode) in cases {
                    checked += 1;
                    let mut settings = DifferenceSettings::default();
                    settings.largest_unit = Some(largest);
                    settings.smallest_unit = Some(smallest);
                    settings.rounding_mode = Some(mode);
                    settings.increment = Some(RoundingIncrement::try_new(increment).unwrap());
                    let got = za.until_with_provider(&zb, settings, &EXACT).ok().map(|d| fields(&d));
                    let want = until_rounded(a, b, tz, largest, smallest, increment.into(), mode);
                    if got != want && wrong.len() < 12 {
                        wrong.push(format!("{id} {a} until {b} {largest:?}/{smallest:?} {mode:?}: host {got:?}, oracle {want:?}"));
                    }
                }
            }
        }
    }
    eprintln!("until rounded, every zone: {checked} differences checked");
    assert!(wrong.is_empty(), "the rounded difference disagrees with the oracle:\n{}", wrong.join("\n"));
}
