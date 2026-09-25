import TemporalHost
import TemporalStrftime
import TemporalPlain

## TC39 Temporal-shaped calendar and time-zone arithmetic.
##
## This is the surface to use. It sits over the primitive `TemporalHost`
## interface the way `File` sits over `Fs` (D-T1-1): the leaves take every
## option explicitly, and the defaults live here.
##
## `PlainDate`, `PlainTime` and `Duration` are plain records of ISO fields in
## both directions, a `Calendar` is a tag and a `TimeZone` is its identifier;
## `ZonedDateTime` is the one refcounted host resource (D-T2-4, D-T2-5).
## There is deliberately no `PlainDateTime` (D-T1-11) — a zoned
## value or a date-and-time pair covers what a CLI platform needs, and a third
## type would double the arithmetic surface.
Temporal :: [].{
	TimeZone : TemporalHost.TimeZone
	Calendar : TemporalHost.Calendar
	Unit : TemporalHost.Unit
	Overflow : TemporalHost.Overflow
	Disambiguation : TemporalHost.Disambiguation
	Err : TemporalHost.Err
	ParseErr : TemporalStrftime.Err
	## `Before`/`Same`/`After` rather than the web's -1/0/1: a tag says which
	## way round it is without anyone recalling the convention.
	Order : TemporalPlain.Order
	CalendarFields : TemporalHost.CalendarFields
	RoundingMode : TemporalHost.RoundingMode
	Direction : TemporalHost.Direction
	## The host's `RelativeTo` carries a bare record; this one carries the
	## package's own date, so a caller writes `ToDate(feb)` with the value they
	## already have. Crossing the ABI with the wrong one is not a type error —
	## it panics the compiler at specialisation time — so the conversion is
	## explicit and lives in `lower_relative`.
	RelativeTo : [Unanchored, ToDate(PlainDate)]
	DiffOptions : TemporalHost.DiffOptions
	RoundOptions : TemporalHost.RoundOptions
	## The host's `Transition` carries a raw handle; this one carries the type
	## the rest of the package speaks, so a caller can chain off `At(z)`.
	Transition : [NoTransition, At(ZonedDateTime)]
	Sign : TemporalPlain.Sign


	## `Duration.negate`: every sign flipped, saturating at I64's limit.
	negate : Duration -> Duration
	negate = |d| d.negate()

	# ---- calendars and zones ----

	## Resolves an identifier and its aliases — `"islamic"` is `IslamicCivil`.
	## A tag written directly needs no call at all.
	calendar_from_id! : Str => Try(Calendar, Err)
	calendar_from_id! = |id| TemporalHost.calendar_from_id!(id)

	calendar_id! : Calendar => Str
	calendar_id! = |c| TemporalHost.calendar_id!(c)

	## Validates an identifier and answers it as the zone database spells it —
	## an alias stays an alias (`Asia/Calcutta`), as TC39 keeps it; `equals!`
	## treats the two as the same zone.
	time_zone_from_id! : Str => Try(TimeZone, Err)
	time_zone_from_id! = |id| TemporalHost.time_zone_from_id!(id)

	## A zone IS its identifier now, so this is the identity — kept because
	## callers read better for saying what they mean.
	time_zone_id : TimeZone -> Str
	time_zone_id = |t| t

	# ---- constructing values ----

	## A date is three ISO fields and a calendar, which defaults to `Iso`. It does
	## NOT validate — the operation that consumes it does, exactly as the web
	## defers validation past `with`. This names the type for a bare literal;
	## wherever the type is already known, the record alone is enough.
	plain_date : PlainDate -> PlainDate
	plain_date = |d| d

	## A date on `Iso` from a record typed elsewhere — a `Toml.LocalDate`'s
	## fields, say, which `plain_date` does not take (D-S3-37.1).
	plain_date_from_fields : { year : I32, month : U8, day : U8 } -> PlainDate
	plain_date_from_fields = |fields| PlainDate.lift(fields, Iso)

	plain_time_from_fields : { hour : U8, minute : U8, second : U8, millisecond : U16, microsecond : U16, nanosecond : U16 } -> PlainTime
	plain_time_from_fields = |fields| PlainTime.new(fields)

	# ---- constructing zoned values ----

	## On the ISO calendar; `.with_calendar!(Hebrew)` moves it to another.
	zoned_from_epoch_ns! : I128, TimeZone => Try(ZonedDateTime, Err)
	zoned_from_epoch_ns! = |ns, t| TemporalHost.zdt_from_epoch_ns!(ns, t, Iso).map_ok(ZonedDateTime.wrap)

	## What instant a wall-clock date and time name in a zone. Ambiguity is
	## resolved `Compatible`, TC39's default — later for a spring-forward gap,
	## earlier for a fall-back overlap. `zoned_with!` takes the choice,
	## including `Reject` to be told rather than guessed for.
	## The calendar is the date's.
	zoned! : PlainDate, PlainTime, TimeZone => Try(ZonedDateTime, Err)
	zoned! = |d, t, z| TemporalHost.zdt_from_wall_clock!(d.rec(), t.rec(), z, d.cal, Compatible).map_ok(ZonedDateTime.wrap)

	zoned_with! : PlainDate, PlainTime, TimeZone, Disambiguation => Try(ZonedDateTime, Err)
	zoned_with! = |d, t, z, dis| TemporalHost.zdt_from_wall_clock!(d.rec(), t.rec(), z, d.cal, dis).map_ok(ZonedDateTime.wrap)

	## An offset date-time, as the date methods decode one, as a value in the
	## fixed-offset zone it names (`-00:30`), on `Iso`: an instant, not a place
	## (D-S3-25). A fixed offset has no gaps or overlaps to resolve.
	zoned_from_offset! : { date : TemporalPlain.Date, time : TemporalPlain.Time, offset : TemporalPlain.Offset } => Try(ZonedDateTime, Err)
	zoned_from_offset! = |moment|
		TemporalHost.zdt_from_wall_clock!(moment.date, moment.time, TemporalPlain.offset_zone_id(moment.offset), Iso, Reject).map_ok(ZonedDateTime.wrap)

	## IXDTF. A string carrying an offset that disagrees with its zone is an
	## error rather than a silent preference for one of them (D-T1-12).
	zoned_from_str! : Str => Try(ZonedDateTime, Err)
	zoned_from_str! = |s| TemporalHost.zdt_from_str!(s).map_ok(ZonedDateTime.wrap)

	## IXDTF: `2026-03-08`, optionally annotated `2026-03-08[u-ca=hebrew]`.
	date_from_str! : Str => Try(PlainDate, Err)
	date_from_str! = |s| TemporalHost.date_from_str!(s).map_ok(|p| PlainDate.lift(p.date, p.calendar))

	## ISO 8601: `P1Y2M3D`, `PT4H5M`, `PT0S` for nothing.
	duration_from_str! : Str => Try(Duration, Err)
	duration_from_str! = |s| TemporalHost.duration_from_str!(s).map_ok(Duration.new)

	# ---- difference and rounding options ----

	## TC39's defaults for everything but the largest unit: the smallest unit
	## the type has, `Trunc`, increment 1 (D-T2-15). Dates cannot round below a
	## day, which is why the two constructors differ. The mode only shows once a
	## caller raises `smallest`: January 1 to February 20 by months is P1M.
	date_diff : Unit -> DiffOptions
	date_diff = |largest| { largest: largest, smallest: Day, mode: Trunc, increment: 1 }

	zoned_diff : Unit -> DiffOptions
	zoned_diff = |largest| { largest: largest, smallest: Nanosecond, mode: Trunc, increment: 1 }

	round_to : Unit -> RoundOptions
	round_to = |smallest| { smallest: smallest, mode: HalfExpand, increment: 1 }

	# ---- parsing (pure) ----

	## Strict (D-T1-10): literals match exactly, the whole input is consumed,
	## nothing defaults, and the result must be a date — `"2026-02-30"` is
	## `BadInput`, not a record for `day_of_week` to refuse later (D-T2-21). `%b`/`%a`/`%p` are case-insensitive; a `%a` weekday is
	## checked against the date rather than ignored.
	## `default_year` stands in when the pattern names no year, so `"%d/%m"` is
	## a whole date. `Now.date_parse!` passes the current year; this stays pure,
	## which is what a test can pin.
	date_parse_in : Str, Str, I32 -> Try(PlainDate, ParseErr)
	date_parse_in = |input, pattern, default_year|
		match TemporalStrftime.parse_fields(input, pattern, default_year) {
			Err(e) => Err(e)
			Ok(f) =>
				if !f.had_date {
					Err(BadPattern("no month and day in the pattern, and no %j either"))
				} else if !date_exists(f.parts.year, f.parts.month, f.parts.day) {
					Err(BadInput("${f.parts.year.to_str()}-${f.parts.month.to_str()}-${f.parts.day.to_str()} is not a date in Temporal's range"))
				} else {
					Ok(PlainDate.lift({ year: f.parts.year, month: f.parts.month, day: f.parts.day }, Iso))
				}
		}

	## A time has no year, month or day, and demanding them is why this could
	## not parse a time at all: `time_parse("09:30", "%H:%M")` returned
	## `BadPattern: no year in the pattern`.
	time_parse : Str, Str -> Try(PlainTime, ParseErr)
	time_parse = |input, pattern|
		match TemporalStrftime.parse_fields(input, pattern, 1970) {
			Err(e) => Err(e)
			Ok(f) =>
				if !f.had_time {
					Err(BadPattern("no time in the pattern; a parsed time cannot default one"))
				} else {
					p = f.parts
					Ok(
						PlainTime.new({
							hour: p.hour,
							minute: p.minute,
							second: p.second,
							millisecond: if p.millisecond != 0 { p.millisecond } else { U32.to_u16_wrap(p.nanosecond // 1_000_000) },
							microsecond: U32.to_u16_wrap((p.nanosecond // 1_000) % 1_000),
							nanosecond: U32.to_u16_wrap(p.nanosecond % 1_000),
						}),
					)
				}
		}

	is_leap_year : I32 -> Bool
	is_leap_year = |y| TemporalStrftime.is_leap(y)

	## A calendar date, as ISO fields. Nominal so that it can carry the operations
	## that belong to it — `jan31.add!(...)` rather than `Temporal.add!(jan31, ...)`
	## — but its fields stay readable, so `d.year` works and a record built from
	## them goes straight back in (D-T1-2).
	##
	## Changing a field needs no method: `Temporal.plain_date({ ..jan31, day: 1 })`
	## is the language's own record update, and is what the web spells `with`.
	## Changing the calendar is the same update: `{ ..jan31, cal: Hebrew }`.
	##
	## The calendar selects which rules the ARITHMETIC follows. The fields are
	## ISO whatever it is, so a Hebrew date still reads back `{ 2024, 1, 31 }`.
	PlainDate := { year : I32, month : U8, day : U8, cal : Calendar ?? Iso }.{
		## A host answer, which is ISO fields alone, back on a calendar.
		lift : TemporalHost.PlainDate, Calendar -> PlainDate
		lift = |r, c| PlainDate.{ year: r.year, month: r.month, day: r.day, cal: c }

		## The plain record the host leaves take. A bare `{ ..d }` does not parse,
		## so the field list is written once, here.
		rec : PlainDate -> TemporalHost.PlainDate
		rec = |d| { year: d.year, month: d.month, day: d.day }

		## Decodes from any format with the date methods (trantor-encoding's TOML
		## and CSV) onto `Iso`, with no dependency on one (D-S3-22).
		parser_for : format -> (state -> Try({ value : PlainDate, rest : state }, [Mismatch({ path : List([Key(Str), Index(U64)]), expected : Str })]))
			where [format.parse_local_date : format, state -> Try({ value : TemporalPlain.Date, rest : state }, [Mismatch({ path : List([Key(Str), Index(U64)]), expected : Str })])]
		parser_for = |format| |state| {
			parsed = format.parse_local_date(state) ? |Mismatch(problem)| Mismatch(problem)
			Ok({ value: PlainDate.lift(parsed.value, Iso), rest: parsed.rest })
		}

		## Encodes the ISO fields; the calendar is dropped (D-S3-23).
		encoder_for : encoder -> (PlainDate, state -> Try(state, err))
			where [encoder.encode_local_date : encoder, TemporalPlain.Date, state -> Try(state, err)]
		encoder_for = |encoder| |d, state| encoder.encode_local_date(d.rec(), state)

		## TC39's `equals`: the same ISO day AND the same calendar.
		is_eq : PlainDate, PlainDate -> Bool
		is_eq = |a, b| a.year == b.year and a.month == b.month and a.day == b.day and a.cal == b.cal

		## ISO fields order lexicographically, so this ignores the calendar, as
		## TC39's `PlainDate.compare` does: a Hebrew and an ISO date naming the
		## same day are `Same` here and not `==`.
		compare : PlainDate, PlainDate -> TemporalPlain.Order
		compare = |a, b| TemporalPlain.compare_date(a.rec(), b.rec())

		# ---- arithmetic ----

		## Overflow defaults to `Constrain`, TC39's default: 2024-01-31 + 1 month is
		## 2024-02-29. A malformed date is rejected either way (D-T1-5).
		add! : PlainDate, Duration => Try(PlainDate, Err)
		add! = |d, dur| TemporalHost.date_add!(d.rec(), dur.rec(), d.cal, Constrain).map_ok(|r| PlainDate.lift(r, d.cal))

		add_with! : PlainDate, Duration, Overflow => Try(PlainDate, Err)
		add_with! = |d, dur, o| TemporalHost.date_add!(d.rec(), dur.rec(), d.cal, o).map_ok(|r| PlainDate.lift(r, d.cal))

		subtract! : PlainDate, Duration => Try(PlainDate, Err)
		subtract! = |d, dur| TemporalHost.date_add!(d.rec(), dur.negate().rec(), d.cal, Constrain).map_ok(|r| PlainDate.lift(r, d.cal))

		## Answers in days, TC39's default for a date difference: 2024-01-01 to
		## 2024-12-25 is 359 days. `until_in!` with `Year` reads it as 11 months
		## and 24 days instead.
		## A difference counts months by one calendar's rules, so two dates on
		## different calendars have none: `OutOfRange`, as TC39's RangeError.
		until! : PlainDate, PlainDate => Try(Duration, Err)
		until! = |a, b| {
			c = same_calendar(a, b)?
			TemporalHost.date_until!(a.rec(), b.rec(), c, Day).map_ok(Duration.new)
		}

		until_in! : PlainDate, PlainDate, Unit => Try(Duration, Err)
		until_in! = |a, b, u| {
			c = same_calendar(a, b)?
			TemporalHost.date_until!(a.rec(), b.rec(), c, u).map_ok(Duration.new)
		}

		## `until!` with the arguments swapped, so the result points backwards.
		since! : PlainDate, PlainDate => Try(Duration, Err)
		since! = |a, b| {
			c = same_calendar(a, b)?
			TemporalHost.date_until!(b.rec(), a.rec(), c, Day).map_ok(Duration.new)
		}

		until_rounded! : PlainDate, PlainDate, DiffOptions => Try(Duration, Err)
		until_rounded! = |a, b, o| {
			c = same_calendar(a, b)?
			TemporalHost.date_until_rounded!(a.rec(), b.rec(), c, o).map_ok(Duration.new)
		}

		## TC39's `since`: `until` from the same receiver, rounded the other way
		## and negated. Swapping the arguments is not the same — months counted
		## back from `a` are not months counted forward from `b`, so 2024-01-01
		## since 2023-11-17 is P1M14D where the swap said P1M15D (D-T2-12).
		since_rounded! : PlainDate, PlainDate, DiffOptions => Try(Duration, Err)
		since_rounded! = |a, b, o| {
			c = same_calendar(a, b)?
			TemporalHost.date_until_rounded!(a.rec(), b.rec(), c, { ..o, mode: negated_mode(o.mode) }).map_ok(|r| Duration.new(r).negate())
		}

		# ---- what a calendar derives ----

		## Everything the calendar derives, in one crossing rather than twelve.
		## `week_of_year`/`year_of_week` are 0 where the calendar has no week
		## numbering and `era` is empty where it has no eras. An ISO week in week
		## year 0 (the ISO year 0000) has `year_of_week` 0 too; its nonzero
		## `week_of_year` is what tells it apart.
		fields! : PlainDate => Try(CalendarFields, Err)
		fields! = |d| TemporalHost.date_fields!(d.rec(), d.cal)

		## Day of week, Monday = 1 … Sunday = 7. Pure, because a record is ISO
		## fields, and every supported calendar has the ISO week (D-T2-8).
		## `OutOfRange` for a record that names no date — February 30, or a day
		## outside Temporal's range — as every host call refuses one (D-T1-5).
		day_of_week : PlainDate -> Try(U8, Err)
		day_of_week = |d|
			if date_exists(d.year, d.month, d.day) {
				Ok(TemporalStrftime.day_of_week(d.year, d.month, d.day))
			} else {
				Err(OutOfRange("${d.year.to_str()}-${d.month.to_str()}-${d.day.to_str()} is not a date in Temporal's range"))
			}

		## Day of the ISO year, 1 … 366. `OutOfRange` for a record that names no
		## date, as `day_of_week` (D-T2-18).
		iso_day_of_year : PlainDate -> Try(U16, Err)
		iso_day_of_year = |d|
			if date_exists(d.year, d.month, d.day) {
				Ok(TemporalStrftime.day_of_year(d.year, d.month, d.day))
			} else {
				Err(OutOfRange("${d.year.to_str()}-${d.month.to_str()}-${d.day.to_str()} is not a date in Temporal's range"))
			}

		# ---- rendering ----

		## ISO 8601, zero-padded: `2026-03-08`. A year outside 0000-9999 takes a
		## sign and six digits (`-010000-01-01`, `+275760-09-13`), as IXDTF requires
		## and `date_from_str!` reads back; strftime's `%Y` did not, so such a date
		## printed a string nothing parses (D-T2-25). A date on any calendar but
		## `Iso` ends with its annotation, `2026-06-15[u-ca=hebrew]`, as TC39's
		## `calendarName: "auto"` prints it, so it parses back on that calendar
		## (D-T2-32).
		to_str : PlainDate -> Str
		to_str = |d| "${iso_year(d.year)}${TemporalStrftime.format(date_parts(d.rec()), "-%m-%d")}${calendar_annotation(d.cal)}"

		## strftime (D-T1-9). `%Y %y %m %d %e %b %B %a %A %j %H %I %p %M %S %L %N
		## %F %T %% %n %t`; `%z %:z %Z` need a zone, so on a plain date they render
		## literally. An unknown directive renders literally too.
		format : PlainDate, Str -> Str
		format = |d, pattern| TemporalStrftime.format(date_parts(d.rec()), pattern)
	}

	## A time of day, as ISO fields. Every field defaults to 0 (`??` in the
	## declaration), so a record naming only `hour` is a whole time.
	PlainTime := { hour : U8 ?? 0, minute : U8 ?? 0, second : U8 ?? 0, millisecond : U16 ?? 0, microsecond : U16 ?? 0, nanosecond : U16 ?? 0 }.{
		new : { hour : U8, minute : U8, second : U8, millisecond : U16, microsecond : U16, nanosecond : U16 } -> PlainTime
		new = |r| PlainTime.{ hour: r.hour, minute: r.minute, second: r.second, millisecond: r.millisecond, microsecond: r.microsecond, nanosecond: r.nanosecond }

		rec : PlainTime -> TemporalHost.PlainTime
		rec = |t| { hour: t.hour, minute: t.minute, second: t.second, millisecond: t.millisecond, microsecond: t.microsecond, nanosecond: t.nanosecond }

		## Decodes from any format with the date methods, as `PlainDate` does.
		parser_for : format -> (state -> Try({ value : PlainTime, rest : state }, [Mismatch({ path : List([Key(Str), Index(U64)]), expected : Str })]))
			where [format.parse_local_time : format, state -> Try({ value : TemporalPlain.Time, rest : state }, [Mismatch({ path : List([Key(Str), Index(U64)]), expected : Str })])]
		parser_for = |format| |state| {
			parsed = format.parse_local_time(state) ? |Mismatch(problem)| Mismatch(problem)
			Ok({ value: PlainTime.new(parsed.value), rest: parsed.rest })
		}

		encoder_for : encoder -> (PlainTime, state -> Try(state, err))
			where [encoder.encode_local_time : encoder, TemporalPlain.Time, state -> Try(state, err)]
		encoder_for = |encoder| |t, state| encoder.encode_local_time(t.rec(), state)

		is_eq : PlainTime, PlainTime -> Bool
		is_eq = |a, b| TemporalPlain.compare_time(a.rec(), b.rec()) == Same

		compare : PlainTime, PlainTime -> TemporalPlain.Order
		compare = |a, b| TemporalPlain.compare_time(a.rec(), b.rec())

		## ISO 8601 as TC39's `toString` prints it: `09:30:00`, with the fraction
		## of a second when there is one and no trailing zeros — `09:30:00.5`,
		## `09:30:00.123456789` (D-T2-28).
		to_str : PlainTime -> Str
		to_str = |t| {
			whole = TemporalStrftime.format(time_parts(t.rec()), "%H:%M:%S")
			ns = sub_second_nanos(t.rec())
			digits = ns.to_str()
			length = Str.count_utf8_bytes(digits)
			if ns == 0 {
				whole
			} else if length > 9 {
				# Fields past their range name no time; print what they hold rather
				# than crash on the padding or trim it into a smaller fraction.
				"${whole}.${digits}"
			} else {
				var $fraction = Str.to_utf8("${Str.repeat("0", 9 - length)}${digits}")
				while List.last($fraction) == Ok('0') {
					$fraction = List.drop_last($fraction, 1)
				}
				"${whole}.${Str.from_utf8_lossy($fraction)}"
			}
		}

		format : PlainTime, Str -> Str
		format = |t, pattern| TemporalStrftime.format(time_parts(t.rec()), pattern)
	}

	## An amount of time, as the ten fields TC39 names. Every field defaults to 0,
	## so `{ months: 1 }` is a whole duration wherever one is expected. A default
	## is a property of this NOMINAL type: the host's unnamed record has none.
	Duration := { years : I64 ?? 0, months : I64 ?? 0, weeks : I64 ?? 0, days : I64 ?? 0, hours : I64 ?? 0, minutes : I64 ?? 0, seconds : I64 ?? 0, milliseconds : I64 ?? 0, microseconds : I64 ?? 0, nanoseconds : I64 ?? 0 }.{
		new : { years : I64, months : I64, weeks : I64, days : I64, hours : I64, minutes : I64, seconds : I64, milliseconds : I64, microseconds : I64, nanoseconds : I64 } -> Duration
		new = |r| Duration.{ years: r.years, months: r.months, weeks: r.weeks, days: r.days, hours: r.hours, minutes: r.minutes, seconds: r.seconds, milliseconds: r.milliseconds, microseconds: r.microseconds, nanoseconds: r.nanoseconds }

		rec : Duration -> TemporalHost.Duration
		rec = |d| { years: d.years, months: d.months, weeks: d.weeks, days: d.days, hours: d.hours, minutes: d.minutes, seconds: d.seconds, milliseconds: d.milliseconds, microseconds: d.microseconds, nanoseconds: d.nanoseconds }

		is_eq : Duration, Duration -> Bool
		is_eq = |a, b|
			a.years == b.years and a.months == b.months and a.weeks == b.weeks and a.days == b.days
			and a.hours == b.hours and a.minutes == b.minutes and a.seconds == b.seconds
			and a.milliseconds == b.milliseconds and a.microseconds == b.microseconds and a.nanoseconds == b.nanoseconds

		## Every field's sign flipped. A record is not validated, so a field at
		## I64's most negative value, which has no positive twin, saturates to the
		## most positive rather than aborting the program — as `to_str` does.
		negate : Duration -> Duration
		negate = |d|
			Duration.{
				years: flipped(d.years),
				months: flipped(d.months),
				weeks: flipped(d.weeks),
				days: flipped(d.days),
				hours: flipped(d.hours),
				minutes: flipped(d.minutes),
				seconds: flipped(d.seconds),
				milliseconds: flipped(d.milliseconds),
				microseconds: flipped(d.microseconds),
				nanoseconds: flipped(d.nanoseconds),
			}

		is_zero : Duration -> Bool
		is_zero = |d| TemporalPlain.duration_is_zero(d.rec())

		## Temporal requires every field to share a sign, so the first non-zero one
		## settles it.
		sign : Duration -> TemporalPlain.Sign
		sign = |d| TemporalPlain.duration_sign(d.rec())

		valid : Duration -> Bool
		valid = |d| TemporalPlain.duration_valid(d.rec())

		## `Err(MixedSigns)` for a record whose fields disagree in sign — TC39
		## refuses to construct one, and this refuses to print one. `Err(TooLarge)`
		## where the seconds and sub-second fields cannot be carried into an I64.
		to_str : Duration -> Try(Str, [MixedSigns, TooLarge])
		to_str = |d| TemporalPlain.duration_to_str(d.rec())

		## A duration in calendar units has no fixed length until it is anchored to
		## a date — "one month" is 28 to 31 days — so these take a `RelativeTo`.
		## `Unanchored` is an error for those units rather than a guess; it is fine
		## for hours and below.
		round! : Duration, DiffOptions, RelativeTo => Try(Duration, Err)
		round! = |d, o, rel| TemporalHost.duration_round!(d.rec(), o, lower_relative(rel), calendar_of(rel)).map_ok(Duration.new)

		## The duration as one fractional count of `Unit` — "how many hours is
		## this", which the record itself cannot answer.
		total! : Duration, Unit, RelativeTo => Try(F64, Err)
		total! = |d, u, rel| TemporalHost.duration_total!(d.rec(), u, lower_relative(rel), calendar_of(rel))

		compare! : Duration, Duration, RelativeTo => Try(TemporalPlain.Order, Err)
		compare! = |a, b, rel|
			TemporalHost.duration_compare!(a.rec(), b.rec(), lower_relative(rel), calendar_of(rel)).map_ok(
				|n| if n == 0 { Same } else if n < 0 { Before } else { After },
			)
	}

	## An instant, in a zone, on a calendar. The one refcounted resource in the
	## package: a nominal type directly over the host handle, which
	## `ZonedDateTime.(h)` wraps and unwraps (D-T2-7).
	ZonedDateTime := TemporalHost.ZonedDateTime.{
		wrap : TemporalHost.ZonedDateTime -> ZonedDateTime
		wrap = |h| ZonedDateTime.(h)

		handle : ZonedDateTime -> TemporalHost.ZonedDateTime
		handle = |ZonedDateTime.(h)| h

		# ---- reading ----

		epoch_ns! : ZonedDateTime => I128
		epoch_ns! = |z| TemporalHost.zdt_epoch_ns!(z.handle())

		time_zone! : ZonedDateTime => Try(TimeZone, Err)
		time_zone! = |z| TemporalHost.zdt_time_zone!(z.handle())

		calendar! : ZonedDateTime => Calendar
		calendar! = |z| TemporalHost.zdt_calendar!(z.handle())

		plain_date! : ZonedDateTime => PlainDate
		plain_date! = |z| PlainDate.lift(TemporalHost.zdt_plain_date!(z.handle()), TemporalHost.zdt_calendar!(z.handle()))

		plain_time! : ZonedDateTime => PlainTime
		plain_time! = |z| PlainTime.new(TemporalHost.zdt_plain_time!(z.handle()))

		offset_seconds! : ZonedDateTime => I64
		offset_seconds! = |z| TemporalHost.zdt_offset_seconds!(z.handle())

		## The instant as an offset date-time for the date methods to encode:
		## the offset rounded to whole minutes (half away from zero) and the ISO
		## wall clock recomputed at it, so the instant is kept and the wall clock
		## absorbs what rounding removed (D-S3-25, D-S3-37.9). The zone's name
		## and the calendar are dropped.
		to_offset_datetime! : ZonedDateTime => { date : TemporalPlain.Date, time : TemporalPlain.Time, offset : TemporalPlain.Offset }
		to_offset_datetime! = |z| {
			offset = TemporalPlain.offset_from_seconds(TemporalHost.zdt_offset_seconds!(z.handle()))
			wall = TemporalPlain.wall_clock_at(TemporalHost.zdt_epoch_ns!(z.handle()), offset)
			{ date: wall.date, time: wall.time, offset }
		}

		# ---- arithmetic ----

		## Date parts move in wall-clock time and time parts in exact time, so a day
		## across a DST boundary is 23 or 25 hours rather than 86400e9 nanoseconds.
		add! : ZonedDateTime, Duration => Try(ZonedDateTime, Err)
		add! = |z, dur| TemporalHost.zdt_add!(z.handle(), dur.rec(), Constrain).map_ok(ZonedDateTime.wrap)

		add_with! : ZonedDateTime, Duration, Overflow => Try(ZonedDateTime, Err)
		add_with! = |z, dur, o| TemporalHost.zdt_add!(z.handle(), dur.rec(), o).map_ok(ZonedDateTime.wrap)

		subtract! : ZonedDateTime, Duration => Try(ZonedDateTime, Err)
		subtract! = |z, dur| TemporalHost.zdt_add!(z.handle(), dur.negate().rec(), Constrain).map_ok(ZonedDateTime.wrap)

		## Answers in hours, TC39's default for a zoned difference.
		until! : ZonedDateTime, ZonedDateTime => Try(Duration, Err)
		until! = |a, b| TemporalHost.zdt_until!(a.handle(), b.handle(), Hour).map_ok(Duration.new)

		until_in! : ZonedDateTime, ZonedDateTime, Unit => Try(Duration, Err)
		until_in! = |a, b, u| TemporalHost.zdt_until!(a.handle(), b.handle(), u).map_ok(Duration.new)

		since! : ZonedDateTime, ZonedDateTime => Try(Duration, Err)
		since! = |a, b| TemporalHost.zdt_until!(b.handle(), a.handle(), Hour).map_ok(Duration.new)

		until_rounded! : ZonedDateTime, ZonedDateTime, DiffOptions => Try(Duration, Err)
		until_rounded! = |a, b, o| TemporalHost.zdt_until_rounded!(a.handle(), b.handle(), o).map_ok(Duration.new)

		since_rounded! : ZonedDateTime, ZonedDateTime, DiffOptions => Try(Duration, Err)
		since_rounded! = |a, b, o| TemporalHost.zdt_until_rounded!(a.handle(), b.handle(), { ..o, mode: negated_mode(o.mode) }).map_ok(|r| Duration.new(r).negate())

		## Rounds to the nearest `Unit`, ties away from zero. To `Day`, TC39 assumes
		## an instant precedes the start of the next date; where a zone breaks that —
		## a transition at 00:01 local, as Creston's in 1944 and Newfoundland's until
		## 2011 — the spec has no answer and this gives temporal_rs's (D-T2-16).
		round! : ZonedDateTime, Unit => Try(ZonedDateTime, Err)
		round! = |z, u| TemporalHost.zdt_round!(z.handle(), { smallest: u, mode: HalfExpand, increment: 1 }).map_ok(ZonedDateTime.wrap)

		round_with! : ZonedDateTime, RoundOptions => Try(ZonedDateTime, Err)
		round_with! = |z, o| TemporalHost.zdt_round!(z.handle(), o).map_ok(ZonedDateTime.wrap)

		# ---- the day, and the zone's rules ----

		## DST-aware midnight: some days begin at 01:00, not 00:00, and some are
		## entered twice.
		start_of_day! : ZonedDateTime => Try(ZonedDateTime, Err)
		start_of_day! = |z| TemporalHost.zdt_start_of_day!(z.handle()).map_ok(ZonedDateTime.wrap)

		## 23, 24 or 25 on a day the zone changes its rules.
		hours_in_day! : ZonedDateTime => Try(F64, Err)
		hours_in_day! = |z| TemporalHost.zdt_hours_in_day!(z.handle())

		## When the zone next changes its rules, if it ever does again — a
		## fixed-offset zone answers `NoTransition` in both directions.
		next_transition! : ZonedDateTime => Try(Transition, Err)
		next_transition! = |z| TemporalHost.zdt_transition!(z.handle(), Next).map_ok(lift_transition)

		previous_transition! : ZonedDateTime => Try(Transition, Err)
		previous_transition! = |z| TemporalHost.zdt_transition!(z.handle(), Previous).map_ok(lift_transition)

		# ---- changing one part ----

		with_time_zone! : ZonedDateTime, TimeZone => Try(ZonedDateTime, Err)
		with_time_zone! = |z, t| TemporalHost.zdt_with_time_zone!(z.handle(), t).map_ok(ZonedDateTime.wrap)

		## Resolving the new wall clock can be ambiguous, so these take the same
		## `Compatible` default as construction.
		## The calendars consolidate as TC39's do: an ISO side yields to the other,
		## and two different non-ISO calendars are `OutOfRange` rather than one
		## being dropped silently.
		with_plain_date! : ZonedDateTime, PlainDate => Try(ZonedDateTime, Err)
		with_plain_date! = |z, d| with_date_resolved!(z, d, Compatible)

		with_plain_date_with! : ZonedDateTime, PlainDate, Disambiguation => Try(ZonedDateTime, Err)
		with_plain_date_with! = |z, d, dis| with_date_resolved!(z, d, dis)

		with_calendar! : ZonedDateTime, Calendar => ZonedDateTime
		with_calendar! = |z, c| ZonedDateTime.wrap(TemporalHost.zdt_with_calendar!(z.handle(), c))

		# ---- comparison ----

		## Temporal's `equals`: the same instant, in the same zone, on the same
		## calendar. Comparing `epoch_ns!` alone answers a different question —
		## whether two values name the same moment, regardless of where.
		equals! : ZonedDateTime, ZonedDateTime => Try(Bool, Err)
		equals! = |a, b| TemporalHost.zdt_equals!(a.handle(), b.handle())

		## Ordering by instant alone, which is the question "which happened first".
		## `equals!` is the stricter one: same instant, zone AND calendar.
		compare! : ZonedDateTime, ZonedDateTime => TemporalPlain.Order
		compare! = |a, b| {
			x = TemporalHost.zdt_epoch_ns!(a.handle())
			y = TemporalHost.zdt_epoch_ns!(b.handle())
			if x == y { Same } else if x < y { Before } else { After }
		}

		# ---- rendering ----

		## IXDTF: `2024-03-10T01:30:00-05:00[America/New_York]`.
		to_str! : ZonedDateTime => Try(Str, Err)
		to_str! = |z| TemporalHost.zdt_to_str!(z.handle())

		## An effect because reading a resource is one; it composes the leaves
		## rather than spending another.
		format! : ZonedDateTime, Str => Try(Str, Err)
		format! = |z, pattern| {
			d = TemporalHost.zdt_plain_date!(z.handle())
			t = TemporalHost.zdt_plain_time!(z.handle())
			zone = TemporalHost.zdt_time_zone!(z.handle())?
			Ok(
				TemporalStrftime.format(
					{
						year: d.year,
						month: d.month,
						day: d.day,
						hour: t.hour,
						minute: t.minute,
						second: t.second,
						millisecond: t.millisecond,
						nanosecond: nanos_in_second(t),
						offset_seconds: TemporalHost.zdt_offset_seconds!(z.handle()),
						zone: zone,
						has_zone: Bool.True,
					},
					pattern,
				),
			)
		}
	}

}

## `0 - v`, saturating at I64's most negative value.
flipped : I64 -> I64
flipped = |v| if v == -9223372036854775807 - 1 { 9223372036854775807 } else { 0 - v }

## `zdt.with_plain_date!`, with TC39's ConsolidateCalendars.
with_date_resolved! : Temporal.ZonedDateTime, Temporal.PlainDate, TemporalHost.Disambiguation => Try(Temporal.ZonedDateTime, Temporal.Err)
with_date_resolved! = |z, d, dis| {
	zc = TemporalHost.zdt_calendar!(z.handle())
	c =
		if d.cal == Iso or d.cal == zc {
			zc
		} else if zc == Iso {
			d.cal
		} else {
			return Err(OutOfRange("the zoned value and the date are on different calendars"))
		}
	moved = TemporalHost.zdt_with_plain_date!(z.handle(), d.rec(), dis)?
	Ok(Temporal.ZonedDateTime.wrap(TemporalHost.zdt_with_calendar!(moved, c)))
}

## The calendar two dates share, or `OutOfRange` where they differ.
same_calendar : Temporal.PlainDate, Temporal.PlainDate -> Try(Temporal.Calendar, Temporal.Err)
same_calendar = |a, b|
	if a.cal == b.cal {
		Ok(a.cal)
	} else {
		Err(OutOfRange("the two dates are on different calendars; move one with { ..date, cal: ... } first"))
	}

## A duration anchored to a date is measured on that date's calendar; an
## unanchored one has no calendar units to measure, so ISO is as good as any.
calendar_of : Temporal.RelativeTo -> Temporal.Calendar
calendar_of = |r|
	match r {
		Unanchored => Iso
		ToDate(d) => d.cal
	}

## FormatCalendarAnnotation with `auto`: nothing for ISO, the identifier
## otherwise. Pure, so `to_str` stays pure; `tests/strings` holds each
## identifier to the host's by parsing it back.
calendar_annotation : Temporal.Calendar -> Str
calendar_annotation = |c| {
	id = match c {
		Iso => ""
		Buddhist => "buddhist"
		Chinese => "chinese"
		Coptic => "coptic"
		Dangi => "dangi"
		Ethioaa => "ethioaa"
		Ethiopic => "ethiopic"
		Gregory => "gregory"
		Hebrew => "hebrew"
		Indian => "indian"
		IslamicCivil => "islamic-civil"
		IslamicTbla => "islamic-tbla"
		IslamicUmalqura => "islamic-umalqura"
		Japanese => "japanese"
		Persian => "persian"
		Roc => "roc"
	}
	if id == "" { "" } else { "[u-ca=${id}]" }
}

## TC39 PadISOYear: four digits for 0000-9999, otherwise a sign and six.
iso_year : I32 -> Str
iso_year = |y| {
	# Padded only when short: a record is not validated, and a year of more
	# than six digits underflowed the padding width and crashed.
	digits = |n, width| {
		text = n.to_str()
		length = Str.count_utf8_bytes(text)
		if length < width { "${Str.repeat("0", width - length)}${text}" } else { text }
	}
	if y >= 0 and y <= 9999 {
		digits(I32.to_u64_wrap(y), 4)
	} else if y < 0 {
		"-${digits(I64.to_u64_wrap(0 - I32.to_i64(y)), 6)}"
	} else {
		"+${digits(I32.to_u64_wrap(y), 6)}"
	}
}

## Whether ISO fields name a day Temporal can represent: a real month and
## day, from -271821-04-19 to +275760-09-13.
date_exists : I32, U8, U8 -> Bool
date_exists = |year, month, day| {
	lengths = [31, if TemporalStrftime.is_leap(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
	in_month = month >= 1 and month <= 12 and day >= 1 and day <= (List.get(lengths, U8.to_u64(month) - 1) ?? 0)
	after_min = year > -271821 or (year == -271821 and (month > 4 or (month == 4 and day >= 19)))
	before_max = year < 275760 or (year == 275760 and (month < 9 or (month == 9 and day <= 13)))
	in_month and after_min and before_max
}

## A rounding mode as it applies to the negated value: toward positive
## infinity becomes toward negative, and the rest are symmetric.
negated_mode : Temporal.RoundingMode -> Temporal.RoundingMode
negated_mode = |m|
	match m {
		Ceil => Floor
		Floor => Ceil
		HalfCeil => HalfFloor
		HalfFloor => HalfCeil
		other => other
	}

lower_relative : Temporal.RelativeTo -> TemporalHost.RelativeTo
lower_relative = |r|
	match r {
		Unanchored => Unanchored
		ToDate(d) => ToDate(d.rec())
	}

lift_transition : TemporalHost.Transition -> Temporal.Transition
lift_transition = |t|
	match t {
		NoTransition => NoTransition
		At(h) => At(Temporal.ZonedDateTime.wrap(h))
	}

date_parts : TemporalHost.PlainDate -> TemporalStrftime.Parts
date_parts = |d| {
	year: d.year,
	month: d.month,
	day: d.day,
	hour: 0,
	minute: 0,
	second: 0,
	millisecond: 0,
	nanosecond: 0,
	offset_seconds: 0,
	zone: "",
	has_zone: Bool.False,
}

## Month and day stay valid so `%a`-style directives cannot divide by a zero
## month; a time has no date, and 1970-01-01 is the conventional stand-in.
## `%N` is the nanoseconds within the second, so the millisecond and
## microsecond fields fold in: formatting passed the sub-microsecond field alone,
## and 09:30:00.123456789 printed `%N` as 000000789 (D-T2-26).
nanos_in_second : TemporalHost.PlainTime -> U32
nanos_in_second = |t| {
	# Clamped: the fields are not validated, and a millisecond of 4295 or more
	# overflowed U32 and crashed every format of the time.
	total = sub_second_nanos(t)
	if total > 4_294_967_295 { 4_294_967_295 } else { U64.to_u32_wrap(total) }
}

## The sub-second fields as nanoseconds, unclamped.
sub_second_nanos : TemporalHost.PlainTime -> U64
sub_second_nanos = |t| U16.to_u64(t.millisecond) * 1_000_000 + U16.to_u64(t.microsecond) * 1_000 + U16.to_u64(t.nanosecond)

time_parts : TemporalHost.PlainTime -> TemporalStrftime.Parts
time_parts = |t| {
	year: 1970,
	month: 1,
	day: 1,
	hour: t.hour,
	minute: t.minute,
	second: t.second,
	millisecond: t.millisecond,
	nanosecond: nanos_in_second(t),
	offset_seconds: 0,
	zone: "",
	has_zone: Bool.False,
}

unit_str : I64, Str -> Str
unit_str = |value, suffix| if value == 0 { "" } else { Str.concat(value.to_str(), suffix) }
