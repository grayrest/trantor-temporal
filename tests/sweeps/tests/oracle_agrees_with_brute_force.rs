//! The oracle is only worth trusting if something simpler agrees with it. A
//! second-by-second scan shares no reasoning with it, and is too slow to run
//! over every zone, so it checks the oracle on the shapes that have broken
//! things: days entered twice, days with no midnight, and the range edges.
use temporal_sweeps::oracle::{first_instant, first_instant_by_scan, local_date, LIMIT};
use temporal_sweeps::oracle::Ymd;
use temporal_rs::TimeZone;

fn tz(id: &str) -> TimeZone {
    TimeZone::try_from_identifier_str(id).unwrap()
}

#[test]
fn should_find_the_same_first_instant_as_a_one_second_scan() {
    let hard: &[(&str, i32, u8, u8)] = &[
        ("America/St_Johns", 1988, 10, 30),  // entered twice, midnight repeats
        ("America/Goose_Bay", 1988, 10, 30), // entered twice
        ("Antarctica/Casey", 2010, 3, 5),    // entered twice after a 3-hour step back
        ("Africa/Cairo", 2026, 4, 24),       // no midnight at all
        ("America/Havana", 2026, 3, 8),      // no midnight at all
        ("America/New_York", 2026, 3, 8),    // ordinary spring-forward, not at midnight
        ("Pacific/Apia", 2011, 12, 31),      // the day after a skipped day
        ("Asia/Kathmandu", 2026, 6, 1),      // ordinary, odd offset
    ];
    let mut wrong = vec![];
    for &(id, year, month, day) in hard {
        let d = Ymd { year, month, day };
        let (o, s) = (first_instant(d, tz(id)), first_instant_by_scan(d, tz(id)));
        if o != s {
            wrong.push(format!("{id} {d:?}: oracle {o:?}, scan {s:?}"));
        }
    }
    // Both edges of the representable range, east and west of UTC.
    for id in ["UTC", "America/New_York", "Asia/Tokyo"] {
        for edge in [-LIMIT, LIMIT] {
            let d = local_date(edge, tz(id)).unwrap();
            let (o, s) = (first_instant(d, tz(id)), first_instant_by_scan(d, tz(id)));
            if o != s {
                wrong.push(format!("{id} edge {d:?}: oracle {o:?}, scan {s:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "oracle and scan disagree:\n{}", wrong.join("\n"));
}
