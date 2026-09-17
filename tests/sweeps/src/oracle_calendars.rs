//! Non-ISO date arithmetic as TC39's era and month-code proposal specifies it
//! (spec/era-monthcode.html NonISODateAdd, NonISODateUntil,
//! NonISODateSurpasses, BalanceNonISODate, ConstrainMonthCode,
//! MonthCodeToOrdinal; spec/temporal CompareSurpasses), over a model of each
//! calendar built from ICU4C's table rather than from ICU4X. A year the model
//! does not know — outside the table, outside Chinese and Dangi's published
//! ranges, or holding a month start ICU4C misplaces — makes the answer `None`:
//! the oracle declines rather than guesses.
use std::cell::Cell;
use std::collections::BTreeMap;

use crate::icu_reference::{is_trusted, rows};
use crate::oracle_dates::from_day_number;

/// A month: its code, first day as a day number, and length.
#[derive(Clone, Copy)]
struct Month { code: &'static str, start: i64, days: i64 }

pub struct Model {
    calendar: &'static str,
    /// Arithmetic year to its months in order; only whole, trusted years.
    years: BTreeMap<i64, Vec<Month>>,
    /// Day number of each month start, to the year and ordinal holding it.
    starts: BTreeMap<i64, (i64, usize)>,
    /// Set when a lookup reached a year the model does not know, so a caller
    /// whose own answer is only an `Option` can tell declining from refusing.
    declined: Cell<bool>,
}

/// A Calendar Date Record, as far as the arithmetic reads one.
#[derive(Clone, Copy, Debug)]
pub struct Parts { pub year: i64, pub month: usize, pub code: &'static str, pub day: i64 }

/// A year the model does not know.
#[derive(Clone, Copy, Debug)]
pub struct Unknown;

/// The table's year as TC39's arithmetic year (Table 4).
fn arithmetic_year(calendar: &str, era: &str, year: i64, start: i64) -> i64 {
    match (calendar, era) {
        ("japanese", _) => from_day_number(start).0,
        ("roc", "B.R.O.C.") => 1 - year,
        _ => year,
    }
}

impl Model {
    pub fn new(calendar: &'static str) -> Model {
        let all = rows();
        let runs = &all[calendar];
        let mut grouped: BTreeMap<i64, (u16, Vec<Month>)> = BTreeMap::new();
        for row in runs.iter().filter(|r| r.day == 1) {
            let year = arithmetic_year(calendar, row.era, row.year.into(), row.start);
            let entry = grouped.entry(year).or_insert((row.months_in_year, vec![]));
            entry.1.push(Month { code: row.code, start: row.start, days: row.days_in_month.into() });
        }
        let mut years = BTreeMap::new();
        let mut starts = BTreeMap::new();
        for (year, (count, months)) in grouped {
            let whole = count != 0 && months.len() == usize::from(count);
            let trusted = months.iter().all(|m| (m.start..m.start + m.days).all(|n| is_trusted(calendar, n)));
            if whole && trusted {
                for (i, m) in months.iter().enumerate() { starts.insert(m.start, (year, i)); }
                years.insert(year, months);
            }
        }
        Model { calendar, years, starts, declined: Cell::new(false) }
    }

    pub fn calendar(&self) -> &'static str { self.calendar }

    /// Whether a lookup has declined since the last call, clearing the flag.
    pub fn take_declined(&self) -> bool { self.declined.replace(false) }

    fn decline<T>(&self) -> Result<T, Unknown> {
        self.declined.set(true);
        Err(Unknown)
    }

    /// CalendarDateAdd with `constrain`, on day numbers.
    pub fn add_constrained(&self, n: i64, date: [i64; 4]) -> Option<i64> {
        self.add(n, date[0], date[1], date[2], date[3], false).ok().flatten()
    }

    /// Every day of every year the model knows, as day numbers.
    pub fn known_days(&self) -> impl Iterator<Item = i64> + '_ {
        self.years.values().flat_map(|ms| ms.iter().flat_map(|m| m.start..m.start + m.days))
    }

    /// CalendarISOToDate.
    pub fn parts(&self, n: i64) -> Result<Parts, Unknown> {
        let Some((&start, &(year, i))) = self.starts.range(..=n).next_back() else { return self.decline() };
        let month = self.years[&year][i];
        if n >= start + month.days { return self.decline(); }
        Ok(Parts { year, month: i + 1, code: month.code, day: n - start + 1 })
    }

    /// A year's first day as a day number, its length in days, and its months.
    pub fn year_span(&self, year: i64) -> Result<(i64, i64, usize), Unknown> {
        let months = self.months(year)?;
        Ok((months[0].start, months.iter().map(|m| m.days).sum(), months.len()))
    }

    fn months(&self, year: i64) -> Result<&Vec<Month>, Unknown> {
        match self.years.get(&year) { Some(months) => Ok(months), None => self.decline() }
    }

    /// YearContainsMonthCode.
    fn contains(&self, year: i64, code: &str) -> Result<bool, Unknown> {
        Ok(self.months(year)?.iter().any(|m| m.code == code))
    }

    /// ConstrainMonthCode: `Ok(None)` is the RangeError under reject.
    fn constrain(&self, year: i64, code: &'static str, reject: bool) -> Result<Option<&'static str>, Unknown> {
        if self.contains(year, code)? { return Ok(Some(code)); }
        if reject { return Ok(None); }
        Ok(Some(match (self.calendar, code) {
            ("hebrew", "M05L") => "M06",
            (_, c) if c.ends_with('L') => &c[..3],
            (_, c) => panic!("{} has no month {c} to constrain", self.calendar),
        }))
    }

    /// MonthCodeToOrdinal.
    fn ordinal(&self, year: i64, code: &str) -> Result<i64, Unknown> {
        let i = self.months(year)?.iter().position(|m| m.code == code).ok_or(Unknown)?;
        Ok(i as i64 + 1)
    }

    fn months_in(&self, year: i64) -> Result<i64, Unknown> { Ok(self.months(year)?.len() as i64) }

    fn days_in(&self, year: i64, month: i64) -> Result<i64, Unknown> {
        Ok(self.months(year)?[(month - 1) as usize].days)
    }

    /// BalanceNonISODate.
    fn balance(&self, year: i64, month: i64, day: i64) -> Result<(i64, i64, i64), Unknown> {
        let (mut y, mut m) = (year, month);
        let mut in_year = self.months_in(y)?;
        while m <= 0 { y -= 1; in_year = self.months_in(y)?; m += in_year; }
        while m > in_year { m -= in_year; y += 1; in_year = self.months_in(y)?; }
        let mut d = day;
        let mut in_month = self.days_in(y, m)?;
        while d <= 0 {
            m -= 1;
            if m == 0 { y -= 1; in_year = self.months_in(y)?; m = in_year; }
            in_month = self.days_in(y, m)?;
            d += in_month;
        }
        while d > in_month {
            d -= in_month;
            m += 1;
            if m > in_year { y += 1; in_year = self.months_in(y)?; m = 1; }
            in_month = self.days_in(y, m)?;
        }
        Ok((y, m, d))
    }

    /// CalendarIntegersToISO, as a day number.
    fn to_day(&self, year: i64, month: i64, day: i64) -> Result<i64, Unknown> {
        Ok(self.months(year)?[(month - 1) as usize].start + day - 1)
    }

    /// NonISODateAdd: `Ok(None)` is a RangeError.
    pub fn add(&self, n: i64, years: i64, months: i64, weeks: i64, days: i64, reject: bool) -> Result<Option<i64>, Unknown> {
        let parts = self.parts(n)?;
        let y0 = parts.year + years;
        let Some(code) = self.constrain(y0, parts.code, reject)? else { return Ok(None) };
        let m0 = self.ordinal(y0, code)?;
        let end = self.balance(y0, m0 + months + 1, 0)?;
        let day = if parts.day <= end.2 { parts.day } else if reject { return Ok(None) } else { end.2 };
        let (y, m, d) = self.balance(end.0, end.1, day + 7 * weeks + days)?;
        self.to_day(y, m, d).map(Some)
    }

    /// NonISODateSurpasses.
    fn surpasses(&self, sign: i64, one: i64, two: i64, (years, months, weeks, days): (i64, i64, i64, i64)) -> Result<bool, Unknown> {
        let parts = self.parts(one)?;
        let target = self.parts(two)?;
        let y0 = parts.year + years;
        if compare_code(sign, y0, parts.code, parts.day, &target) { return Ok(true); }
        let code = self.constrain(y0, parts.code, false)?.ok_or(Unknown)?;
        let m0 = self.ordinal(y0, code)?;
        let added = self.balance(y0, m0 + months, 1)?;
        if compare_month(sign, added.0, added.1, parts.day, &target) { return Ok(true); }
        if weeks == 0 && days == 0 { return Ok(false); }
        let end = self.balance(added.0, added.1 + 1, 0)?;
        let day = parts.day.min(end.2);
        let (y, m, d) = self.balance(end.0, end.1, day + 7 * weeks + days)?;
        Ok(compare_month(sign, y, m, d, &target))
    }

    /// NonISODateUntil; `largest` is 0 day, 1 week, 2 month, 3 year.
    pub fn until(&self, one: i64, two: i64, largest: u8) -> Result<[i64; 4], Unknown> {
        let sign = (two - one).signum();
        if sign == 0 { return Ok([0; 4]); }
        let mut out = [0i64; 4];
        let fields = |o: [i64; 4]| (o[0], o[1], o[2], o[3]);
        for (index, unit) in [(0usize, 3u8), (1, 2), (2, 1), (3, 0)] {
            let counts = match unit { 3 => largest == 3, 2 => largest >= 2, 1 => largest == 1, _ => true };
            if !counts { continue; }
            loop {
                let mut candidate = out;
                candidate[index] += sign;
                if self.surpasses(sign, one, two, fields(candidate))? { break; }
                out = candidate;
            }
        }
        Ok(out)
    }
}

/// CompareSurpasses with a month code.
fn compare_code(sign: i64, year: i64, code: &str, day: i64, target: &Parts) -> bool {
    if year != target.year { return sign * (year - target.year) > 0; }
    if code != target.code { return if sign > 0 { code > target.code } else { target.code > code }; }
    day != target.day && sign * (day - target.day) > 0
}

/// CompareSurpasses with an ordinal month.
fn compare_month(sign: i64, year: i64, month: i64, day: i64, target: &Parts) -> bool {
    if year != target.year { return sign * (year - target.year) > 0; }
    if month != target.month as i64 { return sign * (month - target.month as i64) > 0; }
    day != target.day && sign * (day - target.day) > 0
}
