# renpet

The ancient Egyptian calendar and hours, carried forward unbroken, on the
[astrometry](..) ephemeris. *Renpet* (rnpt) is the Egyptian word for "year". Apache-2.0.

Not on crates.io; as a git dependency, at the same tag as astrometry:

```toml
renpet = { git = "https://github.com/storskegg/astrometry", tag = "v2026.47.1" }
```

## What it has

- **`civil`**: the 365-day civil calendar, by pure arithmetic on Julian Day Numbers. The three
  seasons (Akhet, Peret, Shemu), the twelve months of thirty days and the five epagomenal
  days, as `CivilDay`; the decade (1-36) and the ordinal ("III Peret 9"); the year in the Era
  of Nabonassar and in the Sothic cycle. `civil_date_of` takes a Gregorian date to its
  Egyptian day, `CivilDate::gregorian` takes it back, and `next` finds the day's next return.
  The civil year has no leap day, so it passes through the seasons once in 1,461 years.
- **`hours`**: the day of twelve seasonal hours of daylight and twelve of night (`day`), each
  half divided equally, from the Sun's rising and setting at a place; `civil_date_at` and
  `civil_day_start` for the Egyptian day a moment is in, which begins at sunrise (or at 06:00
  local mean solar time where the Sun doesn't rise).

Decans and Sirius are to come. With the `serde` feature, everything serializes; the enums
have stable ids (`month.djehuty`) and Egyptological transliterations. The months'
transliterations are provisional: the Ramesside-period forms are to be confirmed.

## Range

The calendar runs over the Gregorian years -9999 to 9999 (proleptic, as jiff's); beyond them
it's an error. Dates before 2781 BCE fall in Sothic cycle 0 and below, and before 747 BCE the
Nabonassar year is 0 or negative. The hours need the ephemeris: the built-in DE440 slice
(1800-2200) and the series beyond it, as astrometry has them.

## Anchors

- Era of Nabonassar: 1 Thoth of year 1 is JDN 1448638 (26 February 747 BCE Julian, -746-02-18
  proleptic Gregorian). Ptolemy, *Almagest* III.7.
- Sothic cycle 1 begins on 1 Thoth at JDN 705498 (-2781-06-27 proleptic Gregorian); cycle 3
  begins at JDN 1772028 (20 July 139 CE Julian), the new cycle of Censorinus, *De die
  natali* 21.10; cycle 4 at JDN 2305293 (1599-07-30).
- The civil date of 7 March 238 BCE Julian (JDN 1634559) is I Peret 17, the Canopus Decree,
  OGIS 56.
