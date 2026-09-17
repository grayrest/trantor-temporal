//! `equals` over every pair of zone identifiers against ECMA-402's primary
//! identifiers (spec/ecma402 §6.5): each Zone is primary and each Link resolves
//! to its Zone, except that "UTC" is primary and the Etc/UTC, Etc/GMT and GMT
//! families resolve to it, a Link in zone.tab's TZ column is primary, and a
//! Link outside zone.tab resolves within its own country — the one rule the
//! tz files cannot decide, so its cases are listed below, each checked by hand.
use std::collections::{BTreeMap, BTreeSet};
use temporal_rs::{Calendar, TimeZone, ZonedDateTime};
use temporal_sweeps::oracle::{offset_at, HOUR};
use temporal_sweeps::{zoned_ops, zones};

const UTC_FAMILY: [&str; 4] = ["UTC", "Etc/UTC", "Etc/GMT", "GMT"];

/// Links outside zone.tab whose IANA Zone is in another country, with the
/// primary in the link's own country that ECMA-402 resolves them to. Every
/// other such link was checked to lie in its Zone's country, or to name no
/// place (`CET`, `EST5EDT`), except `Pacific/Johnston`: Johnston Atoll is UM,
/// no UM primary keeps Hawaii's time, and it resolves to `Pacific/Honolulu`
/// as CLDR has it.
const SAME_COUNTRY: &[(&str, &str)] = &[
    ("Africa/Asmera", "Africa/Asmara"),               // ER, not Nairobi's KE
    ("Africa/Timbuktu", "Africa/Bamako"),             // ML, not Abidjan's CI
    ("America/Coral_Harbour", "America/Atikokan"),    // CA, not Panama's PA
    ("America/Virgin", "America/St_Thomas"),          // VI, not Puerto Rico's PR
    ("Antarctica/South_Pole", "Antarctica/McMurdo"),  // AQ, not Auckland's NZ
    ("Atlantic/Jan_Mayen", "Arctic/Longyearbyen"),    // SJ, ECMA-402's example
    ("Iceland", "Atlantic/Reykjavik"),                // IS, not Abidjan's CI
    ("Pacific/Ponape", "Pacific/Pohnpei"),            // FM, not Guadalcanal's SB
    ("Pacific/Truk", "Pacific/Chuuk"),                // FM, not Port Moresby's PG
    ("Pacific/Yap", "Pacific/Chuuk"),                 // FM, not Port Moresby's PG
];

struct Tzdb {
    links: BTreeMap<&'static str, &'static str>,
    listed: BTreeSet<&'static str>,
}

fn tzdb() -> Tzdb {
    let (mut links, mut listed) = (BTreeMap::new(), BTreeSet::new());
    for line in include_str!("../links.txt").lines().filter(|l| !l.starts_with('#')) {
        match line.split(' ').collect::<Vec<_>>()[..] {
            ["L", link, zone] => { links.insert(link, zone); }
            ["T", zone, _country] => { listed.insert(zone); }
            _ => panic!("links.txt: {line}"),
        }
    }
    Tzdb { links, listed }
}

fn primary(db: &Tzdb, id: &'static str) -> &'static str {
    let zone = db.links.get(id).copied().unwrap_or(id);
    if UTC_FAMILY.contains(&id) || UTC_FAMILY.contains(&zone) {
        return "UTC";
    }
    if let Some((_, own)) = SAME_COUNTRY.iter().find(|(link, _)| *link == id) {
        return own;
    }
    if db.listed.contains(id) { id } else { zone }
}

#[test]
fn should_equal_exactly_the_zones_ecma402_makes_one() {
    let db = tzdb();
    let all = zones();
    let instant = 1_781_541_296_789_000_000;
    let values: Vec<ZonedDateTime> = all.iter().map(|(_, tz)| ZonedDateTime::try_new(instant, *tz, Calendar::ISO).unwrap()).collect();
    let (mut checked, mut wrong) = (0u64, vec![]);
    for (i, (a, _)) in all.iter().enumerate() {
        for (j, (b, _)) in all.iter().enumerate() {
            let want = primary(&db, a) == primary(&db, b);
            let got = zoned_ops::equals(&values[i], &values[j]).unwrap();
            checked += 1;
            if got != want && wrong.len() < 20 {
                wrong.push(format!("{a} {b}: host {got}, oracle {want} ({} / {})", primary(&db, a), primary(&db, b)));
            }
        }
    }
    eprintln!("zone aliases: {checked} pairs");
    assert!(wrong.is_empty(), "equals disagrees with ECMA-402:\n{}", wrong.join("\n"));
}

#[test]
fn should_name_a_zone_in_any_case_and_keep_the_iana_casing() {
    let mut wrong = vec![];
    for (id, tz) in zones() {
        for spelling in [id.to_lowercase(), id.to_uppercase()] {
            let got = TimeZone::try_from_identifier_str(&spelling).and_then(|z| z.identifier());
            if got.as_deref() != Ok(id) { wrong.push(format!("{spelling}: {got:?}")); }
            let same = ZonedDateTime::try_new(0, TimeZone::try_from_identifier_str(&spelling).unwrap(), Calendar::ISO).unwrap();
            if !zoned_ops::equals(&same, &ZonedDateTime::try_new(0, tz, Calendar::ISO).unwrap()).unwrap() {
                wrong.push(format!("{spelling} is not {id}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Equal zones share one history: weekly offsets from 1850 to 2100 agree.
#[test]
fn should_equal_only_zones_with_one_history() {
    let week = 168 * HOUR;
    let (from, to) = (-3_786_825_600 * 1_000_000_000i128, 4_102_444_800 * 1_000_000_000i128);
    let all = zones();
    let (mut pairs, mut wrong) = (0u64, vec![]);
    for (a, x) in &all {
        for (b, y) in &all {
            let (p, q) = (ZonedDateTime::try_new(0, *x, Calendar::ISO).unwrap(), ZonedDateTime::try_new(0, *y, Calendar::ISO).unwrap());
            if a >= b || !zoned_ops::equals(&p, &q).unwrap() { continue; }
            pairs += 1;
            if let Some(t) = (0..).map(|k| from + k * week).take_while(|t| *t < to).find(|t| offset_at(*t, *x) != offset_at(*t, *y)) {
                wrong.push(format!("{a} and {b} are equal but differ at {t}"));
            }
        }
    }
    eprintln!("zone aliases: {pairs} equal pairs share weekly offsets over 1850-2100");
    assert!(pairs > 0 && wrong.is_empty(), "{}", wrong.join("\n"));
}
