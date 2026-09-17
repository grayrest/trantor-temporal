app [main!] { pf: platform "../target/trantor/app/platform/main.roc" }

import pf.OsStr
import pf.Stdout
import pf.Temporal

## Offset date-times to and from `ZonedDateTime` (D-S3-25, D-S3-37.9): a
## negative offset under an hour, an IANA zone losing its name, and
## local-mean-time offsets with seconds, rounded to minutes with the instant
## kept. Each line shows the offset record's fields, the zoned value it makes
## and whether the instant survived.

Moment : { date : { year : I32, month : U8, day : U8 }, time : { hour : U8, minute : U8, second : U8, millisecond : U16, microsecond : U16, nanosecond : U16 }, offset : { minutes : I16 } }

tag : Temporal.Err -> Str
tag = |e|
	match e {
		OutOfRange(_) => "OutOfRange"
		Invalid(_) => "Invalid"
		Other(_) => "Other"
	}

fields : Moment -> Str
fields = |m| {
	date = Temporal.plain_date_from_fields(m.date).to_str()
	time = Temporal.plain_time_from_fields(m.time).to_str()
	"${date} ${time} ${m.offset.minutes.to_str()} min"
}

## `moment` to a zoned value, and that value back to an offset date-time.
from_offset! : Str, Moment => Str
from_offset! = |label, moment|
	match Temporal.zoned_from_offset!(moment) {
		Ok(zoned) => {
			back = zoned.to_offset_datetime!()
			"${label}: ${zoned.to_str!() ?? "<unprintable>"}; back: ${fields(back)}, equal: ${Str.inspect(back == moment)}"
		}
		Err(e) => "${label}: ${tag(e)}"
	}

zoned_in! : Str, Temporal.PlainDate, Temporal.PlainTime => Try(Temporal.ZonedDateTime, Temporal.Err)
zoned_in! = |zone_id, date, time| {
	zone = Temporal.time_zone_from_id!(zone_id)?
	Temporal.zoned!(date, time, zone)
}

## A zoned value in `zone_id` to an offset date-time and back to a zoned value.
through_offset! : Str, Str, Temporal.PlainDate, Temporal.PlainTime => Str
through_offset! = |label, zone_id, date, time| {
	match zoned_in!(zone_id, date, time) {
		Err(e) => "${label}: ${tag(e)}"
		Ok(zoned) => {
			moment = zoned.to_offset_datetime!()
			match Temporal.zoned_from_offset!(moment) {
				Err(e) => "${label}: ${tag(e)}"
				Ok(fixed) => {
					kept = fixed.epoch_ns!() == zoned.epoch_ns!()
					"${label}: ${zoned.to_str!() ?? "<unprintable>"} (${zoned.offset_seconds!().to_str()} s) -> ${fields(moment)} -> ${fixed.to_str!() ?? "<unprintable>"}; instant kept: ${Str.inspect(kept)}"
				}
			}
		}
	}
}

main! : List(OsStr) => Try({}, _)
main! = |_args| {
	noon = { hour: 12, minute: 0, second: 0, millisecond: 0, microsecond: 0, nanosecond: 0 }
	lines = [
		from_offset!("under an hour west", { date: { year: 2026, month: 3, day: 8 }, time: noon, offset: { minutes: -30 } }),
		from_offset!("half an hour east, nanoseconds", { date: { year: 1969, month: 12, day: 31 }, time: { hour: 23, minute: 59, second: 59, millisecond: 999, microsecond: 999, nanosecond: 999 }, offset: { minutes: 330 } }),
		from_offset!("UTC", { date: { year: 2026, month: 3, day: 8 }, time: noon, offset: { minutes: 0 } }),
		from_offset!("past a day", { date: { year: 2026, month: 3, day: 8 }, time: noon, offset: { minutes: 1440 } }),
		through_offset!("an IANA zone", "America/New_York", Temporal.plain_date({ year: 2026, month: 3, day: 8 }), Temporal.PlainTime.new({ hour: 10, minute: 40, second: 0, millisecond: 0, microsecond: 0, nanosecond: 0 })),
		through_offset!("New York's LMT", "America/New_York", Temporal.plain_date({ year: 1850, month: 1, day: 1 }), Temporal.PlainTime.new({ hour: 12, minute: 0, second: 0, millisecond: 0, microsecond: 0, nanosecond: 0 })),
		through_offset!("Monrovia's half minute", "Africa/Monrovia", Temporal.plain_date({ year: 1970, month: 1, day: 1 }), Temporal.PlainTime.new({ hour: 0, minute: 0, second: 0, millisecond: 0, microsecond: 0, nanosecond: 0 })),
		through_offset!("a Hebrew date's calendar", "Asia/Jerusalem", Temporal.plain_date({ year: 2026, month: 3, day: 8, cal: Hebrew }), Temporal.PlainTime.new({ hour: 9, minute: 0, second: 0, millisecond: 0, microsecond: 0, nanosecond: 0 })),
	]
	Stdout.line!(Str.join_with(lines, "\n"))
}
