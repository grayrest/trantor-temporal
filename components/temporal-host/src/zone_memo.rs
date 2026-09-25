//! The memo every time zone is resolved through (D-T2-6). Its own file, and
//! free of the C ABI, so `tests/sweeps` compiles it from this same source the
//! way it compiles the rest of the host's algorithms.
use temporal_rs::{TemporalError, TimeZone};

thread_local! {
    /// Identifier -> resolved zone. A temporal_rs `TimeZone` holds two resolved
    /// indices into the tzdb; re-resolving one per call measured 6.3x the cost
    /// of the operation it enables, so this memo is part of the design and not
    /// an optimisation (D-T2-6). Thread-local rather than locked: a TimeZone is
    /// 24 bytes and `Copy`, so duplicating the table per thread is cheaper than
    /// contending for one.
    ///
    /// The key is what the zone calls ITSELF, never what the caller spelled.
    /// `TimeZone : Str`, so an identifier reaches `zoned!` or `with_time_zone!`
    /// straight from a config file, a CSV column or a request parameter without
    /// passing `time_zone_from_id!` first, and `TimeZone::try_from_str` accepts
    /// every case permutation of a name — over a billion for the thirty letters
    /// of `America/Argentina/ComodRivadavia` — as well as any IXDTF string that
    /// carries a zone annotation. Keyed on the caller's spelling the map grew
    /// one entry per accepted input and never shrank, so ordinary untrusted
    /// input could grow a long-running process without bound. Keyed on the
    /// zone's own identifier it holds one entry per tzdb name plus one per
    /// minute-precision offset, which is all `try_from_str` can produce.
    pub static ZONES: core::cell::RefCell<std::collections::HashMap<String, TimeZone>> =
        core::cell::RefCell::new(std::collections::HashMap::new());
}

/// A zone by identifier, resolved once per zone.
///
/// A caller who spells a zone some way other than the tzdb does misses the memo
/// and resolves it again every call — the cost D-T2-6 measured, which is what
/// this code did before the memo existed — and leaves the map where it was.
pub fn zone_of(id: &str) -> Result<TimeZone, TemporalError> {
    ZONES.with(|c| {
        if let Some(z) = c.borrow().get(id) {
            return Ok(*z);
        }
        let z = TimeZone::try_from_str(id)?;
        // A zone the provider has just resolved has a spelling. If a later
        // version disagrees, going unmemoised is the right answer rather than
        // failing an operation that already holds its zone.
        if let Ok(canonical) = z.identifier() {
            c.borrow_mut().insert(canonical, z);
        }
        Ok(z)
    })
}
