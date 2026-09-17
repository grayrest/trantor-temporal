app [main!] { pf: platform "../target/trantor/app/platform/main.roc" }

import pf.OsStr
import pf.Stdout
import pf.Temporal

## Every option tag the package hands the host, through each host call that
## converts one, against answers written out here from TC39's definitions — a
## swapped arm in the host's tables (a mode, a unit, an overflow or
## disambiguation, a direction, a duration field, the calendar of a relative
## date) changes a line. Each line counts disagreements and shows the first few.

report : Str, U64, List(Str) -> Str
report = |label, total, bad| {
	shown = List.take_first(bad, 3) |> Str.join_with("; ")
	"${label}: ${List.len(bad).to_str()} of ${total.to_str()} differ${if List.is_empty(bad) { "" } else { " — ${shown}" }}"
}

shown : Try(Temporal.Duration, Temporal.Err) -> Str
shown = |r|
	match r {
		Ok(d) => match d.to_str() { Ok(s) => s, Err(_) => "unprintable" }
		Err(_) => "refused"
	}

zoned_shown! : Try(Temporal.ZonedDateTime, Temporal.Err) => Str
zoned_shown! = |r|
	match r {
		Ok(z) => z.to_str!() ?? "unprintable"
		Err(_) => "refused"
	}

## `n` of `designator` as TC39 prints a duration of that one field.
one : I64, Str -> Str
one = |n, designator|
	if n == 0 { if designator == "D" { "PT0S" } else { "PT0S" } } else if n < 0 { "-${one(0 - n, designator)}" } else if designator == "D" { "P${n.to_str()}D" } else { "PT${n.to_str()}${designator}" }

## Each mode rounding ±1:30, ±2:30, 1:20 and 1:40 to a whole unit — a row no
## other mode shares.
modes : List((Temporal.RoundingMode, Str, List(I64)))
modes = [
	(Ceil, "ceil", [2, -1, 3, -2, 2, 2]),
	(Floor, "floor", [1, -2, 2, -3, 1, 1]),
	(Expand, "expand", [2, -2, 3, -3, 2, 2]),
	(Trunc, "trunc", [1, -1, 2, -2, 1, 1]),
	(HalfCeil, "halfCeil", [2, -1, 3, -2, 1, 2]),
	(HalfFloor, "halfFloor", [1, -2, 2, -3, 1, 2]),
	(HalfExpand, "halfExpand", [2, -2, 3, -3, 1, 2]),
	(HalfTrunc, "halfTrunc", [1, -1, 2, -2, 1, 2]),
	(HalfEven, "halfEven", [2, -2, 2, -2, 1, 2]),
]

modes! : {} => Try(Str, Temporal.Err)
modes! = |{}| {
	utc = Temporal.time_zone_from_id!("UTC")?
	origin = Temporal.zoned_from_epoch_ns!(0, utc)?
	jan10 = Temporal.plain_date({ year: 2024, month: 1, day: 10, cal: Iso })
	minutes = [90, -90, 150, -150, 80, 100]
	# The same values as days in increments: 3 and 5 days by twos, 4 and 5 by threes.
	days = [(3, 2), (-3, 2), (5, 2), (-5, 2), (4, 3), (5, 3)]
	var $bad = []
	var $total = 0
	for (mode, name, row) in modes {
		var $i = 0
		for want in row {
			m = List.get(minutes, $i) ?? 0
			(span, increment) = List.get(days, $i) ?? (0, 1)
			d : Temporal.Duration
			d = { minutes: m }
			by_hour = { largest: Hour, smallest: Hour, mode: mode, increment: 1 }
			later = Temporal.zoned_from_epoch_ns!(I64.to_i128(m) * 60000000000, utc)?
			step : Temporal.Duration
			step = { days: span }
			other = jan10.add!(step)?
			by_days = { largest: Day, smallest: Day, mode: mode, increment: I64.to_u32_wrap(increment) }
			for (path, got, expected) in [
				("duration round", shown(d.round!(by_hour, Unanchored)), one(want, "H")),
				("zoned until", shown(origin.until_rounded!(later, by_hour)), one(want, "H")),
				("date until", shown(jan10.until_rounded!(other, by_days)), one(want * increment, "D")),
			] {
				$total = $total + 1
				if got != expected { $bad = List.append($bad, "${path} ${m.to_str()}min ${name}: ${got}, want ${expected}") }
			}
			if m > 0 {
				rounded = later.round_with!({ smallest: Hour, mode: mode, increment: 1 })?
				$total = $total + 1
				if rounded.epoch_ns!() != I64.to_i128(want) * 3600000000000 { $bad = List.append($bad, "zoned round ${m.to_str()}min ${name}") }
			}
			$i = $i + 1
		}
	}
	Ok(report("rounding modes", $total, $bad))
}

units : List((Temporal.Unit, Str))
units = [(Year, "year"), (Month, "month"), (Week, "week"), (Day, "day"), (Hour, "hour"), (Minute, "minute"), (Second, "second"), (Millisecond, "millisecond"), (Microsecond, "microsecond"), (Nanosecond, "nanosecond")]

units! : {} => Try(Str, Temporal.Err)
units! = |{}| {
	utc = Temporal.time_zone_from_id!("UTC")?
	jan1 = Temporal.plain_date({ year: 2024, month: 1, day: 1, cal: Iso })
	day : Temporal.Duration
	day = { days: 1 }
	totals = [1.0 / 366.0, 1.0 / 31.0, 1.0 / 7.0, 1.0, 24.0, 1440.0, 86400.0, 86400000.0, 86400000000.0, 86400000000000.0]
	# 1970-01-01T00:00 to 01:30:01.001001001, and 1970-01-02T13:47:31.123456789.
	start = Temporal.zoned_from_epoch_ns!(0, utc)?
	finish = Temporal.zoned_from_epoch_ns!(5401001001001, utc)?
	spans = ["PT1H30M1.001001001S", "PT1H30M1.001001001S", "PT1H30M1.001001001S", "PT1H30M1.001001001S", "PT1H30M1.001001001S", "PT90M1.001001001S", "PT5401.001001001S", "PT5401.001001001S", "PT5401.001001001S", "PT5401.001001001S"]
	fields = [(0, 0, 0, 0, 1, 30, 1, 1, 1, 1), (0, 0, 0, 0, 1, 30, 1, 1, 1, 1), (0, 0, 0, 0, 1, 30, 1, 1, 1, 1), (0, 0, 0, 0, 1, 30, 1, 1, 1, 1), (0, 0, 0, 0, 1, 30, 1, 1, 1, 1), (0, 0, 0, 0, 0, 90, 1, 1, 1, 1), (0, 0, 0, 0, 0, 0, 5401, 1, 1, 1), (0, 0, 0, 0, 0, 0, 0, 5401001, 1, 1), (0, 0, 0, 0, 0, 0, 0, 0, 5401001001, 1), (0, 0, 0, 0, 0, 0, 0, 0, 0, 5401001001001)]
	dates = ["P1Y2M14D", "P14M14D", "P62W5D", "P439D", "refused", "refused", "refused", "refused", "refused", "refused"]
	instant = Temporal.zoned_from_epoch_ns!(136051123456789, utc)?
	rounds = [-1, -1, -1, 172800000000000, 136800000000000, 136080000000000, 136051000000000, 136051123000000, 136051123457000, 136051123456789]
	jan2023 = Temporal.plain_date({ year: 2023, month: 1, day: 1, cal: Iso })
	mar2024 = Temporal.plain_date({ year: 2024, month: 3, day: 15, cal: Iso })
	var $bad = []
	var $total = 0
	var $i = 0
	for (unit, name) in units {
		want_total = List.get(totals, $i) ?? 0.0
		got_total = day.total!(unit, ToDate(jan1)) ?? -1.0
		$total = $total + 4
		if F64.abs(got_total - want_total) > want_total * 0.000000000001 { $bad = List.append($bad, "total ${name}: ${got_total.to_str()}") }
		(y, mo, w, d, h, mi, s, ms, us, ns) = List.get(fields, $i) ?? (0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
		want_span = Temporal.Duration.new({ years: y, months: mo, weeks: w, days: d, hours: h, minutes: mi, seconds: s, milliseconds: ms, microseconds: us, nanoseconds: ns })
		got_span = start.until_in!(finish, unit)
		if got_span != Ok(want_span) or shown(got_span) != (List.get(spans, $i) ?? "") { $bad = List.append($bad, "zoned until_in ${name}: ${shown(got_span)}") }
		got_date = shown(jan2023.until_in!(mar2024, unit))
		if got_date != (List.get(dates, $i) ?? "") { $bad = List.append($bad, "date until_in ${name}: ${got_date}") }
		want_round = List.get(rounds, $i) ?? 0
		got_round = match instant.round_with!({ smallest: unit, mode: HalfExpand, increment: 1 }) { Ok(r) => r.epoch_ns!(), Err(_) => -1 }
		if got_round != want_round { $bad = List.append($bad, "zoned round ${name}: ${got_round.to_str()}") }
		$i = $i + 1
	}
	long : Temporal.Duration
	long = { hours: 49, minutes: 30 }
	$total = $total + 1
	split = shown(long.round!({ largest: Day, smallest: Hour, mode: HalfExpand, increment: 1 }, Unanchored))
	if split != "P2DT2H" { $bad = List.append($bad, "largest day, smallest hour: ${split}") }
	Ok(report("units", $total, $bad))
}

choices! : {} => Try(Str, Temporal.Err)
choices! = |{}| {
	utc = Temporal.time_zone_from_id!("UTC")?
	ny = Temporal.time_zone_from_id!("America/New_York")?
	jan31 = Temporal.plain_date({ year: 2024, month: 1, day: 31, cal: Iso })
	month : Temporal.Duration
	month = { months: 1 }
	zoned_jan31 = Temporal.zoned!(jan31, { hour: 0 }, utc)?
	gap_day = Temporal.plain_date({ year: 2026, month: 3, day: 8, cal: Iso })
	overlap_day = Temporal.plain_date({ year: 2026, month: 11, day: 1, cal: Iso })
	march1 = Temporal.zoned!(Temporal.plain_date({ year: 2026, month: 3, day: 1, cal: Iso }), { hour: 2, minute: 30 }, ny)?
	june = Temporal.zoned!(Temporal.plain_date({ year: 2026, month: 6, day: 1, cal: Iso }), { hour: 12 }, ny)?
	feb1 = Temporal.plain_date({ year: 2024, month: 2, day: 1, cal: Iso })
	gap = |at| "2026-03-08T${at}[America/New_York]"
	overlap = |at| "2026-11-01T${at}[America/New_York]"
	cases = [
		("date constrain", match jan31.add_with!(month, Constrain) { Ok(d) => d.to_str(), Err(_) => "refused" }, "2024-02-29"),
		("date reject", match jan31.add_with!(month, Reject) { Ok(d) => d.to_str(), Err(_) => "refused" }, "refused"),
		("zoned constrain", zoned_shown!(zoned_jan31.add_with!(month, Constrain)), "2024-02-29T00:00:00+00:00[UTC]"),
		("zoned reject", zoned_shown!(zoned_jan31.add_with!(month, Reject)), "refused"),
		("gap compatible", zoned_shown!(Temporal.zoned_with!(gap_day, { hour: 2, minute: 30 }, ny, Compatible)), gap("03:30:00-04:00")),
		("gap earlier", zoned_shown!(Temporal.zoned_with!(gap_day, { hour: 2, minute: 30 }, ny, Earlier)), gap("01:30:00-05:00")),
		("gap later", zoned_shown!(Temporal.zoned_with!(gap_day, { hour: 2, minute: 30 }, ny, Later)), gap("03:30:00-04:00")),
		("gap reject", zoned_shown!(Temporal.zoned_with!(gap_day, { hour: 2, minute: 30 }, ny, Reject)), "refused"),
		("overlap compatible", zoned_shown!(Temporal.zoned_with!(overlap_day, { hour: 1, minute: 30 }, ny, Compatible)), overlap("01:30:00-04:00")),
		("overlap earlier", zoned_shown!(Temporal.zoned_with!(overlap_day, { hour: 1, minute: 30 }, ny, Earlier)), overlap("01:30:00-04:00")),
		("overlap later", zoned_shown!(Temporal.zoned_with!(overlap_day, { hour: 1, minute: 30 }, ny, Later)), overlap("01:30:00-05:00")),
		("overlap reject", zoned_shown!(Temporal.zoned_with!(overlap_day, { hour: 1, minute: 30 }, ny, Reject)), "refused"),
		("with date earlier", zoned_shown!(march1.with_plain_date_with!(gap_day, Earlier)), gap("01:30:00-05:00")),
		("with date later", zoned_shown!(march1.with_plain_date_with!(gap_day, Later)), gap("03:30:00-04:00")),
		("with date reject", zoned_shown!(march1.with_plain_date_with!(gap_day, Reject)), "refused"),
		("next transition", match june.next_transition!() { Ok(At(t)) => t.to_str!() ?? "unprintable", _ => "none" }, overlap("01:00:00-05:00")),
		("previous transition", match june.previous_transition!() { Ok(At(t)) => t.to_str!() ?? "unprintable", _ => "none" }, gap("03:00:00-04:00")),
		("iso month in days", (month.total!(Day, ToDate(feb1)) ?? -1.0).to_str(), "29"),
		("hebrew month in days", (month.total!(Day, ToDate({ ..feb1, cal: Hebrew })) ?? -1.0).to_str(), "30"),
	]
	bad = List.keep_if(cases, |(_, got, want)| got != want) |> List.map(|(name, got, want)| "${name}: ${got}, want ${want}")
	Ok(report("overflow, disambiguation, direction, relative calendar", List.len(cases), bad))
}

## Each duration field alone rounds to itself in its own unit, so a field
## crossing the host in another's place shows.
fields! : {} => Try(Str, Temporal.Err)
fields! = |{}| {
	jan1 = Temporal.plain_date({ year: 2024, month: 1, day: 1, cal: Iso })
	var $bad = []
	var $k = List.len(units) - List.len(units)
	for (unit, name) in units {
		v = |i| if i == $k { 3 } else { 0 }
		d = Temporal.Duration.new({ years: v(0), months: v(1), weeks: v(2), days: v(3), hours: v(4), minutes: v(5), seconds: v(6), milliseconds: v(7), microseconds: v(8), nanoseconds: v(9) })
		relative = if $k < 3 { ToDate(jan1) } else { Unanchored }
		back = d.round!({ largest: unit, smallest: unit, mode: Trunc, increment: 1 }, relative)
		if back != Ok(d) { $bad = List.append($bad, "${name}: ${shown(back)}") }
		$k = $k + 1
	}
	Ok(report("duration fields across the host", List.len(units), $bad))
}

main! : List(OsStr) => Try({}, _)
main! = |_args| {
	for line in [modes!({}), units!({}), choices!({}), fields!({})] {
		match line {
			Ok(text) => Stdout.line!(text)?
			Err(_) => Stdout.line!("FAILED to build values")?
		}
	}
	Ok({})
}
