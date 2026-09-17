app [main!] { pf: platform "../target/trantor/app/platform/main.roc" }

import pf.OsStr
import pf.Stdout
import pf.Temporal

## Round trips through the package's own printers and parsers, with the host's
## IXDTF parsers as the other side where one exists: durations, plain dates and
## times as `to_str` prints them, and every day of eight years through strftime
## patterns; and zoned `format!` in real zones — LMT offsets with seconds,
## negative and half-hour ones, years beyond four digits — against the same
## value's IXDTF string, with `%z` truncating the offset's seconds. Each line
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

days_in : I64, I64 -> I64
days_in = |y, m|
	if m == 2 {
		if (y % 4 == 0 and y % 100 != 0) or y % 400 == 0 { 29 } else { 28 }
	} else if m == 4 or m == 6 or m == 9 or m == 11 {
		30
	} else {
		31
	}

## TC39 TemporalDurationToString with automatic precision, from the fields.
spec_duration : List(I64) -> Str
spec_duration = |f| {
	get = |k| List.get(f, k) ?? 0
	abs = |v| if v < 0 { 0 - v } else { v }
	negative = List.any(f, |v| v < 0)
	part = |k, designator| if get(k) != 0 { "${abs(get(k)).to_str()}${designator}" } else { "" }
	date = "${part(0, "Y")}${part(1, "M")}${part(2, "W")}${part(3, "D")}"
	sub = abs(get(6) * 1000000000 + get(7) * 1000000 + get(8) * 1000 + get(9))
	above_seconds_zero = List.all(List.take_first(f, 6), |v| v == 0)
	seconds =
		if sub != 0 or above_seconds_zero {
			frac_digits = (sub % 1000000000).to_str()
			var $fraction = Str.to_utf8("${Str.repeat("0", 9 - Str.count_utf8_bytes(frac_digits))}${frac_digits}")
			while List.last($fraction) == Ok('0') {
				$fraction = List.drop_last($fraction, 1)
			}
			point = if List.is_empty($fraction) { "" } else { "." }
			"${(sub // 1000000000).to_str()}${point}${Str.from_utf8_lossy($fraction)}S"
		} else {
			""
		}
	time = "${part(4, "H")}${part(5, "M")}${seconds}"
	"${if negative { "-" } else { "" }}P${date}${if time == "" { "" } else { "T${time}" }}"
}

durations! : {} => Str
durations! = |{}| {
	var $bad = []
	var $total = 0
	for mask in upto(1, 1024) {
		for v in [1, 59, 999, 123456789] {
			f = |bit| if (mask // bit) % 2 == 1 { v } else { 0 }
			small = |bit| if (mask // bit) % 2 == 1 { v % 1000 } else { 0 }
			positive : Temporal.Duration
			positive = Temporal.Duration.new({
				years: f(1), months: f(2), weeks: f(4), days: f(8), hours: f(16), minutes: f(32), seconds: f(64),
				milliseconds: small(128), microseconds: small(256), nanoseconds: small(512),
			})
			fields = [f(1), f(2), f(4), f(8), f(16), f(32), f(64), small(128), small(256), small(512)]
			for (d, spec) in [(positive, spec_duration(fields)), (positive.negate(), spec_duration(fields.map(|x| 0 - x)))] {
				match d.to_str() {
					Ok(s) => {
						if s != spec { $bad = List.append($bad, "${s} printed where the spec prints ${spec}") }
						$total = $total + 1
						issue = match Temporal.duration_from_str!(s) {
							Ok(back) => if back != d { "${s} parsed back differently" } else { "" }
							Err(_) => "${s} did not parse"
						}
						if issue != "" { $bad = List.append($bad, issue) }
					}
					Err(_) => {}
				}
			}
		}
	}
	report("duration to_str and back", $total, $bad)
}

dates! : {} => Str
dates! = |{}| {
	var $bad = []
	var $total = 0
	calendars : List(Temporal.Calendar)
	calendars = [Iso, Buddhist, Chinese, Coptic, Dangi, Ethioaa, Ethiopic, Gregory, Hebrew, Indian, IslamicCivil, IslamicTbla, IslamicUmalqura, Japanese, Persian, Roc]
	for cal in calendars {
		# The annotation `to_str` writes, from the host's identifier.
		annotation = if cal == Iso { "" } else { "[u-ca=${Temporal.calendar_id!(cal)}]" }
		for y in [-271821, -100000, -10000, -9999, -1000, -1, 0, 1, 999, 1000, 9999, 10000, 99999, 275760] {
			for m in upto(1, 13) {
				for day in [1, 28, 29, 30, 31] {
					in_range = (y > -271821 or m > 4 or (m == 4 and day >= 19)) and (y < 275760 or m < 9 or (m == 9 and day <= 13))
					if day <= days_in(y, m) and in_range {
						d = Temporal.plain_date({ year: I64.to_i32_wrap(y), month: I64.to_u8_wrap(m), day: I64.to_u8_wrap(day), cal: cal })
						s = d.to_str()
						$total = $total + 1
						annotated = if annotation == "" { !Str.ends_with(s, "]") } else { Str.ends_with(s, annotation) }
						if !annotated {
							$bad = List.append($bad, "${s} does not end with \"${annotation}\"")
						}
						issue = match Temporal.date_from_str!(s) {
							Ok(back) => if back != d { "${s} parsed back as ${back.to_str()}" } else { "" }
							Err(_) => "${s} did not parse"
						}
						if issue != "" { $bad = List.append($bad, issue) }
					}
				}
			}
		}
	}
	report("date to_str and back", $total, $bad)
}

times! : {} => Str
times! = |{}| {
	var $bad = []
	var $total = 0
	for h in [0, 9, 23] {
		for mi in [0, 30, 59] {
			for sub in [0, 5, 500000000, 123456789, 100] {
				t = Temporal.PlainTime.new({ hour: h, minute: mi, second: 59, millisecond: U32.to_u16_wrap(sub // 1000000), microsecond: U32.to_u16_wrap((sub // 1000) % 1000), nanosecond: U32.to_u16_wrap(sub % 1000) })
				s = t.to_str()
				$total = $total + 1
				# Read back through the host's IXDTF parser as a time of day in UTC.
				issue = match Temporal.zoned_from_str!("1970-01-01T${s}+00:00[UTC]") {
					Ok(z) => if z.plain_time!() != t { "${s} parsed back differently" } else { "" }
					Err(_) => "${s} did not parse"
				}
				if issue != "" { $bad = List.append($bad, issue) }
			}
		}
	}
	report("time to_str and back", $total, $bad)
}

patterns : {} -> Str
patterns = |{}| {
	var $bad = []
	var $total = 0
	for y in [1, 99, 100, 1582, 1900, 2000, 2024, 9999] {
		for m in upto(1, 13) {
			for day in upto(1, days_in(y, m) + 1) {
				d = Temporal.plain_date({ year: I64.to_i32_wrap(y), month: I64.to_u8_wrap(m), day: I64.to_u8_wrap(day), cal: Iso })
				for pattern in ["%Y-%m-%d", "%d/%m/%Y", "%A %e %B %Y", "%j %Y", "%a %b %d %Y"] {
					s = d.format(pattern)
					$total = $total + 1
					issue = match Temporal.date_parse_in(s, pattern, 2026) {
						Ok(back) => if back != d { "${pattern} ${s} parsed back as ${back.to_str()}" } else { "" }
						Err(_) => "${pattern} \"${s}\" did not parse"
					}
					if issue != "" { $bad = List.append($bad, issue) }
				}
			}
		}
	}
	for h in upto(0, 24) {
		for mi in [0, 1, 29, 30, 59] {
			for sub in [0, 123000000, 5, 123456789] {
			t = Temporal.PlainTime.new({ hour: I64.to_u8_wrap(h), minute: I64.to_u8_wrap(mi), second: 7, millisecond: U32.to_u16_wrap(sub // 1000000), microsecond: U32.to_u16_wrap((sub // 1000) % 1000), nanosecond: U32.to_u16_wrap(sub % 1000) })
			# %L carries milliseconds only, so it round-trips a whole-millisecond time.
			whole_ms = sub % 1000000 == 0
			fraction_patterns = if whole_ms { ["%H:%M:%S.%N", "%T.%L"] } else { ["%H:%M:%S.%N"] }
			plain_patterns = if sub == 0 { ["%H:%M:%S", "%I:%M:%S %p", "%T"] } else { [] }
			for pattern in List.concat(plain_patterns, fraction_patterns) {
				s = t.format(pattern)
				$total = $total + 1
				issue = match Temporal.time_parse(s, pattern) {
					Ok(back) => if back != t { "${pattern} ${s} parsed back differently" } else { "" }
					Err(_) => "${pattern} \"${s}\" did not parse"
				}
				if issue != "" { $bad = List.append($bad, issue) }
			}
			}
		}
	}
	report("strftime format and parse", $total, $bad)
}

index_of : List(U8), (U8 -> Bool), U64 -> U64
index_of = |b, is_match, from| {
	var $i = from
	while $i < List.len(b) and !(match List.get(b, $i) { Ok(c) => is_match(c), Err(_) => Bool.False }) {
		$i = $i + 1
	}
	$i
}

slice : List(U8), U64, U64 -> Str
slice = |b, start, end| Str.from_utf8_lossy(List.sublist(b, { start: start, len: end - start }))

## strftime's %z and %:z: whole hours and minutes of the offset, truncated.
spec_offset : I64, Bool -> Str
spec_offset = |seconds, colon| {
	total = if seconds < 0 { 0 - seconds } else { seconds }
	two = |n| if n < 10 { "0${n.to_str()}" } else { n.to_str() }
	"${if seconds < 0 { "-" } else { "+" }}${two(total // 3600)}${if colon { ":" } else { "" }}${two((total % 3600) // 60)}"
}

## The offset TC39 prints: rounded to the minute, half away from zero.
rounded_offset : I64 -> Str
rounded_offset = |seconds| spec_offset(60 * (if seconds < 0 { 0 - ((30 - seconds) // 60) } else { (seconds + 30) // 60 }), Bool.True)

## An IXDTF year as %Y writes it: no `+`, at least four digits.
strftime_year : Str -> Str
strftime_year = |printed| {
	b = Str.to_utf8(printed)
	signed = List.get(b, 0) == Ok('+') or List.get(b, 0) == Ok('-')
	var $digits = if signed { List.drop_first(b, 1) } else { b }
	while List.len($digits) > 4 and List.get($digits, 0) == Ok('0') {
		$digits = List.drop_first($digits, 1)
	}
	"${if List.get(b, 0) == Ok('-') { "-" } else { "" }}${Str.from_utf8_lossy($digits)}"
}

zoned_format! : {} => Try(Str, Temporal.Err)
zoned_format! = |{}| {
	var $bad = []
	var $total = 0
	pattern = "%Y-%m-%d %H:%M:%S.%N %z %:z %Z"
	for id in ["UTC", "America/New_York", "Africa/Abidjan", "Europe/London", "Europe/Dublin", "Africa/Monrovia", "Asia/Kolkata", "Asia/Kathmandu", "America/St_Johns", "America/Sitka", "Asia/Manila", "Pacific/Niue", "Pacific/Kiritimati", "America/Montevideo", "Australia/Lord_Howe", "+05:30", "-23:59"] {
		tz = Temporal.time_zone_from_id!(id)?
		for y in [-271820, -10000, -1, 0, 1, 1000, 1800, 1850, 1900, 1937, 1950, 1971, 2026, 9999, 10000, 275759] {
			for (m, h, sub) in [(1, 0, 0), (4, 12, 123456789), (7, 23, 5), (10, 1, 120000000)] {
				t = Temporal.PlainTime.new({ hour: h, minute: 17, second: 5, millisecond: U32.to_u16_wrap(sub // 1000000), microsecond: U32.to_u16_wrap((sub // 1000) % 1000), nanosecond: U32.to_u16_wrap(sub % 1000) })
				z = Temporal.zoned!(Temporal.plain_date({ year: I64.to_i32_wrap(y), month: m, day: 1, cal: Iso }), t, tz)?
				got = z.format!(pattern)?
				# Everything but the offset's seconds, read from TC39's printing.
				s = z.to_str!()?
				b = Str.to_utf8(s)
				at_t = index_of(b, |c| c == 'T', 0)
				after_seconds = at_t + 9
				fraction_end = index_of(b, |c| c < '0' or c > '9', after_seconds + 1)
				digits = if List.get(b, after_seconds) == Ok('.') { slice(b, after_seconds + 1, fraction_end) } else { "" }
				offset_at = if digits == "" { after_seconds } else { fraction_end }
				zone_end = index_of(b, |c| c == ']', offset_at)
				seconds = z.offset_seconds!()
				fraction = "${digits}${Str.repeat("0", 9 - Str.count_utf8_bytes(digits))}"
				want = "${strftime_year(slice(b, 0, at_t - 6))}-${slice(b, at_t - 5, at_t)} ${slice(b, at_t + 1, after_seconds)}.${fraction} ${spec_offset(seconds, Bool.False)} ${spec_offset(seconds, Bool.True)} ${slice(b, offset_at + 7, zone_end)}"
				$total = $total + 1
				if got != want { $bad = List.append($bad, "${s}: ${got}, want ${want}") }
				if slice(b, offset_at, offset_at + 6) != rounded_offset(seconds) { $bad = List.append($bad, "${s}: ${seconds.to_str()}s does not round to its offset") }
			}
		}
	}
	Ok(report("zoned strftime against IXDTF", $total, $bad))
}

main! : List(OsStr) => Try({}, _)
main! = |_args| {
	Stdout.line!(durations!({}))?
	Stdout.line!(dates!({}))?
	Stdout.line!(times!({}))?
	Stdout.line!(patterns({}))?
	match zoned_format!({}) {
		Ok(line) => Stdout.line!(line)
		Err(_) => Stdout.line!("zoned strftime: FAILED to build values")
	}
}
