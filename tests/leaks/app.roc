app [main!] { pf: platform "../target/trantor/app/platform/main.roc" }

import pf.OsStr
import pf.Stdout
import pf.Temporal
import pf.Now

## `o` for success, `x` for failure: the script checks each call took the path
## it was written to take before trusting the resource counts.
mark : Try(a, e) -> Str
mark = |r| match r {
	Ok(_) => "o"
	Err(_) => "x"
}

## Every host call that takes or returns a zoned value, once where it succeeds
## and once where it fails — the failing path is where a handle leaked before
## (D-T2-14) — plus the Roc methods built on them.
exercise! : {} => Try(Str, Temporal.Err)
exercise! = |{}| {
	ny = Temporal.time_zone_from_id!("America/New_York")?
	z = Temporal.zoned!({ year: 2026, month: 3, day: 7, cal: Iso }, { hour: 12 }, ny)?
	early = Temporal.zoned!({ year: 2026, month: 3, day: 7, cal: Iso }, { hour: 2, minute: 30 }, ny)?
	far = Temporal.zoned_from_epoch_ns!(-8640000000000000000000, ny)
	hebrew = z.with_calendar!(Hebrew)
	built = [
		mark(Temporal.zoned_from_epoch_ns!(0, "Not/AZone")),
		mark(Temporal.zoned_with!({ year: 2026, month: 3, day: 8, cal: Iso }, { hour: 2, minute: 30 }, ny, Reject)),
		mark(Temporal.zoned_from_str!("2026-03-08T12:00:00-04:00[America/New_York]")),
		mark(Temporal.zoned_from_str!("2026-03-08T12:00:00+09:00[America/New_York]")),
		mark(far),
	]
	moved = [
		mark(z.with_time_zone!("Asia/Tokyo")),
		mark(z.with_time_zone!("Not/AZone")),
		mark(z.with_plain_date!(Temporal.plain_date({ year: 2026, month: 3, day: 9, cal: Iso }))),
		mark(early.with_plain_date_with!(Temporal.plain_date({ year: 2026, month: 3, day: 8, cal: Iso }), Reject)),
		mark(Ok(hebrew)),
		mark(z.add!({ days: 1 })),
		mark(z.add!({ years: 300000 })),
		mark(z.subtract!({ hours: 5 })),
	]
	read = [
		mark(Ok(z.epoch_ns!())),
		mark(Ok(z.calendar!())),
		mark(z.time_zone!()),
		mark(Ok(z.plain_date!())),
		mark(Ok(z.plain_time!())),
		mark(Ok(z.offset_seconds!())),
		mark(z.to_str!()),
		mark(z.format!("%F %T %z")),
		mark(z.equals!(hebrew)),
		mark(Ok(z.compare!(hebrew))),
	]
	later = z.add!({ days: 40 })?
	measured = [
		mark(z.until!(later)),
		mark(z.until_in!(later, Month)),
		mark(z.since!(later)),
		mark(z.until_in!(later.with_time_zone!("Asia/Tokyo")?, Day)),
		mark(z.until_rounded!(later, { largest: Month, smallest: Day, mode: HalfExpand, increment: 1 })),
		mark(z.until_rounded!(later, { largest: Month, smallest: Day, mode: HalfExpand, increment: 0 })),
		mark(z.since_rounded!(later, Temporal.zoned_diff(Day))),
		mark(z.until!(hebrew)),
		mark(hebrew.until!(z)),
	]
	day = [
		mark(z.round!(Hour)),
		mark(z.round_with!({ smallest: Day, mode: HalfExpand, increment: 2 })),
		mark(z.start_of_day!()),
		mark(z.hours_in_day!()),
		mark(far?.start_of_day!()),
		mark(far?.hours_in_day!()),
		mark(z.next_transition!()),
		mark(z.previous_transition!()),
		mark(Now.zoned!({})),
	]
	Ok([built, moved, read, measured, day].map(|marks| Str.join_with(marks, "")) |> Str.join_with(" "))
}

main! : List(OsStr) => Try({}, _)
main! = |_args| {
	first = exercise!({})
	second = exercise!({})
	Stdout.line!(
		match (first, second) {
			(Ok(a), Ok(b)) => "${a} | ${b}"
			_ => "FAILED"
		},
	)
}
