//! The bounds the provider's candidate probing and the oracle's offset scan
//! rely on, checked over the tzdb rather than assumed: no zone's offset reaches
//! the provider's probe window, and no zone keeps an offset for less than the
//! spacing the gap search needs (42 hours, which covers the scan step too).
//! Transitions are listed with temporal_rs's `Previous` lookup
//! (the direction it answers correctly) and must equal the oracle's scan.
use temporal_rs::provider::TransitionDirection;
use temporal_rs::{Calendar, PlainTime, ZonedDateTime};
use temporal_sweeps::oracle::{naive, offset_at, transitions, Ymd, HOUR, SCAN_STEP};
use temporal_sweeps::zones;

/// The provider probes wall clocks ±18 hours every 6 (exact_provider.rs).
const PROBE_REACH: i128 = 18 * HOUR;
const PROBE_STEP: i128 = 6 * HOUR;
/// What the gap search and the oracle's scan need between offset changes.
const MIN_SPACING: i128 = 2 * PROBE_REACH + PROBE_STEP;

fn by_previous(tz: temporal_rs::TimeZone, lo: i128, hi: i128) -> Vec<i128> {
    let mut out = vec![];
    let mut from = hi + 1;
    while let Some(t) = ZonedDateTime::try_new(from, tz, Calendar::ISO)
        .ok()
        .and_then(|z| z.get_time_zone_transition(TransitionDirection::Previous).ok().flatten())
        .map(|z| z.epoch_nanoseconds().as_i128())
    {
        if t >= from || t <= lo {
            break;
        }
        if offset_at(t - 1, tz) != offset_at(t, tz) {
            out.push(t);
        }
        from = t;
    }
    out.reverse();
    out
}

#[test]
fn should_hold_the_bounds_the_provider_and_oracle_assume() {
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    // No zone has a transition before 1800; from 2600 the rules repeat yearly,
    // and the last years before the range's end are checked as that repetition's
    // far end.
    let windows = [(1800, 2600), (275_700, 275_760)];
    const { assert!(PROBE_STEP <= SCAN_STEP) };
    let (mut listed, mut problems) = (0usize, vec![]);
    for (id, tz) in zones() {
        for (first, last) in windows {
            let lo = naive(Ymd { year: first, month: 1, day: 1 }, &midnight).unwrap();
            let hi = naive(Ymd { year: last, month: 1, day: 1 }, &midnight).unwrap();
            let ts = by_previous(tz, lo, hi);
            listed += ts.len();
            if first == 1800 && ts != transitions(tz, lo, hi) {
                problems.push(format!("{id}: the offset scan and the Previous walk list different transitions"));
            }
            let largest = ts.iter().chain([lo].iter()).filter_map(|t| offset_at(*t, tz)).map(i128::abs).max().unwrap_or(0);
            if largest + HOUR > PROBE_REACH {
                problems.push(format!("{id}: an offset of {largest} ns is within an hour of the probe reach"));
            }
            // The gap search needs local time to pass a wall clock only once
            // within the probe reach either side: no two changes closer than
            // twice the reach plus the step.
            if let Some(w) = ts.windows(2).find(|w| w[1] - w[0] < MIN_SPACING) {
                problems.push(format!("{id}: offsets change twice within {} h at {}", (w[1] - w[0]) / HOUR, w[0]));
            }
        }
    }
    eprintln!("provider bounds: {listed} transitions over 1800-2600 and 275700-275760");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
