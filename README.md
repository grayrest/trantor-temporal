# trantor-temporal

TC39 `Temporal`-shaped calendar and time-zone arithmetic for the
[trantor platform](https://github.com/grayrest/trantor).

This is a correctness-focused library. It supports multiple calendar
systems, 23 and 25 hour days (daylight savings), days with no midnight
(timezone file changes), etc. The hard part of all this is handled
by `temporal_rs` which, among other things, backs the implementation in
Chrome's v8 javascript engine.

## Setup

This package does not provide a program entry point and needs to be added
onto one that does like `trantor-cli` or `trantor-http`.

```sh
git clone https://github.com/grayrest/trantor-temporal
```

```toml
# world.toml
[world]
name = "myapp"

[deps]
trantor-cli      = { path = "../trantor-cli" }
trantor-temporal = { path = "../trantor-temporal" }
```

## Example

```roc
import pf.Temporal

jan31 = Temporal.plain_date({ year: 2024, month: 1, day: 31 })

jan31.add!({ months: 1 })?   # 2024-02-29 — constrained
jan31.to_str()               # "2024-01-31"
jan31.day_of_week()?         # 3 — Wednesday
jan31.year                   # 2024 — fields stay readable
```

### Effectful Arithmetic

`add!`, `until!` and `fields!` are calendar arithmetic and are actually pure. They
carry a `!` because they cross into the host, and the Roc compiler compiler treats
every hosted function as an effect. Such is the cost of re-using a hosted calendar
library instead of re-implementing it in Roc.

### Unzoned Date and Time

`PlainDate`, `PlainTime` and `Duration` are **ISO fields in both directions**.
It may be tagged with a `Calendar`:`Iso`, `Hebrew`, `IslamicUmalqura`, etc. The
calendar decides the arithmetic but not the structure so a date's fields are ISO
whatever it carries. Date comparison is done on the structs so it follows ISO
rules for sorting and equality but dates with different calendars are never equal.

There is deliberately no `PlainDateTime`. There are clear use cases for zone-free dates
(birthdays) and times (3PM recurring meeting) but specific time instants should have
a zone attached even if it's UTC.

#### PlainDate

```roc
PlainDate := { year : I32, month : U8, day : U8, cal : Calendar ?? Iso }

Calendar : [Iso, Buddhist, Chinese, Coptic, Dangi, Ethioaa, Ethiopic, Gregory, Hebrew,
            Indian, IslamicCivil, IslamicTbla, IslamicUmalqura, Japanese, Persian, Roc]

# constructing
Temporal.plain_date : PlainDate -> PlainDate           # names the type for a bare record literal
Temporal.plain_date_from_fields : { year : I32, month : U8, day : U8 } -> PlainDate   # on Iso
Temporal.date_from_str! : Str => Try(PlainDate, Err)   # IXDTF, "2026-03-08[u-ca=hebrew]"
Temporal.date_parse_in : Str, Str, I32 -> Try(PlainDate, ParseErr)   # strftime; the I32 fills a missing year

# comparing: `==` is TC39's equals (the ISO day and the calendar)
compare : PlainDate, PlainDate -> Order                # ISO day alone, ignoring the calendar

# arithmetic
add! : PlainDate, Duration => Try(PlainDate, Err)      # overflow Constrain
add_with! : PlainDate, Duration, Overflow => Try(PlainDate, Err)
subtract! : PlainDate, Duration => Try(PlainDate, Err)
until! : PlainDate, PlainDate => Try(Duration, Err)    # in days
until_in! : PlainDate, PlainDate, Unit => Try(Duration, Err)
since! : PlainDate, PlainDate => Try(Duration, Err)
until_rounded! : PlainDate, PlainDate, DiffOptions => Try(Duration, Err)
since_rounded! : PlainDate, PlainDate, DiffOptions => Try(Duration, Err)
DiffOptions : { largest : Unit, smallest : Unit, mode : RoundingMode, increment : U32 }

# what the calendar derives
fields! : PlainDate => Try(CalendarFields, Err)
day_of_week : PlainDate -> Try(U8, Err)                # Monday 1 … Sunday 7
iso_day_of_year : PlainDate -> Try(U16, Err)
Temporal.is_leap_year : I32 -> Bool                    # ISO

CalendarFields : {
    day_of_week : U8, day_of_year : U16, week_of_year : U8, year_of_week : I32,
    days_in_week : U16, days_in_month : U16, days_in_year : U16, months_in_year : U16,
    in_leap_year : Bool, month_code : Str, era : Str, era_year : I32,
}

# rendering
to_str : PlainDate -> Str                              # "2026-03-08", "2026-06-15[u-ca=hebrew]"
format : PlainDate, Str -> Str                         # strftime
```

`Temporal.calendar_from_id!` and `Temporal.calendar_id!` convert a calendar to
and from its identifier (`"islamic-civil"`). `rec` and `lift` convert to and
from the host's plain record.

#### PlainTime

```roc
PlainTime := { hour : U8 ?? 0, minute : U8 ?? 0, second : U8 ?? 0,
               millisecond : U16 ?? 0, microsecond : U16 ?? 0, nanosecond : U16 ?? 0 }

# constructing
new : { hour : U8, minute : U8, second : U8, millisecond : U16, microsecond : U16, nanosecond : U16 } -> PlainTime
Temporal.time_parse : Str, Str -> Try(PlainTime, ParseErr)   # strftime
Temporal.plain_time_from_fields : { hour : U8, minute : U8, second : U8, millisecond : U16, microsecond : U16, nanosecond : U16 } -> PlainTime

# comparing: `==` compares every field
compare : PlainTime, PlainTime -> Order

# rendering
to_str : PlainTime -> Str                              # "09:30:00", "09:30:00.5"
format : PlainTime, Str -> Str                         # strftime
```

## Zoned time

```roc
ny = Temporal.time_zone_from_id!("America/New_York")?
march7 = Temporal.plain_date({ year: 2026, month: 3, day: 7 })

before = Temporal.zoned!(march7, { hour: 10, minute: 40 }, ny)?
after = before.add!({ days: 1 })?

after.to_str!()?                              # "2026-03-08T10:40:00-04:00[America/New_York]"
after.epoch_ns!() - before.epoch_ns!()        # 82800000000000 — 23 hours
```

The wall clock is preserved and 23 hours elapse, because 2026-03-08 is when the
United States springs forward. Adding `24 * 60 * 60 * 1E9` nanoseconds would
be an hour off.

When a wall-clock time is ambiguous — 02:30 does not exist on a spring-forward
morning, and 01:30 happens twice on a fall-back one — `Temporal.zoned!`
resolves it `Compatible`, TC39's default. `Temporal.zoned_with!` offers the
full set of options: `Compatible`, `Earlier`, `Later`, or `Reject`.

### ZonedDateTime

```roc
ZonedDateTime := TemporalHost.ZonedDateTime            # a refcounted host resource
TimeZone : Str                                          # its IANA identifier, or an offset like "+05:30"
Overflow : [Constrain, Reject]
Transition : [NoTransition, At(ZonedDateTime)]
DiffOptions : { largest : Unit, smallest : Unit, mode : RoundingMode, increment : U32 }
Disambiguation : [Compatible, Earlier, Later, Reject]

# constructing
Temporal.time_zone_from_id! : Str => Try(TimeZone, Err)
Temporal.zoned! : PlainDate, PlainTime, TimeZone => Try(ZonedDateTime, Err)   # Compatible
Temporal.zoned_with! : PlainDate, PlainTime, TimeZone, Disambiguation => Try(ZonedDateTime, Err)
Temporal.zoned_from_epoch_ns! : I128, TimeZone => Try(ZonedDateTime, Err)     # on Iso
Temporal.zoned_from_str! : Str => Try(ZonedDateTime, Err)                     # IXDTF
Temporal.zoned_from_offset! : { date : Date, time : Time, offset : { minutes : I16 } } => Try(ZonedDateTime, Err)   # fixed offset, on Iso

# reading
epoch_ns! : ZonedDateTime => I128
time_zone! : ZonedDateTime => Try(TimeZone, Err)
calendar! : ZonedDateTime => Calendar
plain_date! : ZonedDateTime => PlainDate
plain_time! : ZonedDateTime => PlainTime
offset_seconds! : ZonedDateTime => I64
to_offset_datetime! : ZonedDateTime => { date : Date, time : Time, offset : { minutes : I16 } }   # offset rounded to minutes

# arithmetic
add! : ZonedDateTime, Duration => Try(ZonedDateTime, Err)
add_with! : ZonedDateTime, Duration, Overflow => Try(ZonedDateTime, Err)
subtract! : ZonedDateTime, Duration => Try(ZonedDateTime, Err)
until! : ZonedDateTime, ZonedDateTime => Try(Duration, Err)   # in hours
until_in! : ZonedDateTime, ZonedDateTime, Unit => Try(Duration, Err)
since! : ZonedDateTime, ZonedDateTime => Try(Duration, Err)
until_rounded! : ZonedDateTime, ZonedDateTime, DiffOptions => Try(Duration, Err)
since_rounded! : ZonedDateTime, ZonedDateTime, DiffOptions => Try(Duration, Err)
round! : ZonedDateTime, Unit => Try(ZonedDateTime, Err)       # HalfExpand
round_with! : ZonedDateTime, RoundOptions => Try(ZonedDateTime, Err)

# the day, and the zone's rules
start_of_day! : ZonedDateTime => Try(ZonedDateTime, Err)
hours_in_day! : ZonedDateTime => Try(F64, Err)                # 23, 24 or 25 on a DST day
next_transition! : ZonedDateTime => Try(Transition, Err)
previous_transition! : ZonedDateTime => Try(Transition, Err)

# changing one part
with_time_zone! : ZonedDateTime, TimeZone => Try(ZonedDateTime, Err)
with_plain_date! : ZonedDateTime, PlainDate => Try(ZonedDateTime, Err)   # Compatible
with_plain_date_with! : ZonedDateTime, PlainDate, Disambiguation => Try(ZonedDateTime, Err)
with_calendar! : ZonedDateTime, Calendar => ZonedDateTime

# comparing
equals! : ZonedDateTime, ZonedDateTime => Try(Bool, Err)      # instant, zone and calendar
compare! : ZonedDateTime, ZonedDateTime => Order              # instant alone

# rendering
to_str! : ZonedDateTime => Try(Str, Err)                      # IXDTF
format! : ZonedDateTime, Str => Try(Str, Err)                 # strftime, with %z %:z %Z
```

## Reading and Writing Dates in Files

`PlainDate` and `PlainTime` decode and encode through any format with the date
methods, such as trantor-encoding's TOML and CSV, without this package
depending on it:

```roc
import pf.Toml
import pf.Temporal

Config : { start : Temporal.PlainDate, reminder : Temporal.PlainTime }

config : Try(Config, _)
config = Toml.decode("start = 2026-03-08\nreminder = 09:30:00")
```

- JSON has no date methods, so a `PlainDate` field in a JSON type is a compile
  error. Keep the text in a `Str` field and parse it with `date_from_str!`.
- A date decodes onto `Iso`. Encoding writes its ISO fields and drops the
  calendar, so a Hebrew date reads back as the same day on `Iso`.
- Formats with only an offset date-time (TOML's `1979-05-27T07:32:00-08:00`)
  convert with `Temporal.zoned_from_offset!` and `to_offset_datetime!`. The
  zone is a fixed offset, so the zone name is dropped: keep
  `America/New_York` in a key of its own and move the value into it with
  `with_time_zone!`. The offset is rounded to whole minutes, which only matters
  for local mean time before standard zones, and the wall clock is recomputed
  so the instant does not change.

`Date` and `Time` in these signatures are the plain records
`{ year : I32, month : U8, day : U8 }` and
`{ hour : U8, minute : U8, second : U8, millisecond : U16, microsecond : U16, nanosecond : U16 }`;
a `Toml.LocalDate` passes to `plain_date_from_fields`, though not to
`plain_date`.

## Duration Units

`until!` answers in the unit TC39 defaults to — days for dates, hours for zoned
values. `until_in!` takes the unit:

```roc
jan1 = Temporal.plain_date({ year: 2024, month: 1, day: 1 })
dec25 = Temporal.plain_date({ year: 2024, month: 12, day: 25 })

jan1.until!(dec25)?            # P359D — 359 days
jan1.until_in!(dec25, Year)?   # P11M24D — 11 months, 24 days
```

### Duration

```roc
Duration := { years : I64 ?? 0, months : I64 ?? 0, weeks : I64 ?? 0, days : I64 ?? 0,
              hours : I64 ?? 0, minutes : I64 ?? 0, seconds : I64 ?? 0,
              milliseconds : I64 ?? 0, microseconds : I64 ?? 0, nanoseconds : I64 ?? 0 }

# constructing
new : { years : I64, months : I64, weeks : I64, days : I64, hours : I64, minutes : I64,
        seconds : I64, milliseconds : I64, microseconds : I64, nanoseconds : I64 } -> Duration
Temporal.duration_from_str! : Str => Try(Duration, Err)   # "P1Y2M3D", "PT4H5M"

# reading: `==` compares every field
negate : Duration -> Duration                             # also Temporal.negate
is_zero : Duration -> Bool
sign : Duration -> Sign                                   # [Negative, Zero, Positive]
valid : Duration -> Bool                                  # every field shares a sign
to_str : Duration -> Try(Str, [MixedSigns, TooLarge])

# calendar units need a date to measure from
DiffOptions : { largest : Unit, smallest : Unit, mode : RoundingMode, increment : U32 }
RelativeTo : [Unanchored, ToDate(PlainDate)]
Unit : [Year, Month, Week, Day, Hour, Minute, Second, Millisecond, Microsecond, Nanosecond]
RoundingMode : [Ceil, Floor, Expand, Trunc, HalfCeil, HalfFloor, HalfExpand, HalfTrunc, HalfEven]
RoundOptions : { smallest : Unit, mode : RoundingMode, increment : U32 }
Order : [Before, Same, After]

round! : Duration, DiffOptions, RelativeTo => Try(Duration, Err)
total! : Duration, Unit, RelativeTo => Try(F64, Err)
compare! : Duration, Duration, RelativeTo => Try(Order, Err)

# options with TC39's defaults
Temporal.date_diff : Unit -> DiffOptions     # smallest Day, Trunc, increment 1
Temporal.zoned_diff : Unit -> DiffOptions    # smallest Nanosecond, Trunc, increment 1
Temporal.round_to : Unit -> RoundOptions     # HalfExpand, increment 1
```

## Formatting and Parsing

The default formatting is ISO 8601 with a calendar annotation when on a
non-ISO calendar (e.g. `2026-06-15[u-ca=hebrew]`). For other formatting,
`format` takes a strftime pattern with the formatters shown below.

```roc
Temporal.plain_date({ year: 1970, month: 1, day: 1 }).to_str()   # "1970-01-01"
march8.format("%A %e %B %Y")                                     # "Sunday  8 March 2026"
```

| | |
|---|---|
| `%Y` `%y` | year, 4-digit / 2-digit (parsing: 00–68 → 2000s, 69–99 → 1900s, POSIX) |
| `%m` `%d` `%e` | month, day zero-padded, day space-padded |
| `%b` `%B` | month name, abbreviated / full |
| `%a` `%A` | weekday name, abbreviated / full |
| `%j` | day of year |
| `%H` `%I` `%p` | hour 24, hour 12, AM/PM |
| `%M` `%S` `%L` `%N` | minute, second, millisecond, nanosecond |
| `%z` `%:z` `%Z` | `+0400`, `+04:00`, zone id — zoned values only |
| `%F` `%T` | `%Y-%m-%d`, `%H:%M:%S` |
| `%%` `%n` `%t` | literal `%`, newline, tab |

Parsing uses the same formatters to convert a string to a date. **Parsing is strict.**
Literal characters must match exactly, the whole input must be consumed, and every
field the pattern names must be present. Month and weekday names are case-insensitive.
A `%a` weekday is checked against the date the rest of the pattern built.

```roc
Temporal.date_parse_in("08 MAR 2026", "%d %b %Y", 2026)?         # 2026-03-08
Temporal.time_parse("09:30:15", "%T")?                           # a time needs no date
# …but it does need a time:
Temporal.time_parse("2026-03-08", "%Y-%m-%d")
# Err(BadPattern("no time in the pattern; a parsed time cannot default one"))
# and a pattern naming the day twice is an error, not a choice between them:
Temporal.date_parse_in("2026-03-08 067", "%Y-%m-%d %j", 2026)
# Err(BadPattern("%j and %m name the day twice; use one or the other"))
# and what it reads must be a date:
Temporal.date_parse_in("2026-02-30", "%Y-%m-%d", 2026)
# Err(BadInput("2026-2-30 is not a date in Temporal's range"))
# or a time:
Temporal.time_parse("23:60", "%H:%M")
# Err(BadInput("minute 60 is outside 0-59"))
Temporal.date_parse_in("Monday 2026-03-08", "%A %Y-%m-%d", 2026)
# Err(BadInput("weekday Monday does not match 2026-03-08, which is a Sunday"))
```

The third argument is the year to use when the pattern names none, so `"%d/%m"`
is a whole date. `Now.date_parse!` fills it from the clock instead — `"25/12"`
means one date in December and another in January, so that answer depends on
when you ask, which is why it is an effect and `date_parse_in` is not.

`Temporal.date_from_str!`, `Temporal.zoned_from_str!` and
`Temporal.duration_from_str!` parse IXDTF — the machine formats, including
calendar annotations. An unknown calendar is `OutOfRange` and a zoned string
whose offset disagrees with its zone is an error.

A duration's `to_str` returns `Err(MixedSigns)` where its fields disagree in
sign, and `Err(TooLarge)` where the seconds and sub-second fields cannot be
carried into an I64.

## "Now"

The current moment arrives through a separate interface, `temporal-now`,
with its own host component. This is the only part of the package that reads
off the host so it's isolated to allow independent replacement if needed.

The instant is a Rust `std::time::SystemTime` and the zone is `iana-time-zone`.

```roc
import pf.Now
import pf.Temporal

Now.epoch_ns!({})?                                  # nanoseconds since the epoch
Now.time_zone_id!({})?                              # the machine's own, such as "America/New_York"
Now.zoned!({})?                     # the moment, in the machine's zone
Now.zoned_in!("Asia/Tokyo")?        # the moment, somewhere else
Now.plain_date_in!("UTC")?          # an unzoned date, calcluated fom the UTC timzone
Now.date_parse!("12/25", "%m/%d")?  # the current year fills the gap
```

The baseline's `Clocks` also has an instant, and works just as well as a
source but it does not read the machine's timezone:

```roc
import pf.Clocks

## Its own function because `Temporal.Err` is nominal while `Clocks` reports a
## tag union, and one `?` cannot carry both out.
render! : U128, Str => Try(Str, Temporal.Err)
render! = |ns, zone_id| {
	zone = Temporal.time_zone_from_id!(zone_id)?
	zdt = Temporal.zoned_from_epoch_ns!(U128.to_i128_wrap(ns), zone)?
	zdt.format!("%A %e %B %Y, %H:%M %Z")
}
```

## Errors

`Temporal.Err` is `[OutOfRange(Str), Invalid(Str), Other(Str)]` — out of range
or an unknown identifier; a field missing or of the wrong kind; anything else.
The payload is the message alone, since the kind is already the tag.

## Known Issues

- Chinese and Dangi dates outside their published ranges are not handled since
  the proposal leaves these implementation-defined.
- Zoned arithmetic, rounding and differences past 2041 other than in 2060,
  2100, 2150 and 2199–2200, where the zones only repeat standing rules, and
  before 1800 other than at the range's ends.
- The runtime on a calendar that steps month by month is variable: a
  Chinese difference in months takes 5 ms across 1,000 years, a third of a second
  across 10,000, and across the whole U64 range was still running when stopped
  after ten minutes. Hebrew and Umm al-Qura grow the same way more slowly.
  temporal_rs computes these, and nothing here bounds them.
- rounding to a day assumes an instant comes before the start of the next date. A
  zone whose transition falls at 00:01 local breaks that for the minutes after it —
  Creston's clock read 1944-01-01 for one minute, then went back to 23:01 on
  December 31, and Newfoundland, Moncton and Goose Bay changed at 00:01 until 2011 —
  so the next date has already started. `round!` gives temporal_rs's answer there,
  and the test sweep counts those instants (114 in the sampled years) rather
  than comparing them.
- one place the oracle does not follow the spec's words: `ComputeNudgeWindow`
  starts the rounding window at the origin "if r1 = 0", which read literally
  measures from the wrong date whenever a larger unit remains — New York,
  2024-03-08 02:17 back to 02-06 15:17 is -1 month -1 day -11 hours, and to the
  week it would round to -P1M1W. The oracle starts at the origin only when the
  whole start duration is zero, as temporal_rs and the spec's own reference
  polyfill do; read literally, 8,803 checks in `until_rounded` alone (and some in
  every other rounding sweep) of the swept cases would differ.
