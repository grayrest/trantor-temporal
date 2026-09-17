//! Zoned arithmetic by the spec's definition: step the DATE in wall-clock
//! time, resolve that wall clock `Compatible`, then add the time part as exact
//! nanoseconds. The oracle resolves with its own transition walk, so a wrong
//! provider cannot make this agree with itself.
use temporal_sweeps::oracle::{naive, resolve, HOUR};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::Ymd;
use temporal_rs::options::{Disambiguation, Overflow};
use temporal_rs::{Calendar, Duration, PlainTime, TimeZone, ZonedDateTime};

use temporal_sweeps::{days_in, ZONES};

#[test]
fn should_keep_the_wall_clock_across_a_day_step_and_add_time_exactly() {
    let steps: [(i64, i128); 4] = [(1, 0), (-1, 0), (1, 2), (0, 5)]; // (days, hours)
    let (mut checked, mut wrong) = (0u64, vec![]);
    for id in ZONES {
        let tz = TimeZone::try_from_identifier_str(id).unwrap();
        for year in [2021, 2024, 2026] {
            for month in 1..=12u8 {
                for day in 1..=days_in(month, year) {
                    for hour in 0..24u8 {
                        let d = Ymd { year, month, day };
                        let t = PlainTime::try_new(hour, 30, 0, 0, 0, 0).unwrap();
                        // Only wall clocks that name exactly one instant make a base.
                        let w = naive(d, &t).unwrap();
                        let (Some(e), Some(l)) = (resolve(w, tz, Disambiguation::Earlier), resolve(w, tz, Disambiguation::Later)) else { continue };
                        if e != l { continue }
                        let base = ZonedDateTime::try_new_with_provider(e, tz, Calendar::ISO, &EXACT).unwrap();
                        for (days, hours) in steps {
                            checked += 1;
                            let dur = Duration::new(0, 0, 0, days, hours as i64, 0, 0, 0, 0, 0).unwrap();
                            let (sy, sm, sd) = temporal_sweeps::oracle_dates::add((year, month, day), 0, 0, 0, days, false).unwrap();
                            let want = naive(Ymd { year: sy, month: sm, day: sd }, &t)
                                .and_then(|sw| resolve(sw, tz, Disambiguation::Compatible))
                                .map(|i| i + hours * HOUR);
                            let got = base.add_with_provider(&dur, Some(Overflow::Constrain), &EXACT).ok().map(|z| z.epoch_nanoseconds().as_i128());
                            if got != want && wrong.len() < 12 {
                                wrong.push(format!("{id} {d:?} {hour:02}:30 +P{days}DT{hours}H: host {got:?}, oracle {want:?}"));
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!("add: {checked} operations checked");
    assert!(wrong.is_empty(), "add disagrees with the oracle:\n{}", wrong.join("\n"));
}

/// The same definition from instants near every transition of five eras, in
/// every zone.
#[test]
fn should_add_by_the_definition_near_transitions_in_every_zone() {
    let steps: [(i64, i128); 4] = [(1, 0), (-1, 0), (1, 2), (0, 5)];
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    let (mut checked, mut wrong) = (0u64, vec![]);
    for (id, tz) in temporal_sweeps::zones() {
        for i in temporal_sweeps::near_era_transitions(tz) {
            let Ok(base) = ZonedDateTime::try_new_with_provider(i, tz, Calendar::ISO, &EXACT) else { continue };
            let Some(local) = temporal_sweeps::oracle::local_date(i, tz) else { continue };
            let tod = i + temporal_sweeps::oracle::offset_at(i, tz).unwrap() - naive(local, &PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap()).unwrap();
            for (days, hours) in steps {
                checked += 1;
                let dur = Duration::new(0, 0, 0, days, hours as i64, 0, 0, 0, 0, 0).unwrap();
                let (sy, sm, sd) = temporal_sweeps::oracle_dates::add((local.year, local.month, local.day), 0, 0, 0, days, false).unwrap();
                // No date part: TC39 adds exact time to the instant itself, so a
                // base on the later side of an overlap stays there.
                let want = if days == 0 {
                    Some(i + hours * HOUR)
                } else {
                    naive(Ymd { year: sy, month: sm, day: sd }, &midnight)
                        .map(|n| n + tod)
                        .and_then(|sw| resolve(sw, tz, Disambiguation::Compatible))
                        .map(|r| r + hours * HOUR)
                };
                let got = base.add_with_provider(&dur, Some(Overflow::Constrain), &EXACT).ok().map(|z| z.epoch_nanoseconds().as_i128());
                if got != want && wrong.len() < 12 {
                    wrong.push(format!("{id} {i} +P{days}DT{hours}H: host {got:?}, oracle {want:?}"));
                }
            }
        }
    }
    eprintln!("add, every zone: {checked} operations checked");
    assert!(wrong.is_empty(), "add disagrees with the oracle:\n{}", wrong.join("\n"));
}
