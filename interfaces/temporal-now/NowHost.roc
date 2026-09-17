## roc:temporal-now — the current instant and the machine's own time zone.
##
## The errors carry no message, matching `Clocks.wall_now!`'s shape in the
## baseline — and not only for symmetry: glue lays out `Try(I128, [Tag(Str)])`
## wrongly and its own size assertion refuses to compile. Both failures here
## mean the machine is misconfigured, which a tag says as well as a sentence.
##
## Two leaves and no types of its own: this reads the environment, and
## everything calendrical is `TemporalHost`'s. Wired separately so a world says
## in its own file that it has a clock.
NowHost :: [].{
	## Nanoseconds since the Unix epoch. Fails where the host clock reads
	## before 1970, which is a broken machine rather than a date.
	epoch_ns! : {} => Try(I128, [ClockUnavailable])
	## The IANA identifier the machine is configured for — `Europe/Berlin`.
	## Fails where the host cannot say, rather than guessing UTC.
	system_time_zone_id! : {} => Try(Str, [ZoneUnavailable])
}
