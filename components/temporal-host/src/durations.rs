//! Durations rounded, totalled and compared, measured from a date when their
//! calendar units need one. Here rather than in lib.rs, which is the ABI, so
//! `tests/sweeps` compiles this same file.
use core::cmp::Ordering;
use temporal_rs::options::{RelativeTo, RoundingOptions, Unit};
use temporal_rs::{Calendar, Duration, TemporalError};

use crate::plain_dates;
use crate::zoned_ops::IsoDate;

/// The date calendar units are reckoned from, if any; `None` leaves years,
/// months and weeks without a length, which temporal_rs refuses rather than
/// this guessing.
fn anchor(date: Option<IsoDate>, calendar: Calendar) -> Result<Option<RelativeTo>, TemporalError> {
    date.map(|d| plain_dates::on(d, calendar).map(RelativeTo::PlainDate)).transpose()
}

pub fn round(duration: &Duration, options: RoundingOptions, from: Option<IsoDate>, calendar: Calendar) -> Result<Duration, TemporalError> {
    duration.round(options, anchor(from, calendar)?)
}

pub fn total(duration: &Duration, unit: Unit, from: Option<IsoDate>, calendar: Calendar) -> Result<f64, TemporalError> {
    duration.total(unit, anchor(from, calendar)?).map(|t| t.as_inner())
}

pub fn compare(a: &Duration, b: &Duration, from: Option<IsoDate>, calendar: Calendar) -> Result<Ordering, TemporalError> {
    Duration::compare(a, b, anchor(from, calendar)?)
}

/// An ISO 8601 duration string (D-T2-24). temporal_rs 0.2.6 accepts a unit
/// named twice and keeps the last — `PT2H3H` is three hours, `PT2H0.5H` half of
/// one — where TC39's grammar names each unit at most once; so a string whose
/// designators repeat or run out of order is refused before it is parsed.
pub fn parse(s: &str) -> Result<Duration, TemporalError> {
    let mut rank = 0;
    for c in s.bytes().skip_while(|c| matches!(c, b'+' | b'-')).skip(1) {
        let next = match c.to_ascii_uppercase() {
            b'Y' => 1,
            b'M' if rank < 5 => 2,
            b'W' => 3,
            b'D' => 4,
            b'T' => 5,
            b'H' => 6,
            b'M' => 7,
            b'S' => 8,
            _ => continue,
        };
        if next <= rank {
            return Err(TemporalError::range().with_message("a duration names each unit once, largest first"));
        }
        rank = next;
    }
    Duration::from_utf8(s.as_bytes())
}
