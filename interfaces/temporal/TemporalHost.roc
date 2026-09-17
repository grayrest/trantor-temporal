## roc:temporal — the hosted layer of TC39 Temporal-shaped calendar and
## time-zone arithmetic, backed by temporal_rs (P13). Primitive by design: every
## option is an explicit argument and nothing here has a default. The defaults,
## the formatters and the parsers live in the pure-Roc `Temporal` module over
## this one (D-T1-1), the way `File` sits over `Fs` and `Utc` over `Clocks`.
##
## PlainDate/PlainTime/Duration cross as plain records of ISO fields — on the
## way in AND on the way out (D-T1-2). A `Calendar` therefore selects which
## rules the ARITHMETIC follows and never how a record is spelled.
##
## `ZonedDateTime` is the ONLY resource (P5). A `Calendar` is a tag and a
## `TimeZone` is its identifier, because neither was ever big enough to be
## worth refcounting: temporal_rs holds a Calendar in 8 bytes and a TimeZone in
## 24, both `Copy` (D-T2-4, D-T2-5).
TemporalHost :: [].{
	ZonedDateTime :: Box(U64)

	## Which rules the arithmetic follows. A closed set, because temporal_rs
	## 0.2.6 supports exactly these 16 and a tag costs nothing to pass —
	## `islamic`/`islamicc` are aliases of `IslamicCivil` and
	## `ethiopic-amete-alem` of `Ethioaa`, which `calendar_from_id!` resolves.
	Calendar : [
		Iso, Buddhist, Chinese, Coptic, Dangi, Ethioaa, Ethiopic, Gregory,
		Hebrew, Indian, IslamicCivil, IslamicTbla, IslamicUmalqura, Japanese,
		Persian, Roc,
	]

	## An IANA identifier, `"America/New_York"`. The host memoises the
	## resolution: a temporal_rs `TimeZone` holds two resolved indices into the
	## tzdb, and re-resolving one per call costs 6.3x the operation it enables
	## (D-T2-6). Passing the identifier is the API; caching it is the host's
	## side of that bargain, not an optimisation it may skip.
	TimeZone : Str

	PlainDate : { year : I32, month : U8, day : U8 }
	PlainTime : { hour : U8, minute : U8, second : U8, millisecond : U16, microsecond : U16, nanosecond : U16 }
	Duration : {
		years : I64, months : I64, weeks : I64, days : I64,
		hours : I64, minutes : I64, seconds : I64, milliseconds : I64,
		microseconds : I64, nanoseconds : I64,
	}

	## The largest unit a difference is expressed in. `date_until!` with `Day`
	## answers 2024-01-01 → 2024-12-25 as 359 days; with `Year`, as 11 months
	## and 24 days.
	Unit : [Year, Month, Week, Day, Hour, Minute, Second, Millisecond, Microsecond, Nanosecond]
	## What arithmetic does when a result has no valid date: `Constrain` clamps
	## (2024-01-31 + 1 month = 2024-02-29), `Reject` errors. It governs the
	## ARITHMETIC only — a malformed input record is always rejected (D-T1-5).
	Overflow : [Constrain, Reject]
	## Which instant a wall-clock time means when the zone makes it ambiguous.
	## At a spring-forward gap 02:30 does not exist; at a fall-back overlap
	## 01:30 happens twice. `Compatible` is TC39's default: later for gaps,
	## earlier for overlaps.
	Disambiguation : [Compatible, Earlier, Later, Reject]
	## Rounding, as TC39 names the modes. `HalfExpand` is Temporal's default.
	RoundingMode : [Ceil, Floor, Expand, Trunc, HalfCeil, HalfFloor, HalfExpand, HalfTrunc, HalfEven]
	## Which way to look for the next time-zone rule change.
	Direction : [Next, Previous]
	## A duration in calendar units has no fixed length until it is anchored to
	## a date — "one month" is 28 to 31 days. `Unanchored` is an error for those
	## units rather than a guess.
	RelativeTo : [Unanchored, ToDate(PlainDate)]
	## Options for a difference, as one record rather than four arguments —
	## which is also the shape the web's `until(other, options)` takes.
	DiffOptions : { largest : Unit, smallest : Unit, mode : RoundingMode, increment : U32 }
	RoundOptions : { smallest : Unit, mode : RoundingMode, increment : U32 }
	## Everything a calendar derives from a date. One leaf rather than twelve:
	## these are calendar-aware, so each would otherwise be its own crossing,
	## and twelve crossings for one date is what this package left a baseline to
	## avoid. `week_of_year` and `year_of_week` are 0 when the calendar has no
	## week numbering; `era` is empty when it has no eras.
	CalendarFields : {
		day_of_week : U8, day_of_year : U16,
		week_of_year : U8, year_of_week : I32,
		days_in_week : U16, days_in_month : U16, days_in_year : U16, months_in_year : U16,
		in_leap_year : Bool, month_code : Str, era : Str, era_year : I32,
	}
	## `NoTransition` where the zone has no further rule change in that
	## direction — a fixed-offset zone has none in either.
	Transition : [NoTransition, At(ZonedDateTime)]
	## `OutOfRange` is Temporal's RangeError (out of range, unknown id,
	## ambiguous instant under `Reject`), `Invalid` its TypeError (a required
	## field missing or of the wrong kind), `Other` anything else. The payload
	## is the message alone — the kind is the tag and is not repeated in it.
	Err : [OutOfRange(Str), Invalid(Str), Other(Str)]
	## A parsed date and the calendar its annotation named — `Iso` when it
	## named none.
	ParsedDate : { date : PlainDate, calendar : Calendar }

	## Resolves an identifier and its aliases to a tag. The host owns this
	## table rather than the shim so the alias set tracks temporal_rs.
	calendar_from_id! : Str => Try(Calendar, Err)
	calendar_id! : Calendar => Str
	## Validates an identifier and answers its canonical spelling. There is no
	## `time_zone_id!` any more: a TimeZone IS its identifier, so that leaf was
	## the identity function with a C ABI crossing in the middle.
	time_zone_from_id! : Str => Try(TimeZone, Err)

	date_add! : PlainDate, Duration, Calendar, Overflow => Try(PlainDate, Err)
	date_until! : PlainDate, PlainDate, Calendar, Unit => Try(Duration, Err)
	## IXDTF: `2026-03-08`, optionally annotated `2026-03-08[u-ca=hebrew]`. The
	## fields come back ISO like every other date; the annotation comes back as
	## the calendar, where it used to be parsed and then thrown away.
	date_from_str! : Str => Try(ParsedDate, Err)

	zdt_from_epoch_ns! : I128, TimeZone, Calendar => Try(ZonedDateTime, Err)
	## The wall-clock constructor: which instant `date`+`time` names in `tz`,
	## resolved by `Disambiguation` when the zone makes it ambiguous.
	zdt_from_wall_clock! : PlainDate, PlainTime, TimeZone, Calendar, Disambiguation => Try(ZonedDateTime, Err)
	## IXDTF. A string carrying both an offset and a zone that disagree is an
	## error rather than a silent preference for one of them (D-T1-12).
	zdt_from_str! : Str => Try(ZonedDateTime, Err)
	zdt_epoch_ns! : ZonedDateTime => I128
	zdt_with_time_zone! : ZonedDateTime, TimeZone => Try(ZonedDateTime, Err)
	zdt_calendar! : ZonedDateTime => Calendar
	zdt_plain_date! : ZonedDateTime => PlainDate
	zdt_plain_time! : ZonedDateTime => PlainTime
	zdt_offset_seconds! : ZonedDateTime => I64
	## IXDTF: `2024-03-10T01:30:00-05:00[America/New_York]`.
	zdt_to_str! : ZonedDateTime => Try(Str, Err)
	## The zone this value is in. The shim used to recover this by rendering the
	## value to IXDTF and slicing between the brackets, which spent a string and
	## a parse to read a field the host already had.
	##
	## Fallible for the same reason the old `time_zone_id!` was: naming a zone
	## can fail, and `"UTC"` is a plausible-looking wrong answer. Every zone
	## shape measured answers — named, offset and UTC alike — but that is a
	## property of temporal_rs 0.2.6, not a guarantee of the type.
	zdt_time_zone! : ZonedDateTime => Try(TimeZone, Err)
	## Same instant, same calendar, and the same zone once aliases resolve —
	## `Asia/Calcutta` is `Asia/Kolkata`.
	zdt_equals! : ZonedDateTime, ZonedDateTime => Try(Bool, Err)
	## Date parts move in wall-clock, time parts in exact time — which is why
	## `+P1D` across a spring-forward keeps the wall clock and elapses 23 hours,
	## and why adding 86400e9 nanoseconds is not the same operation.
	zdt_add! : ZonedDateTime, Duration, Overflow => Try(ZonedDateTime, Err)
	zdt_until! : ZonedDateTime, ZonedDateTime, Unit => Try(Duration, Err)
	## DST-aware midnight: some days begin at 01:00, not 00:00.
	zdt_start_of_day! : ZonedDateTime => Try(ZonedDateTime, Err)

	## ISO 8601 duration: `P1Y2M3D`, `PT4H5M`.
	duration_from_str! : Str => Try(Duration, Err)

	## Everything the calendar derives from a date, in one crossing.
	date_fields! : PlainDate, Calendar => Try(CalendarFields, Err)
	## `date_until!` with the rest of TC39's difference options.
	date_until_rounded! : PlainDate, PlainDate, Calendar, DiffOptions => Try(Duration, Err)

	zdt_with_plain_date! : ZonedDateTime, PlainDate, Disambiguation => Try(ZonedDateTime, Err)
	zdt_with_calendar! : ZonedDateTime, Calendar => ZonedDateTime
	zdt_until_rounded! : ZonedDateTime, ZonedDateTime, DiffOptions => Try(Duration, Err)
	zdt_round! : ZonedDateTime, RoundOptions => Try(ZonedDateTime, Err)
	## 23, 24 or 25 on a day the zone changes its rules.
	zdt_hours_in_day! : ZonedDateTime => Try(F64, Err)
	## When this zone next changes its rules, if it ever does again.
	zdt_transition! : ZonedDateTime, Direction => Try(Transition, Err)

	duration_round! : Duration, DiffOptions, RelativeTo, Calendar => Try(Duration, Err)
	## The duration as a single fractional count of `Unit`.
	duration_total! : Duration, Unit, RelativeTo, Calendar => Try(F64, Err)
	## -1, 0, 1 — the shim turns it into a tag.
	duration_compare! : Duration, Duration, RelativeTo, Calendar => Try(I8, Err)
}
