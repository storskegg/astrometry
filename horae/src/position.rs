//! Where a point is in the zodiac, and how fast it moves.

use std::fmt;

use astrometry::apparent::apparent;
use astrometry::derived::{self, true_node, true_node_speed};
use astrometry::source::{self, Source};

use crate::EphemerisError;
use crate::zodiac::{Planet, Point, Sign};

/// Where a planet or node is, and its motion.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Position {
    pub point: Point,
    /// The apparent geocentric ecliptic longitude, degrees, 0–360 from 0° Aries.
    pub longitude: f64,
    /// The daily motion in longitude, degrees; negative is retrograde.
    pub speed: f64,
}

impl Position {
    /// Where `point` is at TT Julian date `tt`.
    pub fn at(src: &dyn Source, point: Point, tt: f64) -> Result<Position, EphemerisError> {
        let (longitude, speed) = match point.planet() {
            Some(p) => {
                let b = body(p);
                (apparent(src, b, tt)?.longitude, derived::speed(src, b, tt)?)
            }
            None => {
                let node = true_node(src, tt)?;
                let node = if point == Point::SouthNode {
                    (node + 180.0) % 360.0
                } else {
                    node
                };
                (node, true_node_speed(src, tt)?)
            }
        };
        Ok(Position {
            point,
            longitude,
            speed,
        })
    }

    /// Every point's position, in [`Point::ALL`] order.
    pub fn all(src: &dyn Source, tt: f64) -> Result<[Position; 9], EphemerisError> {
        let mut out = [Position {
            point: Point::Sun,
            longitude: 0.0,
            speed: 0.0,
        }; 9];
        for p in Point::ALL {
            // The South Node is the North's, turned about.
            out[p.index()] = if p == Point::SouthNode {
                let n = out[Point::NorthNode.index()];
                Position {
                    point: p,
                    longitude: (n.longitude + 180.0) % 360.0,
                    speed: n.speed,
                }
            } else {
                Position::at(src, p, tt)?
            };
        }
        Ok(out)
    }

    pub fn sign(&self) -> Sign {
        Sign::of(self.longitude)
    }

    /// Moving backwards in longitude. The true node usually is, and now and then isn't.
    pub fn is_retrograde(&self) -> bool {
        self.speed < 0.0
    }

    /// The whole degrees, minutes and seconds within the sign, each truncated.
    pub fn degrees_minutes_seconds(&self) -> (u32, u32, u32) {
        degrees_minutes_seconds(self.longitude)
    }
}

/// The place in the sign, to the second, truncated as astrologers do: "14°22′07″ Scorpio".
impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (d, m, s) = self.degrees_minutes_seconds();
        write!(f, "{d}°{m:02}′{s:02}″ {}", self.sign())
    }
}

/// The whole degrees, minutes and seconds of a longitude within its sign, each truncated.
pub fn degrees_minutes_seconds(lon: f64) -> (u32, u32, u32) {
    let secs = (lon.rem_euclid(30.0) * 3600.0).floor() as u32;
    (secs / 3600, secs / 60 % 60, secs % 60)
}

/// An arc in degrees, minutes and seconds, each truncated: "4°07′32″".
pub fn arc(deg: f64) -> String {
    let secs = (deg * 3600.0).floor() as u64;
    format!("{}°{:02}′{:02}″", secs / 3600, secs / 60 % 60, secs % 60)
}

/// The astrometry body for a planet.
pub(crate) fn body(p: Planet) -> source::Body {
    match p {
        Planet::Sun => source::Body::Sun,
        Planet::Moon => source::Body::Moon,
        Planet::Mercury => source::Body::Mercury,
        Planet::Venus => source::Body::Venus,
        Planet::Mars => source::Body::Mars,
        Planet::Jupiter => source::Body::Jupiter,
        Planet::Saturn => source::Body::Saturn,
    }
}

/// The seven planets' longitudes at a TT Julian date, degrees, by planet index: without
/// speeds, for searches that sample many moments.
pub(crate) fn longitudes(src: &dyn Source, tt: f64) -> Result<[f64; 7], EphemerisError> {
    let mut lon = [0.0; 7];
    for p in Planet::ALL {
        lon[p.index()] = apparent(src, body(p), tt)?.longitude;
    }
    Ok(lon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display() {
        let tests = [
            (0.0, "0°00′00″ Aries"),
            (224.9999, "14°59′59″ Scorpio"), // truncated, not rounded
            (359.99, "29°59′24″ Pisces"),
            (90.5, "0°30′00″ Cancer"),
        ];
        for (lon, want) in tests {
            let p = Position {
                point: Point::Sun,
                longitude: lon,
                speed: 0.0,
            };
            assert_eq!(p.to_string(), want, "{lon}");
        }
    }

    #[test]
    fn arcs() {
        assert_eq!(arc(4.1255556), "4°07′32″");
        assert_eq!(arc(0.0), "0°00′00″");
    }
}
