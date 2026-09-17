//! Wall clocks at the exact edges of every gap and overlap: on each side of
//! every transition of 1800-2100 in every zone, at the edge and a nanosecond,
//! a second and an hour either way, plus the middle of the following offset
//! period, in all four modes. The other sweeps sample whole seconds minutes
//! away from any transition, so a provider reading back one second off, or
//! truncating sub-second wall clocks, passed all of them.
use temporal_rs::options::Disambiguation;
use temporal_rs::{Calendar, PlainTime, TimeZone, ZonedDateTime};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::{naive, offset_at, resolve, transitions, Ymd, HOUR, SECOND};
use temporal_sweeps::zones;

/// A wall clock (read as UTC) resolved in `tz` through the host's provider.
fn host(wall: i128, tz: TimeZone, utc: TimeZone, dis: Disambiguation) -> Option<i128> {
    let local = ZonedDateTime::try_new(wall, utc, Calendar::ISO).ok()?.to_plain_date_time();
    local.to_zoned_date_time_with_provider(tz, dis, &EXACT).ok().map(|z| z.epoch_nanoseconds().as_i128())
}

#[test]
fn should_resolve_the_edges_of_every_gap_and_overlap() {
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    let lo = naive(Ymd { year: 1800, month: 1, day: 1 }, &midnight).unwrap();
    let hi = naive(Ymd { year: 2100, month: 1, day: 1 }, &midnight).unwrap();
    let utc = TimeZone::try_from_identifier_str("UTC").unwrap();
    let modes = [Disambiguation::Compatible, Disambiguation::Earlier, Disambiguation::Later, Disambiguation::Reject];
    let (mut checked, mut wrong) = (0u64, vec![]);
    for (id, tz) in zones() {
        let ts = transitions(tz, lo, hi);
        for (k, &t) in ts.iter().enumerate() {
            let (Some(before), Some(after)) = (offset_at(t - 1, tz), offset_at(t, tz)) else { continue };
            let mut walls: Vec<i128> = [t + before, t + after]
                .iter()
                .flat_map(|edge| [-HOUR, -SECOND, -1, 0, 1, SECOND, HOUR].map(|d| edge + d))
                .collect();
            if let Some(next) = ts.get(k + 1) {
                walls.push(t + after + (next - t) / 2);
            }
            for wall in walls {
                for dis in modes {
                    checked += 1;
                    let (got, want) = (host(wall, tz, utc, dis), resolve(wall, tz, dis));
                    if got != want && wrong.len() < 12 {
                        wrong.push(format!("{id} wall {wall} {dis:?}: host {got:?}, oracle {want:?}"));
                    }
                }
            }
        }
    }
    eprintln!("boundaries: {checked} resolutions checked");
    assert!(wrong.is_empty(), "resolution at a transition's edge disagrees with the oracle:\n{}", wrong.join("\n"));
}
