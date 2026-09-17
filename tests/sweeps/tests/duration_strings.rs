//! ISO 8601 duration strings as TC39 parses and prints them, against an oracle
//! built from the spec's grammar (spec/abstractops.html TemporalDurationString,
//! ParseTemporalDurationString) and TemporalDurationToString: generated
//! durations print and parse back, with designators in either case, `+`, and
//! `,` for the decimal point; fractional hours and minutes cascade into smaller
//! units exactly; malformed and out-of-range strings are refused, including a
//! unit named twice, which temporal_rs alone accepts (D-T2-24).
use temporal_rs::Duration;

type Fields = [i128; 10];
const NS: [i128; 4] = [3_600_000_000_000, 60_000_000_000, 1_000_000_000, 1];

/// The spec's grammar and semantics; `None` for a string it refuses.
fn oracle_parse(s: &str) -> Option<Fields> {
    let b = s.as_bytes();
    let mut i = 0;
    let negative = match b.first() { Some(b'-') => { i += 1; true } Some(b'+') => { i += 1; false } _ => false };
    if !matches!(b.get(i), Some(b'P' | b'p')) { return None; }
    i += 1;
    let digits = |i: &mut usize| -> Option<i128> {
        let start = *i;
        while b.get(*i).is_some_and(u8::is_ascii_digit) { *i += 1; }
        if *i == start { return None; }
        s[start..*i].parse::<i128>().ok()
    };
    let mut f: Fields = [0; 10];
    let (mut any, mut rank) = (false, 0);
    // Date parts in order: Y, M, W, D.
    while i < b.len() && b[i].is_ascii_digit() {
        let v = digits(&mut i)?;
        let unit = match b.get(i)?.to_ascii_uppercase() { b'Y' => 1, b'M' => 2, b'W' => 3, b'D' => 4, _ => return None };
        if unit <= rank { return None; }
        rank = unit;
        f[unit - 1] = v;
        i += 1;
        any = true;
    }
    if i < b.len() {
        if !matches!(b[i], b'T' | b't') { return None; }
        i += 1;
        let (mut trank, mut time_any, mut fraction_seen) = (0, false, false);
        while i < b.len() {
            if fraction_seen { return None; }
            let v = digits(&mut i)?;
            let mut fraction: Option<(i128, u32)> = None;
            if matches!(b.get(i), Some(b'.' | b',')) {
                i += 1;
                let start = i;
                while b.get(i).is_some_and(u8::is_ascii_digit) { i += 1; }
                let len = (i - start) as u32;
                if !(1..=9).contains(&len) { return None; }
                fraction = Some((s[start..i].parse().ok()?, len));
            }
            let unit = match b.get(i)?.to_ascii_uppercase() { b'H' => 1, b'M' => 2, b'S' => 3, _ => return None };
            if unit <= trank { return None; }
            trank = unit;
            i += 1;
            // A written value stays in its own field.
            f[3 + unit] = v;
            // A fraction is exact in nanoseconds (at most nine digits of a unit of
            // at least a second) and cascades into the fields below, each floored.
            if let Some((digits, scale)) = fraction {
                let mut rest = digits * NS[unit - 1] / 10i128.pow(scale);
                let below: &[(usize, i128)] = match unit {
                    1 => &[(5, NS[1]), (6, NS[2]), (7, 1_000_000), (8, 1_000), (9, 1)],
                    2 => &[(6, NS[2]), (7, 1_000_000), (8, 1_000), (9, 1)],
                    _ => &[(7, 1_000_000), (8, 1_000), (9, 1)],
                };
                for (k, len) in below {
                    f[*k] = rest / len;
                    rest %= len;
                }
                fraction_seen = true;
            }
            time_any = true;
        }
        if !time_any { return None; }
        any = true;
    }
    if !any { return None; }
    if negative { for v in f.iter_mut() { *v = -*v; } }
    // IsValidDuration: calendar fields below 2^32, whole seconds below 2^53.
    if f[..3].iter().any(|v| v.abs() >= 1 << 32) { return None; }
    let seconds = f[3] * 86_400 + f[4] * 3_600 + f[5] * 60 + f[6] + f[7] / 1_000 + f[8] / 1_000_000 + f[9] / 1_000_000_000;
    if seconds.abs() >= 1 << 53 { return None; }
    Some(f)
}

/// TemporalDurationToString with automatic precision.
fn oracle_print(f: &Fields) -> String {
    let sign = if f.iter().any(|v| *v < 0) { "-" } else { "" };
    let mut date = String::new();
    for (v, d) in f[..4].iter().zip(["Y", "M", "W", "D"]) {
        if *v != 0 { date += &format!("{}{d}", v.abs()); }
    }
    let mut time = String::new();
    if f[4] != 0 { time += &format!("{}H", f[4].abs()); }
    if f[5] != 0 { time += &format!("{}M", f[5].abs()); }
    let sub = (f[6] * 1_000_000_000 + f[7] * 1_000_000 + f[8] * 1_000 + f[9]).abs();
    let zero_above_seconds = f[..6].iter().all(|v| *v == 0);
    if sub != 0 || zero_above_seconds {
        let fraction = format!("{:09}", sub % 1_000_000_000);
        let fraction = fraction.trim_end_matches('0');
        time += &format!("{}{}{}S", sub / 1_000_000_000, if fraction.is_empty() { "" } else { "." }, fraction);
    }
    format!("{sign}P{date}{}{time}", if time.is_empty() { "" } else { "T" })
}

fn host(s: &str) -> Option<Fields> {
    temporal_sweeps::durations::parse(s).ok().map(|d: Duration| {
        [d.years().into(), d.months().into(), d.weeks().into(), d.days().into(), d.hours().into(), d.minutes().into(),
         d.seconds().into(), d.milliseconds().into(), d.microseconds(), d.nanoseconds()]
    })
}

#[test]
fn should_parse_and_print_duration_strings_as_the_spec_does() {
    let values = [1i128, 7, 23, 59, 60, 999, 1_000, 123_456_789];
    let (mut checked, mut wrong) = (0u64, vec![]);
    let mut check = |label: String, got: String, want: String| {
        checked += 1;
        if got != want && wrong.len() < 16 { wrong.push(format!("{label}: host {got}, oracle {want}")); }
    };
    // Every subset of units (2^10), each value, both signs, as printed.
    for mask in 1u32..1024 {
        for (k, v) in values.iter().enumerate() {
            let mut f: Fields = [0; 10];
            for (u, slot) in f.iter_mut().enumerate() {
                if mask & (1 << u) != 0 { *slot = v + (u as i128 * 3 + k as i128) % 5; }
            }
            // Sub-second fields as printed must fit their places.
            for slot in &mut f[7..10] { *slot %= 1000; }
            for negative in [false, true] {
                let g: Fields = if negative { f.map(|x| -x) } else { f };
                let text = oracle_print(&g);
                check(format!("{g:?} printed {text} parses as oracle"), format!("{:?}", host(&text)), format!("{:?}", oracle_parse(&text)));
                let lower = text.to_ascii_lowercase();
                check(lower.clone(), format!("{:?}", host(&lower)), format!("{:?}", oracle_parse(&lower)));
                if !negative {
                    check(format!("+{text}"), format!("{:?}", host(&format!("+{text}"))), format!("{:?}", oracle_parse(&format!("+{text}"))));
                }
                check(format!("{text},"), format!("{:?}", host(&text.replace('.', ","))), format!("{:?}", oracle_parse(&text.replace('.', ","))));
            }
        }
    }
    // Fractions on the smallest time unit given, and refusals.
    for unit in ["H", "M", "S"] {
        for whole in [0i128, 1, 25] {
            for frac in ["5", "25", "000000001", "999999999", "123456789", "1234567891", ""] {
                for prefix in ["PT", "P1DT", "PT2H", "-PT"] {
                    let text = format!("{prefix}{whole}{}{frac}{unit}", if frac.is_empty() { "" } else { "." });
                    check(text.clone(), format!("{:?}", host(&text)), format!("{:?}", oracle_parse(&text)));
                }
            }
        }
    }
    for bad in ["P", "PT", "P1Y1.5M", "PT1.5H30M", "PT1H1.5M2S", "P1DT", "P1D1Y", "1D", "P-1D", " P1D", "P1.5D", "P1W1M",
                "PT1.S", "PT.5S", "PT1H2H", "P1Y2Y", "p1y", "P4294967296Y", "P4294967295Y", "PT9007199254740991S", "PT9007199254740992S",
                "P104249991374DT7H36M31.999999999S", "P104249991375D", "PT1H0.5M", "PT0.5H0M", "+-P1D", "P1D ", "PT1S1H"] {
        check(format!("literal {bad}"), format!("{:?}", host(bad)), format!("{:?}", oracle_parse(bad)));
    }
    eprintln!("duration strings: {checked} checked");
    assert!(wrong.is_empty(), "duration strings disagree with the oracle:\n{}", wrong.join("\n"));
}
