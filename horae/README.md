# horae

Traditional astrology of a moment at a place, on the [astrometry](..) ephemeris: the seven
planets and the lunar nodes (true) in the tropical zodiac, their dignities and closeness to
the Sun, the Ptolemaic aspects with William Lilly's orbs, applying or separating; the Moon's
phase, lunations and course to the end of its sign (void of course); the planets' stations;
the Ascendant and Midheaven; and the planetary days and hours, with the polar days that have
none.

```rust
use astrometry::source::Builtin;
use horae::{Point, Position, time};

let now: jiff::Zoned = "2026-10-20T08:00[America/Chicago]".parse()?;
let sun = Position::at(&Builtin, Point::Sun, time::tt(now.timestamp()))?;
println!("the Sun in {}", sun.sign());
```

The tradition is Ptolemy's, the medievals' and Lilly's, as found in the texts; the code is
this project's. With the `serde` feature, everything serializes.

Begun as the astrology of Geomancer; used by Geomancer and Astro Clock. Apache-2.0, as
astrometry: see the repository's [LICENSE](../LICENSE).
