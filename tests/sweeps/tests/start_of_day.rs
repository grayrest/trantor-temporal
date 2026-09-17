//! A day's first instant and its length in hours, for every zone, on every local date a transition
//! touches between 1900 and 2100, plus ordinary days and both range edges.
use temporal_sweeps::oracle::{first_instant, local_date, naive, transitions, HOUR, LIMIT};
use temporal_sweeps::zones;
use temporal_sweeps::exact_provider::EXACT;
use temporal_sweeps::oracle::Ymd;
use temporal_rs::{Calendar, PlainTime, ZonedDateTime};

#[test]
fn should_match_the_oracle_on_every_local_date_a_transition_touches() {
    let midnight = PlainTime::try_new(0, 0, 0, 0, 0, 0).unwrap();
    let from = naive(Ymd { year: 1900, month: 1, day: 1 }, &midnight).unwrap();
    let to = naive(Ymd { year: 2100, month: 1, day: 1 }, &midnight).unwrap();
    let mut checked = 0u64;
    let mut wrong = vec![];
    for (id, tz) in zones() {
        // An instant on each date, so the date can be handed to `start_of_day`.
        let mut instants: Vec<i128> = vec![-LIMIT, -LIMIT + HOUR, LIMIT - HOUR, LIMIT];
        for t in transitions(tz, from, to) {
            instants.extend([t - 24 * HOUR, t, t + 24 * HOUR]);
        }
        let mut year = 1900;
        while year < 2100 {
            for (month, day) in [(1, 1), (1, 15), (7, 1), (7, 15)] {
                instants.extend(naive(Ymd { year, month, day }, &PlainTime::try_new(12, 0, 0, 0, 0, 0).unwrap()));
            }
            year += 6;
        }
        let mut seen = std::collections::HashSet::new();
        for i in instants {
            let Some(d) = local_date(i, tz) else { continue };
            if !seen.insert((d.year, d.month, d.day)) {
                continue;
            }
            checked += 1;
            let want = first_instant(d, tz);
            let base = ZonedDateTime::try_new_with_provider(i, tz, Calendar::ISO, &EXACT);
            let got = base.as_ref().ok().and_then(|b| b.start_of_day_with_provider(&EXACT).ok()).map(|z| z.epoch_nanoseconds().as_i128());
            if got != want && wrong.len() < 12 {
                wrong.push(format!("{id} {d:?}: start_of_day {got:?}, oracle {want:?}"));
            }
            // TC39 ends the day at the start of the next DATE; where a zone skipped
            // that date whole (Apia's 2011-12-30), its start is the transition
            // out of the gap, which is where the date after it begins.
            let next_start = d.next().and_then(|n| first_instant(n, tz).or_else(|| first_instant(n.next()?, tz)));
            let want_hours = next_start.zip(want).map(|(end, start)| (end - start) as f64 / HOUR as f64);
            let got_hours = base.ok().and_then(|b| b.hours_in_day_with_provider(&EXACT).ok());
            if got_hours != want_hours && wrong.len() < 12 {
                wrong.push(format!("{id} {d:?}: hours_in_day {got_hours:?}, oracle {want_hours:?}"));
            }
        }
    }
    eprintln!("start_of_day: {checked} zone-days checked");
    assert!(checked > 250_000, "only {checked} zone-days checked — the sweep has shrunk");
    assert!(wrong.is_empty(), "start_of_day disagrees with the oracle:\n{}", wrong.join("\n"));
}
