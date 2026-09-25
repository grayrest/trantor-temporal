## Pure helpers over the plain record types — no leaf, no effect. Internal to
## trantor-temporal; `Temporal` re-exports what is public.
##
## Types are spelled structurally rather than imported so this module stays free
## of the hosted layer: a `PlainDate` IS `{ year, month, day }` and nothing
## about these functions needs a host.
TemporalPlain :: [].{
	Date : { year : I32, month : U8, day : U8 }
	Time : { hour : U8, minute : U8, second : U8, millisecond : U16, microsecond : U16, nanosecond : U16 }
	Dur : {
		years : I64, months : I64, weeks : I64, days : I64,
		hours : I64, minutes : I64, seconds : I64, milliseconds : I64,
		microseconds : I64, nanoseconds : I64,
	}
	## What the web spells as -1 / 0 / 1 from `compare`. A tag says which way
	## round it is without anyone having to remember the convention.
	Order : [Before, Same, After]
	Sign : [Negative, Zero, Positive]

	# The web's `with({ day: 1 })` used to need a function per field here,
	# because a partial record literal does not compile. It does not need one
	# any more: `{ ..date, day: 1 }` is a record update, which this compiler
	# does have, and the nine setters said the same thing at nine times the
	# length. (The `{ date & day: 1 }` form is the one it lacks.)

	# ---- ordering ----
	#
	# ISO fields compare lexicographically by definition, so this needs no
	# calendar and no host. Equality is already free — Roc compares records
	# structurally — so only ORDER is worth a function.

	compare_date : Date, Date -> Order
	compare_date = |a, b|
		if a.year != b.year {
			order(a.year < b.year)
		} else if a.month != b.month {
			order(a.month < b.month)
		} else if a.day != b.day {
			order(a.day < b.day)
		} else {
			Same
		}

	compare_time : Time, Time -> Order
	compare_time = |a, b| compare_i64(time_nanos(a), time_nanos(b))

	# ---- durations ----

	duration_is_zero : Dur -> Bool
	duration_is_zero = |d|
		d.years == 0 and d.months == 0 and d.weeks == 0 and d.days == 0 and d.hours == 0
		and d.minutes == 0 and d.seconds == 0 and d.milliseconds == 0
		and d.microseconds == 0 and d.nanoseconds == 0

	## Temporal requires every field of a duration to share a sign, so the first
	## non-zero one settles it.
	duration_sign : Dur -> Sign
	duration_sign = |d| {
		fields = [d.years, d.months, d.weeks, d.days, d.hours, d.minutes, d.seconds, d.milliseconds, d.microseconds, d.nanoseconds]
		List.fold(
			fields,
			Zero,
			|acc, v|
				match acc {
					Zero => if v > 0 { Positive } else if v < 0 { Negative } else { Zero }
					other => other
				},
		)
	}

	## ISO 8601: `P1Y2M3D`, `PT4H5M`, `PT0.5S`, `PT0S` for nothing, and a
	## negative duration signed at the front (`-P1D`) as the format requires —
	## not `P-1D`.
	##
	## Sub-second fields are carried into the seconds value and rendered as a
	## fraction, which is the whole of the format that the first version of this
	## function silently dropped: 500 milliseconds came out `PT0S`.
	## Every non-zero field must share a sign. TC39 refuses to CONSTRUCT a mixed
	## duration (`RangeError: Duration was not valid`, measured), but a Roc
	## record has no constructor to refuse — so the check lands here.
	duration_valid : Dur -> Bool
	duration_valid = |d| {
		fields = [d.years, d.months, d.weeks, d.days, d.hours, d.minutes, d.seconds, d.milliseconds, d.microseconds, d.nanoseconds]
		pos = List.any(fields, |v| v > 0)
		neg = List.any(fields, |v| v < 0)
		!(pos and neg)
	}

	## Rejects a mixed-sign duration rather than inventing a value for it. The
	## first version took the first non-zero field's sign and `abs`'d the rest,
	## so `{days:1, hours:-2}` printed `P1DT2H` — 26 hours, for a duration of
	## 22 — and a duration netting to exactly zero printed `PT2S`. Worse than a
	## crash, because it round-trips through `duration_from_str!`.
	## `TooLarge` where the seconds and the carried sub-second fields together
	## exceed I64. The carry-before-scale rewrite removed the multiplication
	## overflow and left the ADDITION, so a duration near the limit aborted the
	## process — in the function whose whole point is returning `Err`.
	duration_to_str : Dur -> Try(Str, [MixedSigns, TooLarge])
	duration_to_str = |d|
		if !duration_valid(d) {
			Err(MixedSigns)
		} else if !seconds_fit(d) {
			Err(TooLarge)
		} else {
			Ok(rendered(d))
		}

	# ---- fixed offsets ----

	## An offset from UTC in whole minutes (D-S3-25).
	Offset : { minutes : I16 }

	## The zone identifier a fixed offset is: `+05:30`, `-00:30`, `+00:00`.
	offset_zone_id : Offset -> Str
	offset_zone_id = |offset| {
		minutes = I16.to_i64(offset.minutes)
		sign = if minutes < 0 { "-" } else { "+" }
		size = abs(minutes)
		"${sign}${two_digits(size // minutes_per_hour)}:${two_digits(size % minutes_per_hour)}"
	}

	## Seconds east of UTC to the nearest minute, half a minute away from zero:
	## a local-mean-time offset such as New York's -4:56:02 is -4:56.
	offset_from_seconds : I64 -> Offset
	offset_from_seconds = |seconds| {
		size = (abs(seconds) + seconds_per_minute // 2) // seconds_per_minute
		{ minutes: I64.to_i16_wrap(if seconds < 0 { -size } else { size }) }
	}

	## The ISO wall clock at `offset` for an instant in nanoseconds since the
	## epoch.
	wall_clock_at : I128, Offset -> { date : Date, time : Time }
	wall_clock_at = |epoch_ns, offset| {
		local_ns = epoch_ns + I16.to_i128(offset.minutes) * nanos_per_minute
		days = floor_div(local_ns, nanos_per_day)
		{ date: civil_from_days(I128.to_i64_wrap(days)), time: time_from_nanos(local_ns - days * nanos_per_day) }
	}
}

rendered : TemporalPlain.Dur -> Str
rendered = |d| {
	neg = TemporalPlain.duration_sign(d) == Negative
	# Carry each unit to whole seconds BEFORE scaling: `ms * 1_000_000`
	# overflows I64 past ~292 years, and `zdt_until_in!(.., Millisecond)`
	# hands back durations well past that.
	ms = abs(d.milliseconds)
	us = abs(d.microseconds)
	ns = abs(d.nanoseconds)
	loose = (ms % 1_000) * 1_000_000 + (us % 1_000_000) * 1_000 + (ns % 1_000_000_000)
	secs = abs(d.seconds) + ms // 1_000 + us // 1_000_000 + ns // 1_000_000_000 + loose // 1_000_000_000
	frac = loose % 1_000_000_000
	date_part = Str.concat(
		Str.concat(unit_str(abs(d.years), "Y"), unit_str(abs(d.months), "M")),
		Str.concat(unit_str(abs(d.weeks), "W"), unit_str(abs(d.days), "D")),
	)
	sec_part =
		if frac != 0 {
			"${secs.to_str()}.${trim_zeros(nine_digits(frac))}S"
		} else {
			unit_str(secs, "S")
		}
	time_part = Str.concat(Str.concat(unit_str(abs(d.hours), "H"), unit_str(abs(d.minutes), "M")), sec_part)
	body =
		if Str.is_empty(date_part) and Str.is_empty(time_part) {
			"PT0S"
		} else if Str.is_empty(time_part) {
			Str.concat("P", date_part)
		} else {
			Str.concat(Str.concat("P", date_part), Str.concat("T", time_part))
		}
	if neg { Str.concat("-", body) } else { body }
}

## Whether `rendered`'s carry can be done without overflowing I64. Everything
## is compared against the headroom left, never summed first.
seconds_fit : TemporalPlain.Dur -> Bool
seconds_fit = |d| {
	max = 9223372036854775807
	carried = abs(d.milliseconds) // 1_000 + abs(d.microseconds) // 1_000_000 + abs(d.nanoseconds) // 1_000_000_000
	s = abs(d.seconds)
	carried <= max - s and carried + s <= max - 3
}

order : Bool -> [Before, Same, After]
order = |less| if less { Before } else { After }

compare_i64 : I64, I64 -> [Before, Same, After]
compare_i64 = |a, b| if a == b { Same } else { order(a < b) }

## Saturating. `0 - v` overflows at I64's most negative value, and this is a
## printer — a duration that absurd has no faithful rendering anyway. The
## non-negative case is tested FIRST: probing for I64::MIN by adding I64::MAX
## overflows for any positive input, which is a crash of its own.
abs : I64 -> I64
abs = |v| if v >= 0 { v } else if v + 9223372036854775807 < 0 { 9223372036854775807 } else { 0 - v }

time_nanos : TemporalPlain.Time -> I64
time_nanos = |t|
	U8.to_i64(t.hour) * 3_600_000_000_000
	+ U8.to_i64(t.minute) * 60_000_000_000
	+ U8.to_i64(t.second) * 1_000_000_000
	+ U16.to_i64(t.millisecond) * 1_000_000
	+ U16.to_i64(t.microsecond) * 1_000
	+ U16.to_i64(t.nanosecond)

unit_str : I64, Str -> Str
unit_str = |value, suffix| if value == 0 { "" } else { Str.concat(value.to_str(), suffix) }

nine_digits : I64 -> Str
nine_digits = |v| {
	s = v.to_str()
	len = Str.count_utf8_bytes(s)
	if len >= 9 { s } else { Str.concat(Str.repeat("0", 9 - len), s) }
}

## `500000000` -> `5`: ISO 8601 writes the fraction with no trailing zeros.
trim_zeros : Str -> Str
trim_zeros = |s| {
	bytes = Str.to_utf8(s)
	kept = List.fold(bytes, { out: [], pending: [] }, |acc, b|
		if b == '0' {
			{ out: acc.out, pending: List.append(acc.pending, b) }
		} else {
			{ out: List.concat(List.concat(acc.out, acc.pending), [b]), pending: [] }
		})
	Str.from_utf8_lossy(kept.out)
}

minutes_per_hour : I64
minutes_per_hour = 60

seconds_per_minute : I64
seconds_per_minute = 60

nanos_per_minute : I128
nanos_per_minute = 60_000_000_000

nanos_per_day : I128
nanos_per_day = 86_400_000_000_000

## Civil-from-days constants (Howard Hinnant's algorithm): days from
## 0000-03-01 to the epoch, and the days in a 400-year era.
epoch_shift_days : I64
epoch_shift_days = 719_468

days_per_era : I64
days_per_era = 146_097

two_digits : I64 -> Str
two_digits = |v| if v < 10 { "0${v.to_str()}" } else { v.to_str() }

## Division rounding toward negative infinity; `//` truncates.
floor_div : I128, I128 -> I128
floor_div = |n, d| {
	q = n // d
	if n % d < 0 { q - 1 } else { q }
}

## The proleptic Gregorian date `days` after 1970-01-01, with years counted
## from a March 1 so the leap day falls at the end.
civil_from_days : I64 -> TemporalPlain.Date
civil_from_days = |days| {
	shifted = days + epoch_shift_days
	era = (if shifted >= 0 { shifted } else { shifted - days_per_era + 1 }) // days_per_era
	day_of_era = shifted - era * days_per_era
	year_of_era = (day_of_era - day_of_era // 1_460 + day_of_era // 36_524 - day_of_era // 146_096) // 365
	day_of_year = day_of_era - (365 * year_of_era + year_of_era // 4 - year_of_era // 100)
	march_month = (5 * day_of_year + 2) // 153
	day = day_of_year - (153 * march_month + 2) // 5 + 1
	month = if march_month < 10 { march_month + 3 } else { march_month - 9 }
	year = year_of_era + era * 400 + (if month <= 2 { 1 } else { 0 })
	{ year: I64.to_i32_wrap(year), month: I64.to_u8_wrap(month), day: I64.to_u8_wrap(day) }
}

## Nanoseconds into a day, 0 up to a day, as a time.
time_from_nanos : I128 -> TemporalPlain.Time
time_from_nanos = |ns| {
	field = |unit, size| I128.to_u64_wrap((ns // unit) % size)
	{
		hour: U64.to_u8_wrap(field(3_600_000_000_000, 24)),
		minute: U64.to_u8_wrap(field(60_000_000_000, 60)),
		second: U64.to_u8_wrap(field(1_000_000_000, 60)),
		millisecond: U64.to_u16_wrap(field(1_000_000, 1_000)),
		microsecond: U64.to_u16_wrap(field(1_000, 1_000)),
		nanosecond: U64.to_u16_wrap(field(1, 1_000)),
	}
}

midnight : TemporalPlain.Time
midnight = { hour: 0, minute: 0, second: 0, millisecond: 0, microsecond: 0, nanosecond: 0 }

expect TemporalPlain.offset_zone_id({ minutes: -30 }) == "-00:30"
expect TemporalPlain.offset_zone_id({ minutes: 330 }) == "+05:30"
expect TemporalPlain.offset_zone_id({ minutes: 0 }) == "+00:00"
expect TemporalPlain.offset_from_seconds(-17_762) == { minutes: -296 }
expect TemporalPlain.offset_from_seconds(-2_670) == { minutes: -45 }
expect TemporalPlain.offset_from_seconds(1_172) == { minutes: 20 }
expect TemporalPlain.offset_from_seconds(29) == { minutes: 0 }
expect TemporalPlain.wall_clock_at(0, { minutes: 0 }) == { date: { year: 1970, month: 1, day: 1 }, time: midnight }
expect TemporalPlain.wall_clock_at(-1, { minutes: 0 }) == { date: { year: 1969, month: 12, day: 31 }, time: { hour: 23, minute: 59, second: 59, millisecond: 999, microsecond: 999, nanosecond: 999 } }
expect TemporalPlain.wall_clock_at(0, { minutes: -30 }) == { date: { year: 1969, month: 12, day: 31 }, time: { ..midnight, hour: 23, minute: 30 } }
expect TemporalPlain.wall_clock_at(951_782_400_000_000_000, { minutes: 0 }).date == { year: 2000, month: 2, day: 29 }
expect TemporalPlain.wall_clock_at(-62_135_596_800_000_000_000, { minutes: 0 }).date == { year: 1, month: 1, day: 1 }
expect TemporalPlain.wall_clock_at(-62_198_755_200_000_000_000, { minutes: 0 }).date == { year: -1, month: 1, day: 1 }
expect TemporalPlain.wall_clock_at(253_402_300_800_000_000_000, { minutes: 0 }).date == { year: 10000, month: 1, day: 1 }
