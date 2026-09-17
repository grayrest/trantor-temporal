//! Resolving a wall clock through the host's provider in all four
//! disambiguation modes, at every hour of every day of three years and of 2045
//! (past the explicit tzif data, where the zone's rule is extrapolated), in zones chosen for having broken something:
//! southern-hemisphere DST, 30- and 45-minute offsets, a zone that skipped a
//! whole day, and the ones where earlier sweeps sampled the wrong hour.
use temporal_sweeps::oracle::{naive, resolve};
use temporal_sweeps::{days_in, ZONES};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::Ymd;
use temporal_rs::options::Disambiguation;
use temporal_rs::{Calendar, PlainDateTime, PlainTime, TimeZone};

#[test]
fn should_match_the_oracle_in_every_mode_at_every_hour() {
    let modes = [Disambiguation::Compatible, Disambiguation::Earlier, Disambiguation::Later, Disambiguation::Reject];
    let (mut checked, mut wrong) = (0u64, vec![]);
    for id in ZONES {
        let tz = TimeZone::try_from_identifier_str(id).unwrap();
        for year in [2021, 2024, 2026, 2045] {
            for month in 1..=12u8 {
                for day in 1..=days_in(month, year) {
                    for hour in 0..24u8 {
                        let d = Ymd { year, month, day };
                        let t = PlainTime::try_new(hour, 30, 0, 0, 0, 0).unwrap();
                        let w = naive(d, &t).unwrap();
                        for dis in modes {
                            checked += 1;
                            let want = resolve(w, tz, dis);
                            let got = PlainDateTime::try_new(year, month, day, hour, 30, 0, 0, 0, 0, Calendar::ISO)
                                .and_then(|w| w.to_zoned_date_time_with_provider(tz, dis, &EXACT))
                                .ok()
                                .map(|z| z.epoch_nanoseconds().as_i128());
                            if got != want && wrong.len() < 12 {
                                wrong.push(format!("{id} {d:?} {hour:02}:30 {dis:?}: host {got:?}, oracle {want:?}"));
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!("resolution: {checked} resolutions checked");
    assert!(wrong.is_empty(), "resolution disagrees with the oracle:\n{}", wrong.join("\n"));
}
