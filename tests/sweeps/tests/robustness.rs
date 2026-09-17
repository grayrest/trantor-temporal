//! No input panics the host. A panic inside a host call unwinds into Roc and
//! takes the program down, so every host operation the sweeps compile is fed
//! generated input — random and mutated IXDTF and duration strings, date
//! records naming no date, durations at the integer limits, rounding options
//! with any increment, epochs past the range, wall clocks in any zone — and
//! each call must return, `Ok` or `Err`. Generation is seeded, so a failure
//! reproduces; the first input of each panicking kind is shown.
//!
//! The calendars whose arithmetic steps month by month (Chinese, Dangi, Hebrew,
//! the Islamic ones) are given dates in 1800-2200 and spans of centuries:
//! their differences grow faster than the span — a Chinese month difference
//! across 10,000 years takes a third of a second, across the whole range
//! minutes — so a sweep over the full range would not finish (D-T2-38).
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use temporal_rs::options::{DifferenceSettings, Disambiguation, Overflow, RoundingIncrement, RoundingMode as M, RoundingOptions, Unit};
use temporal_rs::provider::TransitionDirection;
use temporal_rs::{Calendar, Duration, PlainTime, TimeZone, ZonedDateTime};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::{annotations, durations, plain_dates, transition, zoned_ops};

/// xorshift64*: small, seeded, and the same on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize { (self.next() % n as u64) as usize }
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T { xs[self.below(xs.len())] }
}

const SEEDS: [&str; 16] = [
    "2026-03-08T02:30:00-05:00[America/New_York]", "2024-02-29T23:59:60.123456789+14:00[Pacific/Kiritimati][u-ca=hebrew]",
    "-271821-04-20T00:00Z[UTC]", "+275760-09-13T00:00:00+00:00[!UTC][!u-ca=chinese]", "2026-06-15[u-ca=japanese][foo=bar]",
    "20260615T123456,5-0330[America/St_Johns]", "1970-01-01T00:00:00+00:00:00.000000001[+00:00]", "2026-W01-1",
    "P1Y2M3W4DT5H6M7.008009010S", "-PT0.000000001S", "PT9007199254740991S", "P99999999999999999999Y",
    "2026-02-30", "--12-25", "2026-13", "T12:30",
];
const ALPHABET: &[u8] = b"0123456789-+:.,TtZz[]=!u-caPYMWDHS /_eEAmrihnosw\x00\xff";
const CALENDARS: [&str; 16] = ["iso8601", "buddhist", "chinese", "coptic", "dangi", "ethioaa", "ethiopic", "gregory", "hebrew", "indian",
    "islamic-civil", "islamic-tbla", "islamic-umalqura", "japanese", "persian", "roc"];
const ZONES: [&str; 8] = ["UTC", "America/New_York", "Australia/Lord_Howe", "Pacific/Apia", "Antarctica/Troll", "+14:00", "-23:59", "Asia/Kathmandu"];
const UNITS: [Unit; 10] = [Unit::Year, Unit::Month, Unit::Week, Unit::Day, Unit::Hour, Unit::Minute, Unit::Second, Unit::Millisecond, Unit::Microsecond, Unit::Nanosecond];
const MODES: [M; 9] = [M::Ceil, M::Floor, M::Expand, M::Trunc, M::HalfCeil, M::HalfFloor, M::HalfExpand, M::HalfTrunc, M::HalfEven];

fn text(rng: &mut Rng) -> String {
    let mut bytes: Vec<u8> = match rng.below(3) {
        0 => (0..rng.below(64)).map(|_| rng.pick(ALPHABET)).collect(),
        1 => (0..rng.below(24)).map(|_| rng.next() as u8).collect(),
        _ => rng.pick(&SEEDS).as_bytes().to_vec(),
    };
    for _ in 0..rng.below(6) {
        let at = if bytes.is_empty() { 0 } else { rng.below(bytes.len() + 1) };
        match rng.below(4) {
            0 => bytes.insert(at, rng.pick(ALPHABET)),
            1 if at < bytes.len() => { bytes.remove(at); }
            2 if at < bytes.len() => bytes[at] = rng.pick(ALPHABET),
            _ => { let chunk = rng.pick(&SEEDS).as_bytes(); let n = rng.below(chunk.len() + 1); bytes.splice(at..at, chunk[..n].iter().copied()); }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn int(rng: &mut Rng) -> i64 {
    match rng.below(4) {
        0 => rng.pick(&[0, 1, -1, 7, 12, 13, 31, 1000, 999_999, i64::MAX, i64::MIN, i64::MAX / 2, 1 << 53, -(1 << 53), 4_294_967_296]),
        1 => rng.next() as i64,
        _ => (rng.next() % 4000) as i64 - 2000,
    }
}

fn date(rng: &mut Rng) -> (i32, u8, u8) {
    let (any, modern) = (rng.next() as i32, 1800 + rng.below(400) as i32);
    let year = rng.pick(&[-271_822, -271_821, -1, 0, 1970, 2024, 275_760, 275_761, i32::MAX, i32::MIN, any, modern]);
    (year, rng.next() as u8 % 16, rng.next() as u8 % 34)
}

fn duration(rng: &mut Rng) -> Option<Duration> {
    let f: Vec<i64> = (0..10).map(|_| if rng.below(3) == 0 { int(rng) } else { 0 }).collect();
    Duration::new(f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8].into(), f[9].into()).ok()
}

fn settings(rng: &mut Rng) -> Option<(Unit, Unit, M, RoundingIncrement)> {
    let any = rng.next() as u32;
    let increment = RoundingIncrement::try_new(rng.pick(&[0, 1, 2, 3, 5, 15, 24, 1000, 1_000_000_000, u32::MAX, any])).ok()?;
    Some((rng.pick(&UNITS), rng.pick(&UNITS), rng.pick(&MODES), increment))
}

fn calendar(rng: &mut Rng) -> Calendar {
    Calendar::try_from_utf8(rng.pick(&CALENDARS).as_bytes()).unwrap()
}

/// Whether the calendar's arithmetic steps month by month.
fn is_stepped(calendar: &Calendar) -> bool {
    matches!(calendar.identifier(), "chinese" | "dangi" | "hebrew" | "islamic-civil" | "islamic-tbla" | "islamic-umalqura")
}

/// A date in 1800-2200, for a stepped calendar.
fn modern(d: (i32, u8, u8)) -> (i32, u8, u8) {
    (1800 + d.0.rem_euclid(400), d.1, d.2)
}

/// A duration spanning at most a few centuries, for a stepped calendar.
fn short(rng: &mut Rng) -> Option<Duration> {
    let limits = [300, 3600, 15_000, 100_000, 2_400_000, 1 << 32, 1 << 38, 1 << 45, 1 << 50, 1 << 60];
    let f: Vec<i64> = limits.iter().map(|l| if rng.below(3) == 0 { (rng.next() % (2 * l)) as i64 - *l as i64 } else { 0 }).collect();
    Duration::new(f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8].into(), f[9].into()).ok()
}

#[test]
fn should_return_from_every_host_operation_whatever_it_is_given() {
    std::panic::set_hook(Box::new(|_| {}));
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut calls = 0u64;
    let mut panics: BTreeMap<&str, (u64, String)> = BTreeMap::new();
    let mut slowest: BTreeMap<&str, (std::time::Duration, String)> = BTreeMap::new();
    let mut guard = |name: &'static str, input: &dyn Fn() -> String, f: &mut dyn FnMut()| {
        calls += 1;
        let began = std::time::Instant::now();
        if catch_unwind(AssertUnwindSafe(f)).is_err() {
            let entry = panics.entry(name).or_insert((0, input()));
            entry.0 += 1;
        }
        let took = began.elapsed();
        if took > slowest.get(name).map_or(std::time::Duration::ZERO, |s| s.0) {
            slowest.insert(name, (took, input()));
        }
    };
    for _ in 0..300_000 {
        let s = text(&mut rng);
        guard("zoned parse", &|| format!("{s:?}"), &mut || { let _ = zoned_ops::parse(&s).map(|z| zoned_ops::to_str(&z)); });
        guard("date parse", &|| format!("{s:?}"), &mut || { let _ = plain_dates::parse(&s); });
        guard("duration parse", &|| format!("{s:?}"), &mut || { let _ = durations::parse(&s); });
        guard("annotations", &|| format!("{s:?}"), &mut || { let _ = annotations::normalize(&s); });
    }
    for _ in 0..60_000 {
        let cal = calendar(&mut rng);
        let (mut a, mut b) = (date(&mut rng), date(&mut rng));
        let mut dur = duration(&mut rng);
        if is_stepped(&cal) {
            (a, b, dur) = (modern(a), modern(b), short(&mut rng));
        }
        let overflow = if rng.below(2) == 0 { Overflow::Constrain } else { Overflow::Reject };
        let opts = settings(&mut rng);
        let input = || format!("{a:?} {b:?} {} {dur:?} {opts:?}", cal.identifier());
        guard("date fields", &input, &mut || { let _ = plain_dates::fields(a, cal.clone()); });
        if let Some(d) = &dur {
            guard("date add", &input, &mut || { let _ = plain_dates::add(a, cal.clone(), d, overflow); });
        }
        if let Some((largest, smallest, mode, increment)) = opts {
            let mut s = DifferenceSettings::default();
            (s.largest_unit, s.smallest_unit, s.rounding_mode, s.increment) = (Some(largest), Some(smallest), Some(mode), Some(increment));
            guard("date until", &input, &mut || { let _ = plain_dates::until(a, b, cal.clone(), s); });
            if let Some(d) = &dur {
                let mut o = RoundingOptions::default();
                (o.largest_unit, o.smallest_unit, o.rounding_mode, o.increment) = (Some(largest), Some(smallest), Some(mode), Some(increment));
                let from = if rng.below(3) == 0 { None } else { Some(a) };
                guard("duration round", &input, &mut || { let _ = durations::round(d, o, from, cal.clone()); });
                guard("duration total", &input, &mut || { let _ = durations::total(d, smallest, from, cal.clone()); });
                if let Some(other) = if is_stepped(&cal) { short(&mut rng) } else { duration(&mut rng) } {
                    guard("duration compare", &input, &mut || { let _ = durations::compare(d, &other, from, cal.clone()); });
                }
            }
        }
    }
    for _ in 0..40_000 {
        let zone = TimeZone::try_from_str(rng.pick(&ZONES)).unwrap();
        let cal = calendar(&mut rng);
        let epoch: i128 = match rng.below(3) {
            0 => rng.pick(&[0, 8_640_000_000_000_000_000_000, -8_640_000_000_000_000_000_000, 8_640_000_000_000_000_000_001, i128::MAX, i128::MIN]),
            1 => (rng.next() as i64 as i128) * 1_000_000_000_000,
            _ => (rng.next() as i64 as i128) % 8_640_000_000_000_000_000_000,
        };
        let (mut d, mut dur, opts) = (date(&mut rng), duration(&mut rng), settings(&mut rng));
        let mut epoch = epoch;
        if is_stepped(&cal) {
            (d, dur) = (modern(d), short(&mut rng));
            epoch = -5_364_662_400_000_000_000 + epoch.rem_euclid(12_622_780_800_000_000_000);
        }
        let time = PlainTime::try_new(rng.next() as u8 % 26, rng.next() as u8 % 62, rng.next() as u8 % 62, rng.next() as u16 % 1002, rng.next() as u16 % 1002, rng.next() as u16 % 1002);
        let dis = rng.pick(&[Disambiguation::Compatible, Disambiguation::Earlier, Disambiguation::Later, Disambiguation::Reject]);
        let input = || format!("{epoch} {d:?} {time:?} {} {dur:?} {opts:?}", cal.identifier());
        if let Ok(t) = &time {
            guard("from wall clock", &input, &mut || { let _ = zoned_ops::from_wall_clock(d, t, zone, cal.clone(), dis); });
        }
        let Ok(z) = ZonedDateTime::try_new_with_provider(epoch, zone, cal.clone(), &EXACT) else { continue };
        guard("zoned printing", &input, &mut || { let _ = zoned_ops::to_str(&z); });
        guard("with plain date", &input, &mut || { let _ = zoned_ops::with_plain_date(&z, d, dis); });
        guard("start of day", &input, &mut || { let _ = z.start_of_day_with_provider(&EXACT); let _ = z.hours_in_day_with_provider(&EXACT); });
        guard("transition", &input, &mut || { let _ = transition::transition(&z, TransitionDirection::Next); let _ = transition::transition(&z, TransitionDirection::Previous); });
        if let Some(d) = &dur {
            guard("zoned add", &input, &mut || { let _ = z.add_with_provider(d, Some(Overflow::Constrain), &EXACT); });
        }
        if let Some((largest, smallest, mode, increment)) = opts {
            let other = ZonedDateTime::try_new_with_provider(epoch / 3, zone, cal.clone(), &EXACT);
            let mut s = DifferenceSettings::default();
            (s.largest_unit, s.smallest_unit, s.rounding_mode, s.increment) = (Some(largest), Some(smallest), Some(mode), Some(increment));
            if let Ok(o) = &other {
                guard("zoned until", &input, &mut || { let _ = z.until_with_provider(o, s, &EXACT); let _ = zoned_ops::equals(&z, o); });
            }
            let mut r = RoundingOptions::default();
            (r.smallest_unit, r.rounding_mode, r.increment) = (Some(smallest), Some(mode), Some(increment));
            guard("zoned round", &input, &mut || { let _ = z.round_with_provider(r, &EXACT); });
        }
    }
    let _ = std::panic::take_hook();
    for (name, (took, input)) in &slowest { eprintln!("  slowest {name}: {took:?}, on {input}"); }
    eprintln!("robustness: {calls} host calls, {} panicking kinds", panics.len());
    for (name, (count, first)) in &panics { eprintln!("  {name}: {count} panics, first on {first}"); }
    assert!(panics.is_empty(), "host operations panicked: {:?}", panics.keys().collect::<Vec<_>>());
}
