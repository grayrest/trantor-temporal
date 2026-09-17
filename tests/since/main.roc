app [main!] { pf: platform "../target/trantor/app/platform/main.roc" }

import pf.OsStr
import pf.Stdout
import pf.Temporal

## `since` as TC39 defines it from `until`, through the package: the rounding
## mode negated by GetNegatedRoundingMode, written out below, and the result
## printed with the opposite sign. Dates over leap and century years and zoned
## values either side of two zones' transitions, in all nine modes. The Rust
## sweep (`since_rounded.rs`) holds that formula to temporal_rs's own `since`
## and the oracles; this holds the package to the formula.

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

leap : I64 -> Bool
leap = |y| (y % 4 == 0 and y % 100 != 0) or y % 400 == 0

days_in : I64, I64 -> I64
days_in = |y, m| if m == 2 { if leap(y) { 29 } else { 28 } } else if m == 4 or m == 6 or m == 9 or m == 11 { 30 } else { 31 }

## GetNegatedRoundingMode, as the spec tabulates it.
spec_negated : Temporal.RoundingMode -> Temporal.RoundingMode
spec_negated = |m|
	match m {
		Ceil => Floor
		Floor => Ceil
		HalfCeil => HalfFloor
		HalfFloor => HalfCeil
		Expand => Expand
		Trunc => Trunc
		HalfExpand => HalfExpand
		HalfTrunc => HalfTrunc
		HalfEven => HalfEven
	}

## A printed duration with its sign flipped; zero has none, nor a refusal.
flipped : Str -> Str
flipped = |s| if s == "PT0S" or Str.starts_with(s, "refused") { s } else if Str.starts_with(s, "-") { Str.drop_prefix(s, "-") } else { "-${s}" }

printed : Try(Temporal.Duration, Temporal.Err) -> Str
printed = |r|
	match r {
		Ok(d) => match d.to_str() { Ok(s) => s, Err(_) => "unprintable" }
		Err(OutOfRange(m)) => "refused: ${m}"
		Err(Invalid(m)) => "refused: ${m}"
		Err(Other(m)) => "refused: ${m}"
	}

modes : List(Temporal.RoundingMode)
modes = [Ceil, Floor, Expand, Trunc, HalfCeil, HalfFloor, HalfExpand, HalfTrunc, HalfEven]

since! : {} => Try(Str, Temporal.Err)
since! = |{}| {
	date_options : List(Temporal.DiffOptions)
	date_options = [{ largest: Year, smallest: Month, mode: Trunc, increment: 1 }, { largest: Month, smallest: Week, mode: Trunc, increment: 1 }, { largest: Year, smallest: Day, mode: Trunc, increment: 5 }]
	var $dates = []
	for y in [1900, 2000, 2023, 2024] {
		for m in upto(1, 13) {
			for d in [1, 15, 28, days_in(y, m)] {
				$dates = List.append($dates, Temporal.plain_date({ year: I64.to_i32_wrap(y), month: I64.to_u8_wrap(m), day: I64.to_u8_wrap(d), cal: Iso }))
			}
		}
	}
	var $bad = []
	var $total = 0
	var $refused = []
	var $i = 0
	for a in $dates {
		var $j = 0
		for b in $dates {
			if ($i * 7 + $j) % 11 == 0 {
				for o in date_options {
					for mode in modes {
						got = printed(a.since_rounded!(b, { ..o, mode: mode }))
						want = flipped(printed(a.until_rounded!(b, { ..o, mode: spec_negated(mode) })))
						$total = $total + 1
						if Str.starts_with(got, "refused") { $refused = List.append($refused, "${a.to_str()} ${b.to_str()} ${got}") }
						if got != want { $bad = List.append($bad, "${a.to_str()} since ${b.to_str()} ${mode_name(mode)}: ${got}, want ${want}") }
					}
				}
				$total = $total + 1
				unrounded = printed(a.since!(b))
				if unrounded != flipped(printed(a.until!(b))) { $bad = List.append($bad, "${a.to_str()} since! ${b.to_str()}: ${unrounded}") }
			}
			$j = $j + 1
		}
		$i = $i + 1
	}
	zoned_options : List(Temporal.DiffOptions)
	zoned_options = [{ largest: Month, smallest: Day, mode: Trunc, increment: 1 }, { largest: Day, smallest: Hour, mode: Trunc, increment: 1 }, { largest: Hour, smallest: Minute, mode: Trunc, increment: 15 }]
	for id in ["America/New_York", "Australia/Lord_Howe"] {
		tz = Temporal.time_zone_from_id!(id)?
		# A difference in days needs one zone, so pairs stay within each.
		var $zoned = []
		for (m, d) in [(3, 7), (3, 8), (4, 5), (10, 4), (10, 5), (11, 1), (11, 2), (12, 31)] {
			for h in [0, 1, 2, 3, 13, 23] {
				$zoned = List.append($zoned, Temporal.zoned!(Temporal.plain_date({ year: 2026, month: m, day: d, cal: Iso }), { hour: h, minute: 17 }, tz)?)
			}
		}
		for a in $zoned {
			for b in $zoned {
				for o in zoned_options {
					for mode in modes {
						got = printed(a.since_rounded!(b, { ..o, mode: mode }))
						want = flipped(printed(a.until_rounded!(b, { ..o, mode: spec_negated(mode) })))
						$total = $total + 1
						if Str.starts_with(got, "refused") { $refused = List.append($refused, got) }
						if got != want { $bad = List.append($bad, "${id} since ${mode_name(mode)}: ${got}, want ${want}") }
					}
				}
				$total = $total + 1
				if printed(a.since!(b)) != flipped(printed(a.until!(b))) { $bad = List.append($bad, "${id} since!: ${printed(a.since!(b))}") }
			}
		}
	}
	Ok("${report("since from until, negated", $total, $bad)}, ${List.len($refused).to_str()} refused")
}

mode_name : Temporal.RoundingMode -> Str
mode_name = |m|
	match m {
		Ceil => "ceil"
		Floor => "floor"
		Expand => "expand"
		Trunc => "trunc"
		HalfCeil => "halfCeil"
		HalfFloor => "halfFloor"
		HalfExpand => "halfExpand"
		HalfTrunc => "halfTrunc"
		HalfEven => "halfEven"
	}

main! : List(OsStr) => Try({}, _)
main! = |_args|
	match since!({}) {
		Ok(line) => Stdout.line!(line)
		Err(_) => Stdout.line!("since from until: FAILED to build values")
	}
