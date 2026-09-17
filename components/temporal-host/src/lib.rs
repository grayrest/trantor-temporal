//! roc:temporal host over temporal_rs (P13). PlainDate/PlainTime/Duration
//! cross as plain records of ISO fields — in BOTH directions (D-T1-2); a
//! Calendar selects arithmetic rules and never a field spelling.
//! ZonedDateTime/TimeZone/Calendar are resources (P5), this crate being the
//! sole vendor of temporal_rs natives (H0c). Blocking, pure computation.
//!
//! This is the primitive layer: every option is an explicit argument and
//! nothing here defaults (D-T1-1, D-T1-4). Defaults live in the Roc `Temporal`
//! module above it.
//!
//! Owned-argument rule: RocStr args are decref'd exactly once on every path,
//! including the error paths. A TIME ZONE now arrives as one of those strings
//! (D-T2-5), so this rule applies to seventeen leaves rather than four —
//! `take_zone` exists so that no leaf has to remember it.
//!
//! ZonedDateTime is the only handle left; it is touched only through
//! resource::with.
use core::mem::ManuallyDrop;
use trantor_abi as abi;
use abi::*;
use temporal_rs::options::{
    Disambiguation, DifferenceSettings, Overflow, RoundingIncrement, RoundingMode, RoundingOptions, Unit,
};
use temporal_rs::provider::TransitionDirection;

mod annotations;
mod durations;
mod exact_provider;
mod plain_dates;
mod transition;
mod zoned_ops;
use exact_provider::EXACT;
use temporal_rs::{Calendar, Duration, PlainDate, PlainTime, TemporalError, TimeZone, ZonedDateTime};

type DateRec = AnonStruct1af1a0fdd5cc23ac;
type TimeRec = AnonStruct2c5a54cbbebfb285;
type DurRec = AnonStructCa37d39356fbfd17;
type Err = InvalidOrOtherOrOutOfRange;
type OverflowArg = ConstrainOrReject;
type UnitArg = DayOrHourOrMicrosecondOrMillisecondOrMinuteOrMonthOrNanosecondOrSecondOrWeekOrYear;
type DisambigArg = CompatibleOrEarlierOrLaterOrReject;
type FieldsRec = AnonStruct8e5146289d80ad5;
type DiffOpts = AnonStruct2156bc6491d737c7;
type RoundOpts = AnonStructF512e6d370f76d6d;
type ModeArg = CeilOrExpandOrFloorOrHalfCeilOrHalfEvenOrHalfExpandOrHalfFloorOrHalfTruncOrTrunc;
type RelArg = ToDateOrUnanchored;
type ParsedRec = AnonStructD2534314a77ad90e;
type CalArg = BuddhistOrChineseOrCopticOrDangiOrEthioaaOrEthiopicOrGregoryOrHebrewOrIndianOrIslamicCivilOrIslamicTblaOrIslamicUmalquraOrIsoOrJapaneseOrPersianOrRoc;
const NANOS_PER_SECOND: i64 = 1_000_000_000;

/// temporal_rs keeps `ErrorKind` private, but `Display` is `"{kind}: {msg}"`
/// (`"RangeError: unknown calendar"`), so the kind is a reliable PREFIX rather
/// than a substring to hunt for in the Debug form — which includes the message
/// and so classified on whatever words the message happened to contain.
///
/// The payload carries the message alone: the kind is the tag, and repeating it
/// inside the string said the same thing twice (D-T1-7).
///
/// `SyntaxError` maps to `Invalid` rather than `Other` — 0.2.6 never constructs
/// one, but a later version that does means "this input is not the right kind
/// of thing", which is what `Invalid` says.
fn to_err(e: TemporalError) -> Err {
    use InvalidOrOtherOrOutOfRangeTag as T;
    let full = format!("{e}");
    let (kind, msg) = full.split_once(": ").unwrap_or((full.as_str(), ""));
    let tag = match kind {
        "RangeError" => T::OutOfRange,
        "TypeError" | "SyntaxError" => T::Invalid,
        _ => T::Other,
    };
    let s = ManuallyDrop::new(RocStr::from_str(msg, abi::host()));
    let payload = match tag {
        T::OutOfRange => InvalidOrOtherOrOutOfRangePayload { out_of_range: s },
        T::Invalid => InvalidOrOtherOrOutOfRangePayload { invalid: s },
        T::Other => InvalidOrOtherOrOutOfRangePayload { other: s },
    };
    Err { payload, tag }
}

fn take_str(s: RocStr) -> String {
    let v = s.as_str().to_string();
    unsafe { s.decref(abi::host()) };
    v
}
fn handle<T: 'static>(v: T) -> *mut u64 {
    abi::resource::new(v) as *mut u64
}
/// A calendar is a tag, so this is a match rather than a lookup. The table is
/// built once because `Calendar::try_from_utf8` parses a string (64 ns) and a
/// tag carries the answer already.
fn cal(c: CalArg) -> Calendar {
    use CalArg as C;
    let id: &[u8] = match c {
        C::Iso => return Calendar::ISO,
        C::Buddhist => b"buddhist",
        C::Chinese => b"chinese",
        C::Coptic => b"coptic",
        C::Dangi => b"dangi",
        C::Ethioaa => b"ethioaa",
        C::Ethiopic => b"ethiopic",
        C::Gregory => b"gregory",
        C::Hebrew => b"hebrew",
        C::Indian => b"indian",
        C::IslamicCivil => b"islamic-civil",
        C::IslamicTbla => b"islamic-tbla",
        C::IslamicUmalqura => b"islamic-umalqura",
        C::Japanese => b"japanese",
        C::Persian => b"persian",
        C::Roc => b"roc",
    };
    // Every arm above is an identifier temporal_rs accepts — the tag union was
    // generated from the set it accepts — so this cannot fail. If a future
    // version drops one, ISO is a wrong answer rather than a crashed process,
    // and `calendar_id!` will show it.
    Calendar::try_from_utf8(id).unwrap_or(Calendar::ISO)
}

/// The identifier a tag stands for, for `calendar_id!` and for turning an
/// answer from temporal_rs back into a tag.
fn cal_tag(c: &Calendar) -> CalArg {
    use CalArg as C;
    match c.identifier() {
        "buddhist" => C::Buddhist,
        "chinese" => C::Chinese,
        "coptic" => C::Coptic,
        "dangi" => C::Dangi,
        "ethioaa" => C::Ethioaa,
        "ethiopic" => C::Ethiopic,
        "gregory" => C::Gregory,
        "hebrew" => C::Hebrew,
        "indian" => C::Indian,
        "islamic-civil" => C::IslamicCivil,
        "islamic-tbla" => C::IslamicTbla,
        "islamic-umalqura" => C::IslamicUmalqura,
        "japanese" => C::Japanese,
        "persian" => C::Persian,
        "roc" => C::Roc,
        _ => C::Iso,
    }
}

thread_local! {
    /// Identifier -> resolved zone. A temporal_rs `TimeZone` holds two resolved
    /// indices into the tzdb; re-resolving one per call measured 6.3x the cost
    /// of the operation it enables, so this memo is part of the design and not
    /// an optimisation (D-T2-6). Thread-local rather than locked: a TimeZone is
    /// 24 bytes and `Copy`, so duplicating the table per thread is cheaper than
    /// contending for one.
    static ZONES: core::cell::RefCell<std::collections::HashMap<String, TimeZone>> =
        core::cell::RefCell::new(std::collections::HashMap::new());
}

fn zone_of(id: &str) -> Result<TimeZone, TemporalError> {
    ZONES.with(|c| {
        if let Some(z) = c.borrow().get(id) {
            return Ok(*z);
        }
        let z = TimeZone::try_from_str(id)?;
        c.borrow_mut().insert(id.to_string(), z);
        Ok(z)
    })
}

/// Resolve an owned zone identifier. Decrefs the argument on EVERY path,
/// including the one where the identifier is not a zone.
fn take_zone(t: RocStr) -> Result<TimeZone, TemporalError> {
    let id = take_str(t);
    zone_of(&id)
}
/// Clones out rather than handing back a borrow: `zdt_until!` holds two
/// ZonedDateTimes at once, and nesting `resource::with` inside itself is not a
/// shape this ABI promises.
fn zdt(z: *mut u64) -> ZonedDateTime {
    unsafe { abi::resource::with(z as RocBox, |x: &mut ZonedDateTime| x.clone()) }
}

fn overflow_of(o: OverflowArg) -> Overflow {
    match o {
        OverflowArg::Constrain => Overflow::Constrain,
        OverflowArg::Reject => Overflow::Reject,
    }
}
fn unit_of(u: UnitArg) -> Unit {
    match u {
        UnitArg::Year => Unit::Year,
        UnitArg::Month => Unit::Month,
        UnitArg::Week => Unit::Week,
        UnitArg::Day => Unit::Day,
        UnitArg::Hour => Unit::Hour,
        UnitArg::Minute => Unit::Minute,
        UnitArg::Second => Unit::Second,
        UnitArg::Millisecond => Unit::Millisecond,
        UnitArg::Microsecond => Unit::Microsecond,
        UnitArg::Nanosecond => Unit::Nanosecond,
    }
}
fn disambig_of(d: DisambigArg) -> Disambiguation {
    match d {
        DisambigArg::Compatible => Disambiguation::Compatible,
        DisambigArg::Earlier => Disambiguation::Earlier,
        DisambigArg::Later => Disambiguation::Later,
        DisambigArg::Reject => Disambiguation::Reject,
    }
}
fn largest(u: UnitArg) -> DifferenceSettings {
    let mut s = DifferenceSettings::default();
    s.largest_unit = Some(unit_of(u));
    s
}

/// A record is ISO fields, so it is read as ISO fields and REJECTED when it
/// names no real date — `{2024, 2, 30}` is a bad value, not a date that
/// overflowed, and constraining it to Feb 29 would hide whatever produced it.
/// The `Overflow` argument governs the arithmetic step only (D-T1-5).
/// The date record's fields as the ABI-free triple `zoned` works in.
fn iso_date(d: DateRec) -> zoned_ops::IsoDate {
    (d.year, d.month, d.day)
}
/// Reads through the ISO calendar, so the record that comes out is the record
/// that would go back in. Without this a Hebrew calendar turns `{2024, 1, 31}`
/// into `{5784, 5, 21}` and a zero duration stops being an identity (D-T1-2).
/// Calendar-aware arithmetic is unaffected: the stepping already happened
/// before the projection.
fn rec_of(d: &PlainDate) -> DateRec {
    let d = d.with_calendar(Calendar::ISO);
    DateRec { year: d.year(), month: d.month(), day: d.day() }
}
fn time_of(r: TimeRec) -> Result<PlainTime, TemporalError> {
    PlainTime::try_new(r.hour, r.minute, r.second, r.millisecond, r.microsecond, r.nanosecond)
}
fn time_rec(t: &PlainTime) -> TimeRec {
    TimeRec {
        hour: t.hour(),
        minute: t.minute(),
        second: t.second(),
        millisecond: t.millisecond(),
        microsecond: t.microsecond(),
        nanosecond: t.nanosecond(),
    }
}
fn duration_of(r: DurRec) -> Result<Duration, TemporalError> {
    Duration::new(
        r.years, r.months, r.weeks, r.days, r.hours, r.minutes, r.seconds, r.milliseconds,
        r.microseconds as i128, r.nanoseconds as i128,
    )
}
/// temporal_rs holds micro- and nanoseconds as `i128`; the record carries
/// `I64`. `as` would wrap silently, so the narrowing is checked and reported.
fn duration_rec(d: &Duration) -> Result<DurRec, TemporalError> {
    let micros = i64::try_from(d.microseconds())
        .map_err(|_| TemporalError::range().with_message("duration microseconds exceed I64"))?;
    let nanos = i64::try_from(d.nanoseconds())
        .map_err(|_| TemporalError::range().with_message("duration nanoseconds exceed I64"))?;
    Ok(DurRec {
        years: d.years(),
        months: d.months(),
        weeks: d.weeks(),
        days: d.days(),
        hours: d.hours(),
        minutes: d.minutes(),
        seconds: d.seconds(),
        milliseconds: d.milliseconds(),
        microseconds: micros,
        nanoseconds: nanos,
    })
}

macro_rules! try_result {
    ($R:ident, $P:ident, $T:ident, $r:expr) => {
        match $r {
            Ok(v) => $R { payload: $P { ok: ManuallyDrop::new(v) }, tag: $T::Ok },
            Err(e) => $R { payload: $P { err: ManuallyDrop::new(to_err(e)) }, tag: $T::Err },
        }
    };
}
macro_rules! calendar_result {
    ($r:expr) => {
        try_result!(
            TemporalHostCalendarFromIdResult,
            TemporalHostCalendarFromIdResultPayload,
            TemporalHostCalendarFromIdResultTag,
            $r
        )
    };
}
macro_rules! tz_result {
    ($r:expr) => {
        try_result!(
            TemporalHostTimeZoneFromIdResult,
            TemporalHostTimeZoneFromIdResultPayload,
            TemporalHostTimeZoneFromIdResultTag,
            $r
        )
    };
}
macro_rules! str_result {
    ($r:expr) => {
        try_result!(
            TemporalHostZdtToStrResult,
            TemporalHostZdtToStrResultPayload,
            TemporalHostZdtToStrResultTag,
            $r.map(|s: String| RocStr::from_str(&s, abi::host()))
        )
    };
}
macro_rules! date_result {
    ($r:expr) => {
        try_result!(
            TemporalHostDateAddResult,
            TemporalHostDateAddResultPayload,
            TemporalHostDateAddResultTag,
            $r
        )
    };
}
macro_rules! duration_result {
    ($r:expr) => {
        try_result!(
            TemporalHostDateUntilResult,
            TemporalHostDateUntilResultPayload,
            TemporalHostDateUntilResultTag,
            $r
        )
    };
}
macro_rules! zdt_result {
    ($r:expr) => {
        try_result!(
            TemporalHostZdtFromEpochNsResult,
            TemporalHostZdtFromEpochNsResultPayload,
            TemporalHostZdtFromEpochNsResultTag,
            $r.map(handle)
        )
    };
}

// ---- calendars and zones ----

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__calendar_from_id(
    id: RocStr,
) -> TemporalHostCalendarFromIdResult {
    let id = take_str(id);
    calendar_result!(Calendar::try_from_utf8(id.as_bytes()).map(|c| cal_tag(&c)))
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__calendar_id(c: CalArg) -> RocStr {
    RocStr::from_str(cal(c).identifier(), abi::host())
}

#[unsafe(no_mangle)]
/// Validates an identifier and answers its canonical spelling, warming the memo
/// on the way. There is no `time_zone_id!` any more — a zone IS its identifier,
/// so that leaf had become the identity function with a C ABI crossing in it.
pub extern "C-unwind" fn trantor__temporal_host__time_zone_from_id(
    id: RocStr,
) -> TemporalHostTimeZoneFromIdResult {
    let id = take_str(id);
    tz_result!(zone_of(&id).and_then(|z| z.identifier()).map(|s| RocStr::from_str(&s, abi::host())))
}

// ---- plain dates ----

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__date_add(
    d: DateRec,
    dur: DurRec,
    c: CalArg,
    ov: OverflowArg,
) -> TemporalHostDateAddResult {
    // The date is checked before the duration, so a call with both wrong
    // reports the date, as it did before the move into plain_dates.rs.
    let r = plain_dates::on(iso_date(d), cal(c))
        .and_then(|_| duration_of(dur))
        .and_then(|dur| plain_dates::add(iso_date(d), cal(c), &dur, overflow_of(ov)))
        .map(|(year, month, day)| DateRec { year, month, day });
    date_result!(r)
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__date_until(
    a: DateRec,
    b: DateRec,
    c: CalArg,
    u: UnitArg,
) -> TemporalHostDateUntilResult {
    let r = plain_dates::until(iso_date(a), iso_date(b), cal(c), largest(u)).and_then(|d| duration_rec(&d));
    duration_result!(r)
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__date_from_str(
    s: RocStr,
) -> TemporalHostDateFromStrResult {
    let s = take_str(s);
    // The calendar is read off the parsed date BEFORE `rec_of` projects the
    // fields to ISO — the projection is exactly what used to lose it.
    let r = plain_dates::parse(&s).map(|((year, month, day), calendar)| ParsedRec {
        date: DateRec { year, month, day },
        calendar: cal_tag(&calendar),
    });
    try_result!(
        TemporalHostDateFromStrResult,
        TemporalHostDateFromStrResultPayload,
        TemporalHostDateFromStrResultTag,
        r
    )
}

// ---- zoned date-times ----

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_from_epoch_ns(
    ns: i128,
    t: RocStr,
    c: CalArg,
) -> TemporalHostZdtFromEpochNsResult {
    zdt_result!(take_zone(t).and_then(|zone| ZonedDateTime::try_new_with_provider(ns, zone, cal(c), &EXACT)))
}

/// What instant a wall-clock date and time name in a zone. The wall clock is
/// resolved in ISO and the calendar applied after, since a calendar changes how
/// a date reads, never which instant it is (D-T2-9).
#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_from_wall_clock(
    d: DateRec,
    t: TimeRec,
    z: RocStr,
    c: CalArg,
    dis: DisambigArg,
) -> TemporalHostZdtFromEpochNsResult {
    let calendar = cal(c);
    let r = take_zone(z)
        .and_then(|zone| time_of(t).and_then(|time| zoned_ops::from_wall_clock(iso_date(d), &time, zone, calendar, disambig_of(dis))));
    zdt_result!(r)
}

/// IXDTF, as `zoned_ops::parse` reads it.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_from_str(
    s: RocStr,
) -> TemporalHostZdtFromEpochNsResult {
    let s = take_str(s);
    let r = zoned_ops::parse(&s);
    zdt_result!(r)
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_epoch_ns(z: *mut u64) -> i128 {
    zdt(z).epoch_nanoseconds().as_i128()
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_with_time_zone(
    z: *mut u64,
    t: RocStr,
) -> TemporalHostZdtFromEpochNsResult {
    // The handle is read before the zone, which can fail: `zdt` is what
    // releases the owned reference, so reading it inside `and_then` leaked the
    // value whenever the zone was not one.
    let base = zdt(z);
    zdt_result!(take_zone(t).and_then(|target| zoned_ops::with_time_zone(&base, target)))
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_calendar(z: *mut u64) -> CalArg {
    cal_tag(zdt(z).calendar())
}

/// The zone this value is in. The shim used to recover this by rendering the
/// value to IXDTF and slicing between the brackets — a string and a parse to
/// read a field that was sitting right here.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_time_zone(
    z: *mut u64,
) -> TemporalHostZdtToStrResult {
    str_result!(zdt(z).time_zone().identifier())
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_plain_date(z: *mut u64) -> DateRec {
    rec_of(&zdt(z).to_plain_date())
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_plain_time(z: *mut u64) -> TimeRec {
    time_rec(&zdt(z).to_plain_time())
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_offset_seconds(z: *mut u64) -> i64 {
    zdt(z).offset_nanoseconds() / NANOS_PER_SECOND
}

/// Fallible because `to_ixdtf_string` is, and an empty string is a
/// plausible-looking answer for a failure.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_to_str(
    z: *mut u64,
) -> TemporalHostZdtToStrResult {
    str_result!(zoned_ops::to_str(&zdt(z)))
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_add(
    z: *mut u64,
    dur: DurRec,
    ov: OverflowArg,
) -> TemporalHostZdtFromEpochNsResult {
    let base = zdt(z);
    let r = duration_of(dur).and_then(|d| base.add_with_provider(&d, Some(overflow_of(ov)), &EXACT));
    zdt_result!(r)
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_until(
    a: *mut u64,
    b: *mut u64,
    u: UnitArg,
) -> TemporalHostDateUntilResult {
    let other = zdt(b);
    let r = zdt(a).until_with_provider(&other, largest(u), &EXACT).and_then(|d| duration_rec(&d));
    duration_result!(r)
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_start_of_day(
    z: *mut u64,
) -> TemporalHostZdtFromEpochNsResult {
    zdt_result!(zdt(z).start_of_day_with_provider(&EXACT))
}

// ---- durations ----

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__duration_from_str(
    s: RocStr,
) -> TemporalHostDateUntilResult {
    let s = take_str(s);
    duration_result!(durations::parse(&s).and_then(|d| duration_rec(&d)))
}


// ---- options ----

fn mode_of(m: ModeArg) -> RoundingMode {
    match m {
        ModeArg::Ceil => RoundingMode::Ceil,
        ModeArg::Floor => RoundingMode::Floor,
        ModeArg::Expand => RoundingMode::Expand,
        ModeArg::Trunc => RoundingMode::Trunc,
        ModeArg::HalfCeil => RoundingMode::HalfCeil,
        ModeArg::HalfFloor => RoundingMode::HalfFloor,
        ModeArg::HalfExpand => RoundingMode::HalfExpand,
        ModeArg::HalfTrunc => RoundingMode::HalfTrunc,
        ModeArg::HalfEven => RoundingMode::HalfEven,
    }
}

fn diff_of(o: DiffOpts) -> Result<DifferenceSettings, TemporalError> {
    let mut s = DifferenceSettings::default();
    s.largest_unit = Some(unit_of(o.largest));
    s.smallest_unit = Some(unit_of(o.smallest));
    s.rounding_mode = Some(mode_of(o.mode));
    s.increment = Some(RoundingIncrement::try_new(o.increment)?);
    Ok(s)
}

/// A duration in calendar units has no fixed length until it is anchored, so
/// `Unanchored` stays `None` and temporal_rs reports the mismatch itself rather
/// than this guessing a date.
fn relative_of(r: RelArg) -> Option<zoned_ops::IsoDate> {
    match r.tag {
        ToDateOrUnanchoredTag::Unanchored => None,
        ToDateOrUnanchoredTag::ToDate => Some(iso_date(unsafe { *r.payload.to_date })),
    }
}

// ---- calendar-derived fields ----

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__date_fields(
    d: DateRec,
    c: CalArg,
) -> TemporalHostDateFieldsResult {
    let r = plain_dates::fields(iso_date(d), cal(c)).map(|f| FieldsRec {
        era: RocStr::from_str(&f.era, abi::host()),
        month_code: RocStr::from_str(&f.month_code, abi::host()),
        era_year: f.era_year,
        year_of_week: f.year_of_week,
        day_of_year: f.day_of_year,
        days_in_month: f.days_in_month,
        days_in_week: f.days_in_week,
        days_in_year: f.days_in_year,
        months_in_year: f.months_in_year,
        day_of_week: f.day_of_week,
        in_leap_year: f.in_leap_year,
        week_of_year: f.week_of_year,
    });
    try_result!(
        TemporalHostDateFieldsResult,
        TemporalHostDateFieldsResultPayload,
        TemporalHostDateFieldsResultTag,
        r
    )
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__date_until_rounded(
    a: DateRec,
    b: DateRec,
    c: CalArg,
    o: DiffOpts,
) -> TemporalHostDateUntilResult {
    let r = diff_of(o)
        .and_then(|settings| plain_dates::until(iso_date(a), iso_date(b), cal(c), settings))
        .and_then(|d| duration_rec(&d));
    duration_result!(r)
}

// ---- zoned: with, round, day length, transitions ----

/// Rebuilt through the wall-clock path rather than temporal_rs's `with`, so it
/// inherits `corrected` — changing the date of a zoned value is exactly the
/// operation the upstream defect spoils.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_with_plain_date(
    z: *mut u64,
    d: DateRec,
    dis: DisambigArg,
) -> TemporalHostZdtFromEpochNsResult {
    zdt_result!(zoned_ops::with_plain_date(&zdt(z), iso_date(d), disambig_of(dis)))
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_with_calendar(
    z: *mut u64,
    c: CalArg,
) -> *mut u64 {
    handle(zdt(z).with_calendar(cal(c)))
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_until_rounded(
    a: *mut u64,
    b: *mut u64,
    o: DiffOpts,
) -> TemporalHostDateUntilResult {
    // Both handles are read BEFORE anything fallible. `zdt` is what releases
    // the owned reference, so leaving it inside `and_then` meant a rejected
    // option (`increment: 0` is one) returned early and leaked the resource —
    // measured at one handle per call.
    let (base, other) = (zdt(a), zdt(b));
    let r = diff_of(o)
        .and_then(|settings| base.until_with_provider(&other, settings, &EXACT))
        .and_then(|d| duration_rec(&d));
    duration_result!(r)
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_round(
    z: *mut u64,
    o: RoundOpts,
) -> TemporalHostZdtFromEpochNsResult {
    // Read the handle first: see `zdt_until_rounded`.
    let base = zdt(z);
    let r = RoundingIncrement::try_new(o.increment).and_then(|increment| {
        let mut options = RoundingOptions::default();
        options.smallest_unit = Some(unit_of(o.smallest));
        options.rounding_mode = Some(mode_of(o.mode));
        options.increment = Some(increment);
        base.round_with_provider(options, &EXACT)
    });
    zdt_result!(r)
}

/// TC39's `equals` (`zoned_ops::equals`); comparing the identifier strings
/// missed an alias and its target being the same zone.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_equals(
    a: *mut u64,
    b: *mut u64,
) -> TemporalHostZdtEqualsResult {
    let (x, y) = (zdt(a), zdt(b));
    try_result!(
        TemporalHostZdtEqualsResult,
        TemporalHostZdtEqualsResultPayload,
        TemporalHostZdtEqualsResultTag,
        zoned_ops::equals(&x, &y)
    )
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_hours_in_day(
    z: *mut u64,
) -> TemporalHostZdtHoursInDayResult {
    let r = zdt(z).hours_in_day_with_provider(&EXACT);
    try_result!(
        TemporalHostZdtHoursInDayResult,
        TemporalHostZdtHoursInDayResultPayload,
        TemporalHostZdtHoursInDayResultTag,
        r
    )
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__zdt_transition(
    z: *mut u64,
    dir: NextOrPrevious,
) -> TemporalHostZdtTransitionResult {
    let direction = match dir {
        NextOrPrevious::Next => TransitionDirection::Next,
        NextOrPrevious::Previous => TransitionDirection::Previous,
    };
    let base = zdt(z);
    let r = transition::transition(&base, direction).map(|found| match found {
        Some(next) => AtOrNoTransition {
            payload: AtOrNoTransitionPayload { at: ManuallyDrop::new(handle(next)) },
            tag: AtOrNoTransitionTag::At,
        },
        None => AtOrNoTransition {
            payload: AtOrNoTransitionPayload { no_transition: [] },
            tag: AtOrNoTransitionTag::NoTransition,
        },
    });
    try_result!(
        TemporalHostZdtTransitionResult,
        TemporalHostZdtTransitionResultPayload,
        TemporalHostZdtTransitionResultTag,
        r
    )
}

// ---- durations: round, total, compare ----

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__duration_round(
    d: DurRec,
    o: DiffOpts,
    rel: RelArg,
    c: CalArg,
) -> TemporalHostDateUntilResult {
    let r = duration_of(d).and_then(|dur| {
        let settings = diff_of(o)?;
        let mut opts = RoundingOptions::default();
        opts.largest_unit = settings.largest_unit;
        opts.smallest_unit = settings.smallest_unit;
        opts.rounding_mode = settings.rounding_mode;
        opts.increment = settings.increment;
        durations::round(&dur, opts, relative_of(rel), cal(c))
    })
    .and_then(|d| duration_rec(&d));
    duration_result!(r)
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__duration_total(
    d: DurRec,
    u: UnitArg,
    rel: RelArg,
    c: CalArg,
) -> TemporalHostZdtHoursInDayResult {
    let r = duration_of(d).and_then(|dur| durations::total(&dur, unit_of(u), relative_of(rel), cal(c)));
    try_result!(
        TemporalHostZdtHoursInDayResult,
        TemporalHostZdtHoursInDayResultPayload,
        TemporalHostZdtHoursInDayResultTag,
        r
    )
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__temporal_host__duration_compare(
    a: DurRec,
    b: DurRec,
    rel: RelArg,
    c: CalArg,
) -> TemporalHostDurationCompareResult {
    let r = duration_of(a)
        .and_then(|a| duration_of(b).and_then(|b| durations::compare(&a, &b, relative_of(rel), cal(c))))
        .map(|o| o as i8);
    try_result!(
        TemporalHostDurationCompareResult,
        TemporalHostDurationCompareResultPayload,
        TemporalHostDurationCompareResultTag,
        r
    )
}
