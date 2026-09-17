//! RFC 9557 strings with annotations, as TC39 reads them (spec/abstractops.html
//! Annotation grammar and ParseISODateTime): the first `u-ca` names the
//! calendar, a second is refused if either is critical, an unknown key is
//! refused only when critical, keys are lower case and values alphanumeric
//! segments, a zone annotation comes before the rest, a second of 60 reads as
//! 59, and a plain date refuses `Z`. Strings are built from parts, so the
//! oracle knows what each means; every one is parsed as a zoned value and as a
//! date through the host.
use temporal_rs::options::Disambiguation;
use temporal_rs::{PlainTime, TimeZone};
use temporal_sweeps::oracle::{naive, resolve, Ymd, HOUR};
use temporal_sweeps::{plain_dates, zoned_ops};

/// (text, well formed, key is u-ca, critical, calendar value lowercased)
const POOL: [(&str, bool, bool, bool, &str); 20] = [
    ("[u-ca=hebrew]", true, true, false, "hebrew"),
    ("[!u-ca=hebrew]", true, true, true, "hebrew"),
    ("[u-ca=japanese]", true, true, false, "japanese"),
    ("[!u-ca=japanese]", true, true, true, "japanese"),
    ("[u-ca=HEBREW]", true, true, false, "hebrew"),
    ("[u-ca=iso8601]", true, true, false, "iso8601"),
    ("[u-ca=nosuch]", true, true, false, "nosuch"),
    ("[foo=bar]", true, false, false, ""),
    ("[!foo=bar]", true, false, true, ""),
    ("[_x1-y=a-b-9]", true, false, false, ""),
    ("[f=a]", true, false, false, ""),
    ("[!f=a]", true, false, true, ""),
    ("[foo=ab-c]", true, false, false, ""),
    ("[foo=-ab]", false, false, false, ""),
    ("[foo=ab-]", false, false, false, ""),
    ("[foo=a--b]", false, false, false, ""),
    ("[u-ca=hebrew-]", false, false, false, ""),
    ("[fo=o=x]", false, false, false, ""),
    ("[Foo=bar]", false, false, false, ""),
    ("[u-ca=]", false, false, false, ""),
];

/// The calendar the annotations name, or `None` where the spec refuses them.
fn calendar_of(annotations: &[usize]) -> Option<String> {
    let mut calendar: Option<&str> = None;
    let mut critical = false;
    for &k in annotations {
        let (_, ok, is_calendar, is_critical, value) = POOL[k];
        if !ok { return None; }
        if is_calendar {
            if calendar.is_none() {
                calendar = Some(value);
                critical = is_critical;
            } else if is_critical || critical {
                return None;
            }
        } else if is_critical {
            return None;
        }
    }
    match calendar.unwrap_or("iso8601") {
        "nosuch" => None,
        id => Some(id.to_string()),
    }
}

#[test]
fn should_read_annotations_as_the_spec_does() {
    let ny = TimeZone::try_from_identifier_str("America/New_York").unwrap();
    let date = Ymd { year: 2026, month: 6, day: 15 };
    let wall_of = |second: u8| naive(date, &PlainTime::try_new(12, 34, second, 789, 0, 0).unwrap()).unwrap();
    let mut sequences: Vec<Vec<usize>> = vec![vec![]];
    for a in 0..POOL.len() {
        sequences.push(vec![a]);
        for b in 0..POOL.len() {
            sequences.push(vec![a, b]);
            for c in 0..POOL.len() { sequences.push(vec![a, b, c]); }
        }
    }
    let (mut checked, mut wrong) = (0u64, vec![]);
    for second in ["56", "60"] {
        let wall = wall_of(if second == "60" { 59 } else { 56 });
        // (text, epoch it names in New York, or None for a refusal; allowed on a date)
        let offsets: [(&str, Option<i128>, bool); 5] = [
            ("", resolve(wall, ny, Disambiguation::Compatible), true),
            ("-04:00", Some(wall + 4 * HOUR), true),
            ("-0400", Some(wall + 4 * HOUR), true),
            ("+01:00", None, true),
            ("Z", Some(wall), false),
        ];
        for (offset, epoch, date_allows) in offsets {
            for zone in ["[America/New_York]", "[!America/New_York]", "", "AFTER"] {
                for seq in &sequences {
                    let annotations: String = seq.iter().map(|k| POOL[*k].0).collect();
                    let text = match zone {
                        "AFTER" => format!("2026-06-15T12:34:{second}.789{offset}{annotations}[America/New_York]"),
                        z => format!("2026-06-15T12:34:{second}.789{offset}{z}{annotations}"),
                    };
                    let calendar = calendar_of(seq);
                    // Zoned: the zone annotation must be there and first.
                    let zone_first = zone == "[America/New_York]" || zone == "[!America/New_York]" || (zone == "AFTER" && seq.is_empty());
                    let want_zoned = if zone_first { calendar.clone().zip(epoch) } else { None };
                    let got_zoned = zoned_ops::parse(&text).ok().map(|z| (z.calendar().identifier().to_string(), z.epoch_nanoseconds().as_i128()));
                    checked += 1;
                    if got_zoned != want_zoned && wrong.len() < 16 {
                        wrong.push(format!("zoned {text}: host {got_zoned:?}, oracle {want_zoned:?}"));
                    }
                    // A date: no Z; a zone annotation may be absent; one placed after
                    // the other annotations is not a zone annotation at all.
                    let want_date = if date_allows && zone != "AFTER" { calendar.map(|c| ((2026, 6, 15), c)) } else if zone == "AFTER" && date_allows && seq.is_empty() { Some(((2026, 6, 15), "iso8601".to_string())) } else { None };
                    let got_date = plain_dates::parse(&text).ok().map(|(d, c)| (d, c.identifier().to_string()));
                    checked += 1;
                    if got_date != want_date && wrong.len() < 16 {
                        wrong.push(format!("date {text}: host {got_date:?}, oracle {want_date:?}"));
                    }
                }
            }
        }
    }
    eprintln!("annotations: {checked} strings checked");
    assert!(wrong.is_empty(), "annotated strings disagree with the spec:\n{}", wrong.join("\n"));
}
