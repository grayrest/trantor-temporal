//! The zoned operations whose answer is more than one upstream call: a wall
//! clock resolved in a zone, a value moved to another date or zone, and IXDTF
//! parsing and printing. They live here rather than in lib.rs, which is the ABI,
//! so `tests/sweeps` compiles this same file and checks it against the oracle.
use temporal_rs::options::{
    Disambiguation, DisplayCalendar, DisplayOffset, DisplayTimeZone, OffsetDisambiguation, ToStringRoundingOptions,
};
use temporal_rs::{Calendar, PlainDateTime, PlainTime, TemporalError, TimeZone, ZonedDateTime};

use crate::annotations;
use crate::exact_provider::EXACT;

/// An ISO date as year, month and day.
pub type IsoDate = (i32, u8, u8);

fn wall(date: IsoDate, time: &PlainTime) -> Result<PlainDateTime, TemporalError> {
    let (year, month, day) = date;
    PlainDateTime::try_new(
        year, month, day, time.hour(), time.minute(), time.second(),
        time.millisecond(), time.microsecond(), time.nanosecond(), Calendar::ISO,
    )
}

/// The instant a wall clock names in `zone`. Resolved in ISO with the calendar
/// applied after, since a calendar changes how a date reads, never which
/// instant it is (D-T2-9).
pub fn from_wall_clock(
    date: IsoDate,
    time: &PlainTime,
    zone: TimeZone,
    calendar: Calendar,
    disambiguation: Disambiguation,
) -> Result<ZonedDateTime, TemporalError> {
    let resolved = wall(date, time)?.to_zoned_date_time_with_provider(zone, disambiguation, &EXACT)?;
    Ok(resolved.with_calendar(calendar))
}

/// `base`'s wall-clock time on another ISO date, in its zone and calendar.
pub fn with_plain_date(base: &ZonedDateTime, date: IsoDate, disambiguation: Disambiguation) -> Result<ZonedDateTime, TemporalError> {
    from_wall_clock(date, &base.to_plain_time(), *base.time_zone(), base.calendar().clone(), disambiguation)
}

/// The same instant read in another zone.
pub fn with_time_zone(base: &ZonedDateTime, zone: TimeZone) -> Result<ZonedDateTime, TemporalError> {
    base.with_time_zone_with_provider(zone, &EXACT)
}

/// TC39's `equals`: the same instant and calendar, in zones with the same
/// primary identifier, so an alias and its target are the same zone.
pub fn equals(a: &ZonedDateTime, b: &ZonedDateTime) -> Result<bool, TemporalError> {
    a.equals_with_provider(b, &EXACT)
}

/// IXDTF. A wall clock with no offset resolves `compatible`; one with an offset
/// must name an instant the zone gives that offset — `OffsetDisambiguation::Reject`
/// — rather than one side silently winning (D-T1-12).
pub fn parse(s: &str) -> Result<ZonedDateTime, TemporalError> {
    ZonedDateTime::from_utf8_with_provider(annotations::normalize(s)?.as_bytes(), Disambiguation::Compatible, OffsetDisambiguation::Reject, &EXACT)
}

/// IXDTF with the offset, the zone, and the calendar unless it is ISO.
pub fn to_str(z: &ZonedDateTime) -> Result<String, TemporalError> {
    z.to_ixdtf_string_with_provider(
        DisplayOffset::Auto,
        DisplayTimeZone::Auto,
        DisplayCalendar::Auto,
        ToStringRoundingOptions::default(),
        &EXACT,
    )
}
