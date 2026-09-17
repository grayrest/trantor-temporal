//! The host's `CalendarFields` record (`plain_dates::fields`, which
//! `date_fields!` copies across the ABI) for every ISO day from 1800 to 2200 on
//! all sixteen calendars. For the fifteen non-ISO ones the reference is
//! ICU4C's table and the model built from it, read by TC39's era and
//! month-code proposal: era, era year, month code, days in the month and
//! months in the year from the table; day of year, days in the year and
//! whether the year is leap — thirteen months for a lunisolar calendar, more
//! days than the shortest year otherwise — from the model; no week numbering,
//! which the proposal defines for `iso8601` alone. ISO's fields, ISO weeks
//! included, come from day numbers here, and reach both range limits.
use std::collections::BTreeMap;
use temporal_rs::Calendar;
use temporal_sweeps::icu_reference::{is_trusted, rows};
use temporal_sweeps::oracle_calendars::Model;
use temporal_sweeps::oracle_dates::{day_number, days_in_month, from_day_number, is_leap};
use temporal_sweeps::plain_dates::{fields, Fields};

/// ICU4C's English era label as TC39's era code.
fn era_code(label: &str) -> &'static str {
    match label {
        "AM" => "am", "AA" => "aa", "AH" => "ah", "Śaka" => "shaka", "AP" => "ap", "BE" => "be", "AD" => "ce",
        "Minguo" => "roc", "B.R.O.C." => "broc", "Meiji" => "meiji", "Taishō" => "taisho", "Shōwa" => "showa",
        "Heisei" => "heisei", "Reiwa" => "reiwa",
        other => panic!("no era code for ICU4C's {other}"),
    }
}

/// ISO weekday, Monday 1, from a day number (1970-01-01 was a Thursday).
fn weekday(n: i64) -> u8 {
    ((n + 3).rem_euclid(7) + 1) as u8
}

/// ISO week and week-numbering year.
fn iso_week(n: i64) -> (u8, i32) {
    let (y, _, _) = from_day_number(n);
    let weeks_in = |year: i64| {
        let jan1 = weekday(day_number(year, 1, 1));
        if jan1 == 4 || (jan1 == 3 && is_leap(year)) { 53 } else { 52 }
    };
    let ordinal = n - day_number(y, 1, 1) + 1;
    let week = (ordinal - i64::from(weekday(n)) + 10).div_euclid(7);
    if week < 1 { return (weeks_in(y - 1) as u8, (y - 1) as i32); }
    if week > weeks_in(y) { return (1, (y + 1) as i32); }
    (week as u8, y as i32)
}

fn iso_fields(n: i64) -> Fields {
    let (y, m, _) = from_day_number(n);
    let (week_of_year, year_of_week) = iso_week(n);
    Fields {
        era: String::new(), era_year: 0, month_code: format!("M{m:02}"), day_of_week: weekday(n),
        day_of_year: (n - day_number(y, 1, 1) + 1) as u16, week_of_year, year_of_week, days_in_week: 7,
        days_in_month: days_in_month(y, m) as u16, days_in_year: if is_leap(y) { 366 } else { 365 },
        months_in_year: 12, in_leap_year: is_leap(y),
    }
}

fn iso(n: i64) -> (i32, u8, u8) {
    let (y, m, d) = from_day_number(n);
    (y as i32, m as u8, d as u8)
}

#[test]
fn should_derive_iso_fields_from_day_numbers() {
    let limits = [(day_number(-271_821, 4, 19), day_number(-271_820, 1, 10)), (day_number(275_759, 12, 20), day_number(275_760, 9, 14))];
    let days = (day_number(1800, 1, 1)..day_number(2201, 1, 1)).chain(limits.iter().flat_map(|&(a, b)| a..b));
    let (mut checked, mut wrong) = (0u64, vec![]);
    for n in days {
        checked += 1;
        let got = fields(iso(n), Calendar::ISO).ok();
        let want = Some(iso_fields(n));
        if got != want && wrong.len() < 8 { wrong.push(format!("{:?}: host {got:?}, oracle {want:?}", iso(n))); }
    }
    eprintln!("iso record: {checked} days checked");
    assert!(wrong.is_empty(), "ISO fields disagree:\n{}", wrong.join("\n"));
}

#[test]
fn should_derive_calendar_fields_as_icu4c_and_the_proposal_do() {
    let meiji_calendar_years = day_number(1873, 1, 1);
    let (mut checked, mut skipped) = (0u64, 0u64);
    let mut wrong: BTreeMap<String, (u64, Vec<String>)> = BTreeMap::new();
    for (calendar, runs) in rows() {
        let model = Model::new(calendar);
        let cal = Calendar::try_from_utf8(calendar.as_bytes()).unwrap();
        let lunisolar = matches!(calendar, "hebrew" | "chinese" | "dangi");
        let shortest = if calendar.starts_with("islamic") { 354 } else { 365 };
        for (i, row) in runs.iter().enumerate() {
            let end = runs.get(i + 1).map_or(row.start + i64::from(row.days_in_month) - i64::from(row.day) + 1, |next| next.start);
            for n in row.start..end {
                let span = model.parts(n).and_then(|p| model.year_span(p.year));
                let Ok((first, days, months)) = span else { skipped += 1; continue };
                if !is_trusted(calendar, n) || row.months_in_year == 0 { skipped += 1; continue; }
                let (era, era_year) = match row.era {
                    "-" => (String::new(), 0),
                    _ if calendar == "japanese" && n < meiji_calendar_years => ("ce".to_string(), from_day_number(n).0 as i32),
                    label => (era_code(label).to_string(), row.year),
                };
                let want = Fields {
                    era, era_year, month_code: row.code.to_string(), day_of_week: weekday(n),
                    day_of_year: (n - first + 1) as u16, week_of_year: 0, year_of_week: 0, days_in_week: 7,
                    days_in_month: row.days_in_month, days_in_year: days as u16, months_in_year: months as u16,
                    in_leap_year: if lunisolar { months == 13 } else { days > shortest },
                };
                checked += 1;
                let got = fields(iso(n), cal.clone()).ok();
                if got.as_ref() != Some(&want) {
                    let differing = match &got {
                        Some(g) => format!("{:?}", diff(g, &want)),
                        None => "refused".to_string(),
                    };
                    let entry = wrong.entry(format!("{calendar} {differing}")).or_default();
                    entry.0 += 1;
                    if entry.1.len() < 2 { entry.1.push(format!("{:?}: host {got:?}, oracle {want:?}", iso(n))); }
                }
            }
        }
    }
    eprintln!("calendar record: {checked} days checked, {skipped} outside the model's trusted years");
    for (key, (count, examples)) in &wrong { eprintln!("  {key}: {count} differ — {}", examples.join("; ")); }
    assert!(wrong.is_empty(), "the fields record disagrees in {} ways", wrong.len());
}

/// The names of the fields that differ.
fn diff(a: &Fields, b: &Fields) -> Vec<&'static str> {
    let pairs: [(&str, bool); 12] = [
        ("era", a.era == b.era), ("era_year", a.era_year == b.era_year), ("month_code", a.month_code == b.month_code),
        ("day_of_week", a.day_of_week == b.day_of_week), ("day_of_year", a.day_of_year == b.day_of_year),
        ("week_of_year", a.week_of_year == b.week_of_year), ("year_of_week", a.year_of_week == b.year_of_week),
        ("days_in_week", a.days_in_week == b.days_in_week), ("days_in_month", a.days_in_month == b.days_in_month),
        ("days_in_year", a.days_in_year == b.days_in_year), ("months_in_year", a.months_in_year == b.months_in_year),
        ("in_leap_year", a.in_leap_year == b.in_leap_year),
    ];
    pairs.iter().filter(|(_, same)| !same).map(|(name, _)| *name).collect()
}
