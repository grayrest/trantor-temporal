//! ISO plain-date arithmetic transcribed from the spec (spec/calendar.html
//! CalendarDateAdd and CalendarDateUntil, spec/plaindate.html ISODateSurpasses,
//! CompareSurpasses, RegulateISODate, ISODateWithinLimits), on day numbers
//! computed here — no temporal_rs date arithmetic.

/// An ISO date as year, month, day.
pub type Iso = (i32, u8, u8);

pub fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => if is_leap(y) { 29 } else { 28 },
    }
}

/// Days since 1970-01-01 (Hinnant's days_from_civil).
pub fn day_number(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day number (Hinnant's civil_from_days).
pub fn from_day_number(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// ISODateWithinLimits: -271821-04-19 through +275760-09-13.
pub fn within_limits(y: i64, m: i64, d: i64) -> bool {
    (y, m, d) >= (-271_821, 4, 19) && (y, m, d) <= (275_760, 9, 13)
}

fn balance_year_month(y: i64, m: i64) -> (i64, i64) {
    let months = y * 12 + (m - 1);
    (months.div_euclid(12), months.rem_euclid(12) + 1)
}

/// CalendarDateAdd for ISO: years and months, the day regulated (`reject`
/// refuses what `constrain` clamps), then weeks and days. `None` for a
/// refused day or a result outside the limits.
pub fn add(date: Iso, years: i64, months: i64, weeks: i64, days: i64, reject: bool) -> Option<Iso> {
    let (y, m) = balance_year_month(i64::from(date.0) + years, i64::from(date.1) + months);
    let length = days_in_month(y, m);
    let day = i64::from(date.2);
    if reject && day > length {
        return None;
    }
    let (ry, rm, rd) = from_day_number(day_number(y, m, day.min(length)) + 7 * weeks + days);
    within_limits(ry, rm, rd).then_some((ry as i32, rm as u8, rd as u8))
}

/// CompareSurpasses for integer months.
fn compare_surpasses(sign: i64, y: i64, m: i64, d: i64, target: (i64, i64, i64)) -> bool {
    if y != target.0 {
        return sign * (y - target.0) > 0;
    }
    if m != target.1 {
        return sign * (m - target.1) > 0;
    }
    d != target.2 && sign * (d - target.2) > 0
}

/// ISODateSurpasses.
fn surpasses(sign: i64, base: (i64, i64, i64), target: (i64, i64, i64), years: i64, months: i64, weeks: i64, days: i64) -> bool {
    let y0 = base.0 + years;
    if compare_surpasses(sign, y0, base.1, base.2, target) {
        return true;
    }
    if months == 0 {
        return false;
    }
    let (my, mm) = balance_year_month(y0, base.1 + months);
    if compare_surpasses(sign, my, mm, base.2, target) {
        return true;
    }
    if weeks == 0 && days == 0 {
        return false;
    }
    let regulated = day_number(my, mm, base.2.min(days_in_month(my, mm)));
    let (by, bm, bd) = from_day_number(regulated + 7 * weeks + days);
    compare_surpasses(sign, by, bm, bd, target)
}

/// CalendarDateUntil for ISO, largest unit 3 = year, 2 = month, 1 = week, 0 = day.
/// The day and week loops are closed-form: each only counts whole days or weeks
/// up to the target, which is what stepping one at a time reaches.
pub fn until(one: Iso, two: Iso, largest: u8) -> [i64; 4] {
    let a = (i64::from(one.0), i64::from(one.1), i64::from(one.2));
    let b = (i64::from(two.0), i64::from(two.1), i64::from(two.2));
    let sign = match a.cmp(&b) { std::cmp::Ordering::Less => 1, std::cmp::Ordering::Greater => -1, _ => return [0; 4] };
    let mut years = 0;
    if largest == 3 {
        let mut candidate = sign;
        while !surpasses(sign, a, b, candidate, 0, 0, 0) {
            years = candidate;
            candidate += sign;
        }
    }
    let mut months = 0;
    if largest >= 2 {
        let mut candidate = sign;
        while !surpasses(sign, a, b, years, candidate, 0, 0) {
            months = candidate;
            candidate += sign;
        }
    }
    let (my, mm) = balance_year_month(a.0 + years, a.1 + months);
    let start = day_number(my, mm, a.2.min(days_in_month(my, mm)));
    let span = day_number(b.0, b.1, b.2) - start;
    let weeks = if largest == 1 { span / 7 } else { 0 };
    [years, months, weeks, span - 7 * weeks]
}
