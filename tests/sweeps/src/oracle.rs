//! Answers to the questions `zoned` answers, derived a different way.
//!
//! `zoned` finds the instants a wall clock could mean by probing offsets and
//! reading each candidate's wall fields back. That cannot be its own check —
//! the first sweeps did exactly that and could only agree with themselves. The
//! oracle instead walks the zone's TRANSITION list, takes the offsets in force
//! between them, and accepts a candidate instant when the offset in force AT
//! that instant is the one that produced it. Instant-to-offset is the direction
//! temporal_rs gets right; the defects were all in the other direction.
use temporal_rs::options::Disambiguation;
use temporal_rs::{Calendar, PlainDate, PlainTime, TimeZone, ZonedDateTime};

use crate::oracle_round::Reckoning;

/// An ISO calendar date, the three fields a wall clock's date is compared by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ymd {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Ymd {
    /// The ISO fields of a date on any calendar.
    pub fn of(d: &PlainDate) -> Ymd {
        let d = d.with_calendar(Calendar::ISO);
        Ymd { year: d.year(), month: d.month(), day: d.day() }
    }

    /// The ISO date after this one, on oracle_dates' day numbers.
    pub fn next(self) -> Option<Ymd> {
        crate::oracle_dates::add((self.year, self.month, self.day), 0, 0, 0, 1, false).map(|(year, month, day)| Ymd { year, month, day })
    }
}


pub const HOUR: i128 = 3_600_000_000_000;
pub const SECOND: i128 = 1_000_000_000;
/// TC39's representable range: 1e8 days either side of the epoch.
pub const LIMIT: i128 = 8_640_000_000_000_000_000_000;
/// No zone offset reaches a day, so this window holds every offset that can
/// matter to a wall clock at its centre.
const WINDOW: i128 = 36 * HOUR;

pub fn offset_at(i: i128, tz: TimeZone) -> Option<i128> {
    ZonedDateTime::try_new(i, tz, Calendar::ISO)
        .ok()
        .map(|z| z.offset_nanoseconds() as i128)
}

/// The local date at an instant: the offset from the provider (the one thing the
/// oracle takes from temporal_rs's zone data), the civil date from day numbers
/// computed here.
pub fn local_date(i: i128, tz: TimeZone) -> Option<Ymd> {
    if !(-LIMIT..=LIMIT).contains(&i) {
        return None;
    }
    let (y, m, d) = crate::oracle_dates::from_day_number(((i + offset_at(i, tz)?).div_euclid(24 * HOUR)) as i64);
    Some(Ymd { year: y as i32, month: m as u8, day: d as u8 })
}

/// A wall clock read as though it were UTC, for a date that exists within the
/// representable range.
pub fn naive(d: Ymd, t: &PlainTime) -> Option<i128> {
    use crate::oracle_dates::{day_number, days_in_month, within_limits};
    let (y, m, dd) = (i64::from(d.year), i64::from(d.month), i64::from(d.day));
    if !(1..=12).contains(&m) || dd < 1 || dd > days_in_month(y, m) || !within_limits(y, m, dd) {
        return None;
    }
    let noon = i128::from(day_number(y, m, dd)) * 24 * HOUR + 12 * HOUR;
    Some(
        noon - 12 * HOUR
            + t.hour() as i128 * HOUR
            + t.minute() as i128 * 60 * SECOND
            + t.second() as i128 * SECOND
            + t.millisecond() as i128 * 1_000_000
            + t.microsecond() as i128 * 1_000
            + t.nanosecond() as i128,
    )
}

/// How often `transitions` reads the offset. Exact while no zone changes its
/// offset twice within this span; `provider_bounds.rs` checks that over the
/// tzdb (the shortest offset period, 1800-2100, is 167 hours).
pub const SCAN_STEP: i128 = 12 * HOUR;

/// The instants in `(lo, hi]` where the offset changes, found by reading the
/// offset every `SCAN_STEP` and bisecting each change. temporal_rs's own
/// transition lookup is not used: its `Next` stops at the end of a zone's tzif
/// table (20 zones) and misses transitions mid-table, and a walk built on it
/// silently checked fewer dates than it claimed.
pub fn transitions(tz: TimeZone, lo: i128, hi: i128) -> Vec<i128> {
    let (mut p, end) = (lo.max(-LIMIT), hi.min(LIMIT));
    let mut out = vec![];
    let Some(mut o) = offset_at(p, tz) else { return out };
    while p < end {
        let q = (p + SCAN_STEP).min(end);
        let Some(oq) = offset_at(q, tz) else { break };
        if oq != o {
            let (mut l, mut h) = (p, q);
            while h - l > 1 {
                let mid = l + (h - l) / 2;
                if offset_at(mid, tz) == Some(o) { l = mid } else { h = mid }
            }
            out.push(h);
            o = oq;
        }
        p = q;
    }
    out
}

/// The instants a wall clock names, earliest first, and — when there are none
/// — the offsets either side of the gap it fell into.
pub struct Candidates {
    pub instants: Vec<i128>,
    pub gap: Option<(i128, i128)>,
}

pub fn candidates(w: i128, tz: TimeZone) -> Candidates {
    // Clamped to the range: a probe before its start has no offset, and
    // near the start that dropped the offset in force from the candidates.
    let (lo, hi) = ((w - WINDOW).max(-LIMIT), (w + WINDOW).min(LIMIT));
    let edges = transitions(tz, lo, hi);
    let mut offsets: Vec<i128> = [lo].iter().chain(edges.iter()).filter_map(|i| offset_at(*i, tz)).collect();
    offsets.sort_unstable();
    offsets.dedup();
    let mut instants: Vec<i128> = offsets.iter().map(|o| w - o).filter(|i| offset_at(*i, tz) == Some(w - i)).collect();
    instants.sort_unstable();
    instants.dedup();
    let gap = if instants.is_empty() {
        edges.iter().find_map(|t| {
            let (before, after) = (offset_at(t - 1, tz)?, offset_at(*t, tz)?);
            (after > before && t + before <= w && w < t + after).then_some((before, after))
        })
    } else {
        None
    };
    Candidates { instants, gap }
}

/// What TC39's DisambiguatePossibleEpochNanoseconds answers, or `None` for an
/// error. In a gap, `earlier` applies the offset from after the transition and
/// `later`/`compatible` the one from before it, which lands after the gap.
pub fn resolve(w: i128, tz: TimeZone, dis: Disambiguation) -> Option<i128> {
    let c = candidates(w, tz);
    match (c.instants.as_slice(), dis) {
        ([], Disambiguation::Reject) => None,
        ([], Disambiguation::Earlier) => c.gap.map(|(_, after)| w - after),
        ([], _) => c.gap.map(|(before, _)| w - before),
        ([only], _) => Some(*only),
        (_, Disambiguation::Reject) => None,
        (many, Disambiguation::Later) => many.last().copied(),
        (many, _) => many.first().copied(),
    }
}

/// The first instant whose local date is `d`: the earliest boundary into the
/// day, where a boundary is either a transition or midnight under some offset.
pub fn first_instant(d: Ymd, tz: TimeZone) -> Option<i128> {
    let midnight = naive(d, &PlainTime::try_new(0, 0, 0, 0, 0, 0).ok()?)?;
    let (lo, hi) = ((midnight - WINDOW).max(-LIMIT), (midnight + WINDOW).min(LIMIT));
    let edges = transitions(tz, lo, hi);
    let probes = [lo, hi].into_iter().chain(edges.iter().flat_map(|t| [t - SECOND, *t]));
    let mut boundaries: Vec<i128> = probes.filter_map(|p| offset_at(p, tz)).map(|o| midnight - o).collect();
    boundaries.extend(edges);
    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries
        .into_iter()
        .find(|i| local_date(*i, tz) == Some(d) && local_date(i - 1, tz) != Some(d))
}

/// `first_instant` by brute force: every second of the surrounding sixty
/// hours. tzdb offsets are whole seconds, so this is exact, and it shares no
/// reasoning with either implementation — it is what checks the oracle.
///
/// One rule is the spec's rather than the scan's: a day whose start falls
/// before the representable range has no first instant, even though the scan
/// finds the range's first second on it. TC39 resolves the date's MIDNIGHT, and
/// throws where that midnight is out of range, so a second found at the very
/// edge counts only when it is midnight itself. That is why UTC's first day
/// has an answer at the minimum instant and New York's and Tokyo's do not.
pub fn first_instant_by_scan(d: Ymd, tz: TimeZone) -> Option<i128> {
    let midnight = naive(d, &PlainTime::try_new(0, 0, 0, 0, 0, 0).ok()?)?;
    let found = (0..=60 * 3600)
        .map(|s| midnight - 30 * HOUR + s * SECOND)
        .find(|i| local_date(*i, tz) == Some(d))?;
    let at_range_edge = local_date(found - 1, tz).is_none();
    let is_midnight = offset_at(found, tz).map(|o| midnight - o) == Some(found);
    (!at_range_edge || is_midnight).then_some(found)
}

/// A zoned difference as TC39's DifferenceZonedDateTime defines it, for a date
/// largest unit and no rounding: step back from the end date until the start's
/// wall clock on it does not overshoot, resolving that wall clock `compatible`
/// with this oracle's own `resolve`, then the ISO date difference plus the
/// exact remainder, as years, months, weeks, days and a time in nanoseconds.
pub fn zoned_difference_internal(a: i128, b: i128, tz: TimeZone, largest: temporal_rs::options::Unit) -> Option<([i64; 4], i128)> {
    zoned_difference_in(a, b, tz, Reckoning::Iso, largest)
}

/// `zoned_difference_internal` on a calendar: the day stepping is ISO and the
/// date difference is the reckoning's (CalendarDateUntil).
pub fn zoned_difference_in(a: i128, b: i128, tz: TimeZone, reckoning: Reckoning, largest: temporal_rs::options::Unit) -> Option<([i64; 4], i128)> {
    if a == b {
        return Some(([0; 4], 0));
    }
    let (sd, ed) = (local_date(a, tz)?, local_date(b, tz)?);
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).ok()?;
    let time_of_day = |i: i128, d: Ymd| Some(i + offset_at(i, tz)? - naive(d, &midnight)?);
    let (st, et) = (time_of_day(a, sd)?, time_of_day(b, ed)?);
    if sd == ed {
        return Some(([0; 4], b - a));
    }
    let sign: i128 = if b < a { -1 } else { 1 };
    let max_correction = if sign == 1 { 2 } else { 1 };
    let mut correction = i128::from((et - st).signum() == -sign);
    while correction <= max_correction {
        let (iy, im, id) = crate::oracle_dates::add((ed.year, ed.month, ed.day), 0, 0, 0, -(correction * sign) as i64, false)?;
        let inter = Ymd { year: iy, month: im, day: id };
        let td = b - resolve(naive(inter, &midnight)? + st, tz, Disambiguation::Compatible)?;
        if sign != -td.signum() {
            let code = match largest { temporal_rs::options::Unit::Year => 3, temporal_rs::options::Unit::Month => 2, temporal_rs::options::Unit::Week => 1, _ => 0 };
            let (from, to) = (crate::oracle_dates::day_number(sd.year.into(), sd.month.into(), sd.day.into()), crate::oracle_dates::day_number(inter.year.into(), inter.month.into(), inter.day.into()));
            let date = match reckoning {
                Reckoning::Iso => crate::oracle_dates::until((sd.year, sd.month, sd.day), (inter.year, inter.month, inter.day), code),
                Reckoning::Model(model) => model.until(from, to, code).ok()?,
            };
            return Some((date, td));
        }
        correction += 1;
    }
    None
}

/// A date part and a time in nanoseconds as a duration's ten fields, the time
/// balanced up to hours (TemporalDurationFromInternal with `hour`).
pub fn duration_fields(date: [i64; 4], td: i128) -> [i128; 10] {
    [
        date[0].into(), date[1].into(), date[2].into(), date[3].into(),
        td / HOUR, td % HOUR / 60_000_000_000, td % 60_000_000_000 / 1_000_000_000,
        td % 1_000_000_000 / 1_000_000, td % 1_000_000 / 1_000, td % 1_000,
    ]
}

/// `zoned_difference_internal` as a duration's fields.
pub fn zoned_difference(a: i128, b: i128, tz: TimeZone, largest: temporal_rs::options::Unit) -> Option<[i128; 10]> {
    zoned_difference_internal(a, b, tz, largest).map(|(date, td)| duration_fields(date, td))
}
