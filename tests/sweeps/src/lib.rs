//! The host's tzdb provider, compiled from the SAME source file the host
//! ships, plus an oracle that answers the same questions a different way.

#[path = "../../../components/temporal-host/src/annotations.rs"]
pub mod annotations;

#[path = "../../../components/temporal-host/src/exact_provider.rs"]
pub mod exact_provider;

#[path = "../../../components/temporal-host/src/transition.rs"]
pub mod transition;

#[path = "../../../components/temporal-host/src/zoned_ops.rs"]
pub mod zoned_ops;

#[path = "../../../components/temporal-host/src/plain_dates.rs"]
pub mod plain_dates;

#[path = "../../../components/temporal-host/src/durations.rs"]
pub mod durations;

#[path = "../../../components/temporal-host/src/zone_memo.rs"]
pub mod zone_memo;

pub mod icu_reference;
pub mod oracle;
pub mod oracle_calendars;
pub mod oracle_dates;
pub mod oracle_durations;
pub mod oracle_plain_round;
pub mod oracle_round;
pub mod rounding;

/// Every IANA identifier in `zones.txt`, resolved. A name the pinned
/// temporal_rs no longer resolves is a failure, not a skip: a sweep that
/// quietly covers fewer zones reports the same "0 wrong".
pub fn zones() -> Vec<(&'static str, temporal_rs::TimeZone)> {
    include_str!("../zones.txt")
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|id| {
            let tz = temporal_rs::TimeZone::try_from_identifier_str(id)
                .unwrap_or_else(|e| panic!("zones.txt lists {id}, which temporal_rs does not resolve: {e}"));
            (id, tz)
        })
        .collect()
}

/// Zones chosen for having broken something: southern-hemisphere DST, 30- and
/// 45-minute offsets, a zone that skipped a whole day, and the ones where an
/// earlier sweep sampled the wrong hour.
pub const ZONES: &[&str] = &[
    "America/New_York", "America/Chicago", "Europe/Berlin", "Europe/London", "Europe/Dublin",
    "Asia/Jerusalem", "Africa/Cairo", "Australia/Sydney", "Australia/Lord_Howe", "Pacific/Auckland",
    "Pacific/Chatham", "America/Santiago", "America/Havana", "America/Nuuk", "Antarctica/Troll",
    "Asia/Kathmandu", "Asia/Tokyo", "America/Sao_Paulo", "America/St_Johns", "Europe/Lisbon",
];

pub fn days_in(month: u8, year: i32) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        _ => 28,
    }
}

/// Years whose transitions the all-zone sweeps sample, with how many two-hour
/// steps either side of each. Every year from 1800 to 2041 is in one: the
/// dense eras (wartime double summer time, the 1970s rule changes, the
/// late-1990s EU harmonisation, the years around now, 2040) at fifteen steps,
/// and the years between them — local mean time giving way to standard time,
/// the First World War, postwar and 1980s rule churn — at three. Past 2041 the
/// zones repeat standing rules, so four years out to 2200 stand for the rest.
pub const ERAS: [(i32, i32, i128); 14] = [
    (1800, 1941, 3), (1942, 1947, 15), (1948, 1969, 3), (1970, 1975, 15), (1976, 1994, 3), (1995, 2000, 15), (2001, 2020, 3),
    (2021, 2026, 15), (2027, 2039, 3), (2040, 2041, 15), (2060, 2060, 3), (2100, 2100, 3), (2150, 2150, 3), (2199, 2200, 3),
];

/// Instants near every transition of `ERAS` in `tz`, every 2 hours out to the
/// era's step count, on the transition's own minute and 37:29.5 off it.
pub fn near_era_transitions(tz: temporal_rs::TimeZone) -> Vec<i128> {
    use oracle::{naive, transitions, Ymd, HOUR};
    let midnight = temporal_rs::PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    let mut out = vec![];
    for (first, last, steps) in ERAS {
        let from = naive(Ymd { year: first, month: 1, day: 1 }, &midnight).unwrap();
        let to = naive(Ymd { year: last + 1, month: 1, day: 1 }, &midnight).unwrap();
        for t in transitions(tz, from, to) {
            for k in -steps..=steps {
                out.extend([t + 2 * k * HOUR, t + 2 * k * HOUR + 37 * 60_000_000_000 + 29_500_000_000]);
            }
        }
    }
    out
}

/// `near_era_transitions` thinned to every `every`-th pair, keeping both the
/// on-the-minute and the off-the-minute instant of each. Stepping the flat list
/// by an even number kept only the on-the-minute half, so every thinned sweep
/// sampled whole hours and quarter-hours alone.
pub fn near_era_sample(tz: temporal_rs::TimeZone, every: usize) -> Vec<i128> {
    near_era_transitions(tz).chunks(2).step_by(every.max(1)).flatten().copied().collect()
}
