app [main!] { pf: platform "../target/trantor/app/platform/main.roc" }

import pf.OsStr
import pf.Stdout
import pf.Temporal

## No input aborts the program. Records are not validated when they are built,
## so every entry point — the pure printers and patterns as much as the host
## calls — is given dates, times and durations at and past their limits,
## options with any increment, epochs past the range, and seeded random
## strftime patterns and texts. A crash ends the run, so the lines below are
## only printed when every call returned; they count the calls that succeeded.

## A linear congruential step small enough never to overflow a U64.
step : U64 -> U64
step = |s| (s * 1103515245 + 12345) % 2147483648

## `count` pieces drawn from `pieces`, and the state after drawing them.
draw : U64, List(Str), U64 -> (Str, U64)
draw = |seed, pieces, count| {
	var $s = seed
	var $out = ""
	var $i = 0
	while $i < count {
		$s = step($s)
		$out = Str.concat($out, List.get(pieces, $s % List.len(pieces)) ?? "")
		$i = $i + 1
	}
	($out, $s)
}

patterns : {} -> Str
patterns = |{}| {
	directives = ["%Y", "%y", "%m", "%d", "%e", "%b", "%B", "%a", "%A", "%j", "%H", "%I", "%p", "%M", "%S", "%L", "%N", "%z", "%:z", "%Z", "%F", "%T", "%%", "%n", "%t", "%", "%:", "%q", "-", "/", ":", " ", "T", "0", "9", "x"]
	texts = ["0", "1", "2", "9", "12", "31", "60", "99", "2026", "-", "+", "/", ":", " ", "Mar", "March", "Tue", "AM", "PM", "pm", "x", "%", "\t", "\n", "ä", "99999999999999999999", "-0", "00", "366", "000000000"]
	var $s = 20260914
	var $calls = List.len([]) - List.len([])
	var $parsed = List.len([]) - List.len([])
	for _ in List.repeat({}, 40000) {
		$s = step($s)
		(pattern, s1) = draw($s, directives, $s % 7)
		(input, s2) = draw(s1, texts, s1 % 9)
		$s = s2
		$calls = $calls + 2
		if Temporal.date_parse_in(input, pattern, 2026).is_ok() { $parsed = $parsed + 1 }
		if Temporal.time_parse(input, pattern).is_ok() { $parsed = $parsed + 1 }
		# Its own formatting read back, through the same pattern.
		date = Temporal.plain_date({ year: I64.to_i32_wrap(U64.to_i64_wrap(s2 % 20000) - 10000), month: U64.to_u8_wrap(s2 % 14), day: U64.to_u8_wrap(s2 % 33), cal: Iso })
		time = Temporal.PlainTime.new({ hour: U64.to_u8_wrap(s2 % 26), minute: U64.to_u8_wrap(s2 % 61), second: U64.to_u8_wrap(s2 % 62), millisecond: U64.to_u16_wrap(s2 % 1001), microsecond: 0, nanosecond: U64.to_u16_wrap(s2 % 1002) })
		$calls = $calls + 4
		if Temporal.date_parse_in(date.format(pattern), pattern, 2026).is_ok() { $parsed = $parsed + 1 }
		if Temporal.time_parse(time.format(pattern), pattern).is_ok() { $parsed = $parsed + 1 }
		if Str.count_utf8_bytes(date.to_str()) > 0 { $parsed = $parsed + 1 }
		if Str.count_utf8_bytes(time.to_str()) > 0 { $parsed = $parsed + 1 }
	}
	"strftime patterns: ${$calls.to_str()} calls returned, ${$parsed.to_str()} succeeded"
}

records! : {} => Try(Str, Temporal.Err)
records! = |{}| {
	utc = Temporal.time_zone_from_id!("UTC")?
	valid = Temporal.plain_date({ year: 2024, month: 1, day: 31, cal: Iso })
	day : Temporal.Duration
	day = { days: 1 }
	var $calls = List.len([]) - List.len([])
	var $ok = List.len([]) - List.len([])
	for year in [-2147483648, -271822, 0, 275761, 2147483647] {
		for month in [0, 13, 255] {
			for d in [0, 32, 255] {
				for cal in [Iso, Hebrew, Chinese] {
					date = Temporal.plain_date({ year: year, month: month, day: d, cal: cal })
					text = "${date.to_str()}${date.format("%Y %y %m %d %e %b %B %a %A %j %F")}"
					results = [date.day_of_week().is_ok(), date.iso_day_of_year().is_ok(), date.fields!().is_ok(), date.add!(day).is_ok(), date.until!(valid).is_ok(), valid.until!(date).is_ok(), Temporal.zoned!(date, { hour: 0 }, utc).is_ok(), Str.count_utf8_bytes(text) > 0]
					$calls = $calls + List.len(results)
					$ok = $ok + List.len(List.keep_if(results, |r| r))
				}
			}
		}
	}
	for (h, mi, s, ms, us, ns) in [(24, 0, 0, 0, 0, 0), (255, 255, 255, 65535, 65535, 65535), (0, 60, 0, 0, 0, 0), (0, 0, 60, 0, 0, 0), (0, 0, 0, 1000, 0, 0), (0, 0, 0, 0, 1000, 0), (0, 0, 0, 0, 0, 1000)] {
		time = Temporal.PlainTime.new({ hour: h, minute: mi, second: s, millisecond: ms, microsecond: us, nanosecond: ns })
		text = "${time.to_str()}${time.format("%H %I %p %M %S %L %N %T")}"
		results = [Temporal.zoned!(valid, time, utc).is_ok(), time.compare(time) == Same, Str.count_utf8_bytes(text) > 0]
		$calls = $calls + List.len(results)
		$ok = $ok + List.len(List.keep_if(results, |r| r))
	}
	big = 9223372036854775807
	small = -9223372036854775808
	zoned = Temporal.zoned!(valid, { hour: 12 }, utc)?
	for f in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9] {
		for v in [big, small] {
			pick = |k| if k == f { v } else if k == 9 - f and f != 9 - f { 0 - 1 } else { 0 }
			d = Temporal.Duration.new({ years: pick(0), months: pick(1), weeks: pick(2), days: pick(3), hours: pick(4), minutes: pick(5), seconds: pick(6), milliseconds: pick(7), microseconds: pick(8), nanoseconds: pick(9) })
			results = [
				d.to_str().is_ok(), d.valid(), d.is_zero(), d.sign() == Zero, d.negate().valid(), Temporal.negate(d).negate() == d,
				valid.subtract!(d).is_ok(), zoned.subtract!(d).is_ok(),
				d.round!({ largest: Day, smallest: Hour, mode: HalfExpand, increment: 1 }, Unanchored).is_ok(),
				d.total!(Hour, ToDate(valid)).is_ok(), d.compare!(day, ToDate(valid)).is_ok(),
				valid.add!(d).is_ok(), zoned.add!(d).is_ok(),
			]
			$calls = $calls + List.len(results)
			$ok = $ok + List.len(List.keep_if(results, |r| r))
		}
	}
	for increment in [0, 7, 1000000000, 4294967295] {
		for unit in [Year, Day, Hour, Nanosecond] {
			results = [
				day.round!({ largest: unit, smallest: unit, mode: HalfEven, increment: increment }, ToDate(valid)).is_ok(),
				zoned.round_with!({ smallest: unit, mode: Ceil, increment: increment }).is_ok(),
				zoned.until_rounded!(zoned, { largest: unit, smallest: unit, mode: Floor, increment: increment }).is_ok(),
				valid.until_rounded!(valid, { largest: unit, smallest: unit, mode: Expand, increment: increment }).is_ok(),
			]
			$calls = $calls + List.len(results)
			$ok = $ok + List.len(List.keep_if(results, |r| r))
		}
	}
	for epoch in [170141183460469231731687303715884105727, -170141183460469231731687303715884105728, 8640000000000000000001, -8640000000000000000001] {
		$calls = $calls + 1
		if Temporal.zoned_from_epoch_ns!(epoch, utc).is_ok() { $ok = $ok + 1 }
	}
	Ok("records at and past their limits: ${$calls.to_str()} calls returned, ${$ok.to_str()} succeeded")
}

main! : List(OsStr) => Try({}, _)
main! = |_args| {
	Stdout.line!(patterns({}))?
	match records!({}) {
		Ok(line) => Stdout.line!(line)
		Err(_) => Stdout.line!("records: FAILED to build values")
	}
}
