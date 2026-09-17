app [main!] { pf: platform "../target/trantor/app/platform/main.roc" }

import pf.OsStr
import pf.Stdout
import pf.Temporal
import pf.Now

tag : Temporal.Err -> Str
tag = |e| match e {
	OutOfRange(_) => "OutOfRange"
	Invalid(_) => "Invalid"
	Other(_) => "Other"
}

msg : Temporal.Err -> Str
msg = |e| match e {
	OutOfRange(m) => m
	Invalid(m) => m
	Other(m) => m
}

zoned_or_tag! : Try(Temporal.ZonedDateTime, Temporal.Err) => Str
zoned_or_tag! = |r| match r {
	Ok(z) => z.to_str!() ?? "<unprintable>"
	Err(e) => tag(e)
}

date_or_tag : Try(Temporal.PlainDate, Temporal.Err) -> Str
date_or_tag = |r| match r {
	Ok(d) => d.to_str()
	Err(e) => tag(e)
}

parsed : Try(Temporal.PlainDate, Temporal.ParseErr) -> Str
parsed = |r| match r {
	Ok(d) => d.to_str()
	Err(BadPattern(_)) => "BadPattern"
	Err(BadInput(_)) => "BadInput"
}

trans! : Try(Temporal.Transition, Temporal.Err) => Str
trans! = |r| match r {
	Ok(At(z)) => z.to_str!() ?? "<unprintable>"
	Ok(NoTransition) => "NoTransition"
	Err(_) => "error"
}

## A parsed time's sub-second fields spelled out, so a carry into the wrong
## field shows.
t_parse : Try(Temporal.PlainTime, Temporal.ParseErr) -> Str
t_parse = |r| match r { Ok(v) => "${v.hour.to_str()}:${v.minute.to_str()}.${v.millisecond.to_str()}/${v.microsecond.to_str()}/${v.nanosecond.to_str()}", Err(BadPattern(_m)) => "BadPattern", Err(BadInput(_m)) => "BadInput" }

## The instant a day begins at, as a number, so the gate pins what was measured
## rather than a rendering of it.
## A zoned value as IXDTF, or the error tag.
zstr! : Try(Temporal.ZonedDateTime, Temporal.Err) => Str
zstr! = |r| match r {
	Ok(z) => z.to_str!() ?? "<unprintable>"
	Err(e) => tag(e)
}

## A day of the year, or the error tag for a record that names no date.
doy_or_tag : Temporal.PlainDate -> Str
doy_or_tag = |d| match d.iso_day_of_year() { Ok(n) => n.to_str(), Err(e) => tag(e) }

## A weekday, or the error tag for a record that names no date.
dow_or_tag : Temporal.PlainDate -> Str
dow_or_tag = |d| match d.day_of_week() { Ok(n) => n.to_str(), Err(e) => tag(e) }

sod! : Try(Temporal.ZonedDateTime, Temporal.Err) => Str
sod! = |r| match r {
	Ok(z) => z.epoch_ns!().to_str()
	Err(e) => tag(e)
}

## A parsed date with the calendar it carries, or the error tag.
dcal! : Try(Temporal.PlainDate, Temporal.Err) => Str
dcal! = |r| match r {
	Ok(d) => "${d.to_str()}/${Temporal.calendar_id!(d.cal)}"
	Err(e) => tag(e)
}

## A duration that cannot be rendered says so rather than inventing a value.
ds : Temporal.Duration -> Str
ds = |d| match d.to_str() { Ok(s) => s, Err(MixedSigns) => "MixedSigns", Err(TooLarge) => "TooLarge" }

ord : Temporal.Order -> Str
ord = |o| match o { Before => "Before", Same => "Same", After => "After" }

dur : I64, I64, I64, I64 -> Temporal.Duration
dur = |da, s, ms, ns| { days: da, seconds: s, milliseconds: ms, nanoseconds: ns }

yn : Bool -> Str
yn = |b| if b { "yes" } else { "no" }

report! : {} => Try(Str, Temporal.Err)
report! = |{}| {
	ny = Temporal.time_zone_from_id!("America/New_York")?
	jan31 = Temporal.plain_date({ year: 2024, month: 1, day: 31 })
	jan31h = { ..jan31, cal: Hebrew }
	march8 = Temporal.plain_date({ year: 2026, month: 3, day: 8 })
	t0230 : Temporal.PlainTime
	t0230 = { hour: 2, minute: 30 }
	t1040 : Temporal.PlainTime
	t1040 = { hour: 10, minute: 40 }

	before = Temporal.zoned!({ year: 2026, month: 3, day: 7 }, t1040, ny)?
	after = before.add!({ days: 1 })?
	back = after.subtract!({ days: 1 })?
	rt = Temporal.zoned_from_str!("2024-03-10T01:30:00-05:00[America/New_York]")?
	jan1 = Temporal.plain_date({ year: 2024, month: 1, day: 1 })
	dec25 = Temporal.plain_date({ year: 2024, month: 12, day: 25 })

	l00 = "iso-pad: ${Temporal.plain_date({ year: 1970, month: 1, day: 1 }).to_str()}"
	l01 = "hebrew-identity: ${date_or_tag(jan31h.add!({}))}"
	l02 = "hebrew-month: ${date_or_tag(jan31h.add!({ months: 1 }))}"
	l03 = "constrain: ${date_or_tag(jan31.add!({ months: 1 }))}"
	l04 = "reject: ${date_or_tag(jan31.add_with!({ months: 1 }, Reject))}"
	l05 = "feb30: ${date_or_tag(Temporal.plain_date({ year: 2024, month: 2, day: 30 }).add!({}))}"
	l06 = "bad-calendar: ${match Temporal.calendar_from_id!("nosuchcal") { Ok(_) => "accepted", Err(e) => "${tag(e)}(${msg(e)})" }}"
	l07 = "until: ${(jan1.until!(dec25)?).days.to_str()}d ${ds(jan1.until_in!(dec25, Year)?)}"
	l08 = "gap-compatible: ${zoned_or_tag!(Temporal.zoned_with!(march8, t0230, ny, Compatible))}"
	l09 = "gap-earlier: ${zoned_or_tag!(Temporal.zoned_with!(march8, t0230, ny, Earlier))}"
	l10 = "gap-later: ${zoned_or_tag!(Temporal.zoned_with!(march8, t0230, ny, Later))}"
	l11 = "gap-reject: ${zoned_or_tag!(Temporal.zoned_with!(march8, t0230, ny, Reject))}"
	l12 = "dst-before: ${before.to_str!()?}"
	l13 = "dst-add: ${after.to_str!()?} ${(after.epoch_ns!() - before.epoch_ns!()).to_str()}"
	l14 = "dst-subtract-roundtrip: ${yn(back.equals!(before)?)}"
	l15 = "zdt-until: ${ds(before.until!(after)?)} ${ds(before.until_in!(after, Day)?)}"
	l16 = "start-of-day: ${after.start_of_day!()?.to_str!()?}"
	l17 = "zone-id: ${Temporal.time_zone_id(ny)}"
	l18 = "ixdtf-roundtrip: ${rt.to_str!()?}"
	l19 = "offset-conflict: ${zoned_or_tag!(Temporal.zoned_from_str!("2024-03-10T01:30:00+09:00[America/New_York]"))}"
	l20 = "wall-from-str: ${zoned_or_tag!(Temporal.zoned_from_str!("2026-03-07T10:40:00[America/New_York]"))}"
	l21 = "zdt-calendar: ${Temporal.calendar_id!(rt.calendar!())}"
	l22 = "zdt-format: ${after.format!("%F %T %z %:z %Z")?}"
	l23 = "format: ${march8.format("%A %e %B %Y %j %a %I%p %F %%")}"
	l24 = "parse-ok: ${parsed(Temporal.date_parse_in("08 MAR 2026", "%d %b %Y", 2026))} ${parsed(Temporal.date_parse_in("2026 067", "%Y %j", 2026))} ${parsed(Temporal.date_parse_in("68-03-08", "%y-%m-%d", 2026))} ${parsed(Temporal.date_parse_in("69-03-08", "%y-%m-%d", 2026))}"
	l25 = "parse-strict: ${parsed(Temporal.date_parse_in("2026-03-08 extra", "%Y-%m-%d", 2026))} ${parsed(Temporal.date_parse_in("2026x03x08", "%Y-%m-%d", 2026))} ${parsed(Temporal.date_parse_in("Monday 2026-03-08", "%A %Y-%m-%d", 2026))} ${parsed(Temporal.date_parse_in("03-08", "%m-%d", 2026))}"
	l26 = "parse-weekday-ok: ${parsed(Temporal.date_parse_in("Sunday 2026-03-08", "%A %Y-%m-%d", 2026))}"
	l27 = "duration: ${ds({})} ${ds({ years: 1 })} ${ds({ hours: 4 })}"
	l28 = "pure: ${march8.day_of_week()?.to_str()} ${march8.iso_day_of_year()?.to_str()} ${yn(Temporal.is_leap_year(2024))} ${yn(Temporal.is_leap_year(2026))}"
	l29 = "duration-subsecond: ${ds(dur(0,0,500,0))} ${ds(dur(0,1,0,250))} ${ds(dur(0,0,1500,0))}"
	l30 = "duration-sign: ${ds(dur(-1,0,0,0))} ${ds(dur(0,0,0,0))}"
	l31 = "duration-roundtrip: ${ds(Temporal.duration_from_str!("PT0.5S")?)} ${ds(Temporal.duration_from_str!("P1Y2M3D")?)}"
	l32 = "compare: ${ord(march8.compare({ year: 2026, month: 3, day: 9 }))} ${ord(march8.compare(march8))} ${ord(t0230.compare({ ..t0230, minute: 31 }))}"
	l33 = "with: ${Temporal.plain_date({ ..march8, day: 1 }).to_str()} ${{ ..t0230, hour: 23 }.to_str()}"
	feb = Temporal.plain_date({ year: 2024, month: 2, day: 1 })
	fl = feb.fields!()?
	l34 = "fields: dim=${fl.days_in_month.to_str()} diy=${fl.days_in_year.to_str()} miy=${fl.months_in_year.to_str()} dow=${fl.day_of_week.to_str()} woy=${fl.week_of_year.to_str()} mc=${fl.month_code} leap=${yn(fl.in_leap_year)}"
	l35 = "until-rounded: ${ds(jan1.until_rounded!(dec25, { largest: Month, smallest: Month, mode: Floor, increment: 1 })?)} ${ds(jan1.since_rounded!(dec25, Temporal.date_diff(Year))?)}"
	utc = Temporal.time_zone_from_id!("UTC")?
	on_dst = Temporal.zoned!(march8, t1040, ny)?
	l36 = "hours-in-day: ${on_dst.hours_in_day!()?.to_str()} ${before.hours_in_day!()?.to_str()}"
	l37 = "transitions: ${trans!(before.next_transition!())} | ${trans!(Temporal.zoned!(march8, t1040, utc)?.next_transition!())}"
	odd = Temporal.zoned!(before.plain_date!(), { hour: 10, minute: 37, second: 30 }, ny)?
	l38 = "with-zoned: ${odd.to_str!()?} ${before.with_plain_date!(march8)?.to_str!()?}"
	l39 = "round-hour: ${odd.round!(Hour)?.to_str!()?}"
	l40 = "start-of-day-before-jump: ${before.start_of_day!()?.to_str!()?}"
	l41 = "compare-zdt: ${ord(before.compare!(on_dst))} ${ord(before.compare!(before))}"
	ninety : Temporal.Duration
	ninety = { hours: 1, minutes: 30 }
	month1 : Temporal.Duration
	month1 = { months: 1 }
	l42 = "totals: ${ninety.total!(Hour, Unanchored)?.to_str()} ${month1.total!(Day, ToDate(feb))?.to_str()}"
	l43 = "total-unanchored-calendar: ${match month1.total!(Day, Unanchored) { Ok(v) => v.to_str(), Err(e) => tag(e) }}"
	l44 = "duration-round: ${ds(ninety.round!({ largest: Hour, smallest: Hour, mode: HalfExpand, increment: 1 }, Unanchored)?)}"
	l45 = "compare-duration: ${ord(ninety.compare!(month1, ToDate(feb))?)}"
	# The clock is the one thing here that reads the world, so these assert
	# invariants rather than values — a gate that pins a timestamp dies tomorrow.
	now_ns = match Now.epoch_ns!({}) { Ok(v) => v, Err(ClockUnavailable) => 0 }
	now_zone = match Now.time_zone_id!({}) { Ok(v) => v, Err(ZoneUnavailable) => "" }
	utc_now = Now.zoned_in!("UTC")?
	here_now = Now.zoned!({})?
	today = Now.plain_date_in!("UTC")?
	l46 = "now-clock: after-2020=${yn(now_ns > 1577836800000000000)} zone-named=${yn(!Str.is_empty(now_zone))} utc-tag=${utc_now.format!("%Z")?}"
	l47 = "now-zone: machine-matches=${yn(here_now.format!("%Z")? == now_zone)} date-plausible=${yn(today.year >= 2026 and today.month >= 1 and today.month <= 12)}"
	l48 = "now-convert: ${yn(here_now.with_time_zone!(ny)?.epoch_ns!() == here_now.epoch_ns!())}"
	# --- zones beyond New_York. Every DST number in the design log was measured
	# --- in Berlin, Sydney and Santiago, and none of them was pinned here.
	ber = Temporal.time_zone_from_id!("Europe/Berlin")?
	syd = Temporal.time_zone_from_id!("Australia/Sydney")?
	stgo = Temporal.time_zone_from_id!("America/Santiago")?
	# a week clear of Berlin's 2026-03-29 jump: it must NOT be moved
	l49 = "berlin-ordinary: ${zoned_or_tag!(Temporal.zoned!({ year: 2026, month: 3, day: 22 }, t0230, ber))}"
	l50 = "berlin-jump: ${zoned_or_tag!(Temporal.zoned!({ year: 2026, month: 3, day: 29 }, t0230, ber))}"
	syd_apr = Temporal.zoned!({ year: 2026, month: 4, day: 5 }, t1040, syd)?
	stgo_sep = Temporal.zoned!({ year: 2026, month: 9, day: 6 }, t1040, stgo)?
	l51 = "southern-hours: sydney=${syd_apr.hours_in_day!()?.to_str()} santiago=${stgo_sep.hours_in_day!()?.to_str()}"
	l52 = "santiago-start-of-day: ${zoned_or_tag!(stgo_sep.start_of_day!())}"
	# a value must agree with itself: the accessors and the string are one instant
	si = before.plain_time!()
	l53 = "self-consistent: ${si.hour.to_str()}:${si.minute.to_str()} vs ${before.to_str!()?} off=${before.offset_seconds!().to_str()}"
	# --- error paths. `Invalid` and `Other` appeared in no expected line before.
	l54 = "err-tags: ${match Temporal.plain_date({ year: 2024, month: 2, day: 30 }).add!({}) { Ok(_) => "accepted", Err(e) => tag(e) }} ${match month1.total!(Day, Unanchored) { Ok(_) => "accepted", Err(e) => tag(e) }} ${match before.round_with!({ smallest: Hour, mode: Floor, increment: 0 }) { Ok(_) => "accepted", Err(e) => tag(e) }}"
	# --- out of range, for the pure functions that used to ABORT on it
	l55 = "out-of-range: '${Temporal.plain_date({ year: 10000, month: 1, day: 1 }).format("%Y")}' '${Temporal.plain_date({ year: 2026, month: 200, day: 1 }).format("%m %b")}' '${Temporal.plain_date({ year: 2026, month: 0, day: 1 }).format("%A")}' dow=${dow_or_tag(Temporal.plain_date({ year: 2026, month: 0, day: 1 }))}"
	l56 = "negative-years: ${Temporal.plain_date({ year: 0, month: 1, day: 1 }).day_of_week()?.to_str()} ${Temporal.plain_date({ year: -1, month: 3, day: 1 }).day_of_week()?.to_str()} ${Temporal.plain_date({ year: -1000, month: 1, day: 1 }).day_of_week()?.to_str()}"
	l57 = "mixed-signs: ${ds({ days: 1, hours: -2 })} ${ds({ seconds: 1, milliseconds: -1000 })}"
	l58 = "parse-strictness: ${parsed(Temporal.date_parse_in("2026-00-10", "%Y-%m-%d", 2026))} ${t_parse(Temporal.time_parse("09:30", "%H:%M"))} ${t_parse(Temporal.time_parse("123456789", "%N"))}"
	l59 = "parse-default-year: ${parsed(Temporal.date_parse_in("25/12", "%d/%m", 2031))}"
	# --- the second adversarial review. Every line below is a defect that a
	# --- green gate shipped, because the gate sampled one hour and one zone.
	chi = Temporal.time_zone_from_id!("America/Chicago")?
	jer = Temporal.time_zone_from_id!("Asia/Jerusalem")?
	cai = Temporal.time_zone_from_id!("Africa/Cairo")?
	t0130 : Temporal.PlainTime
	t0130 = { hour: 1, minute: 30 }
	t0030 : Temporal.PlainTime
	t0030 = { hour: 0, minute: 30 }
	# an overlap: Earlier and Later must differ, and Reject must refuse
	l60 = "overlap: ${zoned_or_tag!(Temporal.zoned_with!({ year: 2021, month: 11, day: 7 }, t0130, chi, Earlier))} | ${zoned_or_tag!(Temporal.zoned_with!({ year: 2021, month: 11, day: 7 }, t0130, chi, Later))} | ${zoned_or_tag!(Temporal.zoned_with!({ year: 2021, month: 11, day: 7 }, t0130, chi, Reject))}"
	# a gap in a zone the gate never named: Compatible lands AFTER it, Reject refuses
	l61 = "gap-jerusalem: ${zoned_or_tag!(Temporal.zoned_with!({ year: 2026, month: 3, day: 27 }, t0230, jer, Compatible))} | ${zoned_or_tag!(Temporal.zoned_with!({ year: 2026, month: 3, day: 27 }, t0230, jer, Reject))}"
	# arithmetic at the hour the old sweeps never sampled
	cairo = Temporal.zoned!({ year: 2024, month: 4, day: 20 }, t0030, cai)?
	l62 = "midnight-arith: ${cairo.add!({ days: 1 })?.to_str!()?} hours=${cairo.hours_in_day!()?.to_str()}"
	late = Temporal.zoned!({ year: 2026, month: 10, day: 20 }, { hour: 23, minute: 40 }, ber)?
	l63 = "late-arith: ${late.add!({ days: 1 })?.to_str!()?}"
	# totality at the edges, where two functions used to abort the process
	l64 = "edges: dow=${dow_or_tag(Temporal.plain_date({ year: 1800000000, month: 3, day: 1 }))} huge=${ds({ seconds: 9223372036854775807, milliseconds: 1000 })}"
	# a parser that requires what it claims to
	l65 = "parse-requires: ${t_parse(Temporal.time_parse("2026-03-08", "%Y-%m-%d"))} ${parsed(Temporal.date_parse_in("2026-03-08 365", "%Y-%m-%d %j", 2026))}"
	# a transition must advance, or there is none
	l66 = "transition-advances: ${trans!(Temporal.zoned_from_epoch_ns!(846363600000000000, Temporal.time_zone_from_id!("Europe/Bucharest")?)?.next_transition!())}"
	# --- start of day, where the local date is NOT entered exactly once. A
	# --- bisection assumes it is. These pin one of each shape; tests/sweeps
	# --- checks every such date in every zone.
	stj = Temporal.time_zone_from_id!("America/St_Johns")?
	cas = Temporal.time_zone_from_id!("Antarctica/Casey")?
	hav = Temporal.time_zone_from_id!("America/Havana")?
	noon : Temporal.PlainTime
	noon = { hour: 12 }
	# entered twice: the day begins at the FIRST midnight, not the second
	l67 = "sod-twice: ${sod!(Temporal.zoned!({ year: 1988, month: 10, day: 30 }, noon, stj)?.start_of_day!())} ${sod!(Temporal.zoned!({ year: 2010, month: 3, day: 5 }, noon, cas)?.start_of_day!())}"
	# no midnight at all: the day begins where the transition drops into it
	l68 = "sod-gap: ${sod!(Temporal.zoned!({ year: 2026, month: 4, day: 24 }, noon, cai)?.start_of_day!())} ${sod!(Temporal.zoned!({ year: 2026, month: 3, day: 8 }, noon, hav)?.start_of_day!())}"
	# the first representable instant IS a start of day in UTC, so asking must
	# not error -- but west of it that day begins before the range, and must
	l69 = "sod-min: ${sod!(Temporal.zoned_from_epoch_ns!(-8640000000000000000000, utc)?.start_of_day!())} ${sod!(Temporal.zoned_from_epoch_ns!(-8640000000000000000000, ny)?.start_of_day!())}"
	# --- the calendar lives on the date: == sees it, compare does not, a
	# --- difference across two calendars refuses, and arithmetic keeps it.
	same = if jan31h == jan31 { "yes" } else { "no" }
	cross = match jan31h.until!(jan31) { Ok(_) => "accepted", Err(e) => tag(e) }
	kept = Temporal.calendar_id!(jan31h.add!({ months: 1 })?.cal)
	l70 = "date-calendar: eq=${same} cmp=${ord(jan31h.compare(jan31))} until=${cross} kept=${kept}"
	# --- an IXDTF calendar annotation is the date's calendar, not a comment
	l71 = "date-from-str-calendar: ${dcal!(Temporal.date_from_str!("2026-03-08"))} ${dcal!(Temporal.date_from_str!("2026-03-08[u-ca=hebrew]"))} ${dcal!(Temporal.date_from_str!("2026-03-08[u-ca=islamic]"))} ${dcal!(Temporal.date_from_str!("2026-03-08[!u-ca=japanese]"))} ${dcal!(Temporal.date_from_str!("2026-03-08[u-ca=nosuch]"))}"
	# --- looking a calendar up by id must give the calendar asked for, not ISO.
	# --- The tag form (l01, l02) cannot catch a host that resolves every id to
	# --- ISO; this line exists because a cleanup once removed the only check.
	looked_up = Temporal.calendar_from_id!("hebrew")?
	l72 = "calendar-from-id: ${Temporal.calendar_id!(looked_up)} ${Temporal.calendar_id!(Temporal.calendar_from_id!("islamic")?)} ${date_or_tag({ ..jan31, cal: looked_up }.add!({ months: 1 }))}"
	# --- a non-ISO calendar resolves a wall clock like ISO does: the calendar
	# --- names how a date is shown, never which instant a wall clock means
	lon = Temporal.time_zone_from_id!("Europe/London")?
	scl = Temporal.time_zone_from_id!("America/Santiago")?
	h1025 = { year: 2026, month: 10, day: 25, cal: Hebrew }
	h0308 = { year: 2026, month: 3, day: 8, cal: Hebrew }
	hplain = Temporal.zoned_with!(h0308, { hour: 1, minute: 30 }, lon, Reject)
	hnoon = Temporal.zoned_with!(h1025, { hour: 12 }, lon, Earlier)
	iso0308 = Temporal.plain_date({ year: 2026, month: 3, day: 8, cal: Iso })
	l73a = "calendar-overlap: ${zstr!(Temporal.zoned!(h1025, { hour: 1, minute: 30 }, lon))} ${zstr!(hnoon)} ${zstr!(hplain)} ${zstr!(Temporal.zoned_from_str!("2026-10-25T01:30[Europe/London][u-ca=hebrew]"))}"
	l73b = "calendar-gap: ${zstr!(Temporal.zoned!(h0308, { hour: 2, minute: 30 }, ny))} ${zstr!(Temporal.zoned_with!(h0308, { hour: 2, minute: 30 }, ny, Earlier))} ${zstr!(Temporal.zoned!({ year: 2026, month: 9, day: 6, cal: Hebrew }, { hour: 12 }, scl)?.start_of_day!())}"
	l73c = "calendar-with: ${zstr!(Temporal.zoned!(hplain?.plain_date!(), { hour: 12 }, lon))} ${zstr!(Temporal.zoned_with!(hnoon?.plain_date!(), { hour: 1, minute: 30 }, lon, Later))} ${zstr!(hnoon?.with_plain_date_with!(iso0308, Reject))}"
	# --- rounding a zoned value by the real day and the wall clock: 12:00 on a
	# --- 24-hour day is half way, 12:00 on the 23-hour spring day is past it
	# --- by less than half, and an exact hour stays where it is
	r0307 = Temporal.zoned!({ year: 2026, month: 3, day: 7, cal: Iso }, { hour: 12 }, ny)?
	r0308 = Temporal.zoned!(march8, { hour: 12 }, ny)?
	rmid = Temporal.zoned!({ year: 2026, month: 3, day: 7, cal: Iso }, { hour: 0 }, ny)?
	rheb = Temporal.zoned!({ year: 2026, month: 3, day: 7, cal: Hebrew }, { hour: 12 }, ny)
	l74 = "round-zoned: ${zstr!(r0307.round!(Day))} ${zstr!(r0308.round!(Day))} ${zstr!(rmid.round!(Hour))} ${zstr!(rheb?.round!(Day))} ${zstr!(r0307.round_with!({ smallest: Day, mode: HalfExpand, increment: 2 }))}"
	# --- since counts from its receiver: months back from 2024-01-01 reach
	# --- 2023-12-01, then 14 days; and the mode flips with the sign, so 9 days
	# --- since, floored to months, is none rather than a month
	jan1st = Temporal.plain_date({ year: 2024, month: 1, day: 1, cal: Iso })
	nov17 = Temporal.plain_date({ year: 2023, month: 11, day: 17, cal: Iso })
	jan10 = Temporal.plain_date({ year: 2024, month: 1, day: 10, cal: Iso })
	l75 = "since-rounded: ${ds(jan1st.since_rounded!(nov17, Temporal.date_diff(Year))?)} ${ds(jan10.since_rounded!(jan1st, { largest: Month, smallest: Month, mode: Floor, increment: 1 })?)}"
	# --- a zoned difference in days steps the wall clock, so a day is 23 hours
	# --- only where the zone changes: 03:17 to 02:17 two days before the change
	# --- is 23 hours, not a day; across it, 12:00 to 11:30 is 22.5 hours of a
	# --- 23-hour day, which rounds to one day from either end
	e0307 = Temporal.zoned!({ year: 2024, month: 3, day: 7, cal: Iso }, { hour: 3, minute: 17 }, ny)?
	e0308 = Temporal.zoned!({ year: 2024, month: 3, day: 8, cal: Iso }, { hour: 2, minute: 17 }, ny)?
	e0309 = Temporal.zoned!({ year: 2024, month: 3, day: 9, cal: Iso }, { hour: 12 }, ny)?
	e0310 = Temporal.zoned!({ year: 2024, month: 3, day: 10, cal: Iso }, { hour: 11, minute: 30 }, ny)?
	byday = { largest: Day, smallest: Day, mode: HalfExpand, increment: 1 }
	l76 = "zdt-until-days: ${ds(e0307.until_in!(e0308, Day)?)} ${ds(e0309.until_in!(e0310, Day)?)} ${ds(e0309.until_rounded!(e0310, byday)?)} ${ds(e0310.since_rounded!(e0309, byday)?)}"
	# --- review round: an alias zone is the same zone; a transition is an offset
	# --- change, found even from a zone's last tzif entry, and one that keeps the
	# --- offset is not one; a weekday needs a date; the difference helpers
	# --- truncate as TC39's defaults do; a zoned since counts from its receiver
	calcutta = Temporal.zoned_from_epoch_ns!(0, Temporal.time_zone_from_id!("Asia/Calcutta")?)?
	kolkata = Temporal.zoned_from_epoch_ns!(0, Temporal.time_zone_from_id!("Asia/Kolkata")?)?
	eq_alias = if calcutta.equals!(kolkata)? { "yes" } else { "no" }
	lisbon = Temporal.zoned_from_epoch_ns!(820454400000000000, Temporal.time_zone_from_id!("Europe/Lisbon")?)?
	feb20 = Temporal.plain_date({ year: 2024, month: 2, day: 20, cal: Iso })
	znov17 = Temporal.zoned!({ year: 2023, month: 11, day: 17, cal: Iso }, { hour: 12 }, ny)?
	zjan1 = Temporal.zoned!({ year: 2024, month: 1, day: 1, cal: Iso }, { hour: 12 }, ny)?
	l77 = "review: eq-alias=${eq_alias} lisbon-next=${trans!(lisbon.next_transition!())} dow=${dow_or_tag(Temporal.plain_date({ year: 2024, month: 2, day: 30, cal: Iso }))},${dow_or_tag(Temporal.plain_date({ year: -271821, month: 4, day: 18, cal: Iso }))},${dow_or_tag(Temporal.plain_date({ year: -271821, month: 4, day: 19, cal: Iso }))} diff-default=${ds(jan1st.until_rounded!(feb20, { ..Temporal.date_diff(Year), smallest: Month })?)} zdt-since=${ds(zjan1.since_rounded!(znov17, { largest: Month, smallest: Day, mode: Trunc, increment: 1 })?)}"
	# --- a day of the year needs a date too: February 29 of a leap year is day 60,
	# --- December 31 of one is 366, February 30 is none
	l78 = "day-of-year: ${doy_or_tag(Temporal.plain_date({ year: 2024, month: 2, day: 29, cal: Iso }))} ${doy_or_tag(Temporal.plain_date({ year: 2024, month: 12, day: 31, cal: Iso }))} ${doy_or_tag(Temporal.plain_date({ year: 2024, month: 2, day: 30, cal: Iso }))}"
	# --- lines that tell plausible bugs apart (from the audit): a true tie to the
	# --- hour and to the duration, a comparison only an anchor decides, a day
	# --- step into a gap and across a 25-hour day, a date moved into a gap and
	# --- into an overlap under reject, and ISO week numbering at a year's end
	tie = Temporal.zoned!({ year: 2026, month: 3, day: 7, cal: Iso }, { hour: 10, minute: 30 }, ny)?
	by_hour = |m| { smallest: Hour, mode: m, increment: 1 }
	d2h30 : Temporal.Duration
	d2h30 = { hours: 2, minutes: 30 }
	days29 : Temporal.Duration
	days29 = { days: 29 }
	dur_hour = |m| { largest: Hour, smallest: Hour, mode: m, increment: 1 }
	cairo_step = Temporal.zoned!({ year: 2024, month: 4, day: 25, cal: Iso }, { hour: 0, minute: 30 }, cai)?.add!({ days: 1 })?
	berlin = Temporal.time_zone_from_id!("Europe/Berlin")?
	berlin_step = Temporal.zoned!({ year: 2026, month: 10, day: 24, cal: Iso }, { hour: 23, minute: 40 }, berlin)?.add!({ days: 1 })?
	early = Temporal.zoned!({ year: 2026, month: 3, day: 7, cal: Iso }, { hour: 2, minute: 30 }, ny)?
	lon_early = Temporal.zoned!({ year: 2026, month: 10, day: 24, cal: Iso }, { hour: 1, minute: 30 }, lon)?
	oct25 = Temporal.plain_date({ year: 2026, month: 10, day: 25, cal: Iso })
	wk = Temporal.plain_date({ year: 2024, month: 12, day: 30, cal: Iso }).fields!()?
	l79 = "strong: tie=${tie.round!(Hour)?.to_str!()?},${tie.round_with!(by_hour(HalfEven))?.to_str!()?},${tie.round_with!(by_hour(HalfTrunc))?.to_str!()?} dur-tie=${ds(d2h30.round!(dur_hour(HalfExpand), Unanchored)?)},${ds(d2h30.round!(dur_hour(HalfEven), Unanchored)?)} anchored=${ord(days29.compare!({ months: 1 }, ToDate(Temporal.plain_date({ year: 2024, month: 2, day: 1, cal: Iso })))?)},${ord(days29.compare!({ months: 1 }, ToDate(Temporal.plain_date({ year: 2023, month: 2, day: 1, cal: Iso })))?)} cairo=${cairo_step.to_str!()?},${cairo_step.hours_in_day!()?.to_str()} berlin=${berlin_step.to_str!()?},${berlin_step.hours_in_day!()?.to_str()} move=${zstr!(early.with_plain_date!(march8))},${zstr!(lon_early.with_plain_date_with!(oct25, Reject))} week=${wk.week_of_year.to_str()}/${wk.year_of_week.to_str()}"
	# --- a parsed date must exist: month 13, February 30 and 29 in a common year
	# --- are input errors, a leap day is a date
	l80 = "parse-exists: ${parsed(Temporal.date_parse_in("2026-13-01", "%Y-%m-%d", 2026))} ${parsed(Temporal.date_parse_in("2026-02-30", "%Y-%m-%d", 2026))} ${parsed(Temporal.date_parse_in("2025-02-29", "%Y-%m-%d", 2026))} ${parsed(Temporal.date_parse_in("2024-02-29", "%Y-%m-%d", 2026))} ${parsed(Temporal.date_parse_in("31/04", "%d/%m", 2026))}"
	# --- a parsed time must exist: hour 24 and 25, minute and second 60, a 12-hour
	# --- hour of 0 or 13, and 13 PM through %H are input errors; 23:59:59 and 12 AM
	# --- (midnight) are times
	l81 = "parse-time-exists: ${t_parse(Temporal.time_parse("24:00", "%H:%M"))} ${t_parse(Temporal.time_parse("25:00", "%H:%M"))} ${t_parse(Temporal.time_parse("23:60", "%H:%M"))} ${t_parse(Temporal.time_parse("23:59:60", "%T"))} ${t_parse(Temporal.time_parse("00 AM", "%I %p"))} ${t_parse(Temporal.time_parse("13 PM", "%I %p"))} ${t_parse(Temporal.time_parse("13 PM", "%H %p"))} ${t_parse(Temporal.time_parse("23:59:59", "%T"))} ${t_parse(Temporal.time_parse("12 AM", "%I %p"))}"
	# --- strings: a unit named twice is not a duration; a year outside 0000-9999
	# --- prints with a sign and six digits, and parses back
	dup = match Temporal.duration_from_str!("PT2H3H") { Ok(d) => ds(d), Err(e) => tag(e) }
	far = Temporal.plain_date({ year: -10000, month: 1, day: 1, cal: Iso })
	near_max = Temporal.plain_date({ year: 275760, month: 9, day: 13, cal: Iso })
	fine = Temporal.PlainTime.new({ hour: 9, minute: 30, second: 0, millisecond: 123, microsecond: 456, nanosecond: 789 })
	half_sec = Temporal.PlainTime.new({ hour: 9, minute: 30, second: 0, millisecond: 500, microsecond: 0, nanosecond: 0 })
	l82 = "strings: time=${fine.to_str()},${half_sec.to_str()} nanos=${fine.format("%N")},${fine.format("%L")} dup=${dup} ${far.to_str()} ${near_max.to_str()} ${Temporal.plain_date({ year: 9999, month: 12, day: 31, cal: Iso }).to_str()} back=${Temporal.date_from_str!(far.to_str())?.to_str()}"
	# --- functions nothing else calls: the previous transition, since!, the zone,
	# --- a zoned value on another calendar, a duration's zero and validity, the
	# --- option helpers; and Now's readers by what must hold whatever the clock
	zero_d : Temporal.Duration
	zero_d = {}
	mixed : Temporal.Duration
	mixed = { hours: 1, minutes: -1 }
	one_h : Temporal.Duration
	one_h = { hours: 1 }
	today_utc = Now.plain_date_in!("UTC")?
	parsed_now = Now.date_parse!("25/12", "%d/%m")?
	same_year = if parsed_now.year == today_utc.year and parsed_now.month == 12 and parsed_now.day == 25 { "yes" } else { "no" }
	now_time = match Now.plain_time_in!("UTC") { Ok(_) => "ok", Err(e) => tag(e) }
	l83 = "untested: previous=${trans!(before.previous_transition!())} since=${ds(e0310.since!(e0309)?)} zone=${e0309.time_zone!()?} hebrew=${e0309.with_calendar!(Hebrew).to_str!()?} zero=${yn(zero_d.is_zero())},${yn(one_h.is_zero())} valid=${yn(one_h.valid())},${yn(mixed.valid())} zoned-diff=${ds(e0309.until_rounded!(e0310, Temporal.zoned_diff(Day))?)} round-to=${tie.round_with!(Temporal.round_to(Hour))?.to_str!()?} now-date-parse=${same_year} now-time=${now_time}"
	# --- records past their fields' range print without crashing: a year of
	# --- more than six digits, a millisecond of 1000 or more
	big_years = [1000000, -2147483648, 2147483647].map(|y| Temporal.plain_date({ year: y, month: 1, day: 1, cal: Iso }).to_str()) |> Str.join_with(",")
	over_ms = Temporal.PlainTime.new({ hour: 0, minute: 0, second: 0, millisecond: 1000, microsecond: 0, nanosecond: 0 })
	huge_ms = Temporal.PlainTime.new({ hour: 0, minute: 0, second: 0, millisecond: 65535, microsecond: 0, nanosecond: 0 })
	l84 = "unvalidated: ${big_years} ${over_ms.to_str()} ${huge_ms.format("%H:%M %N")}"
	# --- a pattern that reads a field twice, %L with %N, or %p with no hour is
	# --- the caller's mistake; one field each still parses
	pat = |input, pattern| t_parse(Temporal.time_parse(input, pattern))
	l85 = "pattern-repeats: ${pat("09 13", "%H %H")} ${pat("11 AM PM", "%I %p %p")} ${pat("09:30:00.123.456789012", "%T.%L.%N")} ${pat("PM", "%p")} ${parsed(Temporal.date_parse_in("08 March 03 2026", "%d %B %m %Y", 2026))} ${pat("11 PM", "%I %p")} ${pat("09:30:00.123", "%T.%L")}"
	# --- the upper end of the range: +275760-09-13 is a date (a Saturday, the 257th
	# --- day of a leap year), the day after is not; the lower end's day of year
	top = Temporal.plain_date({ year: 275760, month: 9, day: 13, cal: Iso })
	past_top = Temporal.plain_date({ year: 275760, month: 9, day: 14, cal: Iso })
	bottom = Temporal.plain_date({ year: -271821, month: 4, day: 19, cal: Iso })
	l86 = "range-ends: ${dow_or_tag(top)},${doy_or_tag(top)} ${dow_or_tag(past_top)},${doy_or_tag(past_top)} ${doy_or_tag(bottom)}"
	Ok("${l00}\n${l01}\n${l02}\n${l03}\n${l04}\n${l05}\n${l06}\n${l07}\n${l08}\n${l09}\n${l10}\n${l11}\n${l12}\n${l13}\n${l14}\n${l15}\n${l16}\n${l17}\n${l18}\n${l19}\n${l20}\n${l21}\n${l22}\n${l23}\n${l24}\n${l25}\n${l26}\n${l27}\n${l28}\n${l29}\n${l30}\n${l31}\n${l32}\n${l33}\n${l34}\n${l35}\n${l36}\n${l37}\n${l38}\n${l39}\n${l40}\n${l41}\n${l42}\n${l43}\n${l44}\n${l45}\n${l46}\n${l47}\n${l48}\n${l49}\n${l50}\n${l51}\n${l52}\n${l53}\n${l54}\n${l55}\n${l56}\n${l57}\n${l58}\n${l59}\n${l60}\n${l61}\n${l62}\n${l63}\n${l64}\n${l65}\n${l66}\n${l67}\n${l68}\n${l69}\n${l70}\n${l71}\n${l72}\n${l73a}\n${l73b}\n${l73c}\n${l74}\n${l75}\n${l76}\n${l77}\n${l78}\n${l79}\n${l80}\n${l81}\n${l82}\n${l83}\n${l84}\n${l85}\n${l86}")
}

main! : List(OsStr) => Try({}, _)
main! = |_args|
	match report!({}) {
		Ok(text) => Stdout.line!(text)
		Err(e) => Stdout.line!("FAILED: ${tag(e)}(${msg(e)})")
	}
