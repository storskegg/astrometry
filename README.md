# astrometry

[![Tests](https://github.com/storskegg/astrometry/actions/workflows/tests.yml/badge.svg?branch=main)](https://github.com/storskegg/astrometry/actions/workflows/tests.yml)

Apparent geocentric positions of the Sun, the Moon and the planets from Mercury to Saturn,
as close to JPL Horizons and the Swiss Ephemeris as we can make them, offline and in pure
Rust.

- **Built in, no files needed:** a slice of JPL's DE440 for 1800–2200 (the Chebyshev records
  the Sun, Moon and classical planets need, re-encoded to 8 MB, within 0.000015″ of DE440
  itself), with VSOP87E and ELP/MPP02 beyond. A full JPL `.bsp` file can be read instead.
- **The reduction as Horizons and the Swiss Ephemeris have it:** light time, the Sun's
  deflection of light, relativistic annual aberration, IAU 2006 precession and IAU
  2000A_R06 nutation, to the true equinox and ecliptic of date.
- **Time:** UTC, UT1, TT and TDB, with ΔT from Horizons, the IERS and USNO, then the
  IERS's predictions a year ahead (held after, as Horizons holds its own), and the leap
  seconds. `cargo xtask fixtures usno` brings them up to date.
- **Derived quantities:** speeds in longitude, the Moon's true and mean nodes and how it's
  lit, sidereal time, the Sun's altitude and its crossings (rise and set), the Ascendant and
  Midheaven, the planets' stations, and the search for events such as new and full moons and
  ingresses.

And beside it, in [`horae/`](horae), **horae**: the traditional astrology these are for. The
signs, the dignities, the Ptolemaic aspects with Lilly's orbs, the Moon's phases and course
(void of course), and the planetary days and hours. Both are Apache-2.0: an ephemeris and an
astrology that are neither copyleft nor costly to license.

Beside horae too, in [`renpet/`](renpet), **renpet**: the ancient Egyptian civil calendar and
the seasonal hours of the day and night, carried forward unbroken on the same ephemeris.

## Accuracy

Against the Swiss Ephemeris in its JPL mode (the same DE440), 1800–2200, every body: within
0.0033″ in longitude and 0.0015″ in latitude. Speeds within 0.27″ a day; the true node within
0.020″; new and full moons and ingresses within 0.003 s. Against Horizons: sunrise and sunset
within a second; sidereal time within 0.032 s where UT1 is observed. Horizons' own ecliptic
longitudes rest on older precession outside its central years, and differ from these by its
documented model difference; the tests say where.

## Using it

```rust
use astrometry::apparent::apparent;
use astrometry::source::{Body, Builtin};
use astrometry::time;

// 18 October 2026, 17:00 UTC, as a civil Julian date, then TT.
let tt = time::tt_from_civil(time::julian_date(2026, 10, 18, 17, 0, 0.0));
let sun = apparent(&Builtin, Body::Sun, tt)?;
println!("{:.5}° {:.5}°", sun.longitude, sun.latitude);
```

`cargo run --example sun` runs it. Rust 1.99 or later. Not on crates.io yet; as a git
dependency:

```toml
astrometry = { git = "https://github.com/storskegg/astrometry", tag = "v2026.47.1" }
horae = { git = "https://github.com/storskegg/astrometry", tag = "v2026.47.1" }
```

The two are versioned together and tagged once: take both at the same tag, or Cargo sees two
astrometries.

From 2026-11-09 the versions are calendar versions, `YYYY.WW.PATCH`: the ISO 8601 week-year
and week (Monday to Sunday) of the release, and its count within that week from 0, without
zero-padding: `2026.46.0`, `2026.46.1`, then `2026.47.0`, and `2027.1.0` in January. Tags are
`v` and the version. The last semantic version was 1.6.0. A number says when, not what changed,
and Cargo would take any two in a year for compatible, so pin an exact tag and read the release
notes, which are the tagging commit's message. Breaking changes are avoided; when one is needed,
the old item is deprecated first, naming its replacement, and removed no sooner than four weeks
later, in a release whose notes say "Breaking:". The `embedded` feature, on by default, compiles the DE440 slice in; with
`default-features = false` on both, `astrometry::slice::install(&bytes)` takes
`data/de440-1800-2200.bin` at run time instead (a browser's build, say), and without `theories`, also on by default, dates beyond 1800–2200
are errors, not VSOP87E and ELP/MPP02's. The `outer-planets` feature, off by default, adds Uranus,
Neptune and Pluto (DE440's barycentres, 0.7 MB, 1800–2200 only; outside it they are errors):
compiled in with `embedded`, otherwise `astrometry::slice::install_outer(&bytes)` takes
`data/de440-outer-1800-2200.bin`, and horae's `chart` then gives them. The `serde` feature
(of either) serializes their results; horae's names its
members, as "North Node" and "Under the Beams".

### Fixed stars

`astrometry::stars` gives the apparent position of a fixed star, from a catalogue row in
`data/stars.tsv` (Sirius, HIP 32349, for now; the astrometry is from the Hipparcos new
reduction, van Leeuwen 2007, and the radial velocity from SIMBAD). The reduction is the one
the bodies have, with a star's own steps first: rigorous space motion with the radial
velocity from the catalogue epoch, parallax from the Earth's barycentric position, the Sun's
light deflection, annual aberration, then IAU 2006 precession and IAU 2000A_R06 nutation.
`stars::apparent` gives the same `Apparent` as the bodies, `stars::equatorial` the right
ascension and declination of date, and `stars::altitude` and `stars::crossings` the altitude
and the rising and setting, as `horizon` does for the Sun. Against the Swiss Ephemeris, fed
the same catalogue row, the reduction agrees within 0.0005 mas in the ICRS axes over
1800–2200, and positions of date within 2.4 mas, which is the two programs' precession and
nutation models (the Swiss Ephemeris' defaults differ from IAU 2006 and 2000A_R06, for every
body). Sirius's risings and settings agree within 0.15 s.

```rust
use astrometry::source::Builtin;
use astrometry::stars::{equatorial, sirius};

let (ra, dec) = equatorial(&Builtin, &sirius(), tt)?;
```

## The tests and the data

`cargo test` runs offline, against the built-in data and the checked-in reference numbers,
among them a small binary fixture made from the full DE440, `tests/data/de440/reference.bin`
(below). The tests that read the full
[`de440.bsp`](https://ssd.jpl.nasa.gov/ftp/eph/planets/bsp/de440.bsp) (120 MB) from the file
named by `ASTROMETRY_DE440` are skipped without it. No external ephemeris stays on disk.

- `cargo xtask check` runs the whole CI matrix locally: fmt, clippy, the tests, the wasm32
  and MSRV (1.99) checks and the iOS builds. Every local build starts from a clean, so it
  runs `cargo clean` first and again at the end: builds start from nothing each time. It
  downloads nothing.
- `cargo xtask fixtures [PART...]` fetches the reference data again: JPL Horizons, the IERS,
  USNO, the IANA leap seconds, the theories, and the Swiss Ephemeris' numbers (which are
  made in a throwaway environment under `target/swisseph`, deleted afterwards).
- `cargo xtask fixtures stars` makes `tests/data/swisseph/stars.tsv`: the Swiss Ephemeris'
  Sirius, from this crate's own `data/stars.tsv` through a fixed-star file, in the same
  throwaway environment: its apparent positions, its risings and settings at Luxor and
  Heliopolis, and its heliacal risings (for later use; nothing tests them yet). Near 1300 BCE
  it reads its own ephemeris files, which it downloads there too, and deletes.
- `cargo xtask fixtures de440` downloads `de440.bsp` to `local/jpl/`, rebuilds the two
  built-in files from it, writes `reference.bin`, runs every test against the full file, and
  deletes the download, whatever happened. The files it rebuilds must be byte-identical to
  the committed ones if the download is the one recorded here.
- `cargo xtask de440` and `cargo xtask outer` cut the built-in slice and the outer planets'
  file from a `de440.bsp` you already have (`ASTROMETRY_DE440`, or `local/jpl/de440.bsp`).

SHA-256, as recorded in `xtask/src/de440.rs`:

| file | SHA-256 |
|---|---|
| `de440.bsp`, as JPL serves it | `a4ce9bf9b3282becc9f4b2ac3cebe03a2ae7599981aabd7265fd8482fff7c4b5` |
| `data/de440-1800-2200.bin` | `68e5bd49234e959d3be8946cbc9361d4d2e1b85578b2919b16c091384da1c324` |
| `data/de440-outer-1800-2200.bin` | `29e651b2c6ab2a9c63071532c135a8d0442fdcb4e2b12460c8eab656c254e805` |

### The built-in slice, `ASTRDE02`

The slice is DE440's Chebyshev records for the segments the Sun, Moon and classical planets
need, 1800-2200, with each coefficient rounded to a fixed step (1 cm for the Moon about the
Earth-Moon barycentre, 1 m for the rest), written as zigzag varints and deflated in blocks of
about a year's records. All numbers are little-endian; the file as a whole isn't deflated.

- The file starts with the ASCII magic `ASTRDE02` and the segment count (u32).
- Then, for each segment, its head followed by its block table. The head is: target and
  centre NAIF ids (i32 each); the start of the first record and each record's span (f64, TDB
  seconds past J2000); the number of coefficients per coordinate (u32) and of records (u32);
  the rounding step (f64, km); the records per block (u32) and the block count (u32). The
  table has, for each block, its byte offset from the start of the file (u64) and its length
  in bytes (u32).
- Then the blocks. Each is raw-deflated (RFC 1951) on its own; inflated, it is the zigzag
  varints of its records' coefficients (in units of the step, the integer `q` stored as
  `(q << 1) ^ (q >> 63)`, then 7 bits at a time, low first, the high bit set on all but the
  last byte), record after record, each record being the x coefficients, then y, then z.
  A coefficient in km is `q * step`.
- The Earth about the Earth-Moon barycentre isn't kept: it is the Moon's offset divided by the
  Earth/Moon mass ratio (81.30056822149722).

With the `outer-planets` feature there is a second file of the same format and span,
`data/de440-outer-1800-2200.bin`: Uranus, Neptune and Pluto's barycentres (7, 8, 9) about the
solar-system barycentre, rounded to 1 m.

### The reference fixture, `ASTRREF1`

`tests/data/de440/reference.bin` (about 0.9 MB, uncompressed, little-endian) holds the full
DE440's numbers at 2,000 instants spread over 1800-2200 (`2378496.5 + 0.37 + i * 146097 /
2000`), which the tests compare the slice and the analytical theories with.

- The magic `ASTRREF1`, then the instant, segment and body counts (u32 each).
- The instants: Julian dates (f64), taken as TT for the apparent positions and as TDB for the
  segments.
- The geometric section: for each segment of the two built-in files, its target and centre
  (i32 each), then at each instant the position x, y, z (f64, km) of the target about the
  centre.
- The apparent section: for each body, its id (u32: the `Body` enum's order, Sun 0, Moon 1,
  Mercury 2, Venus 3, Earth 4, Mars 5, Jupiter 6, Saturn 7, Uranus 8, Neptune 9, Pluto 10),
  then at each instant the apparent ecliptic-of-date longitude and latitude (f64, degrees)
  from `astrometry::apparent::apparent`.

## License

Apache-2.0 (see [LICENSE](LICENSE)). The data built in are under their own terms: see
[NOTICE.md](NOTICE.md).
