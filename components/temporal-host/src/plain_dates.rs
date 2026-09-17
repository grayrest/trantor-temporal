//! Plain-date operations: ISO fields in, with the calendar that reckons months
//! and years, and ISO fields out, so the record that comes back is the record
//! that would go back in (D-T1-2). They live here rather than in lib.rs, which
//! is the ABI, so `tests/sweeps` compiles this same file.
use temporal_rs::options::{DifferenceSettings, Overflow};
use temporal_rs::{Calendar, Duration, PlainDate, TemporalError};

use crate::annotations;
use crate::zoned_ops::IsoDate;

/// A date from ISO fields on `calendar`. A record naming no real date is
/// rejected, not constrained.
pub fn on(date: IsoDate, calendar: Calendar) -> Result<PlainDate, TemporalError> {
    PlainDate::try_new(date.0, date.1, date.2, calendar)
}

/// The ISO fields of a date on any calendar.
pub fn iso_fields(d: &PlainDate) -> IsoDate {
    let iso = d.with_calendar(Calendar::ISO);
    (iso.year(), iso.month(), iso.day())
}

pub fn add(date: IsoDate, calendar: Calendar, duration: &Duration, overflow: Overflow) -> Result<IsoDate, TemporalError> {
    on(date, calendar)?.add(duration, Some(overflow)).map(|d| iso_fields(&d))
}

/// A difference with the settings given; both dates are on `calendar`.
pub fn until(a: IsoDate, b: IsoDate, calendar: Calendar, settings: DifferenceSettings) -> Result<Duration, TemporalError> {
    on(a, calendar.clone())?.until(&on(b, calendar)?, settings)
}

/// IXDTF, answering the ISO fields and the calendar the annotation named.
pub fn parse(s: &str) -> Result<(IsoDate, Calendar), TemporalError> {
    PlainDate::from_utf8(annotations::normalize(s)?.as_bytes()).map(|d| (iso_fields(&d), d.calendar().clone()))
}

/// What a calendar derives for a date, shaped as the ABI's `CalendarFields`:
/// an absent era is empty, and an absent era year, week or week year is 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fields {
    pub era: String,
    pub era_year: i32,
    pub month_code: String,
    pub day_of_week: u8,
    pub day_of_year: u16,
    pub week_of_year: u8,
    pub year_of_week: i32,
    pub days_in_week: u16,
    pub days_in_month: u16,
    pub days_in_year: u16,
    pub months_in_year: u16,
    pub in_leap_year: bool,
}

/// The fields `calendar` derives for an ISO date.
pub fn fields(date: IsoDate, calendar: Calendar) -> Result<Fields, TemporalError> {
    let d = on(date, calendar)?;
    Ok(Fields {
        era: d.era().map(|e| e.to_string()).unwrap_or_default(),
        era_year: d.era_year().unwrap_or(0),
        month_code: d.month_code().as_str().to_string(),
        day_of_week: d.day_of_week() as u8,
        day_of_year: d.day_of_year(),
        week_of_year: d.week_of_year().unwrap_or(0),
        year_of_week: d.year_of_week().unwrap_or(0),
        days_in_week: d.days_in_week(),
        days_in_month: d.days_in_month(),
        days_in_year: d.days_in_year(),
        months_in_year: d.months_in_year(),
        in_leap_year: d.in_leap_year(),
    })
}
