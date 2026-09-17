//! Non-ISO date arithmetic through the host's own code (`plain_dates.rs`)
//! against the proposal's NonISODateAdd and NonISODateUntil transcribed in
//! `oracle_calendars.rs` over ICU4C's calendars: years and months added from
//! leap months (Hebrew Adar I, Chinese and Dangi leap months), the thirteenth
//! month of Coptic and Ethiopic, and the last days of months, under constrain
//! and reject, with weeks and days balanced after; and differences in every
//! date unit across the same dates.
use std::collections::BTreeMap;
use temporal_rs::options::{DifferenceSettings, Overflow, Unit};
use temporal_rs::{Calendar, Duration};
use temporal_sweeps::oracle_calendars::Model;
use temporal_sweeps::oracle_dates::{day_number, from_day_number};
use temporal_sweeps::plain_dates;

const CALENDARS: [&str; 15] = ["buddhist", "chinese", "coptic", "dangi", "ethioaa", "ethiopic", "gregory", "hebrew", "indian",
    "islamic-civil", "islamic-tbla", "islamic-umalqura", "japanese", "persian", "roc"];

/// (years, months, weeks, days)
const DURATIONS: [(i64, i64, i64, i64); 22] = [
    (1, 0, 0, 0), (-1, 0, 0, 0), (2, 0, 0, 0), (-3, 0, 0, 0), (8, 0, 0, 0), (19, 0, 0, 0), (-19, 0, 0, 0),
    (0, 1, 0, 0), (0, -1, 0, 0), (0, 12, 0, 0), (0, -12, 0, 0), (0, 13, 0, 0), (0, -13, 0, 0), (0, 25, 0, 0),
    (1, 1, 0, 0), (-1, -1, 0, 0), (2, 5, 0, 0), (1, 0, 0, 29), (0, 1, 0, 1), (0, 0, 2, 0), (0, 0, 0, 30), (-1, 0, -1, -1),
];

const SPANS: [i64; 19] = [1, 29, 30, 58, 59, 177, 353, 354, 355, 383, 384, 385, 767, 2000, -1, -30, -354, -384, -2000];

fn iso(n: i64) -> (i32, u8, u8) {
    let (y, m, d) = from_day_number(n);
    (y as i32, m as u8, d as u8)
}

struct Tally { checked: u64, unknown: u64, refused: u64, leap_constrained: u64, wrong: BTreeMap<String, (u64, Vec<String>)> }

fn sweep(calendar: &'static str) -> Tally {
    let (from, to) = (day_number(1900, 1, 1), day_number(2101, 1, 1));
    let model = Model::new(calendar);
    let cal = Calendar::try_from_utf8(calendar.as_bytes()).unwrap();
    let mut tally = Tally { checked: 0, unknown: 0, refused: 0, leap_constrained: 0, wrong: BTreeMap::new() };
    let starts: Vec<i64> = model.known_days().filter(|&n| (from..to).contains(&n)).filter(|&n| {
        let p = model.parts(n).unwrap();
        p.code.ends_with('L') || p.code == "M13" || p.day >= 28 || n % 23 == 0 || (calendar == "hebrew" && p.code == "M06")
    }).collect();
    for &n in &starts {
        let parts = model.parts(n).unwrap();
        for (y, m, w, d) in DURATIONS {
            let duration = Duration::new(y, m, w, d, 0, 0, 0, 0, 0, 0).unwrap();
            let Ok(constrained) = model.add(n, y, m, w, d, false) else { tally.unknown += 2; continue };
            let Ok(rejected) = model.add(n, y, m, w, d, true) else { tally.unknown += 2; continue };
            if parts.code.ends_with('L') && y != 0 && rejected.is_none() && constrained.is_some() { tally.leap_constrained += 1; }
            for (overflow, want) in [(Overflow::Constrain, constrained), (Overflow::Reject, rejected)] {
                tally.checked += 1;
                tally.refused += u64::from(want.is_none());
                let got = plain_dates::add(iso(n), cal.clone(), &duration, overflow).ok();
                if got != want.map(iso) {
                    note(&mut tally, "add", format!("{:?} ({} {} day {}) + {y}Y{m}M{w}W{d}D {overflow:?}: host {got:?}, oracle {:?}", iso(n), parts.year, parts.code, parts.day, want.map(iso)));
                }
            }
        }
        if n % 5 != 0 && !parts.code.ends_with('L') { continue; }
        for span in SPANS {
            for (unit, code) in [(Unit::Year, 3u8), (Unit::Month, 2), (Unit::Week, 1), (Unit::Day, 0)] {
                let Ok(want) = model.until(n, n + span, code) else { tally.unknown += 1; continue };
                tally.checked += 1;
                let mut settings = DifferenceSettings::default();
                settings.largest_unit = Some(unit);
                let got = plain_dates::until(iso(n), iso(n + span), cal.clone(), settings).ok().map(|x| [x.years(), x.months(), x.weeks(), x.days()]);
                if got != Some(want) {
                    note(&mut tally, "until", format!("{:?} ({} {} day {}) until {:?} {unit:?}: host {got:?}, oracle {want:?}", iso(n), parts.year, parts.code, parts.day, iso(n + span)));
                }
            }
        }
    }
    tally
}

fn note(tally: &mut Tally, kind: &str, example: String) {
    let entry = tally.wrong.entry(kind.to_string()).or_default();
    entry.0 += 1;
    if entry.1.len() < 3 { entry.1.push(example); }
}

#[test]
fn should_add_and_difference_across_leap_months_as_the_proposal_does() {
    let tallies: Vec<(&str, Tally)> = std::thread::scope(|s| {
        let handles: Vec<_> = CALENDARS.iter().map(|&c| s.spawn(move || (c, sweep(c)))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let total = |f: fn(&Tally) -> u64| tallies.iter().map(|(_, t)| f(t)).sum::<u64>();
    eprintln!("leap months: {} checked ({} refused under reject, {} leap months constrained), {} where the model lacks a year",
        total(|t| t.checked), total(|t| t.refused), total(|t| t.leap_constrained), total(|t| t.unknown));
    let mut kinds = 0;
    for (calendar, tally) in &tallies {
        for (kind, (count, examples)) in &tally.wrong {
            kinds += 1;
            eprintln!("  {calendar} {kind}: {count} differ — {}", examples.join("; "));
        }
    }
    assert!(kinds == 0, "non-ISO arithmetic disagrees with the proposal in {kinds} calendar operations");
}
