//! Everything at once: the sky of a moment at a place.

use astrometry::source::{Builtin, Source};
use jiff::Zoned;

use crate::aspect::{Aspect, angle, aspects};
use crate::conditions::{Dignity, Solar, dignities, solar};
use crate::hours::{self, Day};
use crate::moon::{self, Appearance, Course, Lunations};
use crate::position::Position;
use crate::zodiac::{Planet, Point};
use crate::{Error, check_coordinates, time};

/// The astrology of a moment.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sky {
    /// The moment, in the zone it was given in.
    pub time: Zoned,
    /// The planets and nodes, in [`Point::ALL`] order.
    pub bodies: Vec<Body>,
    /// Aspects between the planets, and conjunctions with the nodes, closest first.
    pub aspects: Vec<Aspect>,
    pub moon: Moon,
    pub day: Day,
}

/// A point's position and, for a planet, its conditions.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Body {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub position: Position,
    /// The planet's essential dignities and debilities by sign: Peregrine if none. None for
    /// the nodes.
    pub dignities: Vec<Dignity>,
    /// The planet's closeness to the Sun, if close: never for the Sun or the nodes.
    pub solar: Option<Solar>,
}

/// The Moon's phase, the lunations either side, and its course.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Moon {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub appearance: Appearance,
    pub lunations: Lunations,
    pub course: Course,
}

/// How far on the longitudes are taken to tell applying aspects from separating, days.
const LATER: f64 = 1.0 / 24.0;

impl Sky {
    /// The sky at `t`, seen from latitude `lat` and longitude `lon` in degrees (north and east
    /// positive), from the built-in ephemeris. Times in the result are in `t`'s zone.
    pub fn at(t: &Zoned, lat: f64, lon: f64) -> Result<Sky, Error> {
        Sky::at_with(&Builtin, t, lat, lon)
    }

    /// [`Sky::at`] from another source of positions, such as a full JPL file.
    pub fn at_with(src: &dyn Source, t: &Zoned, lat: f64, lon: f64) -> Result<Sky, Error> {
        check_coordinates(lat, lon)?;
        let tt = time::tt(t.timestamp());
        let tz = t.time_zone();
        let positions = Position::all(src, tt)?;
        let sun = positions[Point::Sun.index()].longitude;
        let bodies = positions
            .iter()
            .map(|&position| {
                let planet = position.point.planet();
                Body {
                    position,
                    dignities: planet.map_or(Vec::new(), |p| dignities(p, position.sign())),
                    solar: planet
                        .filter(|p| *p != Planet::Sun)
                        .and_then(|_| solar(angle(position.longitude, sun))),
                }
            })
            .collect();
        let now = positions.map(|p| p.longitude);
        let later = positions.map(|p| p.longitude + p.speed * LATER);
        Ok(Sky {
            time: t.clone(),
            bodies,
            aspects: aspects(&now, &later),
            moon: Moon {
                appearance: moon::appearance(src, tt)?,
                lunations: moon::lunations(src, tt, tz)?,
                course: moon::course(src, tt, tz)?,
            },
            day: hours::day(src, t, lat, lon)?,
        })
    }

    /// The body for `p`.
    pub fn body(&self, p: impl Into<Point>) -> &Body {
        &self.bodies[p.into().index()]
    }
}
