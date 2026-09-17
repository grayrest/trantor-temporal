// Writes icu_calendars.txt: what ICU4C (through Node's Intl) says each ISO day
// is on the fifteen non-ISO calendars, 1800-2200, for calendar_fields.rs to
// hold temporal_rs (ICU4X) to. Run from tests/sweeps: `node icu/generate.mjs`.
//
// One row per run of days sharing era, year and month:
//   <iso start> <era or -> <year> <month code> <day at start> <days in month> <months in year>
// under a `# <calendar>` header. Month codes are TC39's; eras are ICU4C's
// English labels with spaces as `_`, which the sweep maps to temporal_rs's codes.
import { writeFileSync } from "node:fs";

const CALENDARS = ["buddhist", "chinese", "coptic", "dangi", "ethioaa", "ethiopic", "gregory", "hebrew", "indian",
  "islamic-civil", "islamic-tbla", "islamic-umalqura", "japanese", "persian", "roc"];
const HEBREW = { Tishri: "M01", Heshvan: "M02", Kislev: "M03", Tevet: "M04", Shevat: "M05", "Adar I": "M05L",
  "Adar II": "M06", Adar: "M06", Nisan: "M07", Iyar: "M08", Sivan: "M09", Tamuz: "M10", Av: "M11", Elul: "M12" };
const DAY_MS = 86_400_000;
const FROM = Date.UTC(1800, 0, 1), TO = Date.UTC(2201, 0, 1);

function monthCode(calendar, label) {
  if (calendar === "hebrew") {
    if (!(label in HEBREW)) throw new Error(`unknown Hebrew month ${label}`);
    return HEBREW[label];
  }
  const m = /^(?:Mo)?(\d+)(bis)?$/.exec(label);
  if (!m) throw new Error(`unknown ${calendar} month ${label}`);
  return `M${m[1].padStart(2, "0")}${m[2] ? "L" : ""}`;
}

function iso(ms) {
  return new Date(ms).toISOString().slice(0, 10);
}

const out = [`# ICU4C ${process.versions.icu} (Unicode ${process.versions.unicode}) through Node ${process.versions.node}; see icu/generate.mjs.`];
for (const calendar of CALENDARS) {
  const format = new Intl.DateTimeFormat(`en-u-ca-${calendar}`, { timeZone: "UTC", era: "short", year: "numeric", month: "numeric", day: "numeric" });
  const days = [];
  for (let t = FROM; t < TO; t += DAY_MS) {
    const p = Object.fromEntries(format.formatToParts(t).map(x => [x.type, x.value]));
    const year = p.relatedYear ?? p.year;
    days.push({ t, era: (p.era ?? "-").replaceAll(" ", "_"), year, code: monthCode(calendar, p.month), day: Number(p.day) });
  }
  // Month lengths and months per year ignore eras, which can change mid-month.
  const monthLength = new Map(), yearMonths = new Map();
  const yearKey = d => calendar === "japanese" ? new Date(d.t).getUTCFullYear() : `${d.era} ${d.year}`;
  let start = 0;
  for (let i = 1; i <= days.length; i++) {
    const a = days[i - 1], b = days[i];
    if (!b || b.day <= a.day) {
      const first = days[start];
      monthLength.set(start, first.day - 1 + (i - start));
      const key = yearKey(first);
      yearMonths.set(key, (yearMonths.get(key) ?? new Set()).add(first.code));
      for (let k = start; k < i; k++) days[k].month = start;
      start = i;
    }
  }
  // Partial months at either end of the range have no known length.
  const firstMonth = days[0].month, lastMonth = days[days.length - 1].month;
  const wholeYears = new Set([...yearMonths.keys()].slice(1, -1));
  out.push(`# ${calendar}`);
  for (let i = 0; i < days.length; i++) {
    const d = days[i], prev = days[i - 1];
    if (d.month === firstMonth || d.month === lastMonth) continue;
    if (prev && prev.month === d.month && prev.era === d.era && prev.year === d.year) continue;
    const key = yearKey(days[d.month]);
    const inYear = wholeYears.has(key) ? yearMonths.get(key).size : 0;
    out.push(`${iso(d.t)} ${d.era} ${d.year} ${d.code} ${d.day} ${monthLength.get(d.month)} ${inYear}`);
  }
}
writeFileSync("icu_calendars.txt", out.join("\n") + "\n");
console.log(`${out.length} lines`);
