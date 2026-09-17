import NowHost
import TemporalHost
import Temporal

## The current moment — the one part of this package that reads the world
## rather than computing from what it was given.
##
## It is a SEPARATE interface (`temporal-now`) from the calendar arithmetic, so
## a world's wiring says it has a clock. Reading the wall clock and the
## machine's configured zone is ambient authority, and a datetime library
## should not grant it silently.
##
## The baseline's `Clocks` also has an instant; this adds the one thing it
## cannot answer, which is what zone the machine is actually in.
Now :: [].{
	## Nanoseconds since the Unix epoch.
	epoch_ns! : {} => Try(I128, [ClockUnavailable])
	epoch_ns! = |{}| NowHost.epoch_ns!({})

	## The IANA identifier the machine is configured for.
	time_zone_id! : {} => Try(Str, [ZoneUnavailable])
	time_zone_id! = |{}| NowHost.system_time_zone_id!({})

	## The current instant in a named zone. Clock and zone failures arrive as
	## `Other` so that one error type carries the whole calculation — a caller
	## wanting to tell them apart uses `epoch_ns!` and `time_zone_id!` directly.
	## On the ISO calendar; `.with_calendar!(Hebrew)` moves it to another.
	zoned_in! : Str => Try(Temporal.ZonedDateTime, Temporal.Err)
	zoned_in! = |zone_id| {
		ns = clock!({})?
		zone = TemporalHost.time_zone_from_id!(zone_id)?
		Temporal.zoned_from_epoch_ns!(ns, zone)
	}

	## The current instant in the machine's own zone.
	zoned! : {} => Try(Temporal.ZonedDateTime, Temporal.Err)
	zoned! = |{}| {
		id = match NowHost.system_time_zone_id!({}) {
			Ok(v) => v
			Err(ZoneUnavailable) => return Err(Other("the host has no IANA time zone"))
		}
		zoned_in!(id)
	}

	## strftime parsing with the CURRENT year standing in when the pattern names
	## none — `"%d/%m"` is one date in December and a different one in January,
	## so the answer depends on when you ask. Parse errors arrive as `Invalid`
	## so that one error type carries the whole call; `Temporal.date_parse_in`
	## is the pure form, for when the year should not depend on the clock.
	date_parse! : Str, Str => Try(Temporal.PlainDate, Temporal.Err)
	date_parse! = |input, pattern| {
		today = plain_date_in!("UTC")?
		match Temporal.date_parse_in(input, pattern, today.year) {
			Ok(d) => Ok(d)
			Err(BadPattern(m)) => Err(Invalid(m))
			Err(BadInput(m)) => Err(Invalid(m))
		}
	}

	## Today's date in a named zone.
	plain_date_in! : Str => Try(Temporal.PlainDate, Temporal.Err)
	plain_date_in! = |zone_id| Ok(zoned_in!(zone_id)?.plain_date!())

	## The time of day in a named zone.
	plain_time_in! : Str => Try(Temporal.PlainTime, Temporal.Err)
	plain_time_in! = |zone_id| Ok(zoned_in!(zone_id)?.plain_time!())
}

## The clock, with its failure translated into the calendar's error type.
clock! : {} => Try(I128, TemporalHost.Err)
clock! = |{}|
	match NowHost.epoch_ns!({}) {
		Ok(ns) => Ok(ns)
		Err(ClockUnavailable) => Err(Other("the system clock reads before 1970"))
	}
