//! Plain dates and durations on the fifteen non-ISO calendars, by what must hold
//! whatever a calendar's reckoning: differences and additions in days and weeks
//! equal ISO's, adding a month or year difference back to the start reaches the
//! end, a date's fields agree with themselves (day of year within the year,
//! the rest of the month reaching day 1, a leap year longer), and a month
//! compares with 30 days as the day count between them says.
use temporal_rs::options::{DifferenceSettings, Overflow, Unit};
use temporal_rs::{Calendar, Duration, PlainDate};
use temporal_sweeps::durations;
use temporal_sweeps::oracle_dates::{day_number, from_day_number};
use temporal_sweeps::plain_dates::{add, iso_fields, on, until};

const CALENDARS: [&str; 15] = ["buddhist", "chinese", "coptic", "dangi", "ethioaa", "ethiopic", "gregory", "hebrew", "indian",
    "islamic-civil", "islamic-tbla", "islamic-umalqura", "japanese", "persian", "roc"];

fn settings(unit: Unit) -> DifferenceSettings {
    let mut s = DifferenceSettings::default();
    s.largest_unit = Some(unit);
    s
}

fn iso(n: i64) -> (i32, u8, u8) {
    let (y, m, d) = from_day_number(n);
    (y as i32, m as u8, d as u8)
}

#[test]
fn should_keep_the_calendar_invariants() {
    let (from, to) = (day_number(1900, 1, 1), day_number(2100, 1, 1));
    let starts: Vec<i64> = (from..to).step_by(211).collect();
    let spans = [1i64, 29, 30, 59, 354, 355, 383, 385, 3653];
    let (mut checked, mut wrong) = (0u64, vec![]);
    let mut check = |label: String, ok: bool| {
        checked += 1;
        if !ok && wrong.len() < 16 { wrong.push(label); }
    };
    for name in CALENDARS {
        let cal = Calendar::try_from_utf8(name.as_bytes()).unwrap();
        // The shortest year in months and in days over the sample.
        let shortest = starts.iter().map(|n| on(iso(*n), cal.clone()).unwrap()).fold((u16::MAX, u16::MAX), |(m, d), p| (m.min(p.months_in_year()), d.min(p.days_in_year())));
        for &n in &starts {
            let a = iso(n);
            let pa = on(a, cal.clone()).unwrap();
            // The date's own fields.
            let doy = pa.day_of_year();
            check(format!("{name} {a:?} day of year {doy} within {}", pa.days_in_year()), doy >= 1 && doy <= pa.days_in_year());
            let to_next_month = i64::from(pa.days_in_month()) - i64::from(pa.day()) + 1;
            let next = PlainDate::try_new(a.0, a.1, a.2, Calendar::ISO).unwrap().add(&Duration::new(0, 0, 0, to_next_month, 0, 0, 0, 0, 0, 0).unwrap(), None).unwrap().with_calendar(cal.clone());
            check(format!("{name} {a:?} the rest of the month reaches day 1"), next.day() == 1);
            // A leap year is the longer kind: by month count where the calendar
            // adds a month (lunisolar), otherwise by day count.
            let lunisolar = matches!(name, "chinese" | "dangi" | "hebrew");
            let longer = if lunisolar { pa.months_in_year() > shortest.0 } else { pa.days_in_year() > shortest.1 };
            check(format!("{name} {a:?} in_leap_year {} but the year is {} months, {} days", pa.in_leap_year(), pa.months_in_year(), pa.days_in_year()), pa.in_leap_year() == longer);
            for span in spans {
                for b in [iso(n + span), iso(n - span)] {
                    for unit in [Unit::Day, Unit::Week] {
                        let got = until(a, b, cal.clone(), settings(unit)).ok().map(|d| (d.weeks(), d.days()));
                        let want = until(a, b, Calendar::ISO, settings(unit)).ok().map(|d| (d.weeks(), d.days()));
                        check(format!("{name} {a:?} until {b:?} in {unit:?} matches ISO: {got:?} vs {want:?}"), got == want);
                    }
                    for unit in [Unit::Month, Unit::Year] {
                        let Ok(diff) = until(a, b, cal.clone(), settings(unit)) else {
                            check(format!("{name} {a:?} until {b:?} in {unit:?} refused"), false);
                            continue;
                        };
                        let back = add(a, cal.clone(), &diff, Overflow::Constrain);
                        check(format!("{name} {a:?} + ({a:?} until {b:?} in {unit:?} = {diff}) = {back:?}, not {b:?}"), back.as_ref().ok() == Some(&b));
                    }
                }
                let days = Duration::new(0, 0, 0, span, 0, 0, 0, 0, 0, 0).unwrap();
                check(format!("{name} {a:?} + {span} days matches ISO"), add(a, cal.clone(), &days, Overflow::Constrain).ok() == add(a, Calendar::ISO, &days, Overflow::Constrain).ok());
            }
            let month = Duration::new(0, 1, 0, 0, 0, 0, 0, 0, 0, 0).unwrap();
            let thirty = Duration::new(0, 0, 0, 30, 0, 0, 0, 0, 0, 0).unwrap();
            if let Ok(later) = add(a, cal.clone(), &month, Overflow::Constrain) {
                let month_days = day_number(later.0.into(), later.1.into(), later.2.into()) - n;
                let want = month_days.cmp(&30);
                let got = durations::compare(&month, &thirty, Some(a), cal.clone());
                check(format!("{name} {a:?} a month ({month_days} days) against 30 days: {got:?}"), got.ok() == Some(want));
            }
            let _ = iso_fields(&pa);
        }
    }
    eprintln!("calendar dates: {checked} checked");
    assert!(wrong.is_empty(), "a calendar invariant does not hold:\n{}", wrong.join("\n"));
}
