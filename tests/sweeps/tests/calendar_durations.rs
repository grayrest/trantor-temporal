//! Durations rounded, totalled and compared from dates on the fifteen non-ISO
//! calendars, through the host's own code (`durations.rs`), against the spec in
//! `oracle_durations.rs` with CalendarDateAdd and CalendarDateUntil the
//! proposal's over ICU4C's calendars: anchored at leap months, the thirteenth
//! month, month ends and a stride of other days, 1900-2100. Totals compare
//! exactly where a double holds the spec's quotient, and to 15 significant
//! digits otherwise; anything reaching a year the model lacks is skipped.
use std::collections::BTreeMap;
use temporal_rs::options::{RoundingIncrement, RoundingMode as M, RoundingOptions, Unit};
use temporal_rs::{Calendar, Duration};
use temporal_sweeps::durations;
use temporal_sweeps::oracle_calendars::Model;
use temporal_sweeps::oracle_dates::{day_number, from_day_number, Iso};
use temporal_sweeps::oracle_durations::{compare, round, total, Fields};
use temporal_sweeps::oracle_round::Reckoning;

const CALENDARS: [&str; 15] = ["buddhist", "chinese", "coptic", "dangi", "ethioaa", "ethiopic", "gregory", "hebrew", "indian",
    "islamic-civil", "islamic-tbla", "islamic-umalqura", "japanese", "persian", "roc"];
const ALL: [M; 9] = [M::Ceil, M::Floor, M::Expand, M::Trunc, M::HalfCeil, M::HalfFloor, M::HalfExpand, M::HalfTrunc, M::HalfEven];
const SOME: [M; 3] = [M::Trunc, M::HalfExpand, M::Ceil];
const EXACT_IN_F64: i128 = 1 << 53;

const SAMPLES: [Fields; 16] = [
    [0, 1, 0, 0, 0, 0, 0, 0, 0, 0], [0, 1, 0, 15, 0, 0, 0, 0, 0, 0], [0, 2, 0, 29, 12, 0, 0, 0, 0, 0],
    [1, 2, 3, 4, 5, 6, 7, 8, 9, 10], [0, -1, 0, -2, 0, 0, 0, 0, 0, 0], [0, 0, 0, 14, 0, 0, 0, 0, 0, 0],
    [0, 0, 3, 0, 0, 0, 0, 0, 0, 0], [0, 0, 0, 400, 0, 0, 0, 0, 0, 0], [1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [-1, -6, 0, 0, 0, 0, 0, 0, 0, 0], [0, 0, 0, 29, 0, 0, 0, 0, 0, 0], [0, 0, 0, 30, 12, 0, 0, 0, 0, 0],
    [0, 13, 0, 0, 0, 0, 0, 0, 0, 0], [2, 0, 0, 0, 0, 0, 0, 0, 0, 0], [0, 0, 0, 383, 0, 0, 0, 0, 0, 0],
    [0, -12, 0, -1, -1, 0, 0, 0, 0, 0],
];

/// (largest, smallest, increment, modes)
const ROUNDS: [(Unit, Unit, u32, &[M]); 9] = [
    (Unit::Month, Unit::Nanosecond, 1, &[M::Trunc]), (Unit::Year, Unit::Month, 1, &ALL), (Unit::Year, Unit::Year, 1, &ALL),
    (Unit::Month, Unit::Day, 1, &SOME), (Unit::Month, Unit::Week, 1, &SOME), (Unit::Year, Unit::Day, 1, &SOME),
    (Unit::Month, Unit::Month, 3, &SOME), (Unit::Day, Unit::Day, 1, &SOME), (Unit::Week, Unit::Day, 2, &SOME),
];

const UNITS: [Unit; 6] = [Unit::Year, Unit::Month, Unit::Week, Unit::Day, Unit::Hour, Unit::Nanosecond];

struct Tally { checked: u64, approximate: u64, declined: u64, wrong: BTreeMap<String, (u64, Vec<String>)> }

impl Tally {
    fn check(&mut self, model: &Model, kind: String, label: impl FnOnce() -> String, agrees: bool) {
        if model.take_declined() {
            self.declined += 1;
            return;
        }
        self.checked += 1;
        if !agrees {
            let entry = self.wrong.entry(kind).or_default();
            entry.0 += 1;
            if entry.1.len() < 2 { entry.1.push(label()); }
        }
    }
}

fn duration(f: &Fields) -> Duration {
    Duration::new(f[0] as i64, f[1] as i64, f[2] as i64, f[3] as i64, f[4] as i64, f[5] as i64, f[6] as i64, f[7] as i64, f[8], f[9]).unwrap()
}

fn fields(d: &Duration) -> Fields {
    [d.years().into(), d.months().into(), d.weeks().into(), d.days().into(), d.hours().into(), d.minutes().into(),
     d.seconds().into(), d.milliseconds().into(), d.microseconds(), d.nanoseconds()]
}

fn sweep(calendar: &'static str) -> Tally {
    let (lo, hi) = (day_number(1900, 1, 1), day_number(2101, 1, 1));
    let model = Model::new(calendar);
    let reckoning = Reckoning::Model(&model);
    let cal = Calendar::try_from_utf8(calendar.as_bytes()).unwrap();
    let mut tally = Tally { checked: 0, approximate: 0, declined: 0, wrong: BTreeMap::new() };
    let anchors: Vec<i64> = model.known_days().filter(|&n| (lo..hi).contains(&n)).filter(|&n| {
        let p = model.parts(n).unwrap();
        (p.code.ends_with('L') && matches!(p.day, 1 | 15 | 29 | 30)) || (p.code == "M13" && p.day >= 5) || (p.day >= 29 && n % 23 == 0) || n % 331 == 0
    }).collect();
    for &n in &anchors {
        let (y, m, d) = from_day_number(n);
        let from: Iso = (y as i32, m as u8, d as u8);
        let parts = model.parts(n).unwrap();
        let at = format!("from {from:?} ({} {} day {})", parts.year, parts.code, parts.day);
        for (k, f) in SAMPLES.iter().enumerate() {
            let dur = duration(f);
            for (largest, smallest, increment, modes) in ROUNDS {
                for &mode in modes {
                    let mut o = RoundingOptions::default();
                    o.largest_unit = Some(largest);
                    o.smallest_unit = Some(smallest);
                    o.rounding_mode = Some(mode);
                    o.increment = Some(RoundingIncrement::try_new(increment).unwrap());
                    model.take_declined();
                    let want = round(f, largest, smallest, increment.into(), mode, Some(from), reckoning);
                    let got = durations::round(&dur, o, Some(from), cal.clone()).ok().map(|r| fields(&r));
                    tally.check(&model, format!("round {largest:?}/{smallest:?}/{increment} {mode:?}"),
                        || format!("{f:?} {at}: host {got:?}, oracle {want:?}"), got == want);
                }
            }
            for unit in UNITS {
                model.take_declined();
                let want = total(f, unit, Some(from), reckoning);
                let got = durations::total(&dur, unit, Some(from), cal.clone()).ok();
                let agrees = match (got, want) {
                    (Some(g), Some((num, den))) if num.abs() <= EXACT_IN_F64 && den <= EXACT_IN_F64 => g == num as f64 / den as f64,
                    (Some(g), Some((num, den))) => {
                        tally.approximate += 1;
                        let w = num as f64 / den as f64;
                        (g - w).abs() <= w.abs() * 1e-15
                    }
                    (g, w) => g.is_none() && w.is_none(),
                };
                tally.check(&model, format!("total {unit:?}"), || format!("{f:?} {at}: host {got:?}, oracle {want:?}"), agrees);
            }
            for other in [&SAMPLES[(k + 1) % SAMPLES.len()], &[0, 0, 0, 30, 0, 0, 0, 0, 0, 0], &[0, 1, 0, 0, 0, 0, 0, 0, 0, 0]] {
                model.take_declined();
                let want = compare(f, other, Some(from), reckoning);
                let got = durations::compare(&dur, &duration(other), Some(from), cal.clone()).ok().map(|o| o as i8);
                tally.check(&model, "compare".into(), || format!("{f:?} against {other:?} {at}: host {got:?}, oracle {want:?}"), got == want);
            }
        }
    }
    tally
}

#[test]
fn should_round_total_and_compare_from_non_iso_dates_as_the_spec_does() {
    let tallies: Vec<(&str, Tally)> = std::thread::scope(|s| {
        let handles: Vec<_> = CALENDARS.iter().map(|&c| s.spawn(move || (c, sweep(c)))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let sum = |f: fn(&Tally) -> u64| tallies.iter().map(|(_, t)| f(t)).sum::<u64>();
    eprintln!("calendar durations: {} checked, {} totals to 15 digits, {} reaching a year the model lacks",
        sum(|t| t.checked), sum(|t| t.approximate), sum(|t| t.declined));
    let mut kinds = 0;
    for (calendar, tally) in &tallies {
        for (kind, (count, examples)) in &tally.wrong {
            kinds += 1;
            eprintln!("  {calendar} {kind}: {count} differ — {}", examples.join("; "));
        }
    }
    assert!(kinds == 0, "non-ISO duration rounding, totals or comparison disagree with the spec in {kinds} calendar operations");
}
