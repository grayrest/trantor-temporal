//! The next and previous time-zone transition, as TC39 defines one: an instant
//! where the zone's UTC offset changes (D-T2-14).
//!
//! temporal_rs 0.2.6 answers `Previous` correctly and `Next` unreliably. Asked
//! from exactly the last entry of a zone's tzif table, `Next` returns the query
//! instant itself (20 zones); in the middle of a table it can return no
//! transition when there is one (Indiana/Petersburg from 2007-03 to 2008-03,
//! among 27 found over 1850-2100). Both directions also report entries where
//! only the abbreviation or DST flag changed. So `Previous` is the one lookup
//! used, entries that keep the offset are skipped, and `next` is found by
//! widening a window until a transition falls in it and bisecting for the first.
//! Measured against a scan of offsets, all zones, 1850-2100: 0 wrong in either
//! direction over 128,552 queries.
use temporal_rs::provider::TransitionDirection;
use temporal_rs::{Calendar, TemporalError, TimeZone, ZonedDateTime};

use crate::exact_provider::EXACT;

const NS_PER_DAY: i128 = 86_400_000_000_000;
/// The representable instants are ±10^8 days of nanoseconds.
const LIMIT: i128 = 100_000_000 * NS_PER_DAY;

fn at(i: i128, zone: TimeZone) -> Result<ZonedDateTime, TemporalError> {
    ZonedDateTime::try_new_with_provider(i, zone, Calendar::ISO, &EXACT)
}

fn changes_offset(t: i128, zone: TimeZone) -> Result<bool, TemporalError> {
    Ok(t <= -LIMIT || at(t - 1, zone)?.offset_nanoseconds() != at(t, zone)?.offset_nanoseconds())
}

/// The last instant before `before` where the offset changes.
fn previous(before: i128, zone: TimeZone) -> Result<Option<i128>, TemporalError> {
    let mut from = before.min(LIMIT);
    loop {
        let Some(found) = at(from, zone)?.get_time_zone_transition_with_provider(TransitionDirection::Previous, &EXACT)? else {
            return Ok(None);
        };
        let t = found.epoch_nanoseconds().as_i128();
        if t >= from {
            return Ok(None);
        }
        if changes_offset(t, zone)? {
            return Ok(Some(t));
        }
        from = t;
    }
}

/// The first instant after `after` where the offset changes: the smallest `y`
/// with a change in `(after, y]`, which `previous(y + 1)` answers.
fn next(after: i128, zone: TimeZone) -> Result<Option<i128>, TemporalError> {
    let found_by = |y: i128| previous(y + 1, zone).map(|t| t.is_some_and(|t| t > after));
    let mut span = NS_PER_DAY;
    let upper = loop {
        let y = after.saturating_add(span).min(LIMIT);
        if found_by(y)? {
            break y;
        }
        if y == LIMIT {
            return Ok(None);
        }
        span *= 2;
    };
    let (mut lo, mut hi) = (after, upper);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if found_by(mid)? {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    previous(hi + 1, zone)
}

/// The transition from `base` in `direction`, carrying `base`'s calendar.
pub fn transition(base: &ZonedDateTime, direction: TransitionDirection) -> Result<Option<ZonedDateTime>, TemporalError> {
    let (zone, calendar) = (*base.time_zone(), base.calendar().clone());
    let from = base.epoch_nanoseconds().as_i128();
    let found = match direction {
        TransitionDirection::Next => next(from, zone)?,
        TransitionDirection::Previous => previous(from, zone)?,
    };
    found.map(|t| ZonedDateTime::try_new_with_provider(t, zone, calendar, &EXACT)).transpose()
}
