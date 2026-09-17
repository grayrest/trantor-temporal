//! The host's own zoned operations (zoned_ops.rs) against the oracle, near every
//! transition of five eras in every zone: IXDTF parsing with no offset, the
//! instant's offset, the other offset in play at that transition, an offset to
//! the second, and `Z`; printing, and printing then parsing back; moving to
//! another zone; and moving to another date in all four modes.
use temporal_rs::options::Disambiguation;
use temporal_rs::{Calendar, PlainTime, TimeZone, ZonedDateTime};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::{candidates, local_date, naive, offset_at, resolve, Ymd, HOUR, SECOND};
use temporal_sweeps::zoned_ops::{from_wall_clock, parse, to_str, with_plain_date, with_time_zone};
use temporal_sweeps::zones;

const MINUTE: i128 = 60 * SECOND;

fn epoch(r: Result<ZonedDateTime, temporal_rs::TemporalError>) -> Option<i128> {
    r.ok().map(|z| z.epoch_nanoseconds().as_i128())
}

/// `2026-03-08T01:30:00.5` for a date and a time of day in nanoseconds.
fn wall_str(d: Ymd, tod: i128) -> String {
    let (h, m, s, frac) = (tod / HOUR, tod % HOUR / MINUTE, tod % MINUTE / SECOND, tod % SECOND);
    let fraction = if frac == 0 { String::new() } else { format!(".{frac:09}").trim_end_matches('0').to_string() };
    format!("{:04}-{:02}-{:02}T{h:02}:{m:02}:{s:02}{fraction}", d.year, d.month, d.day)
}

/// `+05:30`, or `-04:56:02` when `seconds`.
fn offset_str(ns: i128, seconds: bool) -> String {
    let sign = if ns < 0 { '-' } else { '+' };
    let a = ns.abs();
    if seconds {
        format!("{sign}{:02}:{:02}:{:02}", a / HOUR, a % HOUR / MINUTE, a % MINUTE / SECOND)
    } else {
        format!("{sign}{:02}:{:02}", a / HOUR, a % HOUR / MINUTE)
    }
}

/// An offset rounded to the minute, half away from zero, as IXDTF prints it.
fn to_minute(ns: i128) -> i128 {
    let r = (ns.abs() + 30 * SECOND) / MINUTE * MINUTE;
    if ns < 0 { -r } else { r }
}

/// TC39 InterpretISODateTimeOffset with an offset and `reject`: the first
/// instant the wall clock names whose offset is the one given — to the second
/// when the string gave seconds, else to the minute.
fn with_offset(wall: i128, given: i128, exact: bool, tz: TimeZone) -> Option<i128> {
    candidates(wall, tz).instants.into_iter().find(|i| {
        let o = offset_at(*i, tz).unwrap_or(i128::MIN);
        o == given || (!exact && to_minute(o) == given)
    })
}

#[test]
fn should_parse_print_and_move_as_the_oracle_does() {
    let utc = TimeZone::try_from_identifier_str("UTC").unwrap();
    let hebrew = Calendar::try_from_utf8(b"hebrew").unwrap();
    let modes = [Disambiguation::Compatible, Disambiguation::Earlier, Disambiguation::Later, Disambiguation::Reject];
    let (mut checked, mut wrong) = (0u64, vec![]);
    let mut check = |label: String, got: Option<i128>, want: Option<i128>| {
        checked += 1;
        if got != want && wrong.len() < 16 {
            wrong.push(format!("{label}: host {got:?}, oracle {want:?}"));
        }
    };
    let mut text_wrong = vec![];
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    for (id, tz) in zones() {
        for i in temporal_sweeps::near_era_sample(tz, 4) {
            let (Some(off), Some(d)) = (offset_at(i, tz), local_date(i, tz)) else { continue };
            let tod = i + off - naive(d, &midnight).unwrap();
            let wall = naive(d, &midnight).unwrap() + tod;
            let text = wall_str(d, tod);
            let other = [i - 24 * HOUR, i + 24 * HOUR].iter().filter_map(|x| offset_at(*x, tz)).find(|o| *o != off).unwrap_or(off + HOUR);
            check(format!("{id} parse {text}[zone]"), epoch(parse(&format!("{text}[{id}]"))), resolve(wall, tz, Disambiguation::Compatible));
            for (given, exact) in [(to_minute(off), false), (to_minute(other), false), (off, true)] {
                let s = format!("{text}{}[{id}]", offset_str(given, exact));
                check(format!("{id} parse {s}"), epoch(parse(&s)), with_offset(wall, given, exact, tz));
            }
            let utc_d = local_date(i, utc).unwrap();
            let utc_text = wall_str(utc_d, i - naive(utc_d, &midnight).unwrap());
            check(format!("{id} parse {utc_text}Z"), epoch(parse(&format!("{utc_text}Z[{id}][u-ca=hebrew]"))), Some(i));

            let z = ZonedDateTime::try_new_with_provider(i, tz, Calendar::ISO, &EXACT).unwrap();
            for (value, suffix) in [(z.clone(), String::new()), (z.with_calendar(hebrew.clone()), "[u-ca=hebrew]".to_string())] {
                let want = format!("{text}{}[{id}]{suffix}", offset_str(to_minute(off), false));
                match to_str(&value) {
                    Ok(printed) if printed == want => check(format!("{id} round trip {printed}"), epoch(parse(&printed)), Some(i)),
                    other => if text_wrong.len() < 8 { text_wrong.push(format!("{id} {i}: to_str {other:?}, oracle {want}")) },
                }
            }
            let moved = with_time_zone(&z, utc);
            check(format!("{id} {i} with_time_zone UTC"), epoch(moved.clone()), Some(i));
            if moved.as_ref().is_ok_and(|m| m.time_zone().identifier().ok().as_deref() != Some("UTC")) {
                text_wrong.push(format!("{id} {i}: with_time_zone did not change the zone"));
            }
            for days in [-1i64, 1, 30] {
                let (ty, tm, td) = temporal_sweeps::oracle_dates::add((d.year, d.month, d.day), 0, 0, 0, days, false).unwrap();
                let target = Ymd { year: ty, month: tm, day: td };
                for dis in modes {
                    let date = (target.year, target.month, target.day);
                    let want = resolve(naive(target, &midnight).unwrap() + tod, tz, dis);
                    check(format!("{id} {i} with_plain_date {days:+} {dis:?}"), epoch(with_plain_date(&z, date, dis)), want);
                    let fields = z.to_plain_time();
                    let built = from_wall_clock(date, &fields, tz, hebrew.clone(), dis);
                    if built.as_ref().is_ok_and(|b| b.calendar().identifier() != "hebrew") {
                        text_wrong.push(format!("{id} {i}: from_wall_clock dropped the calendar"));
                    }
                    check(format!("{id} {i} from_wall_clock hebrew {days:+} {dis:?}"), epoch(built), want);
                }
            }
        }
    }
    eprintln!("host ops: {checked} checked");
    wrong.extend(text_wrong);
    assert!(wrong.is_empty(), "host operations disagree with the oracle:\n{}", wrong.join("\n"));
}
