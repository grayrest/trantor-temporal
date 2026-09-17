app [main!] { pf: platform "../target/trantor/app/platform/main.roc" }

import pf.OsStr
import pf.Stdout
import pf.Temporal

## The package's pure Roc logic against arithmetic written here: date and time
## ordering, weekday, day of year and leap years against day numbers; every
## sign pattern of a duration's ten fields; and the calendar rules of zoned
## date moves and date differences over every pair of calendars. Each line
## counts disagreements and shows the first few.

upto : I64, I64 -> List(I64)
upto = |lo, hi| {
	var $out = []
	var $i = lo
	while $i < hi {
		$out = List.append($out, $i)
		$i = $i + 1
	}
	$out
}

report : Str, U64, List(Str) -> Str
report = |label, total, bad| {
	shown = List.take_first(bad, 3) |> Str.join_with("; ")
	"${label}: ${List.len(bad).to_str()} of ${total.to_str()} differ${if List.is_empty(bad) { "" } else { " — ${shown}" }}"
}

floor_div : I64, I64 -> I64
floor_div = |a, b| if a % b != 0 and (a < 0) != (b < 0) { a // b - 1 } else { a // b }

## Days since 1970-01-01 (Hinnant's days_from_civil).
day_number : I64, I64, I64 -> I64
day_number = |y0, m, d| {
	y = if m <= 2 { y0 - 1 } else { y0 }
	era = floor_div(y, 400)
	yoe = y - era * 400
	mp = (m + 9) % 12
	doy = (153 * mp + 2) // 5 + d - 1
	doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
	era * 146097 + doe - 719468
}

leap : I64 -> Bool
leap = |y| (y % 4 == 0 and y % 100 != 0) or y % 400 == 0

days_in : I64, I64 -> I64
days_in = |y, m| if m == 2 { if leap(y) { 29 } else { 28 } } else if m == 4 or m == 6 or m == 9 or m == 11 { 30 } else { 31 }

sign_of : I64 -> I64
sign_of = |n| if n < 0 { -1 } else if n > 0 { 1 } else { 0 }

order : I64 -> Temporal.Order
order = |n| if n < 0 { Before } else if n > 0 { After } else { Same }

dates : {} -> Str
dates = |{}| {
	var $sample = []
	for y in [-271820, -4713, -401, -400, -1, 0, 1, 4, 100, 1582, 1900, 1970, 2000, 2024, 2100, 9999, 10000, 275759] {
		for m in upto(1, 13) {
			for d in [1, 28, days_in(y, m)] {
				$sample = List.append($sample, (y, m, d))
			}
		}
	}
	var $bad = []
	var $total = 0
	for (y, m, d) in $sample {
		date = Temporal.plain_date({ year: I64.to_i32_wrap(y), month: I64.to_u8_wrap(m), day: I64.to_u8_wrap(d), cal: Iso })
		n = day_number(y, m, d)
		# 1970-01-01 was a Thursday: ISO weekday 4.
		want_dow = I64.to_u8_wrap(((n + 3) % 7 + 7) % 7 + 1)
		want_doy = I64.to_u16_wrap(n - day_number(y, 1, 1) + 1)
		$total = $total + 1
		if date.day_of_week() != Ok(want_dow) { $bad = List.append($bad, "${date.to_str()} weekday") }
		if date.iso_day_of_year() != Ok(want_doy) { $bad = List.append($bad, "${date.to_str()} day of year") }
		if Temporal.is_leap_year(I64.to_i32_wrap(y)) != leap(y) { $bad = List.append($bad, "${y.to_str()} leap") }
		for (y2, m2, d2) in $sample {
			other = Temporal.plain_date({ year: I64.to_i32_wrap(y2), month: I64.to_u8_wrap(m2), day: I64.to_u8_wrap(d2), cal: Iso })
			diff = n - day_number(y2, m2, d2)
			$total = $total + 1
			if date.compare(other) != order(diff) or (date == other) != (diff == 0) {
				$bad = List.append($bad, "${date.to_str()} against ${other.to_str()}")
			}
		}
	}
	report("date ordering, weekday, day of year, leap year", $total, $bad)
}

times : {} -> Str
times = |{}| {
	var $sample = []
	for h in [0, 1, 11, 12, 23] {
		for mi in [0, 59] {
			for s in [0, 59] {
				for sub in [0, 1, 999, 1000, 999999, 1000000, 999999999] {
					$sample = List.append($sample, (h, mi, s, sub))
				}
			}
		}
	}
	var $bad = []
	var $total = 0
	for (h, mi, s, sub) in $sample {
		t = Temporal.PlainTime.new({ hour: I64.to_u8_wrap(h), minute: I64.to_u8_wrap(mi), second: I64.to_u8_wrap(s), millisecond: I64.to_u16_wrap(sub // 1000000), microsecond: I64.to_u16_wrap((sub // 1000) % 1000), nanosecond: I64.to_u16_wrap(sub % 1000) })
		ns = ((h * 60 + mi) * 60 + s) * 1000000000 + sub
		for (h2, mi2, s2, sub2) in $sample {
			u = Temporal.PlainTime.new({ hour: I64.to_u8_wrap(h2), minute: I64.to_u8_wrap(mi2), second: I64.to_u8_wrap(s2), millisecond: I64.to_u16_wrap(sub2 // 1000000), microsecond: I64.to_u16_wrap((sub2 // 1000) % 1000), nanosecond: I64.to_u16_wrap(sub2 % 1000) })
			diff = ns - (((h2 * 60 + mi2) * 60 + s2) * 1000000000 + sub2)
			$total = $total + 1
			if t.compare(u) != order(diff) or (t == u) != (diff == 0) { $bad = List.append($bad, "${t.to_str()} against ${u.to_str()}") }
		}
	}
	report("time ordering", $total, $bad)
}

durations : {} -> Str
durations = |{}| {
	var $bad = []
	var $total = 0
	for code in upto(0, 59049) {
		# Field k takes sign ((code / 3^k) mod 3) - 1.
		s = |k| {
			var $c = code
			var $i = 0
			while $i < k {
				$c = $c // 3
				$i = $i + 1
			}
			$c % 3 - 1
		}
		d = Temporal.Duration.new({ years: s(0), months: s(1), weeks: s(2), days: s(3), hours: s(4), minutes: s(5), seconds: s(6), milliseconds: s(7), microseconds: s(8), nanoseconds: s(9) })
		signs = List.map(upto(0, 10), s)
		mixed = List.contains(signs, 1) and List.contains(signs, -1)
		first = List.find_first(signs, |x| x != 0) ?? 0
		$total = $total + 1
		if d.valid() == mixed { $bad = List.append($bad, "${code.to_str()} valid") }
		if d.is_zero() != (first == 0) { $bad = List.append($bad, "${code.to_str()} is_zero") }
		if !mixed {
			want_sign = if first < 0 { Negative } else if first > 0 { Positive } else { Zero }
			if d.sign() != want_sign { $bad = List.append($bad, "${code.to_str()} sign") }
			if d.negate().negate() != d { $bad = List.append($bad, "${code.to_str()} negate twice") }
			if d.negate().sign() != (if first < 0 { Positive } else if first > 0 { Negative } else { Zero }) { $bad = List.append($bad, "${code.to_str()} negated sign") }
		}
		issue = match d.to_str() {
			Err(MixedSigns) => if !mixed { "${code.to_str()} refused a valid duration" } else { "" }
			Err(TooLarge) => "${code.to_str()} too large"
			Ok(_) => if mixed { "${code.to_str()} printed a mixed duration" } else { "" }
		}
		if issue != "" { $bad = List.append($bad, issue) }
	}
	report("duration sign, validity, zero, negation, printing", $total, $bad)
}

calendars! : {} => Try(Str, Temporal.Err)
calendars! = |{}| {
	all : List(Temporal.Calendar)
	all = [Iso, Buddhist, Chinese, Coptic, Dangi, Ethioaa, Ethiopic, Gregory, Hebrew, Indian, IslamicCivil, IslamicTbla, IslamicUmalqura, Japanese, Persian, Roc]
	ny = Temporal.time_zone_from_id!("America/New_York")?
	var $bad = []
	var $total = 0
	for zc in all {
		z = Temporal.zoned!({ year: 2026, month: 5, day: 5, cal: zc }, { hour: 9 }, ny)?
		for dc in all {
			moved = z.with_plain_date_with!(Temporal.plain_date({ year: 2026, month: 6, day: 1, cal: dc }), Compatible)
			want = if dc == Iso { Ok(zc) } else if zc == Iso or zc == dc { Ok(dc) } else { Err(Mismatch) }
			got = match moved { Ok(m) => Ok(m.calendar!()), Err(_) => Err(Mismatch) }
			$total = $total + 1
			if got != want { $bad = List.append($bad, "zoned ${Temporal.calendar_id!(zc)} with date ${Temporal.calendar_id!(dc)}") }
			a = Temporal.plain_date({ year: 2026, month: 5, day: 5, cal: zc })
			b = Temporal.plain_date({ year: 2026, month: 6, day: 1, cal: dc })
			refused = match a.until!(b) { Ok(_) => Bool.False, Err(_) => Bool.True }
			$total = $total + 1
			if refused != (zc != dc) { $bad = List.append($bad, "until ${Temporal.calendar_id!(zc)} to ${Temporal.calendar_id!(dc)}") }
		}
	}
	Ok(report("calendar rules for moving and differencing", $total, $bad))
}

main! : List(OsStr) => Try({}, _)
main! = |_args| {
	Stdout.line!(dates({}))?
	Stdout.line!(times({}))?
	Stdout.line!(durations({}))?
	match calendars!({}) {
		Ok(line) => Stdout.line!(line)
		Err(_) => Stdout.line!("calendar rules: FAILED to build values")
	}
}
