//! Rounded plain-date differences on the fifteen non-ISO calendars through the
//! host's own code (`plain_dates.rs`) against DifferenceTemporalPlainDate over
//! the proposal's NonISODateUntil and NonISODateAdd on ICU4C's calendars
//! (`oracle_plain_round.rs`): years, months and weeks rounded in every mode,
//! from leap months, the thirteenth month, month ends and a stride of other
//! days, 1900-2100. A pair the model cannot vouch for is skipped and counted.
use std::collections::BTreeMap;
use temporal_rs::options::{DifferenceSettings, RoundingIncrement, RoundingMode as M, Unit};
use temporal_rs::Calendar;
use temporal_sweeps::oracle_calendars::Model;
use temporal_sweeps::oracle_dates::{day_number, from_day_number};
use temporal_sweeps::oracle_plain_round::plain_until_in;
use temporal_sweeps::plain_dates;

const CALENDARS: [&str; 15] = ["buddhist", "chinese", "coptic", "dangi", "ethioaa", "ethiopic", "gregory", "hebrew", "indian",
    "islamic-civil", "islamic-tbla", "islamic-umalqura", "japanese", "persian", "roc"];
const ALL: [M; 9] = [M::Ceil, M::Floor, M::Expand, M::Trunc, M::HalfCeil, M::HalfFloor, M::HalfExpand, M::HalfTrunc, M::HalfEven];
const SOME: [M; 3] = [M::Trunc, M::HalfExpand, M::Ceil];

/// (largest, smallest, increment, modes)
const CASES: [(Unit, Unit, u32, &[M]); 8] = [
    (Unit::Year, Unit::Month, 1, &ALL), (Unit::Year, Unit::Year, 1, &ALL), (Unit::Month, Unit::Week, 1, &ALL),
    (Unit::Month, Unit::Month, 3, &SOME), (Unit::Year, Unit::Day, 5, &SOME), (Unit::Month, Unit::Day, 2, &SOME),
    (Unit::Week, Unit::Week, 2, &SOME), (Unit::Year, Unit::Week, 1, &SOME),
];

const SPANS: [i64; 16] = [15, 29, 30, 45, 177, 192, 354, 369, 384, 400, 560, 767, -15, -192, -384, -767];

struct Tally { checked: u64, declined: u64, wrong: BTreeMap<String, (u64, Vec<String>)> }

fn iso(n: i64) -> (i32, u8, u8) {
    let (y, m, d) = from_day_number(n);
    (y as i32, m as u8, d as u8)
}

fn sweep(calendar: &'static str) -> Tally {
    let (from, to) = (day_number(1900, 1, 1), day_number(2101, 1, 1));
    let model = Model::new(calendar);
    let cal = Calendar::try_from_utf8(calendar.as_bytes()).unwrap();
    let mut tally = Tally { checked: 0, declined: 0, wrong: BTreeMap::new() };
    let starts: Vec<i64> = model.known_days().filter(|&n| (from..to).contains(&n)).filter(|&n| {
        let p = model.parts(n).unwrap();
        (p.code.ends_with('L') && p.day % 14 == 1) || (p.code == "M13" && p.day >= 5) || (p.day >= 29 && n % 7 == 0) || n % 211 == 0
    }).collect();
    for &a in &starts {
        let parts = model.parts(a).unwrap();
        for span in SPANS {
            let b = a + span;
            for (largest, smallest, increment, modes) in CASES {
                for &mode in modes {
                    model.take_declined();
                    let want = plain_until_in(&model, a, b, largest, smallest, increment.into(), mode);
                    if model.take_declined() { tally.declined += 1; continue; }
                    tally.checked += 1;
                    let mut settings = DifferenceSettings::default();
                    settings.largest_unit = Some(largest);
                    settings.smallest_unit = Some(smallest);
                    settings.rounding_mode = Some(mode);
                    settings.increment = Some(RoundingIncrement::try_new(increment).unwrap());
                    let got = plain_dates::until(iso(a), iso(b), cal.clone(), settings).ok().map(|x| [x.years(), x.months(), x.weeks(), x.days()]);
                    if got != want {
                        let entry = tally.wrong.entry(format!("{largest:?}/{smallest:?}/{increment} {mode:?}")).or_default();
                        entry.0 += 1;
                        if entry.1.len() < 2 {
                            entry.1.push(format!("{:?} ({} {} day {}) until {:?}: host {got:?}, oracle {want:?}", iso(a), parts.year, parts.code, parts.day, iso(b)));
                        }
                    }
                }
            }
        }
    }
    tally
}

#[test]
fn should_round_non_iso_date_differences_as_the_spec_does() {
    let tallies: Vec<(&str, Tally)> = std::thread::scope(|s| {
        let handles: Vec<_> = CALENDARS.iter().map(|&c| s.spawn(move || (c, sweep(c)))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let (checked, declined): (u64, u64) = tallies.iter().fold((0, 0), |(c, d), (_, t)| (c + t.checked, d + t.declined));
    eprintln!("calendar rounding: {checked} rounded differences checked, {declined} where the model lacks a year");
    let mut kinds = 0;
    for (calendar, tally) in &tallies {
        for (case, (count, examples)) in &tally.wrong {
            kinds += 1;
            eprintln!("  {calendar} {case}: {count} differ — {}", examples.join("; "));
        }
    }
    assert!(kinds == 0, "rounded non-ISO differences disagree with the spec in {kinds} calendar settings");
}
