//! The tzdb provider every zoned operation goes through (D-T2-13).
//!
//! temporal_rs 0.2.6 answers instant -> offset correctly, but its wall clock
//! -> instant lookup (`candidate_nanoseconds_for_local_epoch_nanoseconds`) is an
//! estimate that is wrong near transitions, and every zoned operation that
//! resolves a wall clock — construction from fields, `add`, `until`, `round`,
//! `start_of_day` — goes through it. This provider is the compiled one with
//! that lookup computed exactly from the direction that is right: a wall clock
//! means `wall - offset` for each offset in force nearby, when that instant
//! reads back with that offset. Upstream's own spec-shaped code then gives the
//! spec's answers. Measured through it: 0 wrong across resolution, start of
//! day, rounding and differences, where the compiled provider was wrong on
//! thousands of each.
use std::borrow::Cow;
use temporal_rs::provider::{
    CandidateEpochNanoseconds, EpochNanosecondsAndOffset, GapEntryOffsets, TimeZoneId, TimeZoneProvider,
    TransitionDirection, UtcOffsetSeconds, COMPILED_TZ_PROVIDER,
};
use temporal_rs::unix_time::EpochNanoseconds;
use timezone_provider::provider::IsoDateTime;
use timezone_provider::TimeZoneProviderError;

const NS_PER_SECOND: i128 = 1_000_000_000;
const NS_PER_HOUR: i128 = 3_600 * NS_PER_SECOND;
/// A candidate for a wall clock is `wall - offset`, and no zone's offset has
/// reached 16 hours (the largest is 15:56:08), so every offset a candidate can
/// have is in force within this many hours of the wall clock.
const PROBE_HOURS: i128 = 18;
/// No zone keeps an offset for less than a week (the shortest, 167 hours, over
/// 1800-2100), so probing this often sees every offset in the window. Both
/// bounds are checked over the tzdb by `tests/sweeps/tests/provider_bounds.rs`.
const PROBE_STEP_HOURS: i128 = 6;

type Found<T> = Result<T, TimeZoneProviderError>;

pub struct ExactProvider;

/// The one instance; it holds nothing, so sharing it is free.
pub static EXACT: ExactProvider = ExactProvider;

fn offset_ns(id: TimeZoneId, i: i128) -> Found<i128> {
    Ok(i128::from(COMPILED_TZ_PROVIDER.transition_nanoseconds_for_utc_epoch_nanoseconds(id, i)?.0) * NS_PER_SECOND)
}

fn seconds(ns: i128) -> UtcOffsetSeconds {
    UtcOffsetSeconds((ns / NS_PER_SECOND) as i64)
}

fn at(instant: i128, offset: i128) -> EpochNanosecondsAndOffset {
    EpochNanosecondsAndOffset { ns: EpochNanoseconds(instant), offset: seconds(offset) }
}

/// The instants `wall` can mean, earliest first. Two or more is an overlap;
/// the spec takes the first and last of them.
fn candidates(id: TimeZoneId, wall: i128) -> Found<CandidateEpochNanoseconds> {
    let mut offsets: Vec<i128> = Vec::new();
    for k in (-PROBE_HOURS..=PROBE_HOURS).step_by(PROBE_STEP_HOURS as usize) {
        // The compiled provider answers an offset for any instant, inside the
        // representable range or not, so a failure here is the provider's own
        // and is reported rather than dropping an offset. Candidates outside the
        // range are kept: the spec lists them and then throws, which is how an
        // operation at the range's end refuses.
        let o = offset_ns(id, wall + k * NS_PER_HOUR)?;
        if !offsets.contains(&o) {
            offsets.push(o);
        }
    }
    let mut valid: Vec<(i128, i128)> = Vec::new();
    for o in offsets {
        if offset_ns(id, wall - o)? == o {
            valid.push((wall - o, o));
        }
    }
    valid.sort_unstable();
    Ok(match valid.as_slice() {
        [] => CandidateEpochNanoseconds::Zero(gap(id, wall)?),
        [(i, o)] => CandidateEpochNanoseconds::One(at(*i, *o)),
        [(i1, o1), .., (i2, o2)] => CandidateEpochNanoseconds::Two([at(*i1, *o1), at(*i2, *o2)]),
    })
}

/// A wall clock no instant reads as sits in the jump of one transition. Local
/// time `x + offset(x)` rises with `x` between transitions, so with no instant
/// on the wall clock it passes it only by jumping, and bisection finds that
/// jump from the offsets alone. `get_time_zone_transition` is not used: past
/// the end of its tzif data it can answer the query instant itself. The offsets
/// either side choose `earlier`/`later`, and the transition's instant is where
/// a day with no midnight begins.
fn gap(id: TimeZoneId, wall: i128) -> Found<GapEntryOffsets> {
    let past = |x: i128| offset_ns(id, x).map(|o| x + o > wall);
    let (mut lo, mut hi) = (wall - PROBE_HOURS * NS_PER_HOUR, wall + PROBE_HOURS * NS_PER_HOUR);
    if past(lo)? || !past(hi)? {
        return Err(TimeZoneProviderError::Range("a wall clock with no instant has no transition near it"));
    }
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if past(mid)? { hi = mid } else { lo = mid }
    }
    Ok(GapEntryOffsets {
        offset_before: seconds(offset_ns(id, lo)?),
        offset_after: seconds(offset_ns(id, hi)?),
        transition_epoch: EpochNanoseconds(hi),
    })
}

impl TimeZoneProvider for ExactProvider {
    fn get(&self, ident: &[u8]) -> Found<TimeZoneId> {
        COMPILED_TZ_PROVIDER.get(ident)
    }

    fn identifier(&self, id: TimeZoneId) -> Found<Cow<'_, str>> {
        COMPILED_TZ_PROVIDER.identifier(id)
    }

    fn canonicalized(&self, id: TimeZoneId) -> Found<TimeZoneId> {
        COMPILED_TZ_PROVIDER.canonicalized(id)
    }

    fn candidate_nanoseconds_for_local_epoch_nanoseconds(&self, id: TimeZoneId, local: IsoDateTime) -> Found<CandidateEpochNanoseconds> {
        candidates(id, local.as_nanoseconds().0)
    }

    fn transition_nanoseconds_for_utc_epoch_nanoseconds(&self, id: TimeZoneId, ns: i128) -> Found<UtcOffsetSeconds> {
        COMPILED_TZ_PROVIDER.transition_nanoseconds_for_utc_epoch_nanoseconds(id, ns)
    }

    fn get_time_zone_transition(&self, id: TimeZoneId, ns: i128, d: TransitionDirection) -> Found<Option<EpochNanoseconds>> {
        COMPILED_TZ_PROVIDER.get_time_zone_transition(id, ns, d)
    }
}
