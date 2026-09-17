//! The next and previous transition, from on, just before and just after every
//! transition of 1900-2100 in every zone and from between each pair, against
//! the oracle's offset scan. Includes asking `next` from exactly a zone's last
//! tzif entry, where temporal_rs's own lookup stalls.
use temporal_rs::provider::TransitionDirection as Dir;
use temporal_rs::{Calendar, PlainTime, ZonedDateTime};
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::{naive, transitions, Ymd};
use temporal_sweeps::transition::transition;
use temporal_sweeps::zones;

#[test]
fn should_find_the_neighbouring_offset_change_from_anywhere() {
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    let lo = naive(Ymd { year: 1900, month: 1, day: 1 }, &midnight).unwrap();
    let hi = naive(Ymd { year: 2100, month: 1, day: 1 }, &midnight).unwrap();
    let (mut checked, mut wrong) = (0u64, vec![]);
    for (id, tz) in zones() {
        let ts = transitions(tz, lo, hi);
        let mut from: Vec<i128> = ts.iter().flat_map(|t| [t - 1, *t, t + 1]).collect();
        from.extend(ts.windows(2).map(|w| w[0] + (w[1] - w[0]) / 2));
        for f in from {
            if f <= lo || f >= hi { continue }
            let want_next = ts.iter().copied().find(|t| *t > f);
            let want_prev = ts.iter().copied().rev().find(|t| *t < f);
            let base = ZonedDateTime::try_new_with_provider(f, tz, Calendar::ISO, &EXACT).unwrap();
            let got = |d| transition(&base, d).ok().flatten().map(|z| z.epoch_nanoseconds().as_i128());
            for (d, want) in [(Dir::Next, want_next), (Dir::Previous, want_prev)] {
                // Past the scanned range the oracle has no answer to compare.
                if want.is_none() { continue }
                checked += 1;
                let g = got(d);
                if g != want && wrong.len() < 12 {
                    wrong.push(format!("{id} {d:?} from {f}: host {g:?}, oracle {want:?}"));
                }
            }
        }
    }
    // Where the answer is none: nothing before 1800 in any zone, so no previous
    // transition from 1790; and in a zone whose offset stopped changing before
    // 2000 and did not change through 2600, no next transition from 2100.
    let early = naive(Ymd { year: 1790, month: 1, day: 1 }, &midnight).unwrap();
    let (y2000, y2600) = (naive(Ymd { year: 2000, month: 1, day: 1 }, &midnight).unwrap(), naive(Ymd { year: 2600, month: 1, day: 1 }, &midnight).unwrap());
    let mut settled = 0u64;
    for (id, tz) in zones() {
        let base = ZonedDateTime::try_new_with_provider(early, tz, Calendar::ISO, &EXACT).unwrap();
        checked += 1;
        if let Ok(Some(t)) = transition(&base, Dir::Previous) {
            wrong.push(format!("{id} previous from 1790: host {}, oracle none", t.epoch_nanoseconds().as_i128()));
        }
        if transitions(tz, y2000, y2600).is_empty() {
            settled += 1;
            let base = ZonedDateTime::try_new_with_provider(hi, tz, Calendar::ISO, &EXACT).unwrap();
            checked += 1;
            if let Ok(Some(t)) = transition(&base, Dir::Next) {
                wrong.push(format!("{id} next from 2100 in a settled zone: host {}, oracle none", t.epoch_nanoseconds().as_i128()));
            }
        }
    }
    eprintln!("transitions: {checked} lookups checked, {settled} zones with no change after 2000");
    assert!(wrong.is_empty(), "transition lookup disagrees with the oracle:\n{}", wrong.join("\n"));
}
