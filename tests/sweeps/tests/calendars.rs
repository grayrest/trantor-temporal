//! Zoned values on non-ISO calendars, near every transition of 2021-2026 in the
//! sweep's zones. A calendar changes how a date reads, never which instant a
//! wall clock is, so resolution, a day's start and length, and rounding to a
//! day must give the ISO instant; printing adds the calendar's annotation. Where
//! the calendar does change the answer — adding months and years, differences
//! in them, rounded differences — the oracle follows the spec over its own
//! resolution, with CalendarDateAdd and CalendarDateUntil the proposal's over
//! ICU4C's calendars (`oracle_calendars.rs`), not ICU4X's. An answer that
//! reaches a year the model cannot vouch for is skipped and counted.
use temporal_rs::options::{DifferenceSettings, Disambiguation, Overflow, RoundingIncrement, RoundingMode as M, RoundingOptions, Unit};
use temporal_rs::{Calendar, Duration, PlainTime, ZonedDateTime};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::{duration_fields, local_date, naive, offset_at, resolve, transitions, zoned_difference_in, Ymd, HOUR};
use temporal_sweeps::oracle_calendars::Model;
use temporal_sweeps::oracle_dates::{day_number, from_day_number};
use temporal_sweeps::oracle_round::{until_rounded_in, Reckoning};
use temporal_sweeps::zoned_ops::{from_wall_clock, to_str};
use temporal_sweeps::ZONES;

const DAY: i128 = 24 * HOUR;
const CALENDARS: [&str; 7] = ["hebrew", "chinese", "japanese", "islamic-civil", "persian", "ethiopic", "buddhist"];

fn fields(d: &Duration) -> [i128; 10] {
    [d.years().into(), d.months().into(), d.weeks().into(), d.days().into(), d.hours().into(), d.minutes().into(),
     d.seconds().into(), d.milliseconds().into(), d.microseconds(), d.nanoseconds()]
}

/// AddZonedDateTime: exact time alone when there is no date part; otherwise
/// the calendar steps the date, the wall clock resolves `compatible`, and the
/// time is added exactly.
fn want_add(i: i128, tz: temporal_rs::TimeZone, model: &Model, date: [i64; 4], time: i128) -> Option<i128> {
    if date == [0; 4] {
        return Some(i + time);
    }
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).ok()?;
    let d = local_date(i, tz)?;
    let tod = i + offset_at(i, tz)? - naive(d, &midnight)?;
    let stepped = model.add_constrained(day_number(d.year.into(), d.month.into(), d.day.into()), date)?;
    let (year, month, day) = from_day_number(stepped);
    let stepped = Ymd { year: year as i32, month: month as u8, day: day as u8 };
    Some(resolve(naive(stepped, &midnight)? + tod, tz, Disambiguation::Compatible)? + time)
}

#[test]
fn should_keep_instants_and_follow_the_calendar_where_it_matters() {
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    let from = naive(Ymd { year: 2021, month: 1, day: 1 }, &midnight).unwrap();
    let to = naive(Ymd { year: 2027, month: 1, day: 1 }, &midnight).unwrap();
    let steps: [([i64; 4], i128); 6] = [([0, 1, 0, 0], 0), ([0, -1, 0, 0], 0), ([1, 0, 0, 0], 0), ([0, 13, 0, 0], 0), ([0, 1, 0, 1], 2 * HOUR), ([-2, -3, 0, 0], 0)];
    let models: Vec<Model> = CALENDARS.iter().map(|name| Model::new(name)).collect();
    let (mut checked, mut declined, mut wrong) = (0u64, 0u64, vec![]);
    let mut check = |label: String, got: Option<String>, want: Option<String>, model: Option<&Model>| {
        if model.is_some_and(|m| m.take_declined()) {
            declined += 1;
            return;
        }
        checked += 1;
        if got != want && wrong.len() < 16 {
            wrong.push(format!("{label}: host {got:?}, oracle {want:?}"));
        }
    };
    let epoch = |z: Result<ZonedDateTime, temporal_rs::TemporalError>| z.ok().map(|z| z.epoch_nanoseconds().as_i128().to_string());
    for id in ZONES {
        let tz = temporal_rs::TimeZone::try_from_identifier_str(id).unwrap();
        for t in transitions(tz, from, to) {
            for k in -4..=4i128 {
                let i = t + k * 6 * HOUR + 17 * 60_000_000_000;
                let iso = ZonedDateTime::try_new_with_provider(i, tz, Calendar::ISO, &EXACT).unwrap();
                let (d, time) = (iso.to_plain_date(), iso.to_plain_time());
                for (name, model) in CALENDARS.iter().zip(&models) {
                    let calendar = Calendar::try_from_utf8(name.as_bytes()).unwrap();
                    let z = iso.with_calendar(calendar.clone());
                    let label = |what: &str| format!("{id} {i} {name} {what}");
                    let date = (d.year(), d.month(), d.day());
                    check(label("from wall clock"), epoch(from_wall_clock(date, &time, tz, calendar.clone(), Disambiguation::Compatible)), epoch(from_wall_clock(date, &time, tz, Calendar::ISO, Disambiguation::Compatible)), None);
                    check(label("start of day"), epoch(z.start_of_day_with_provider(&EXACT)), epoch(iso.start_of_day_with_provider(&EXACT)), None);
                    check(label("hours in day"), z.hours_in_day_with_provider(&EXACT).ok().map(|h| h.to_string()), iso.hours_in_day_with_provider(&EXACT).ok().map(|h| h.to_string()), None);
                    let mut by_day = RoundingOptions::default();
                    by_day.smallest_unit = Some(Unit::Day);
                    by_day.increment = Some(RoundingIncrement::try_new(1).unwrap());
                    check(label("round to day"), epoch(z.round_with_provider(by_day, &EXACT)), epoch(iso.round_with_provider(by_day, &EXACT)), None);
                    check(label("to_str"), to_str(&z).ok(), to_str(&iso).ok().map(|s| format!("{s}[u-ca={name}]")), None);
                    for (date_part, time_part) in steps {
                        let dur = Duration::new(date_part[0], date_part[1], date_part[2], date_part[3], (time_part / HOUR) as i64, 0, 0, 0, 0, 0).unwrap();
                        check(label(&format!("add {date_part:?}+{time_part}")), epoch(z.add_with_provider(&dur, Some(Overflow::Constrain), &EXACT)), want_add(i, tz, model, date_part, time_part).map(|n| n.to_string()), Some(model));
                    }
                    for b in [i + 40 * DAY + 5 * HOUR, i - 400 * DAY, i + 3 * 365 * DAY] {
                        let zb = ZonedDateTime::try_new_with_provider(b, tz, calendar.clone(), &EXACT).unwrap();
                        for largest in [Unit::Month, Unit::Year] {
                            let mut s = DifferenceSettings::default();
                            s.largest_unit = Some(largest);
                            check(label(&format!("until {b} {largest:?}")), z.until_with_provider(&zb, s, &EXACT).ok().map(|d| format!("{:?}", fields(&d))), zoned_difference_in(i, b, tz, Reckoning::Model(model), largest).map(|(date, td)| format!("{:?}", duration_fields(date, td))), Some(model));
                        }
                        for (largest, smallest, mode) in [(Unit::Month, Unit::Day, M::HalfExpand), (Unit::Year, Unit::Month, M::Trunc), (Unit::Month, Unit::Week, M::Ceil)] {
                            let mut s = DifferenceSettings::default();
                            s.largest_unit = Some(largest);
                            s.smallest_unit = Some(smallest);
                            s.rounding_mode = Some(mode);
                            s.increment = Some(RoundingIncrement::try_new(1).unwrap());
                            check(label(&format!("until rounded {b} {largest:?}/{smallest:?} {mode:?}")), z.until_with_provider(&zb, s, &EXACT).ok().map(|d| format!("{:?}", fields(&d))), until_rounded_in(i, b, tz, Reckoning::Model(model), largest, smallest, 1, mode).map(|f| format!("{f:?}")), Some(model));
                        }
                    }
                }
            }
        }
    }
    eprintln!("calendars: {checked} checked, {declined} reaching a year the ICU4C model lacks");
    assert!(wrong.is_empty(), "a calendar changes a zoned answer the oracle does not:\n{}", wrong.join("\n"));
}

/// Every zone, every 16 hours near each transition of five eras, on three
/// calendars: construction and a day's start must equal ISO, and adding a month
/// follows the spec over the oracle's resolution.
#[test]
fn should_keep_calendar_instants_in_every_zone() {
    let names = ["hebrew", "chinese", "persian"];
    let models: Vec<Model> = names.iter().map(|name| Model::new(name)).collect();
    let (mut checked, mut declined, mut wrong) = (0u64, 0u64, vec![]);
    for (id, tz) in temporal_sweeps::zones() {
        for i in temporal_sweeps::near_era_sample(tz, 8) {
            let Ok(iso) = ZonedDateTime::try_new_with_provider(i, tz, Calendar::ISO, &EXACT) else { continue };
            let (d, time) = (iso.to_plain_date(), iso.to_plain_time());
            for (name, model) in names.iter().zip(&models) {
                let calendar = Calendar::try_from_utf8(name.as_bytes()).unwrap();
                let z = iso.with_calendar(calendar.clone());
                let epoch = |r: Result<ZonedDateTime, temporal_rs::TemporalError>| r.ok().map(|x| x.epoch_nanoseconds().as_i128());
                let date = (d.year(), d.month(), d.day());
                let pairs = [
                    ("from wall clock", epoch(from_wall_clock(date, &time, tz, calendar.clone(), Disambiguation::Compatible)), epoch(from_wall_clock(date, &time, tz, Calendar::ISO, Disambiguation::Compatible))),
                    ("start of day", epoch(z.start_of_day_with_provider(&EXACT)), epoch(iso.start_of_day_with_provider(&EXACT))),
                    ("add a month", epoch(z.add_with_provider(&Duration::new(0, 1, 0, 0, 0, 0, 0, 0, 0, 0).unwrap(), Some(Overflow::Constrain), &EXACT)), want_add(i, tz, model, [0, 1, 0, 0], 0)),
                ];
                if model.take_declined() {
                    declined += 1;
                    continue;
                }
                for (what, got, want) in pairs {
                    checked += 1;
                    if got != want && wrong.len() < 12 {
                        wrong.push(format!("{id} {i} {name} {what}: host {got:?}, oracle {want:?}"));
                    }
                }
            }
        }
    }
    eprintln!("calendars, every zone: {checked} checked, {declined} instants reaching a year the ICU4C model lacks");
    assert!(wrong.is_empty(), "a calendar changes a zoned answer the oracle does not:\n{}", wrong.join("\n"));
}
