app [main!] { pf: platform "../target/trantor/app/platform/main.roc" }

import pf.OsStr
import pf.Stdout
import pf.Temporal
import pf.Toml
import pf.Csv

## `PlainDate` and `PlainTime` through trantor-encoding's formats, which the
## package never imports (D-S3-22): TOML inside a record, a list, a nested
## record and a dict value, both ways, and CSV records. Each line shows what was
## written and whether it decodes back equal.

Event : { day : Temporal.PlainDate, at : Temporal.PlainTime }

Lists : { days : List(Temporal.PlainDate), times : List(Temporal.PlainTime) }

Nested : { outer : { event : Event } }

Dicts : { days : Dict(Str, Temporal.PlainDate), times : Dict(Str, Temporal.PlainTime) }

TomlErrs : [Parse(Toml.Err), Mismatch({ path : List(Toml.Segment), expected : Str }), MissingRequiredField(Str)]

CsvErrs : [Parse(Csv.Err), Mismatch({ path : List(Csv.Segment), expected : Str }), MissingRequiredField(Str)]

march8 : Temporal.PlainDate
march8 = Temporal.plain_date_from_fields({ year: 2026, month: 3, day: 8 })

quarter_past : Temporal.PlainTime
quarter_past = Temporal.plain_time_from_fields({ hour: 7, minute: 15, second: 5, millisecond: 250, microsecond: 0, nanosecond: 1 })

event : Event
event = { day: march8, at: quarter_past }

## Text on one line, lines separated by ` | `.
shown : Str -> Str
shown = |text| text.replace_each("\n", " | ")

toml_line : Str, Try(Str, Toml.EncodeErr), Bool -> Str
toml_line = |label, written, is_equal|
	match written {
		Ok(text) => "TOML ${label}: ${shown(text)}; decodes back equal: ${Str.inspect(is_equal)}"
		Err(_) => "TOML ${label}: not written"
	}

csv_line : Str, Try(Str, Csv.EncodeErr), Bool -> Str
csv_line = |label, written, is_equal|
	match written {
		Ok(text) => "CSV ${label}: ${shown(text)}; decodes back equal: ${Str.inspect(is_equal)}"
		Err(_) => "CSV ${label}: not written"
	}

toml_mismatch : Str, Try(Lists, TomlErrs) -> Str
toml_mismatch = |label, decoded|
	match decoded {
		Err(Mismatch(problem)) => "TOML ${label}: Mismatch at ${Str.inspect(problem.path)}"
		_ => "TOML ${label}: not refused"
	}

csv_mismatch : Str, Try(List(Event), CsvErrs) -> Str
csv_mismatch = |label, decoded|
	match decoded {
		Err(Mismatch(problem)) => "CSV ${label}: Mismatch at ${Str.inspect(problem.path)}"
		_ => "CSV ${label}: not refused"
	}

date_line : Str, Try(Event, TomlErrs) -> Str
date_line = |label, decoded|
	match decoded {
		Ok(back) => "TOML ${label}: decoded as ${back.day.to_str()} ${back.at.to_str()}, on Iso: ${Str.inspect(back.day.cal == Iso)}"
		Err(_) => "TOML ${label}: not decoded"
	}

unwritable : Str, Try(Str, Toml.EncodeErr) -> Str
unwritable = |label, written|
	match written {
		Err(InvalidDate(problem)) => "TOML ${label}: InvalidDate at ${Str.inspect(problem.path)}"
		_ => "TOML ${label}: written"
	}

toml_record : Str, Event -> Str
toml_record = |label, value| {
	back : Try(Event, TomlErrs)
	back = Toml.decode(Toml.encode(value) ?? "")
	toml_line(label, Toml.encode(value), back == Ok(value))
}

toml_nested : Nested -> Str
toml_nested = |value| {
	back : Try(Nested, TomlErrs)
	back = Toml.decode(Toml.encode(value) ?? "")
	toml_line("nested record", Toml.encode(value), back == Ok(value))
}

toml_lists : Lists -> Str
toml_lists = |value| {
	back : Try(Lists, TomlErrs)
	back = Toml.decode(Toml.encode(value) ?? "")
	toml_line("lists", Toml.encode(value), back == Ok(value))
}

toml_dicts : Dicts -> Str
toml_dicts = |value| {
	back : Try(Dicts, TomlErrs)
	back = Toml.decode(Toml.encode(value) ?? "")
	toml_line("dicts", Toml.encode_with(value, { version: V1_1 }), back == Ok(value))
}

## Hand-written TOML, not produced by `encode`.
toml_read : Str -> Str
toml_read = |text| {
	decoded : Try(Event, TomlErrs)
	decoded = Toml.decode(text)
	date_line("hand-written", decoded)
}

toml_refused : Str -> Str
toml_refused = |text| {
	decoded : Try(Lists, TomlErrs)
	decoded = Toml.decode(text)
	toml_mismatch("a time in a date list", decoded)
}

csv_records : List(Event) -> Str
csv_records = |values| {
	back : Try(List(Event), CsvErrs)
	back = Csv.decode(Csv.encode(values) ?? "")
	csv_line("records", Csv.encode(values), back == Ok(values))
}

csv_refused : Str -> Str
csv_refused = |text| {
	decoded : Try(List(Event), CsvErrs)
	decoded = Csv.decode(text)
	csv_mismatch("a date-time in a date column", decoded)
}

## A `Toml.LocalDate` is not a `PlainDate`; its fields make one.
from_toml_date : Toml.LocalDate -> Str
from_toml_date = |date| "a Toml.LocalDate through plain_date_from_fields: ${Temporal.plain_date_from_fields(date).to_str()}"

main! : List(OsStr) => Try({}, _)
main! = |_args| {
	hebrew = Temporal.plain_date({ year: 2026, month: 3, day: 8, cal: Hebrew })
	year_10000 = Temporal.plain_date_from_fields({ year: 10000, month: 1, day: 1 })
	toml_date : Toml.LocalDate
	toml_date = { year: 1979, month: 5, day: 27 }
	lines = [
		toml_record("record", event),
		toml_nested({ outer: { event } }),
		toml_lists({ days: [march8, Temporal.plain_date_from_fields({ year: 1, month: 1, day: 1 })], times: [quarter_past, Temporal.PlainTime.new({ hour: 23, minute: 59, second: 59, millisecond: 999, microsecond: 999, nanosecond: 999 })] }),
		toml_dicts({ days: Dict.from_list([("b", march8), ("a", march8)]), times: Dict.from_list([("t", quarter_past)]) }),
		toml_read("day = 2026-03-08\nat = 07:15:05.2500000019"),
		toml_refused("days = [2026-03-08, 07:15:00]\ntimes = []"),
		toml_record("a Hebrew date, its calendar dropped", { day: hebrew, at: quarter_past }),
		unwritable("a year-10000 date", Toml.encode({ day: year_10000 })),
		csv_records([event, { day: year_10000, at: quarter_past }]),
		csv_refused("day,at\n2026-03-08T07:15:00,07:15:00"),
		from_toml_date(toml_date),
	]
	Stdout.line!(Str.join_with(lines, "\n"))
}
