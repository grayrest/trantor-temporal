## strftime formatting and strict parsing over plain field records (D-T1-9,
## D-T1-10). Internal to trantor-temporal: `Temporal` is the public surface.
##
## Pure — no leaf, no effect. Because a PlainDate record is genuinely ISO
## fields (D-T1-2), weekday and day-of-year are computable here rather than
## across the C ABI, which is what lets `date_format` be a function and not an
## effect.
TemporalStrftime :: [].{
	## Everything any directive can need. Date-only and time-only callers zero
	## the rest and leave `has_zone` false; `%z`/`%Z` then render literally,
	## since a plain date has no zone to speak of.
	Parts : {
		year : I32,
		month : U8,
		day : U8,
		hour : U8,
		minute : U8,
		second : U8,
		millisecond : U16,
		nanosecond : U32,
		offset_seconds : I64,
		zone : Str,
		has_zone : Bool,
	}

	## `BadPattern` is the caller's format string; `BadInput` is the text it was
	## pointed at. Keeping them apart matters because one is a bug and the other
	## is data.
	Err : [BadPattern(Str), BadInput(Str)]

	zero : Parts
	zero = {
		year: 1970,
		month: 1,
		day: 1,
		hour: 0,
		minute: 0,
		second: 0,
		millisecond: 0,
		nanosecond: 0,
		offset_seconds: 0,
		zone: "",
		has_zone: Bool.False,
	}

	## An unknown directive renders literally (`%Q` -> `%Q`), as C strftime
	## does — it is visible in the output rather than silently dropped. Parsing
	## is the strict half (D-T1-10); formatting cannot fail.
	format : Parts, Str -> Str
	format = |p, pattern| {
		final = List.fold(
			Str.to_utf8(pattern),
			{ out: [], mode: Plain },
			|st, b|
				match st.mode {
					Plain =>
						if b == '%' {
							{ out: st.out, mode: Pct }
						} else {
							{ out: List.append(st.out, b), mode: Plain }
						}

					Pct =>
						if b == ':' {
							{ out: st.out, mode: PctColon }
						} else {
							{ out: emit(st.out, p, b, Bool.False), mode: Plain }
						}

					PctColon =>
						if b == 'z' {
							{ out: emit(st.out, p, b, Bool.True), mode: Plain }
						} else {
							# `%:` is only ever `%:z`. Anything else is unknown and
							# renders literally, the same as `%Q`. It used to be
							# passed to `emit`, which EXPANDED it if it happened to
							# be a real directive and then appended the raw byte on
							# top: `%:Y` came out as `%:2024Y`, and `%:Q` as `%:%QQ`.
							{ out: List.append(List.concat(st.out, Str.to_utf8("%:")), b), mode: Plain }
						}
				},
		)
		# A pattern ending mid-directive renders what it has, rather than losing
		# it: `"abc%"` was `"abc"`, which is the silent dropping the doc above
		# says this function does not do. `parse` still REJECTS such a pattern —
		# that asymmetry is the design (D-T1-10), not an oversight: formatting
		# cannot fail, parsing is the strict half.
		out =
			match final.mode {
				Plain => final.out
				Pct => List.append(final.out, '%')
				PctColon => List.concat(final.out, Str.to_utf8("%:"))
			}
		Str.from_utf8_lossy(out)
	}

	## Strict: literals must match exactly, the whole input must be consumed,
	## and every field the pattern names must be present. `%b`/`%B`/`%a`/`%A`
	## and `%p` are case-insensitive, because case carries no information in a
	## month or weekday name and fixed-width reports upper-case them.
	##
	## A `%a`/`%A` weekday is VALIDATED against the date the other directives
	## built rather than ignored — a line claiming Monday for a Tuesday is
	## wrong, and saying so is the point of a strict parser.
	## What a pattern actually named, for callers that do not need a whole date.
	## `parse` wants a year, a month and a day because it yields a date; a TIME
	## has none of those, and demanding them is why `time_parse` could not parse
	## a time. `default_year` stands in when the pattern names no year.
	Fields : { parts : Parts, had_year : Bool, had_date : Bool, had_time : Bool }

	parse_fields : Str, Str, I32 -> Try(Fields, Err)
	parse_fields = |input, pattern, default_year| {
		start = { rest: Str.to_utf8(input), got: [], mode: Plain, err: NoErr }
		final = List.fold(Str.to_utf8(pattern), start, step)
		match final.err {
			Bad(Pattern(m)) => Err(BadPattern(m))
			Bad(Input(m)) => Err(BadInput(m))
			NoErr =>
				if final.mode != Plain {
					Err(BadPattern("pattern ends in an incomplete directive"))
				} else if !List.is_empty(final.rest) {
					Err(BadInput("unconsumed input: ${Str.from_utf8_lossy(final.rest)}"))
				} else {
					gather(final.got, default_year)
				}
		}
	}

	## A whole date, or an error naming what the pattern left out. Unchanged
	## behaviour — `parse_fields` is the general form underneath it.
	parse : Str, Str -> Try(Parts, Err)
	parse = |input, pattern| {
		start = { rest: Str.to_utf8(input), got: [], mode: Plain, err: NoErr }
		final = List.fold(Str.to_utf8(pattern), start, step)
		match final.err {
			Bad(Pattern(m)) => Err(BadPattern(m))
			Bad(Input(m)) => Err(BadInput(m))
			NoErr =>
				if final.mode != Plain {
					Err(BadPattern("pattern ends in an incomplete directive"))
				} else if !List.is_empty(final.rest) {
					Err(BadInput("unconsumed input: ${Str.from_utf8_lossy(final.rest)}"))
				} else {
					require_date(gather(final.got, 0), final.got)
				}
		}
	}

	## ISO day of week, Monday = 1 … Sunday = 7, and **0 for a record that names
	## no real date** — these are unvalidated fields, and a total function has to
	## answer something for `{month: 0}` rather than abort.
	##
	## Sakamoto's method, with FLOOR division: Roc's `//` truncates toward zero,
	## which silently degrades the algorithm for year <= 0. Getting that wrong
	## returned 253 for year -1000 — out of the 1..7 this type promises.
	day_of_week : I32, U8, U8 -> U8
	day_of_week = |year, month, day|
		# Temporal's own range is -271821 to 275760. Outside it there is no date
		# to have a weekday, and the intermediate `y + y//4 + y//400` overflows
		# I32 above about 1.7e9 — which ABORTED, in a function documented three
		# lines up as total.
		if year < -271821 or year > 275760 or month < 1 or month > 12 or day < 1 or day > 31 {
			0
		} else {
			t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4]
			y = if month < 3 { year - 1 } else { year }
			shift = List.get(t, U8.to_u64(month) - 1) ?? 0
			# Floor division, inlined: Roc's `//` truncates toward zero, which
			# breaks the algorithm for y < 0. A member of this block cannot see
			# the module's top-level helpers, hence the repetition.
			q4 = if y % 4 != 0 and y < 0 { y // 4 - 1 } else { y // 4 }
			q100 = if y % 100 != 0 and y < 0 { y // 100 - 1 } else { y // 100 }
			q400 = if y % 400 != 0 and y < 0 { y // 400 - 1 } else { y // 400 }
			raw = y + q4 - q100 + q400 + shift + U8.to_i32(day)
			# Sakamoto yields 0 = Sunday; ISO wants Monday = 1 … Sunday = 7.
			iso = ((raw % 7) + 7) % 7
			if iso == 0 { 7 } else { I32.to_u8_wrap(iso) }
		}

	day_of_year : I32, U8, U8 -> U16
	## 0 for a record that names no real month, for the same reason as
	## `day_of_week`.
	day_of_year = |year, month, day| if month < 1 or month > 12 { 0 } else {
		before = List.fold(
			List.sublist([31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31], { start: 0, len: U8.to_u64(month) - 1 }),
			0,
			|acc, n| acc + n,
		)
		leap = if is_leap(year) and month > 2 { 1 } else { 0 }
		before + leap + U8.to_u16(day)
	}

	is_leap : I32 -> Bool
	is_leap = |y| (y % 4 == 0 and y % 100 != 0) or y % 400 == 0
}

# ---- formatting ----

month_names : List(Str)
month_names = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"]

day_names : List(Str)
day_names = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]

name_at : List(Str), U64, Bool -> Str
name_at = |names, index, abbreviated| {
	full = match List.get(names, index) {
		Ok(n) => n
		Err(_) => "?"
	}
	if abbreviated { Str.from_utf8_lossy(List.sublist(Str.to_utf8(full), { start: 0, len: 3 })) } else { full }
}

## Zero-pad to `width`, and leave anything already wider alone.
##
## The comparison happens BEFORE the subtraction. These are U64, so the old
## `needed = width - count` underflowed for any value wider than its pad and
## aborted the program with "Integer subtraction overflowed" — the `needed > 0`
## guard was evaluated after the subtraction and so never protected anything.
## `%Y` of a five-digit year reached it, which `year_str` right below explicitly
## intends to support.
pad : U64, U64 -> Str
pad = |value, width| {
	s = value.to_str()
	len = Str.count_utf8_bytes(s)
	if width > len { Str.concat(Str.repeat("0", width - len), s) } else { s }
}

## 1-based into 0-based for a name table, mapping anything outside the table
## PAST its end so `List.get` misses and the name renders "?" — a `{month: 0}`
## record used to abort here on `U8.to_u64(0) - 1`.
slot : U8, U64 -> U64
slot = |v, len| if v >= 1 and U8.to_u64(v) <= len { U8.to_u64(v) - 1 } else { len }

abs_year : I32 -> U64
## `0 - y` overflows at I32::MIN and `2147483648` is not an I32 literal, so the
## negation is done one short of the edge and the 1 added back in U64.
abs_year = |y| if y < 0 { I32.to_u64_wrap(0 - (y + 1)) + 1 } else { I32.to_u64_wrap(y) }

## Years are signed and may be outside four digits; a negative one keeps its
## sign in front of the padding rather than inside it.
year_str : I32 -> Str
year_str = |y| if y < 0 { Str.concat("-", pad(abs_year(y), 4)) } else { pad(abs_year(y), 4) }

offset_str : I64, Bool -> Str
offset_str = |seconds, colon| {
    sign = if seconds < 0 { "-" } else { "+" }
    total = if seconds < 0 { 0 - seconds } else { seconds }
    hh = pad(I64.to_u64_wrap(total // 3600), 2)
    mm = pad(I64.to_u64_wrap((total % 3600) // 60), 2)
    Str.concat(sign, if colon { "${hh}:${mm}" } else { "${hh}${mm}" })
}

put : List(U8), Str -> List(U8)
put = |out, s| List.concat(out, Str.to_utf8(s))

emit : List(U8), TemporalStrftime.Parts, U8, Bool -> List(U8)
emit = |out, p, directive, colon|
	if directive == 'Y' {
		put(out, year_str(p.year))
	} else if directive == 'y' {
		put(out, pad(I32.to_u64_wrap(((p.year % 100) + 100) % 100), 2))
	} else if directive == 'm' {
		put(out, pad(U8.to_u64(p.month), 2))
	} else if directive == 'd' {
		put(out, pad(U8.to_u64(p.day), 2))
	} else if directive == 'e' {
		put(out, if p.day < 10 { " ${p.day.to_str()}" } else { p.day.to_str() })
	} else if directive == 'b' {
		put(out, name_at(month_names, slot(p.month, 12), Bool.True))
	} else if directive == 'B' {
		put(out, name_at(month_names, slot(p.month, 12), Bool.False))
	} else if directive == 'a' {
		put(out, name_at(day_names, slot(TemporalStrftime.day_of_week(p.year, p.month, p.day), 7), Bool.True))
	} else if directive == 'A' {
		put(out, name_at(day_names, slot(TemporalStrftime.day_of_week(p.year, p.month, p.day), 7), Bool.False))
	} else if directive == 'j' {
		put(out, pad(U16.to_u64(TemporalStrftime.day_of_year(p.year, p.month, p.day)), 3))
	} else if directive == 'H' {
		put(out, pad(U8.to_u64(p.hour), 2))
	} else if directive == 'I' {
		put(out, pad(U8.to_u64(twelve_hour(p.hour)), 2))
	} else if directive == 'p' {
		put(out, if p.hour < 12 { "AM" } else { "PM" })
	} else if directive == 'M' {
		put(out, pad(U8.to_u64(p.minute), 2))
	} else if directive == 'S' {
		put(out, pad(U8.to_u64(p.second), 2))
	} else if directive == 'L' {
		put(out, pad(U16.to_u64(p.millisecond), 3))
	} else if directive == 'N' {
		put(out, pad(U32.to_u64(p.nanosecond), 9))
	} else if directive == 'z' {
		if p.has_zone { put(out, offset_str(p.offset_seconds, colon)) } else { put(out, if colon { "%:z" } else { "%z" }) }
	} else if directive == 'Z' {
		if p.has_zone { put(out, p.zone) } else { put(out, "%Z") }
	} else if directive == 'F' {
		put(out, "${year_str(p.year)}-${pad(U8.to_u64(p.month), 2)}-${pad(U8.to_u64(p.day), 2)}")
	} else if directive == 'T' {
		put(out, "${pad(U8.to_u64(p.hour), 2)}:${pad(U8.to_u64(p.minute), 2)}:${pad(U8.to_u64(p.second), 2)}")
	} else if directive == '%' {
		List.append(out, '%')
	} else if directive == 'n' {
		List.append(out, '\n')
	} else if directive == 't' {
		List.append(out, '\t')
	} else {
		# Unknown: give the directive back verbatim rather than eat it.
		List.append(List.append(out, '%'), directive)
	}

twelve_hour : U8 -> U8
twelve_hour = |h| {
	m = h % 12
	if m == 0 { 12 } else { m }
}

# ---- parsing ----

## Accumulated fields, so the fold's state stays four fields wide instead of
## eighteen — this compiler has no record-update syntax, and every branch would
## otherwise respell the whole record.
Field : [
	FYear(I32),
	FMonth(U8),
	FDay(U8),
	FHour(U8),
	FMinute(U8),
	FSecond(U8),
	FMilli(U16),
	FNano(U32),
	FHalf(U8),
	FWday(U8),
	FYday(U16),
]

lower : U8 -> U8
lower = |b| if b >= 'A' and b <= 'Z' { b + 32 } else { b }

## Fixed-width unsigned run. Strict: exactly `width` ASCII digits, no sign, no
## spaces.
digits : List(U8), U64 -> Try({ value : U64, rest : List(U8) }, Str)
digits = |input, width|
	if List.len(input) < width {
		Err("expected ${width.to_str()} digits, found ${List.len(input).to_str()} characters")
	} else {
		taken = List.sublist(input, { start: 0, len: width })
		if List.all(taken, |b| b >= '0' and b <= '9') {
			Ok({
				value: List.fold(taken, 0, |acc, b| acc * 10 + U8.to_u64(b - '0')),
				rest: List.sublist(input, { start: width, len: List.len(input) - width }),
			})
		} else {
			Err("expected ${width.to_str()} digits, found \"${Str.from_utf8_lossy(taken)}\"")
		}
	}

## `%e`: one optional leading space, then one or two digits.
space_padded_day : List(U8) -> Try({ value : U64, rest : List(U8) }, Str)
space_padded_day = |input| {
	trimmed = match List.first(input) {
		Ok(' ') => List.drop_first(input, 1)
		_ => input
	}
	match digits(trimmed, 2) {
		Ok(r) => Ok(r)
		Err(_) => digits(trimmed, 1)
	}
}

## Longest case-insensitive match from `names`, returning its 1-based index.
match_name : List(U8), List(Str), Bool -> Try({ value : U64, rest : List(U8) }, Str)
match_name = |input, names, abbreviated| {
	hit = List.fold(
		List.map_with_index(names, |n, i| { text: if abbreviated { Str.from_utf8_lossy(List.sublist(Str.to_utf8(n), { start: 0, len: 3 })) } else { n }, index: i }),
		Err("no name matched"),
		|acc, cand|
			match acc {
				Ok(_) => acc
				Err(_) => {
					want = List.map(Str.to_utf8(cand.text), lower)
					n = List.len(want)
					if List.len(input) >= n and List.map(List.sublist(input, { start: 0, len: n }), lower) == want {
						Ok({ value: cand.index + 1, rest: List.sublist(input, { start: n, len: List.len(input) - n }) })
					} else {
						acc
					}
				}
			},
	)
	match hit {
		Ok(r) => Ok(r)
		Err(_) => Err("expected a name, found \"${Str.from_utf8_lossy(List.sublist(input, { start: 0, len: 3 }))}\"")
	}
}

State : {
	rest : List(U8),
	got : List(Field),
	mode : [Plain, Pct, PctColon],
	err : [NoErr, Bad([Pattern(Str), Input(Str)])],
}

fail_input : State, Str -> State
fail_input = |st, m| { rest: st.rest, got: st.got, mode: Plain, err: Bad(Input(m)) }

fail_pattern : State, Str -> State
fail_pattern = |st, m| { rest: st.rest, got: st.got, mode: Plain, err: Bad(Pattern(m)) }

advance : List(U8), List(Field) -> State
advance = |rest, got| { rest: rest, got: got, mode: Plain, err: NoErr }

## `numeric` for a field with a range. A value outside it names no time, so it
## is an input error here rather than a record for something to refuse later
## (D-T2-22).
ranged : State, U64, U64, U64, Str, (U64 -> Field) -> State
ranged = |st, width, lo, hi, what, wrap|
	match digits(st.rest, width) {
		Ok(r) =>
			if r.value < lo or r.value > hi {
				fail_input(st, "${what} ${r.value.to_str()} is outside ${lo.to_str()}-${hi.to_str()}")
			} else {
				advance(r.rest, List.append(st.got, wrap(r.value)))
			}

		Err(m) => fail_input(st, m)
	}

## Consume `width` digits and record the field `wrap` builds from them.
numeric : State, U64, (U64 -> Field) -> State
numeric = |st, width, wrap|
	match digits(st.rest, width) {
		Ok(r) => advance(r.rest, List.append(st.got, wrap(r.value)))
		Err(m) => fail_input(st, m)
	}

step : State, U8 -> State
step = |st, b|
	match st.err {
		Bad(_) => st
		NoErr =>
			match st.mode {
				Plain =>
					if b == '%' {
						{ rest: st.rest, got: st.got, mode: Pct, err: NoErr }
					} else {
						match List.first(st.rest) {
							Ok(head) =>
								if head == b {
									advance(List.drop_first(st.rest, 1), st.got)
								} else {
									fail_input(st, "expected \"${Str.from_utf8_lossy([b])}\", found \"${Str.from_utf8_lossy([head])}\"")
								}

							Err(_) => fail_input(st, "input ended before the pattern did")
						}
					}

				Pct => directive(st, b)
				PctColon => fail_pattern(st, "%:z is a formatting directive; it cannot be parsed")
			}
	}

directive : State, U8 -> State
directive = |st, b|
	if b == 'Y' {
		numeric(st, 4, |v| FYear(U64.to_i32_wrap(v)))
	} else if b == 'y' {
		# POSIX: 00-68 -> 2000s, 69-99 -> 1900s.
		numeric(st, 2, |v| FYear(if v < 69 { U64.to_i32_wrap(v) + 2000 } else { U64.to_i32_wrap(v) + 1900 }))
	} else if b == 'm' {
		numeric(st, 2, |v| FMonth(U64.to_u8_wrap(v)))
	} else if b == 'd' {
		numeric(st, 2, |v| FDay(U64.to_u8_wrap(v)))
	} else if b == 'e' {
		match space_padded_day(st.rest) {
			Ok(r) => advance(r.rest, List.append(st.got, FDay(U64.to_u8_wrap(r.value))))
			Err(m) => fail_input(st, m)
		}
	} else if b == 'H' {
		ranged(st, 2, 0, 23, "hour", |v| FHour(U64.to_u8_wrap(v)))
	} else if b == 'I' {
		ranged(st, 2, 1, 12, "hour", |v| FHour(U64.to_u8_wrap(v)))
	} else if b == 'M' {
		ranged(st, 2, 0, 59, "minute", |v| FMinute(U64.to_u8_wrap(v)))
	} else if b == 'S' {
		ranged(st, 2, 0, 59, "second", |v| FSecond(U64.to_u8_wrap(v)))
	} else if b == 'L' {
		numeric(st, 3, |v| FMilli(U64.to_u16_wrap(v)))
	} else if b == 'N' {
		numeric(st, 9, |v| FNano(U64.to_u32_wrap(v)))
	} else if b == 'j' {
		numeric(st, 3, |v| FYday(U64.to_u16_wrap(v)))
	} else if b == 'b' {
		name_field(st, month_names, Bool.True, |v| FMonth(U64.to_u8_wrap(v)))
	} else if b == 'B' {
		name_field(st, month_names, Bool.False, |v| FMonth(U64.to_u8_wrap(v)))
	} else if b == 'a' {
		name_field(st, day_names, Bool.True, |v| FWday(U64.to_u8_wrap(v)))
	} else if b == 'A' {
		name_field(st, day_names, Bool.False, |v| FWday(U64.to_u8_wrap(v)))
	} else if b == 'p' {
		match match_name(st.rest, ["AM", "PM"], Bool.False) {
			Ok(r) => advance(r.rest, List.append(st.got, FHalf(U64.to_u8_wrap(r.value))))
			Err(m) => fail_input(st, m)
		}
	} else if b == '%' {
		literal(st, '%')
	} else if b == 'n' {
		literal(st, '\n')
	} else if b == 't' {
		literal(st, '\t')
	} else if b == 'F' {
		List.fold(Str.to_utf8("%Y-%m-%d"), { rest: st.rest, got: st.got, mode: Plain, err: NoErr }, step)
	} else if b == 'T' {
		List.fold(Str.to_utf8("%H:%M:%S"), { rest: st.rest, got: st.got, mode: Plain, err: NoErr }, step)
	} else if b == 'z' or b == 'Z' {
		fail_pattern(st, "%${Str.from_utf8_lossy([b])} is a formatting directive; a zone cannot be parsed into plain fields")
	} else {
		fail_pattern(st, "unknown directive %${Str.from_utf8_lossy([b])}")
	}

name_field : State, List(Str), Bool, (U64 -> Field) -> State
name_field = |st, names, abbreviated, wrap|
	match match_name(st.rest, names, abbreviated) {
		Ok(r) => advance(r.rest, List.append(st.got, wrap(r.value)))
		Err(m) => fail_input(st, m)
	}

literal : State, U8 -> State
literal = |st, want|
	match List.first(st.rest) {
		Ok(head) =>
			if head == want {
				advance(List.drop_first(st.rest, 1), st.got)
			} else {
				fail_input(st, "expected \"${Str.from_utf8_lossy([want])}\"")
			}

		Err(_) => fail_input(st, "input ended before the pattern did")
	}

## Fold the accumulated fields into `Parts`, defaulting what the pattern did
## not name and REPORTING what it did. Nothing here fails for a missing year or
## date: a time-only pattern is a legitimate thing to write, and the caller
## decides what it requires. `require_date` is where a date parse insists.
gather : List(Field), I32 -> Try(TemporalStrftime.Fields, TemporalStrftime.Err)
gather = |got, default_year| {
	seen = |pick| List.fold(got, Err(Missing), |acc, f| match pick(f) { Ok(v) => Ok(v), Err(_) => acc })
	year = seen(|f| match f { FYear(v) => Ok(v), _ => Err(Missing) })
	month = seen(|f| match f { FMonth(v) => Ok(v), _ => Err(Missing) })
	day = seen(|f| match f { FDay(v) => Ok(v), _ => Err(Missing) })
	yday = seen(|f| match f { FYday(v) => Ok(v), _ => Err(Missing) })
	hour = seen(|f| match f { FHour(v) => Ok(v), _ => Err(Missing) })
	minute = seen(|f| match f { FMinute(v) => Ok(v), _ => Err(Missing) })
	second = seen(|f| match f { FSecond(v) => Ok(v), _ => Err(Missing) })
	milli = seen(|f| match f { FMilli(v) => Ok(v), _ => Err(Missing) })
	nano = seen(|f| match f { FNano(v) => Ok(v), _ => Err(Missing) })
	half = seen(|f| match f { FHalf(v) => Ok(v), _ => Err(Missing) })
	wday = seen(|f| match f { FWday(v) => Ok(v), _ => Err(Missing) })

	# A pattern that reads a field twice asks two questions that can disagree,
	# as %j with %m does, and the last answer used to win silently: `%H %H` on
	# "09 13" was 13:00. So is %L with %N, which share the fraction, and %p with
	# no hour, which read as midnight (D-T2-29).
	count = |pick| List.fold(got, 0, |n, f| match pick(f) { Ok(_) => n + 1, Err(_) => n })
	twice =
		[
			("the year (%Y, %y)", count(|f| match f { FYear(v) => Ok(v), _ => Err(Missing) })),
			("the month (%m, %b, %B)", count(|f| match f { FMonth(v) => Ok(v), _ => Err(Missing) })),
			("the day (%d, %e)", count(|f| match f { FDay(v) => Ok(v), _ => Err(Missing) })),
			("the day of the year (%j)", count(|f| match f { FYday(v) => Ok(v), _ => Err(Missing) })),
			("the hour (%H, %I)", count(|f| match f { FHour(v) => Ok(v), _ => Err(Missing) })),
			("the minute (%M)", count(|f| match f { FMinute(v) => Ok(v), _ => Err(Missing) })),
			("the second (%S)", count(|f| match f { FSecond(v) => Ok(v), _ => Err(Missing) })),
			("AM or PM (%p)", count(|f| match f { FHalf(v) => Ok(v), _ => Err(Missing) })),
			("the weekday (%a, %A)", count(|f| match f { FWday(v) => Ok(v), _ => Err(Missing) })),
			("the fraction (%L, %N)", count(|f| match f { FMilli(v) => Ok(U16.to_u32(v)), FNano(v) => Ok(v), _ => Err(Missing) })),
		]
		|> List.find_first(|(_, n)| n > 1)
	no_hour_for_half =
		match (half, hour) {
			(Ok(_), Err(_)) => Bool.True
			_ => Bool.False
		}
	if twice.is_ok() or no_hour_for_half {
		Err(BadPattern(
			match twice {
				Ok((what, _)) => "the pattern reads ${what} twice"
				Err(_) => "%p needs an hour (%I) to apply to"
			},
		))
	} else {
		gather_once(default_year, year, month, day, yday, hour, minute, second, milli, nano, half, wday)
	}
}

## The fields, once each, into parts: the rest of `gather`.
gather_once = |default_year, year, month, day, yday, hour, minute, second, milli, nano, half, wday| {
	y = year ?? default_year
	had_year = match year { Ok(_) => Bool.True, Err(_) => Bool.False }
	# A pattern naming BOTH %j and a month or day is asking two questions that
	# can disagree. `%a` is validated against the date for exactly this reason;
	# letting %j through unchecked was the same defect one directive over.
	resolved =
		match (month, day, yday) {
			(Ok(_m), _, Ok(_n)) => Err(BadPattern("%j and %m name the day twice; use one or the other"))
			(_, Ok(_d), Ok(_n)) => Err(BadPattern("%j and %d name the day twice; use one or the other"))
			(Ok(m), Ok(d), _) => Ok({ month: m, day: d, dated: Bool.True })
			(_, _, Ok(n)) => from_day_of_year(y, n).map_ok(|md| { month: md.month, day: md.day, dated: Bool.True })
			_ => Ok({ month: 1, day: 1, dated: Bool.False })
		}
	# With %p an hour is on the 12-hour clock, 1-12: 0 AM names no hour, and
	# reducing modulo 12 read 13 AM as 01:00.
	twelve_hour_ok =
		match (hour, half) {
			(Ok(h), Ok(_)) => h >= 1 and h <= 12
			_ => Bool.True
		}
	match resolved {
		Err(e) => Err(e)
		Ok(_) if !twelve_hour_ok => Err(BadInput("an hour with AM or PM must be 1-12"))
		Ok(md) => {
			h24 =
				match (hour, half) {
					(Ok(h), Ok(p)) => if p == 1 { h % 12 } else { (h % 12) + 12 }
					(Ok(h), Err(_)) => h
					(Err(_), _) => 0
				}
			parts = {
				year: y,
				month: md.month,
				day: md.day,
				hour: h24,
				minute: minute ?? 0,
				second: second ?? 0,
				millisecond: milli ?? 0,
				nanosecond: nano ?? 0,
				offset_seconds: 0,
				zone: "",
				has_zone: Bool.False,
			}
			had_time =
				match (hour, minute, second) {
					(Err(_), Err(_), Err(_)) =>
						match (milli, nano, half) {
							(Err(_), Err(_), Err(_)) => Bool.False
							_ => Bool.True
						}
					_ => Bool.True
				}
			fields = { parts: parts, had_year: had_year, had_date: md.dated, had_time: had_time }
			match wday {
				Err(_) => Ok(fields)
				Ok(w) =>
					if !md.dated {
						Ok(fields)
					} else {
						actual = TemporalStrftime.day_of_week(y, md.month, md.day)
						if w == actual {
							Ok(fields)
						} else {
							Err(BadInput("weekday ${name_at(day_names, slot(w, 7), Bool.False)} does not match ${year_str(y)}-${pad(U8.to_u64(md.month), 2)}-${pad(U8.to_u64(md.day), 2)}, which is a ${name_at(day_names, slot(actual, 7), Bool.False)}"))
						}
					}
			}
		}
	}
}

## A date parse insists on what `gather` is content to default. The messages are
## the ones `parse` has always given.
require_date : Try(TemporalStrftime.Fields, TemporalStrftime.Err), List(Field) -> Try(TemporalStrftime.Parts, TemporalStrftime.Err)
require_date = |res, _got|
	match res {
		Err(e) => Err(e)
		Ok(f) =>
			if !f.had_year {
				Err(BadPattern("no year in the pattern; a parsed date cannot default one"))
			} else if !f.had_date {
				Err(BadPattern("no month and day in the pattern, and no %j either"))
			} else {
				Ok(f.parts)
			}
	}

from_day_of_year : I32, U16 -> Try({ month : U8, day : U8 }, TemporalStrftime.Err)
from_day_of_year = |year, n| {
	lengths = [31, if TemporalStrftime.is_leap(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
	found = List.fold(
		lengths,
		{ month: 0, left: U16.to_i32(n), done: Bool.False, day: 0 },
		|acc, len|
			if acc.done {
				acc
			} else if acc.left <= len {
				{ month: acc.month + 1, left: acc.left, done: Bool.True, day: I32.to_u8_wrap(acc.left) }
			} else {
				{ month: acc.month + 1, left: acc.left - len, done: Bool.False, day: 0 }
			},
	)
	if found.done and n > 0 {
		Ok({ month: I32.to_u8_wrap(found.month), day: found.day })
	} else {
		Err(BadInput("day of year ${n.to_str()} is not in ${year_str(year)}"))
	}
}

# ---- tests -----------------------------------------------------------------
#
# All of this module is pure, so these are plain unit tests with no fixture
# beyond a `Parts` value. Every expected answer below was computed with
# Python's `datetime` rather than read off this implementation: a calendar
# test whose answers come from the code under test proves only that the code
# agrees with itself.

## This compiler has no record-update syntax (see `Field` above), so a helper
## per shape beats respelling eleven fields in every expect.
plain_dt : I32, U8, U8, U8, U8, U8 -> TemporalStrftime.Parts
plain_dt = |y, mo, d, h, mi, s| {
	year: y,
	month: mo,
	day: d,
	hour: h,
	minute: mi,
	second: s,
	millisecond: 0,
	nanosecond: 0,
	offset_seconds: 0,
	zone: "",
	has_zone: Bool.False,
}

plain_date : I32, U8, U8 -> TemporalStrftime.Parts
plain_date = |y, mo, d| plain_dt(y, mo, d, 0, 0, 0)

## 2024-03-05 14:07:09 with sub-second fields, for %L and %N.
fractional : U16, U32 -> TemporalStrftime.Parts
fractional = |ms, ns| {
	year: 2024,
	month: 3,
	day: 5,
	hour: 14,
	minute: 7,
	second: 9,
	millisecond: ms,
	nanosecond: ns,
	offset_seconds: 0,
	zone: "",
	has_zone: Bool.False,
}

## The same instant, but carrying a zone, for %z/%:z/%Z.
zoned : I64, Str -> TemporalStrftime.Parts
zoned = |offset, name| {
	year: 2024,
	month: 3,
	day: 5,
	hour: 14,
	minute: 7,
	second: 9,
	millisecond: 0,
	nanosecond: 0,
	offset_seconds: offset,
	zone: name,
	has_zone: Bool.True,
}

# -- is_leap: the century rule is the whole point of the function -------------

expect TemporalStrftime.is_leap(2024) == Bool.True
expect TemporalStrftime.is_leap(2023) == Bool.False
expect TemporalStrftime.is_leap(2016) == Bool.True
expect TemporalStrftime.is_leap(1999) == Bool.False
# Divisible by 100 but not 400: NOT a leap year, the case a naive `% 4` misses.
expect TemporalStrftime.is_leap(1900) == Bool.False
expect TemporalStrftime.is_leap(2100) == Bool.False
# Divisible by 400: a leap year after all.
expect TemporalStrftime.is_leap(2000) == Bool.True

# -- day_of_week: ISO, Monday = 1 .. Sunday = 7 ------------------------------

expect TemporalStrftime.day_of_week(2024, 3, 5) == 2    # Tuesday
expect TemporalStrftime.day_of_week(1970, 1, 1) == 4    # Thursday
expect TemporalStrftime.day_of_week(2000, 1, 1) == 6    # Saturday
expect TemporalStrftime.day_of_week(1900, 1, 1) == 1    # Monday
expect TemporalStrftime.day_of_week(1969, 7, 20) == 7   # Sunday — and 7, not 0
expect TemporalStrftime.day_of_week(2024, 1, 1) == 1    # Monday
expect TemporalStrftime.day_of_week(2024, 12, 31) == 2  # Tuesday
expect TemporalStrftime.day_of_week(1999, 12, 31) == 5  # Friday
expect TemporalStrftime.day_of_week(2023, 12, 31) == 7  # Sunday

# January and February take Sakamoto's year-1 shift, so they are the months
# where the leap rules actually bite.
expect TemporalStrftime.day_of_week(2024, 2, 29) == 4   # Thursday
expect TemporalStrftime.day_of_week(2000, 2, 29) == 2   # Tuesday
expect TemporalStrftime.day_of_week(2016, 2, 29) == 1   # Monday
expect TemporalStrftime.day_of_week(2000, 3, 1) == 3    # Wednesday
expect TemporalStrftime.day_of_week(2100, 3, 1) == 1    # Monday — 2100 is not a leap year

# -- day_of_year -------------------------------------------------------------

expect TemporalStrftime.day_of_year(2024, 1, 1) == 1
expect TemporalStrftime.day_of_year(2024, 3, 5) == 65
expect TemporalStrftime.day_of_year(2024, 12, 31) == 366   # leap
expect TemporalStrftime.day_of_year(2023, 12, 31) == 365   # common
expect TemporalStrftime.day_of_year(2024, 2, 29) == 60
expect TemporalStrftime.day_of_year(2000, 2, 29) == 60
# March 1st is where the leap adjustment shows: 61 in a leap year, 60 in a
# common one, and 2100 is common despite being divisible by 100.
expect TemporalStrftime.day_of_year(2000, 3, 1) == 61
expect TemporalStrftime.day_of_year(2100, 3, 1) == 60
expect TemporalStrftime.day_of_year(1900, 1, 1) == 1

# -- format: the directive table ---------------------------------------------

expect TemporalStrftime.format(plain_dt(2024, 3, 5, 14, 7, 9), "%Y-%m-%d %H:%M:%S") == "2024-03-05 14:07:09"
expect TemporalStrftime.format(plain_dt(2024, 3, 5, 14, 7, 9), "%F %T") == "2024-03-05 14:07:09"
expect TemporalStrftime.format(plain_dt(2024, 3, 5, 14, 7, 9), "%j") == "065"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%a %A %b %B") == "Tue Tuesday Mar March"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%y") == "24"
expect TemporalStrftime.format(plain_date(2024, 1, 1), "%Y-%m-%d") == "2024-01-01"

# Literal text is carried through untouched, which is most of what a pattern is.
expect TemporalStrftime.format(plain_date(2024, 3, 5), "on %B %d, %Y") == "on March 05, 2024"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "") == ""
expect TemporalStrftime.format(plain_date(2024, 3, 5), "no directives here") == "no directives here"

# %d is zero-padded, %e space-padded — the difference only shows below the 10th.
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%d") == "05"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%e") == " 5"
expect TemporalStrftime.format(plain_date(2024, 3, 15), "%d") == "15"
expect TemporalStrftime.format(plain_date(2024, 3, 15), "%e") == "15"

# %I/%p: the 12-hour clock's two awkward hours are midnight and noon, where
# the modulus gives 0 and the answer is 12.
expect TemporalStrftime.format(plain_dt(2024, 3, 5, 14, 7, 9), "%I %p") == "02 PM"
expect TemporalStrftime.format(plain_dt(2024, 3, 5, 0, 0, 0), "%I %p") == "12 AM"
expect TemporalStrftime.format(plain_dt(2024, 3, 5, 12, 0, 0), "%I %p") == "12 PM"
expect TemporalStrftime.format(plain_dt(2024, 3, 5, 23, 59, 59), "%I %p") == "11 PM"
expect TemporalStrftime.format(plain_dt(2024, 3, 5, 11, 0, 0), "%I %p") == "11 AM"

# Sub-second fields pad to their own widths, not to a shared one.
expect TemporalStrftime.format(fractional(7, 123), "%L") == "007"
expect TemporalStrftime.format(fractional(7, 123), "%N") == "000000123"
expect TemporalStrftime.format(fractional(999, 999999999), "%L.%N") == "999.999999999"

# Escapes.
expect TemporalStrftime.format(plain_date(2024, 3, 5), "100%%") == "100%"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "a%nb") == "a\nb"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "a%tb") == "a\tb"

# An unknown directive comes back verbatim rather than being eaten, so a typo
# is visible in the output instead of silently shortening it.
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%Q") == "%Q"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "[%Q]") == "[%Q]"

# A plain date has no zone, so the zone directives render literally rather
# than inventing UTC.
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%z") == "%z"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%:z") == "%:z"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%Z") == "%Z"

# With a zone, %z is compact and %:z punctuated; both carry the sign, and
# both render a non-whole-hour offset correctly.
expect TemporalStrftime.format(zoned(3600, "CET"), "%z") == "+0100"
expect TemporalStrftime.format(zoned(3600, "CET"), "%:z") == "+01:00"
expect TemporalStrftime.format(zoned(0, "UTC"), "%z") == "+0000"
expect TemporalStrftime.format(zoned(-18000, "EST"), "%z") == "-0500"
expect TemporalStrftime.format(zoned(-18000, "EST"), "%:z") == "-05:00"
expect TemporalStrftime.format(zoned(19800, "IST"), "%z") == "+0530"
expect TemporalStrftime.format(zoned(19800, "IST"), "%:z") == "+05:30"
expect TemporalStrftime.format(zoned(-1800, "X"), "%:z") == "-00:30"
expect TemporalStrftime.format(zoned(3600, "CET"), "%Z") == "CET"

# A year outside four digits keeps its sign in front of the padding, and %y
# stays in 00-99 for a negative year (Python's `-44 % 100` is 56 as well).
expect TemporalStrftime.format(plain_date(-44, 3, 15), "%Y") == "-0044"
expect TemporalStrftime.format(plain_date(-44, 3, 15), "%y") == "56"
expect TemporalStrftime.format(plain_date(12024, 3, 5), "%Y") == "12024"
expect TemporalStrftime.format(plain_date(999, 3, 5), "%Y") == "0999"
expect TemporalStrftime.format(plain_date(1, 3, 5), "%Y") == "0001"

# A value wider than its pad is printed whole rather than truncated — and,
# before the fix in `pad`, aborted the program with an integer underflow.
expect TemporalStrftime.format(plain_date(12024, 3, 5), "%Y") == "12024"
expect TemporalStrftime.format(plain_date(-12024, 3, 5), "%Y") == "-12024"
expect TemporalStrftime.format(fractional(65535, 0), "%L") == "65535"
expect TemporalStrftime.format(fractional(0, 4294967295), "%N") == "4294967295"

# -- parse: the happy paths --------------------------------------------------
#
# Compared as whole `Parts`, not field by field, so a directive that also
# writes a field it should not is caught.

expect TemporalStrftime.parse("2024-03-05", "%Y-%m-%d") == Ok(plain_date(2024, 3, 5))
expect TemporalStrftime.parse("2024-03-05 14:07:09", "%Y-%m-%d %H:%M:%S") == Ok(plain_dt(2024, 3, 5, 14, 7, 9))
expect TemporalStrftime.parse("2024-03-05 14:07:09", "%F %T") == Ok(plain_dt(2024, 3, 5, 14, 7, 9))
expect TemporalStrftime.parse("05/03/2024", "%d/%m/%Y") == Ok(plain_date(2024, 3, 5))

# A field the pattern never names stays at its zero; nothing is inferred.
expect TemporalStrftime.parse("2024-03-05 14", "%Y-%m-%d %H") == Ok(plain_dt(2024, 3, 5, 14, 0, 0))

# %j resolves to month and day, and the year it is resolved against decides
# whether day 60 is the 29th of February or the 1st of March.
expect TemporalStrftime.parse("2024-065", "%Y-%j") == Ok(plain_date(2024, 3, 5))
expect TemporalStrftime.parse("2024-060", "%Y-%j") == Ok(plain_date(2024, 2, 29))
expect TemporalStrftime.parse("2023-060", "%Y-%j") == Ok(plain_date(2023, 3, 1))
expect TemporalStrftime.parse("2024-366", "%Y-%j") == Ok(plain_date(2024, 12, 31))
expect TemporalStrftime.parse("2023-365", "%Y-%j") == Ok(plain_date(2023, 12, 31))
expect TemporalStrftime.parse("1900-365", "%Y-%j") == Ok(plain_date(1900, 12, 31))

# Month and weekday names are case-insensitive: case carries no information
# here, and fixed-width reports upper-case them.
expect TemporalStrftime.parse("2024 Mar 05", "%Y %b %d") == Ok(plain_date(2024, 3, 5))
expect TemporalStrftime.parse("2024 MAR 05", "%Y %b %d") == Ok(plain_date(2024, 3, 5))
expect TemporalStrftime.parse("2024 mar 05", "%Y %b %d") == Ok(plain_date(2024, 3, 5))
expect TemporalStrftime.parse("2024 March 05", "%Y %B %d") == Ok(plain_date(2024, 3, 5))
expect TemporalStrftime.parse("2024 MARCH 05", "%Y %B %d") == Ok(plain_date(2024, 3, 5))
expect TemporalStrftime.parse("2024 December 05", "%Y %B %d") == Ok(plain_date(2024, 12, 5))

# A stated weekday is checked against the date the rest of the pattern built,
# rather than parsed and dropped.
expect TemporalStrftime.parse("Tue 2024-03-05", "%a %Y-%m-%d") == Ok(plain_date(2024, 3, 5))
expect TemporalStrftime.parse("Tuesday 2024-03-05", "%A %Y-%m-%d") == Ok(plain_date(2024, 3, 5))

# %I needs %p to mean anything, and the two ends of the 12-hour clock are
# where the arithmetic is easy to get wrong.
expect TemporalStrftime.parse("2024-03-05 02 PM", "%Y-%m-%d %I %p") == Ok(plain_dt(2024, 3, 5, 14, 0, 0))
expect TemporalStrftime.parse("2024-03-05 02 AM", "%Y-%m-%d %I %p") == Ok(plain_dt(2024, 3, 5, 2, 0, 0))
expect TemporalStrftime.parse("2024-03-05 12 AM", "%Y-%m-%d %I %p") == Ok(plain_dt(2024, 3, 5, 0, 0, 0))
expect TemporalStrftime.parse("2024-03-05 12 PM", "%Y-%m-%d %I %p") == Ok(plain_dt(2024, 3, 5, 12, 0, 0))
expect TemporalStrftime.parse("2024-03-05 11 pm", "%Y-%m-%d %I %p") == Ok(plain_dt(2024, 3, 5, 23, 0, 0))

# %e accepts the space its formatter emits, and the unpadded two-digit form.
expect TemporalStrftime.parse("2024-03- 5", "%Y-%m-%e") == Ok(plain_date(2024, 3, 5))
expect TemporalStrftime.parse("2024-03-15", "%Y-%m-%e") == Ok(plain_date(2024, 3, 15))

# POSIX's two-digit-year pivot: 00-68 are 2000s, 69-99 are 1900s.
expect TemporalStrftime.parse("68-03-05", "%y-%m-%d") == Ok(plain_date(2068, 3, 5))
expect TemporalStrftime.parse("69-03-05", "%y-%m-%d") == Ok(plain_date(1969, 3, 5))
expect TemporalStrftime.parse("00-03-05", "%y-%m-%d") == Ok(plain_date(2000, 3, 5))
expect TemporalStrftime.parse("99-03-05", "%y-%m-%d") == Ok(plain_date(1999, 3, 5))

# Sub-second fields, and the escapes.
expect TemporalStrftime.parse("2024-03-05 14:07:09.007", "%Y-%m-%d %H:%M:%S.%L") == Ok(fractional(7, 0))
expect TemporalStrftime.parse("2024-03-05 100%", "%Y-%m-%d 100%%") == Ok(plain_date(2024, 3, 5))
expect TemporalStrftime.parse("2024-03-05\n", "%Y-%m-%d%n") == Ok(plain_date(2024, 3, 5))
expect TemporalStrftime.parse("2024-03-05\t", "%Y-%m-%d%t") == Ok(plain_date(2024, 3, 5))

expect TemporalStrftime.parse("2024-03-05 14:07:09.000000123", "%Y-%m-%d %H:%M:%S.%N") == Ok(fractional(0, 123))

# -- parse: the strict half --------------------------------------------------
#
# Asserted by KIND, not by message. Which of the two errors a failure is says
# whether the caller wrote a bad pattern or was handed bad data, and that
# distinction is the module's own (`Err`'s doc: "one is a bug and the other is
# data"). The message text is a diagnostic; pinning it would make every
# rewording a failing test.

err_kind : Try(TemporalStrftime.Parts, TemporalStrftime.Err) -> Str
err_kind = |r|
	match r {
		Ok(_) => "ok"
		Err(BadPattern(_)) => "pattern"
		Err(BadInput(_)) => "input"
	}

err_text : Try(TemporalStrftime.Parts, TemporalStrftime.Err) -> Str
err_text = |r|
	match r {
		Ok(_) => "<parsed>"
		Err(BadPattern(m)) => m
		Err(BadInput(m)) => m
	}

# Bad data. Every one of these is text that does not fit a fine pattern.
expect err_kind(TemporalStrftime.parse("2024-03-05 and more", "%Y-%m-%d")) == "input"
expect err_kind(TemporalStrftime.parse("2024/03/05", "%Y-%m-%d")) == "input"
expect err_kind(TemporalStrftime.parse("2024-03", "%Y-%m-%d")) == "input"
expect err_kind(TemporalStrftime.parse("", "%Y-%m-%d")) == "input"
expect err_kind(TemporalStrftime.parse("20x4-03-05", "%Y-%m-%d")) == "input"
expect err_kind(TemporalStrftime.parse("2024-3-5", "%Y-%m-%d")) == "input"
expect err_kind(TemporalStrftime.parse("2024 Xyz 05", "%Y %b %d")) == "input"
expect err_kind(TemporalStrftime.parse("2024-03-05 02 XX", "%Y-%m-%d %I %p")) == "input"

# A trailing space is unconsumed input, not something to overlook.
expect err_kind(TemporalStrftime.parse("2024-03-05 ", "%Y-%m-%d")) == "input"

# %j out of range for the year it is resolved against. 366 is a date in 2024
# and not one in 2023, which is the whole reason the check needs the year.
expect err_kind(TemporalStrftime.parse("2023-366", "%Y-%j")) == "input"
expect err_kind(TemporalStrftime.parse("2024-367", "%Y-%j")) == "input"
expect err_kind(TemporalStrftime.parse("2024-000", "%Y-%j")) == "input"
expect TemporalStrftime.parse("2024-366", "%Y-%j") == Ok(plain_date(2024, 12, 31))

# A weekday that contradicts the date is bad data, and the message names the
# day the date actually falls on rather than only rejecting it.
expect err_kind(TemporalStrftime.parse("Mon 2024-03-05", "%a %Y-%m-%d")) == "input"
expect Str.contains(err_text(TemporalStrftime.parse("Mon 2024-03-05", "%a %Y-%m-%d")), "Tuesday")
expect err_kind(TemporalStrftime.parse("Sunday 2024-03-05", "%A %Y-%m-%d")) == "input"

# Bad patterns. These are the caller's mistake, and none of them depends on
# the input: a pattern that cannot describe a plain date is wrong before the
# text is looked at.
expect err_kind(TemporalStrftime.parse("03-05", "%m-%d")) == "pattern"
expect err_kind(TemporalStrftime.parse("2024", "%Y")) == "pattern"
expect err_kind(TemporalStrftime.parse("2024-03", "%Y-%m")) == "pattern"
expect err_kind(TemporalStrftime.parse("14:07:09", "%T")) == "pattern"

# The zone directives format but do not parse: plain fields have nowhere to
# put a zone, so accepting one would mean silently dropping it.
expect err_kind(TemporalStrftime.parse("2024-03-05 +0100", "%Y-%m-%d %z")) == "pattern"
expect err_kind(TemporalStrftime.parse("2024-03-05 +01:00", "%Y-%m-%d %:z")) == "pattern"
expect err_kind(TemporalStrftime.parse("2024-03-05 CET", "%Y-%m-%d %Z")) == "pattern"

expect err_kind(TemporalStrftime.parse("2024-03-05", "%Y-%m-%d%Q")) == "pattern"
expect err_kind(TemporalStrftime.parse("2024-03-05", "%Y-%m-%d%")) == "pattern"

# -- round trips -------------------------------------------------------------
#
# The two halves are written independently, so agreeing is worth asserting
# directly rather than inferring from the cases above.

round_trip : TemporalStrftime.Parts, Str -> Bool
round_trip = |p, pattern| TemporalStrftime.parse(TemporalStrftime.format(p, pattern), pattern) == Ok(p)

expect round_trip(plain_date(2024, 3, 5), "%Y-%m-%d")
expect round_trip(plain_date(1900, 1, 1), "%Y-%m-%d")
expect round_trip(plain_date(2000, 2, 29), "%Y-%m-%d")
expect round_trip(plain_dt(2024, 3, 5, 14, 7, 9), "%F %T")
expect round_trip(plain_dt(2024, 12, 31, 23, 59, 59), "%Y-%m-%dT%H:%M:%S")
expect round_trip(plain_date(2024, 3, 5), "%Y %B %d")
expect round_trip(plain_date(2024, 3, 5), "%A, %d %b %Y")
expect round_trip(plain_date(2024, 3, 5), "%Y-%j")
expect round_trip(plain_date(2024, 3, 5), "%Y-%m-%e")
expect round_trip(plain_dt(2024, 3, 5, 14, 7, 9), "%Y-%m-%d %I:%M:%S %p")
expect round_trip(plain_dt(2024, 3, 5, 0, 0, 0), "%Y-%m-%d %I:%M:%S %p")
expect round_trip(plain_dt(2024, 3, 5, 12, 0, 0), "%Y-%m-%d %I:%M:%S %p")

# -- format: incomplete and unknown directives render literally --------------
#
# The module's own rule, stated above `format`: an unknown directive is
# "visible in the output rather than silently dropped".

# `%:` is only ever `%:z`; anything else is unknown. It used to be handed to
# `emit`, which expanded a known directive AND echoed the byte on top.
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%:Y") == "%:Y"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%:m") == "%:m"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%:Q") == "%:Q"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "[%:Q]") == "[%:Q]"

# ...and `%:z` still resolves, with and without a zone.
expect TemporalStrftime.format(zoned(3600, "CET"), "%:z") == "+01:00"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%:z") == "%:z"

# A pattern ending mid-directive keeps what it has. `parse` REJECTS the same
# pattern, and that asymmetry is the design: formatting cannot fail, parsing
# is the strict half.
expect TemporalStrftime.format(plain_date(2024, 3, 5), "abc%") == "abc%"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%") == "%"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "abc%:") == "abc%:"
expect TemporalStrftime.format(plain_date(2024, 3, 5), "%:") == "%:"
expect err_kind(TemporalStrftime.parse("2024-03-05", "%Y-%m-%d%")) == "pattern"
