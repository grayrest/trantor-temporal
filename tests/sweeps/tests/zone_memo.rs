//! What the zone memo can hold (D-T2-6). `TimeZone : Str`, so an identifier
//! reaches `zoned!` or `with_time_zone!` straight from a config file, a CSV
//! column or a request parameter without passing `time_zone_from_id!` first,
//! and `TimeZone::try_from_str` accepts every case permutation of a name as
//! well as any IXDTF string carrying a zone annotation. A memo keyed on what
//! the caller spelled therefore had no ceiling at all, in a table that never
//! shrinks; these measure the one it has keyed on the zone's own identifier.
use std::collections::BTreeSet;
use temporal_rs::TimeZone;
use temporal_sweeps::zone_memo::{zone_of, ZONES};

/// Thirty letters, so its case permutations outnumber anything a test can
/// enumerate: the point is that the memo does not care how many there are.
const LONG_NAME: &str = "America/Argentina/ComodRivadavia";

/// A body on a thread of its own, with the size of the memo it left behind.
/// The memo is thread-local by design, and a count only says something about a
/// table that nothing else has put a zone into.
fn memo_after(body: impl FnOnce() + Send + 'static) -> usize {
    std::thread::spawn(|| {
        body();
        ZONES.with(|c| c.borrow().len())
    })
    .join()
    .expect("the memo thread panicked")
}

/// `n`'s bits choose the case of each ASCII letter, so successive `n` give
/// distinct spellings of one name.
fn cased(name: &str, n: u64) -> String {
    let mut bit = 0;
    name.chars()
        .map(|c| {
            if !c.is_ascii_alphabetic() {
                return c;
            }
            let upper = n >> (bit % 64) & 1 == 1;
            bit += 1;
            if upper { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() }
        })
        .collect()
}

#[test]
fn a_hundred_thousand_spellings_of_one_zone_hold_one_entry() {
    let entries = memo_after(|| {
        let want = TimeZone::try_from_str(LONG_NAME).expect(LONG_NAME);
        for n in 0..100_000 {
            let spelling = cased(LONG_NAME, n);
            let got = zone_of(&spelling).unwrap_or_else(|e| panic!("{spelling}: {e}"));
            assert_eq!(got, want, "{spelling} resolved to another zone");
        }
    });
    assert_eq!(entries, 1, "{LONG_NAME} occupies {entries} entries of the memo");
}

#[test]
fn a_zone_annotation_is_memoised_as_the_zone_it_names() {
    // The datetime in front of the annotation is unbounded on its own: a
    // per-request timestamp would have been a fresh key every call.
    let entries = memo_after(|| {
        let want = TimeZone::try_from_str("America/New_York").expect("America/New_York");
        for minute in 0..10_000 {
            let stamp = format!("2024-01-01T00:00:00+00:00[{}]", cased("America/New_York", minute));
            let got = zone_of(&stamp).unwrap_or_else(|e| panic!("{stamp}: {e}"));
            assert_eq!(got, want, "{stamp} resolved to another zone");
        }
    });
    assert_eq!(entries, 1, "one zone under 10,000 annotated strings took {entries} entries");
}

/// The ceiling, reached: every identifier the pinned temporal_rs resolves —
/// the tzdb's names and the minute-precision offsets, which is all
/// `try_from_str` accepts — each in several spellings. Sub-minute offsets
/// (`+05:30:15`) and unknown names are rejected outright, so there is nothing
/// else for a caller to put in.
#[test]
fn the_whole_accepted_input_space_fits_in_one_entry_per_zone() {
    const OFFSETS: usize = 24 * 60 * 2 - 1; // ±HH:MM, with -00:00 canonical as +00:00.
    let names: Vec<&str> =
        include_str!("../zones.txt").lines().filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    let canonical: BTreeSet<String> = names
        .iter()
        .map(|n| TimeZone::try_from_str(n).unwrap_or_else(|e| panic!("{n}: {e}")).identifier().expect(n))
        .collect();

    let (tzdb_names, want) = (names.len(), canonical.len() + OFFSETS);
    let entries = memo_after(move || {
        for name in &names {
            for spelling in [name.to_string(), name.to_lowercase(), name.to_uppercase()] {
                zone_of(&spelling).unwrap_or_else(|e| panic!("{spelling}: {e}"));
            }
            zone_of(&format!("2024-01-01T00:00:00+00:00[{name}]")).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        for hour in 0..24 {
            for minute in 0..60 {
                for sign in ['+', '-'] {
                    zone_of(&format!("{sign}{hour:02}:{minute:02}")).expect("a minute-precision offset");
                }
            }
        }
    });
    assert!(tzdb_names > 500, "only {tzdb_names} tzdb names — zones.txt has shrunk");
    assert_eq!(entries, want, "{tzdb_names} names in four spellings each, plus every offset, took {entries} entries");
}

#[test]
fn an_identifier_no_zone_answers_to_adds_nothing() {
    let entries = memo_after(|| {
        for n in 0..1_000 {
            assert!(zone_of(&format!("Not/AZone{n}")).is_err(), "Not/AZone{n} resolved");
        }
        assert!(zone_of("+05:30:15").is_err(), "a sub-minute offset resolved");
        assert!(zone_of(" America/New_York").is_err(), "a padded identifier resolved");
    });
    assert_eq!(entries, 0, "{entries} entries from identifiers that resolved to nothing");
}
