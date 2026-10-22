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

`cargo run --example sun` runs it. Rust 1.92 or later. Not on crates.io yet; as a git
dependency:

```toml
astrometry = { git = "https://github.com/storskegg/astrometry", tag = "v1.3.0" }
horae = { git = "https://github.com/storskegg/astrometry", tag = "v1.3.0" }
```

The two are versioned together and tagged once: take both at the same tag, or Cargo sees two
astrometries. The `embedded` feature, on by default, compiles the DE440 slice in; with
`default-features = false` on both, `astrometry::slice::install(&bytes)` takes
`data/de440-1800-2200.bin` at run time instead (a browser's build, say). The `serde` feature
(of either) serializes their results; horae's names its
members, as "North Node" and "Under the beams".

## The tests and the data

`cargo test` runs the tests against the built-in data and the checked-in reference numbers,
offline. With `ASTROMETRY_DE440` naming a downloaded
[`de440.bsp`](https://ssd.jpl.nasa.gov/ftp/eph/planets/bsp/de440.bsp) (120 MB), the JPL
tests run too, against the full file.

- `cargo xtask fixtures [PART...]` fetches the reference data again: JPL Horizons, the IERS,
  USNO, the IANA leap seconds, the theories, and the Swiss Ephemeris' numbers.
- `cargo xtask de440` cuts the built-in slice from `de440.bsp`.

## Licence

Apache-2.0 (see [LICENSE](LICENSE)). The data built in are under their own terms: see
[NOTICE.md](NOTICE.md).
