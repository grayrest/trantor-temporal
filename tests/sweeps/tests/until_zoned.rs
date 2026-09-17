//! A zoned difference in days, weeks, months and years, through the host's
//! provider, against TC39's definition built from the oracle. Pairs start
//! every two hours through the two days either side of each transition of
//! 2024-2026 in the sweep's zones, and end a day (give or take the hour a
//! transition moves), two days, a month and a year either side.
use temporal_rs::options::{DifferenceSettings, Unit};
use temporal_rs::{Calendar, Duration, PlainTime, ZonedDateTime};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::{naive, transitions, zoned_difference, HOUR};
use temporal_sweeps::oracle::Ymd;
use temporal_sweeps::ZONES;

const MINUTE: i128 = 60_000_000_000;
const DAY: i128 = 24 * HOUR;

fn fields(d: &Duration) -> [i128; 10] {
    [d.years().into(), d.months().into(), d.weeks().into(), d.days().into(), d.hours().into(), d.minutes().into(),
     d.seconds().into(), d.milliseconds().into(), d.microseconds(), d.nanoseconds()]
}

#[test]
fn should_match_the_oracle_in_every_date_unit_across_transitions() {
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    let from = naive(Ymd { year: 2024, month: 1, day: 1 }, &midnight).unwrap();
    let to = naive(Ymd { year: 2027, month: 1, day: 1 }, &midnight).unwrap();
    let spans = [23 * HOUR, DAY, 25 * HOUR, 47 * HOUR + 30 * MINUTE, 30 * DAY, 366 * DAY];
    let (mut checked, mut wrong) = (0u64, vec![]);
    for id in ZONES {
        let tz = temporal_rs::TimeZone::try_from_identifier_str(id).unwrap();
        for t in transitions(tz, from, to) {
            for k in (-48..=48i128).step_by(2) {
                let a = t + k * HOUR + 17 * MINUTE;
                for span in spans {
                    for b in [a + span, a - span] {
                        for (x, y) in [(a, b), (b, a)] {
                            let zx = ZonedDateTime::try_new_with_provider(x, tz, Calendar::ISO, &EXACT).unwrap();
                            let zy = ZonedDateTime::try_new_with_provider(y, tz, Calendar::ISO, &EXACT).unwrap();
                            for unit in [Unit::Day, Unit::Week, Unit::Month, Unit::Year] {
                                checked += 1;
                                let mut settings = DifferenceSettings::default();
                                settings.largest_unit = Some(unit);
                                let got = zx.until_with_provider(&zy, settings, &EXACT).ok().map(|d| fields(&d));
                                let want = zoned_difference(x, y, tz, unit);
                                if got != want && wrong.len() < 12 {
                                    wrong.push(format!("{id} {x} until {y} {unit:?}: host {got:?}, oracle {want:?}"));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!("until: {checked} differences checked");
    assert!(wrong.is_empty(), "the zoned difference disagrees with the oracle:\n{}", wrong.join("\n"));
}

/// Near every transition of five eras, in every zone, every 4 hours: a day and
/// a month apart, either way, in days and in months.
#[test]
fn should_match_the_oracle_near_transitions_in_every_zone() {
    let (mut checked, mut wrong) = (0u64, vec![]);
    for (id, tz) in temporal_sweeps::zones() {
        for a in temporal_sweeps::near_era_sample(tz, 2) {
            for b in [a + 23 * HOUR, a - 25 * HOUR, a + 31 * DAY] {
                for (x, y) in [(a, b), (b, a)] {
                    let (Ok(zx), Ok(zy)) = (
                        ZonedDateTime::try_new_with_provider(x, tz, Calendar::ISO, &EXACT),
                        ZonedDateTime::try_new_with_provider(y, tz, Calendar::ISO, &EXACT),
                    ) else { continue };
                    for unit in [Unit::Day, Unit::Month] {
                        checked += 1;
                        let mut settings = DifferenceSettings::default();
                        settings.largest_unit = Some(unit);
                        let got = zx.until_with_provider(&zy, settings, &EXACT).ok().map(|d| fields(&d));
                        let want = zoned_difference(x, y, tz, unit);
                        if got != want && wrong.len() < 12 {
                            wrong.push(format!("{id} {x} until {y} {unit:?}: host {got:?}, oracle {want:?}"));
                        }
                    }
                }
            }
        }
    }
    eprintln!("until, every zone: {checked} differences checked");
    assert!(wrong.is_empty(), "the zoned difference disagrees with the oracle:\n{}", wrong.join("\n"));
}
