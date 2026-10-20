//! Stations: the moments a planet stands still in longitude and turns, retrograde or direct.
//! The Sun and Moon never do. The nodes' turns, frequent and brief, aren't counted. Found by
//! astrometry; here, by planet, and their times zoned.

use astrometry::source::Source;
use astrometry::stations;
use jiff::Zoned;
use jiff::tz::TimeZone;

use crate::position::body;
use crate::zodiac::Planet;
use crate::{EphemerisError, time};

named_enum!(
    /// Which way a planet moves in longitude.
    Direction, "direction", [Direct => "Direct", Retrograde => "Retrograde"]
);

/// A planet's station.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Station {
    pub planet: Planet,
    /// The way it turns: Retrograde, it stations retrograde.
    pub turns: Direction,
    pub at: Zoned,
    /// Where it stands, degrees.
    pub longitude: f64,
}

/// The longest a planet goes between stations, days, with room to spare; none for the Sun
/// and Moon, which never station. astrometry's, by planet.
pub fn longest_between(p: Planet) -> Option<f64> {
    stations::longest_between(body(p))
}

fn station(p: Planet, s: stations::Station, tz: &TimeZone) -> Station {
    Station {
        planet: p,
        turns: if s.retrograde {
            Direction::Retrograde
        } else {
            Direction::Direct
        },
        at: time::zoned(s.tt, tz),
        longitude: s.longitude,
    }
}

/// `p`'s stations between TT Julian dates `from` and `to`, in order, their times in `tz`:
/// astrometry's, named.
pub fn stations(
    src: &dyn Source,
    p: Planet,
    from: f64,
    to: f64,
    tz: &TimeZone,
) -> Result<Vec<Station>, EphemerisError> {
    Ok(stations::stations(src, body(p), from, to)?
        .into_iter()
        .map(|s| station(p, s, tz))
        .collect())
}

/// `p`'s last station at or before TT Julian date `tt`; none for the Sun and Moon.
pub fn previous_station(
    src: &dyn Source,
    p: Planet,
    tt: f64,
    tz: &TimeZone,
) -> Result<Option<Station>, EphemerisError> {
    Ok(stations::previous_station(src, body(p), tt)?.map(|s| station(p, s, tz)))
}

/// `p`'s first station after TT Julian date `tt`; none for the Sun and Moon.
pub fn next_station(
    src: &dyn Source,
    p: Planet,
    tt: f64,
    tz: &TimeZone,
) -> Result<Option<Station>, EphemerisError> {
    Ok(stations::next_station(src, body(p), tt)?.map(|s| station(p, s, tz)))
}
