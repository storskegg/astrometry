//! A chart of chosen points, with aspects of chosen kinds and orbs.
//!
//! This is the configurable counterpart of [`crate::aspects`]: that one is fixed to the
//! seven planets, the nodes, the Ptolemaic aspects and Lilly's moieties, while here the caller
//! picks the points ([`Point`]), the aspects ([`Kind`]) and the orb of every pair and kind.
//!
//! Not every [`Point`] can be had in every build: [`Point::available`] says which (Uranus,
//! Neptune and Pluto need the `outer-planets` feature). Asking [`positions`] for one that isn't
//! gives [`Error::Unavailable`]. The angles, the mean nodes and Lilith need no ephemeris.
//!
//! # Orbs
//!
//! horae has no default orbs: the app owns them. [`aspects`] asks a closure for the orb, in
//! degrees, of each pair of points and kind; it may answer `None` to skip that pair and
//! kind. A per-kind orb, tightened for one pair:
//!
//! ```
//! use horae::chart::{self, Kind, Point};
//!
//! let positions = [
//!     chart::Position { point: Point::Sun, longitude: 0.0, speed: 1.0 },
//!     chart::Position { point: Point::Moon, longitude: 94.0, speed: 13.0 },
//! ];
//! let orb = |a: Point, b: Point, kind: Kind| {
//!     let orb = if kind.is_major() { 6.0 } else { 2.0 };
//!     // The Sun and Moon get less for a square.
//!     if (a, b, kind) == (Point::Sun, Point::Moon, Kind::Square) {
//!         Some(3.0)
//!     } else {
//!         Some(orb)
//!     }
//! };
//! // 4° from the square is beyond the pair's 3°, though within the kind's 6°.
//! assert!(chart::aspects(&positions, &[Kind::Square], orb).is_empty());
//! let wide = |_: Point, _: Point, _: Kind| Some(6.0);
//! assert_eq!(chart::aspects(&positions, &[Kind::Square], wide).len(), 1);
//! ```
//!
//! # Declinations, parallels and out-of-bounds
//!
//! A point's declination is its angle north (positive) or south of the celestial equator.
//! [`declinations`] gives it, with its daily motion, for the same [`Point`]s, and whether the
//! point is out of bounds: farther from the equator than the Sun ever gets, which is to say
//! beyond the true obliquity of the ecliptic of the date. The planets' declinations are
//! apparent, as their longitudes are. The nodes and the angles are on the ecliptic, so theirs
//! follow from their longitudes; Lilith has the latitude of the mean lunar apogee (up to
//! about 5.14°), as the Swiss Ephemeris gives it.
//!
//! [`parallels`] finds the pairs in parallel (the same declination, on the same side of the
//! equator) and in contraparallel (the same declination on opposite sides). As with aspects,
//! horae has no default orbs: the app owns them, by a closure that may answer `None` to skip a
//! pair and kind.
//!
//! ```
//! use horae::chart::{self, Declination, ParallelKind, Point};
//!
//! let at = |point, declination| Declination {
//!     point,
//!     declination,
//!     speed: 0.0,
//!     out_of_bounds: false,
//! };
//! let declinations = [at(Point::Sun, 20.0), at(Point::Mars, -20.5), at(Point::Venus, 20.25)];
//! let orb = |_: Point, _: Point, _: ParallelKind| Some(1.0);
//! let found = chart::parallels(&declinations, &ParallelKind::ALL, orb);
//! // Sun and Venus in parallel at 0.25°, Mars and Venus in contraparallel at 0.25°, and Sun
//! // and Mars in contraparallel at 0.5°.
//! assert_eq!(found.len(), 3);
//! assert_eq!(found[0].kind, ParallelKind::Parallel);
//! ```

use std::fmt;

use astrometry::derived::{
    STEP, declination as body_declination, declination_speed as body_declination_speed, diff,
    ecliptic_declination, mean_apogee, mean_apogee_latitude, mean_apogee_speed, mean_node,
    mean_node_speed, true_node,
};
use astrometry::source::{Body, Source};

use crate::angles::angles;
use crate::aspect::angle as separation;
use crate::{EphemerisError, check_coordinates};

/// Half the interval, days, over which the angles' speeds are taken: one minute.
const ANGLE_STEP: f64 = 1.0 / 1440.0;

named_enum!(
    /// A point of a chart: the planets, the lunar nodes and apogee, and the angles.
    Point, "chart point", [
        Sun => "Sun", Moon => "Moon", Mercury => "Mercury", Venus => "Venus", Mars => "Mars",
        Jupiter => "Jupiter", Saturn => "Saturn",
        Uranus => "Uranus", Neptune => "Neptune", Pluto => "Pluto",
        /// The true (osculating) ascending node.
        NorthNode => "North Node",
        /// The true descending node, opposite the North Node.
        SouthNode => "South Node",
        /// The mean ascending node.
        MeanNorthNode => "Mean North Node",
        /// The mean descending node.
        MeanSouthNode => "Mean South Node",
        /// Black Moon Lilith: the mean lunar apogee.
        Lilith => "Lilith",
        /// The degree of the ecliptic rising in the east.
        Ascendant => "Ascendant",
        /// The degree of the ecliptic culminating on the meridian.
        Midheaven => "Midheaven",
        /// The degree setting in the west, opposite the Ascendant.
        Descendant => "Descendant",
        /// The degree opposite the Midheaven.
        ImumCoeli => "Imum Coeli",
    ]
);

impl Point {
    /// The point's symbol, such as ☉ for the Sun and "Asc" for the Ascendant. The mean nodes
    /// share the true nodes' symbols.
    pub fn symbol(self) -> &'static str {
        [
            "☉", "☽", "☿", "♀", "♂", "♃", "♄", "♅", "♆", "♇", "☊", "☋", "☊", "☋", "⚸", "Asc", "MC",
            "Dsc", "IC",
        ][self as usize]
    }

    /// Whether [`positions`] can give this point in this build: the seven planets, the true
    /// and mean nodes, Lilith and the four angles always. Uranus, Neptune and Pluto only with
    /// the `outer-planets` feature, and, in a build without `embedded`, once
    /// `astrometry::slice::install_outer` has been given their file; a source that has no
    /// data for them makes [`positions`] fail with the ephemeris' error, not
    /// [`Error::Unavailable`].
    pub fn available(self) -> bool {
        cfg!(feature = "outer-planets")
            || !matches!(self, Point::Uranus | Point::Neptune | Point::Pluto)
    }

    /// The corresponding [`crate::Point`], for the planets and the true nodes.
    fn base(self) -> Option<crate::Point> {
        match self {
            Point::NorthNode => Some(crate::Point::NorthNode),
            Point::SouthNode => Some(crate::Point::SouthNode),
            p if p.index() < 7 => Some(crate::Point::ALL[p.index()]),
            _ => None,
        }
    }

    /// The ephemeris body of a planet.
    fn body(self) -> Option<Body> {
        Some(match self {
            Point::Sun => Body::Sun,
            Point::Moon => Body::Moon,
            Point::Mercury => Body::Mercury,
            Point::Venus => Body::Venus,
            Point::Mars => Body::Mars,
            Point::Jupiter => Body::Jupiter,
            Point::Saturn => Body::Saturn,
            Point::Uranus => Body::Uranus,
            Point::Neptune => Body::Neptune,
            Point::Pluto => Body::Pluto,
            _ => return None,
        })
    }

    /// Whether `a` and `b` are always exactly opposite: an angle and its opposite, or a node
    /// and the node opposite it.
    fn fixed_pair(a: Point, b: Point) -> bool {
        use Point::*;
        [
            (Ascendant, Descendant),
            (Midheaven, ImumCoeli),
            (NorthNode, SouthNode),
            (MeanNorthNode, MeanSouthNode),
        ]
        .iter()
        .any(|&(x, y)| (a, b) == (x, y) || (a, b) == (y, x))
    }
}

impl From<crate::Point> for Point {
    fn from(p: crate::Point) -> Point {
        match p {
            crate::Point::NorthNode => Point::NorthNode,
            crate::Point::SouthNode => Point::SouthNode,
            p => Point::ALL[p.index()],
        }
    }
}

named_enum!(
    /// A kind of aspect: the Ptolemaic five, then the minor ones. The semi-square is also
    /// called the octile.
    Kind, "chart aspect", [
        Conjunction => "Conjunction", Sextile => "Sextile", Square => "Square",
        Trine => "Trine", Opposition => "Opposition",
        SemiSextile => "Semi-sextile", SemiSquare => "Semi-square",
        Sesquiquadrate => "Sesquiquadrate", Quincunx => "Quincunx",
        Quintile => "Quintile", BiQuintile => "Bi-quintile", Septile => "Septile",
        Novile => "Novile",
    ]
);

impl Kind {
    /// The aspect's angle, degrees.
    pub fn angle(self) -> f64 {
        [
            0.0,
            60.0,
            90.0,
            120.0,
            180.0,
            30.0,
            45.0,
            135.0,
            150.0,
            72.0,
            144.0,
            360.0 / 7.0,
            40.0,
        ][self as usize]
    }

    /// The aspect's symbol, such as □. The quintiles, septile and novile have no standard
    /// glyph, so they are letters: "Q", "bQ", "S" and "N".
    pub fn symbol(self) -> &'static str {
        [
            "☌", "⚹", "□", "△", "☍", "⚺", "∠", "⚼", "⚻", "Q", "bQ", "S", "N",
        ][self as usize]
    }

    /// Whether this is one of the five Ptolemaic aspects.
    pub fn is_major(self) -> bool {
        self.index() < 5
    }
}

impl From<crate::aspect::Kind> for Kind {
    fn from(k: crate::aspect::Kind) -> Kind {
        Kind::ALL[k.index()]
    }
}

/// Where a point is, and its motion.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Position {
    pub point: Point,
    /// The ecliptic longitude, degrees, 0–360 from 0° Aries.
    pub longitude: f64,
    /// The daily motion in longitude, degrees; negative is retrograde.
    pub speed: f64,
}

/// An aspect between two points of a chart.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Aspect {
    /// The two points, in the order given to [`aspects`].
    pub points: [Point; 2],
    pub kind: Kind,
    /// How far the points are from the exact aspect, degrees.
    pub orb: f64,
    /// The actual separation of the points, degrees, 0–180.
    pub angle: f64,
    /// Becoming exact, rather than separating.
    pub applying: bool,
}

/// What can go wrong in a chart.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The coordinates, or the ephemeris.
    Horae(crate::Error),
    /// A point this release can't give: see [`Point::available`].
    Unavailable(Point),
    /// A house system that doesn't exist this far from the equator: Placidus and Koch need the
    /// latitude to be inside the polar circle, `limit` degrees, the complement of the obliquity.
    Polar {
        /// The latitude asked for, degrees.
        latitude: f64,
        /// The latitude it must be inside, degrees.
        limit: f64,
        /// The house system asked for.
        system: HouseSystem,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Horae(e) => e.fmt(f),
            Error::Unavailable(p) => write!(f, "horae: {} isn't available in this build", p.name()),
            Error::Polar {
                latitude,
                limit,
                system,
            } => write!(
                f,
                "horae: {} houses aren't defined at latitude {latitude}°, beyond {limit:.2}°",
                system.name()
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Horae(e) => Some(e),
            Error::Unavailable(_) | Error::Polar { .. } => None,
        }
    }
}

impl From<crate::Error> for Error {
    fn from(e: crate::Error) -> Error {
        Error::Horae(e)
    }
}

impl From<EphemerisError> for Error {
    fn from(e: EphemerisError) -> Error {
        Error::Horae(e.into())
    }
}

/// The positions of `points`, in that order, at TT Julian date `tt` at latitude `lat` and
/// longitude `lon`, degrees, north and east positive.
///
/// The planets and true nodes are [`crate::Position::all`]'s, bit for bit. The angles need no
/// ephemeris; their speeds are taken over a minute either side. The mean nodes and Lilith (the
/// mean lunar apogee) need none either: they come from `astrometry::derived`'s polynomials. A point that isn't
/// [`Point::available`] gives [`Error::Unavailable`], the first such in `points`.
pub fn positions(
    src: &dyn Source,
    tt: f64,
    lat: f64,
    lon: f64,
    points: &[Point],
) -> Result<Vec<Position>, Error> {
    check_coordinates(lat, lon)?;
    if let Some(&p) = points.iter().find(|p| !p.available()) {
        return Err(Error::Unavailable(p));
    }
    let all = if points.iter().any(|p| p.base().is_some()) {
        Some(crate::Position::all(src, tt)?)
    } else {
        None
    };
    // The outer planets are the ephemeris' own, as `crate::Position::at` takes the planets.
    let mut outer = Vec::new();
    for &point in points {
        let body = match point {
            Point::Uranus => Body::Uranus,
            Point::Neptune => Body::Neptune,
            Point::Pluto => Body::Pluto,
            _ => continue,
        };
        let longitude = astrometry::apparent::apparent(src, body, tt)?.longitude;
        outer.push((
            point,
            (longitude, astrometry::derived::speed(src, body, tt)?),
        ));
    }
    let now = angles(tt, lat, lon);
    let before = angles(tt - ANGLE_STEP, lat, lon);
    let after = angles(tt + ANGLE_STEP, lat, lon);
    let speed = |a: f64, b: f64| diff(a, b) / (2.0 * ANGLE_STEP);
    let opposite = |l: f64| (l + 180.0).rem_euclid(360.0);
    let asc_speed = speed(after.ascendant, before.ascendant);
    let mc_speed = speed(after.midheaven, before.midheaven);
    Ok(points
        .iter()
        .map(|&point| {
            let (longitude, speed) = match point {
                Point::Ascendant => (now.ascendant, asc_speed),
                Point::Descendant => (opposite(now.ascendant), asc_speed),
                Point::Midheaven => (now.midheaven, mc_speed),
                Point::ImumCoeli => (opposite(now.midheaven), mc_speed),
                Point::MeanNorthNode => (mean_node(tt), mean_node_speed(tt)),
                Point::MeanSouthNode => (opposite(mean_node(tt)), mean_node_speed(tt)),
                Point::Lilith => (mean_apogee(tt), mean_apogee_speed(tt)),
                Point::Uranus | Point::Neptune | Point::Pluto => {
                    outer
                        .iter()
                        .find(|(p, ..)| *p == point)
                        .expect("computed")
                        .1
                }
                _ => {
                    let base = point.base().expect("available and not an angle");
                    let p = all.as_ref().expect("computed for a base point")[base.index()];
                    (p.longitude, p.speed)
                }
            };
            Position {
                point,
                longitude,
                speed,
            }
        })
        .collect())
}

/// The aspects among `positions` of the given `kinds`, closest to exact first.
///
/// Every pair `i < j` is checked, in order, so an aspect's `points` are
/// `[positions[i].point, positions[j].point]`, for every kind in `kinds`. `orb` gives the orb,
/// degrees, for the two points and the kind; `None` skips them. horae has no default orbs: the
/// app owns them. The pair is in aspect if its separation is within `orb` of the kind's angle.
/// A pair can match several kinds when the orbs overlap, and every match is returned.
///
/// Pairs that are always exactly opposite are never checked: the Ascendant and Descendant, the
/// Midheaven and Imum Coeli, the North and South Nodes, and the Mean North and South Nodes.
///
/// An aspect is applying if the points' motion over a minute brings it closer to exact. One
/// that is exact and not moving isn't.
pub fn aspects(
    positions: &[Position],
    kinds: &[Kind],
    orb: impl Fn(Point, Point, Kind) -> Option<f64>,
) -> Vec<Aspect> {
    let mut out = Vec::new();
    for (i, a) in positions.iter().enumerate() {
        for b in &positions[i + 1..] {
            if Point::fixed_pair(a.point, b.point) {
                continue;
            }
            let apart = separation(a.longitude, b.longitude);
            let later = separation(
                a.longitude + a.speed * ANGLE_STEP,
                b.longitude + b.speed * ANGLE_STEP,
            );
            for &kind in kinds {
                let Some(limit) = orb(a.point, b.point, kind) else {
                    continue;
                };
                let off = (apart - kind.angle()).abs();
                if off <= limit {
                    out.push(Aspect {
                        points: [a.point, b.point],
                        kind,
                        orb: off,
                        angle: apart,
                        applying: (later - kind.angle()).abs() < off,
                    });
                }
            }
        }
    }
    out.sort_by(|x, y| x.orb.total_cmp(&y.orb));
    out
}

named_enum!(
    /// A kind of parallel: two points at the same declination, or at opposite ones.
    ParallelKind, "parallel", [
        /// The same declination, on the same side of the equator.
        Parallel => "Parallel",
        /// The same distance from the equator, on opposite sides.
        Contraparallel => "Contraparallel",
    ]
);

impl ParallelKind {
    /// The kind's symbol: ∥ for a parallel, ⋕ for a contraparallel (which has no standard
    /// glyph; it is drawn as a parallel with a stroke across it, or by this sign).
    pub fn symbol(self) -> &'static str {
        ["∥", "⋕"][self as usize]
    }
}

/// Where a point is north or south of the celestial equator, and its motion.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Declination {
    pub point: Point,
    /// The declination, degrees, positive north.
    pub declination: f64,
    /// The daily motion in declination, degrees; negative is southward.
    pub speed: f64,
    /// Farther from the equator than the true obliquity of the ecliptic of the date: beyond
    /// the Sun's greatest declination.
    pub out_of_bounds: bool,
}

/// A parallel or contraparallel between two points of a chart.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Parallel {
    /// The two points, in the order given to [`parallels`].
    pub points: [Point; 2],
    pub kind: ParallelKind,
    /// How far the points are from exact, degrees: the difference of the declinations for a
    /// parallel, their sum for a contraparallel, without sign.
    pub orb: f64,
    /// Becoming exact, rather than separating.
    pub applying: bool,
}

/// The declination of `point`, degrees, at TT Julian date `t`, for the points that aren't
/// bodies of the ephemeris: the nodes and the angles (on the ecliptic, with latitude 0) and
/// Lilith. The true nodes alone need the ephemeris.
fn ecliptic_point_declination(
    src: &dyn Source,
    point: Point,
    t: f64,
    lat: f64,
    lon: f64,
) -> Result<f64, Error> {
    let obliquity = astrometry::houses::ramc_and_obliquity(t, lon).1;
    let on_ecliptic = |longitude: f64| ecliptic_declination(longitude, 0.0, obliquity);
    // An opposite point is exactly the negative of its partner.
    Ok(match point {
        Point::NorthNode => on_ecliptic(true_node(src, t)?),
        Point::SouthNode => -on_ecliptic(true_node(src, t)?),
        Point::MeanNorthNode => on_ecliptic(mean_node(t)),
        Point::MeanSouthNode => -on_ecliptic(mean_node(t)),
        Point::Lilith => ecliptic_declination(mean_apogee(t), mean_apogee_latitude(t), obliquity),
        Point::Ascendant => on_ecliptic(angles(t, lat, lon).ascendant),
        Point::Descendant => -on_ecliptic(angles(t, lat, lon).ascendant),
        Point::Midheaven => on_ecliptic(angles(t, lat, lon).midheaven),
        Point::ImumCoeli => -on_ecliptic(angles(t, lat, lon).midheaven),
        _ => unreachable!("{point} is a body of the ephemeris"),
    })
}

/// The declinations of `points`, in that order, at TT Julian date `tt` at latitude `lat` and
/// longitude `lon`, degrees, north and east positive.
///
/// The planets' are apparent, from the same direction as their longitudes, on the true equator
/// of date; their speeds are central differences over 0.01 day either side. The nodes (true
/// and mean) and the angles are on the ecliptic, with latitude 0, so their declinations follow
/// from their longitudes and the true obliquity; the South Node's is the negative of the North
/// Node's, and likewise for the Descendant and the IC. Lilith has the latitude of the mean
/// apogee ([`astrometry::derived::mean_apogee_latitude`]). The speeds of the nodes and Lilith
/// are central differences over 0.01 day either side, and the angles' over a minute, as their
/// longitudes' are.
///
/// A point is out of bounds if its declination is beyond the true obliquity of the date, in
/// either direction. Only the planets and the true nodes need the ephemeris. A point that
/// isn't [`Point::available`] gives [`Error::Unavailable`], the first such in `points`.
pub fn declinations(
    src: &dyn Source,
    tt: f64,
    lat: f64,
    lon: f64,
    points: &[Point],
) -> Result<Vec<Declination>, Error> {
    check_coordinates(lat, lon)?;
    if let Some(&p) = points.iter().find(|p| !p.available()) {
        return Err(Error::Unavailable(p));
    }
    let bound = astrometry::houses::ramc_and_obliquity(tt, lon)
        .1
        .to_degrees();
    points
        .iter()
        .map(|&point| {
            let (declination, speed) = if let Some(body) = point.body() {
                (
                    body_declination(src, body, tt)?,
                    body_declination_speed(src, body, tt)?,
                )
            } else {
                let step = match point {
                    Point::Ascendant | Point::Descendant | Point::Midheaven | Point::ImumCoeli => {
                        ANGLE_STEP
                    }
                    _ => STEP,
                };
                let at = |t| ecliptic_point_declination(src, point, t, lat, lon);
                (at(tt)?, (at(tt + step)? - at(tt - step)?) / (2.0 * step))
            };
            Ok(Declination {
                point,
                declination,
                speed,
                out_of_bounds: declination.abs() > bound,
            })
        })
        .collect()
}

/// The parallels and contraparallels among `declinations` of the given `kinds`, closest to
/// exact first.
///
/// Every pair `i < j` is checked, in order, so a result's `points` are
/// `[declinations[i].point, declinations[j].point]`, for every kind in `kinds`. A parallel is
/// a difference of the declinations of at most `orb` degrees; a contraparallel is a sum of at
/// most `orb`. `orb` gives it for the two points and the kind; `None` skips them. horae has no
/// default orbs: the app owns them. A pair can match both kinds when the orb is wide enough
/// (both near the equator), and both are returned.
///
/// Pairs that are always exactly opposite, and so always in contraparallel, are never checked:
/// the Ascendant and Descendant, the Midheaven and Imum Coeli, the North and South Nodes, and
/// the Mean North and South Nodes.
///
/// A parallel is applying if the points' motion over a minute brings it closer to exact. One
/// that is exact and not moving isn't. Ties keep the order of the pairs.
pub fn parallels(
    declinations: &[Declination],
    kinds: &[ParallelKind],
    orb: impl Fn(Point, Point, ParallelKind) -> Option<f64>,
) -> Vec<Parallel> {
    let mut out = Vec::new();
    for (i, a) in declinations.iter().enumerate() {
        for b in &declinations[i + 1..] {
            if Point::fixed_pair(a.point, b.point) {
                continue;
            }
            let (a2, b2) = (
                a.declination + a.speed * ANGLE_STEP,
                b.declination + b.speed * ANGLE_STEP,
            );
            for &kind in kinds {
                let Some(limit) = orb(a.point, b.point, kind) else {
                    continue;
                };
                let off = |x: f64, y: f64| match kind {
                    ParallelKind::Parallel => (x - y).abs(),
                    ParallelKind::Contraparallel => (x + y).abs(),
                };
                let now = off(a.declination, b.declination);
                if now <= limit {
                    out.push(Parallel {
                        points: [a.point, b.point],
                        kind,
                        orb: now,
                        applying: off(a2, b2) < now,
                    });
                }
            }
        }
    }
    out.sort_by(|x, y| x.orb.total_cmp(&y.orb));
    out
}

named_enum!(
    /// A way of dividing the sky into twelve houses.
    HouseSystem, "house system", [
        WholeSign => "Whole Sign", Equal => "Equal", Placidus => "Placidus", Koch => "Koch",
    ]
);

named_enum!(
    /// A sidereal zodiac, by its ayanamsa: the distance of its 0° from the tropical 0°.
    Ayanamsa, "ayanamsa", [
        FaganBradley => "Fagan-Bradley", Lahiri => "Lahiri",
    ]
);

// By hand: `named_enum!` doesn't derive `Default`.
#[allow(clippy::derivable_impls)]
impl Default for Ayanamsa {
    fn default() -> Ayanamsa {
        Ayanamsa::FaganBradley
    }
}

impl Ayanamsa {
    /// The ayanamsa at TT Julian date `tt`, degrees. It includes the nutation in longitude, so
    /// a sidereal longitude is the apparent tropical longitude minus it.
    pub fn at(self, tt: f64) -> f64 {
        match self {
            Ayanamsa::FaganBradley => astrometry::ayanamsa::fagan_bradley(tt),
            Ayanamsa::Lahiri => astrometry::ayanamsa::lahiri(tt),
        }
    }
}

/// The zodiac longitudes are measured in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Zodiac {
    /// From the equinox of date.
    #[default]
    Tropical,
    /// From a point fixed among the stars, as set by the ayanamsa.
    Sidereal(Ayanamsa),
}

/// The twelve houses of a chart.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Houses {
    pub system: HouseSystem,
    /// The cusps, degrees 0–360 in the chosen zodiac; index 0 is the 1st house.
    pub cusps: [f64; 12],
}

impl Houses {
    /// The house, 1–12, that `longitude` (degrees, in the same zodiac) falls in: the one whose
    /// cusp is the last at or before it, going forward through the zodiac.
    pub fn house_of(&self, longitude: f64) -> u8 {
        let mut best = (f64::INFINITY, 0);
        for (i, cusp) in self.cusps.iter().enumerate() {
            let past = (longitude - cusp).rem_euclid(360.0);
            if past < best.0 {
                best = (past, i);
            }
        }
        best.1 as u8 + 1
    }
}

/// The house cusps at TT Julian date `tt`, at latitude `lat` and longitude `lon`, degrees,
/// north and east positive, in `zodiac`. No ephemeris is needed.
///
/// Cusps 1, 4, 7 and 10 of Placidus and Koch are the Ascendant, Imum Coeli, Descendant and
/// Midheaven of [`positions`]. Whole Sign and Equal work at every valid latitude, as far as
/// the Ascendant does (see [`crate::angles`]); Placidus and Koch are defined only inside the
/// polar circle, where the latitude is less than 90° minus the obliquity, about 66.56°, and
/// give [`Error::Polar`] beyond.
///
/// In the sidereal zodiac the ayanamsa is taken from the cusps; Whole Sign houses are the
/// signs of the *sidereal* Ascendant's, from 0° of its sign.
pub fn houses(
    tt: f64,
    lat: f64,
    lon: f64,
    system: HouseSystem,
    zodiac: Zodiac,
) -> Result<Houses, Error> {
    use astrometry::houses as h;
    check_coordinates(lat, lon)?;
    let shift = match zodiac {
        Zodiac::Tropical => 0.0,
        Zodiac::Sidereal(a) => a.at(tt),
    };
    let (ramc, obliquity) = h::ramc_and_obliquity(tt, lon);
    let latitude = lat.to_radians();
    let ascendant = astrometry::angles::from_ramc(ramc, obliquity, latitude).ascendant;
    let cusps = match system {
        HouseSystem::WholeSign => Some(h::whole_sign((ascendant - shift).rem_euclid(360.0))),
        HouseSystem::Equal => Some(h::equal(ascendant)),
        HouseSystem::Placidus => h::placidus(ramc, obliquity, latitude),
        HouseSystem::Koch => h::koch(ramc, obliquity, latitude),
    };
    let Some(mut cusps) = cusps else {
        return Err(Error::Polar {
            latitude: lat,
            limit: h::polar_limit(obliquity),
            system,
        });
    };
    if system != HouseSystem::WholeSign {
        for c in &mut cusps {
            *c = (*c - shift).rem_euclid(360.0);
        }
    }
    Ok(Houses { system, cusps })
}

#[cfg(test)]
mod houses_tests {
    use super::*;
    use astrometry::source::Builtin;

    const TT: f64 = 2461328.25;
    const PLACE: (f64, f64) = (43.07, -89.4);

    fn cast(system: HouseSystem, zodiac: Zodiac) -> Houses {
        houses(TT, PLACE.0, PLACE.1, system, zodiac).unwrap()
    }

    #[test]
    fn angles_are_the_charts() {
        use Point::*;
        let got = positions(
            &Builtin,
            TT,
            PLACE.0,
            PLACE.1,
            &[Ascendant, ImumCoeli, Descendant, Midheaven],
        )
        .unwrap();
        for system in [HouseSystem::Placidus, HouseSystem::Koch] {
            let h = cast(system, Zodiac::Tropical);
            for (cusp, p) in [0, 3, 6, 9].into_iter().zip([0, 1, 2, 3]) {
                assert_eq!(
                    h.cusps[cusp],
                    got[p].longitude,
                    "{system} cusp {}",
                    cusp + 1
                );
            }
        }
        let e = cast(HouseSystem::Equal, Zodiac::Tropical);
        assert_eq!(e.cusps[0], got[0].longitude);
    }

    #[test]
    fn systems_are_ordered_and_equal_spaced() {
        for system in HouseSystem::ALL {
            let h = cast(system, Zodiac::Tropical);
            assert_eq!(h.system, system);
            let mut total = 0.0;
            for i in 0..12 {
                let step = (h.cusps[(i + 1) % 12] - h.cusps[i]).rem_euclid(360.0);
                assert!(
                    step > 0.0 && step < 90.0,
                    "{system} house {}: {step}",
                    i + 1
                );
                total += step;
                if matches!(system, HouseSystem::Equal | HouseSystem::WholeSign) {
                    assert!((step - 30.0).abs() < 1e-9);
                }
            }
            assert!((total - 360.0).abs() < 1e-9);
        }
        let w = cast(HouseSystem::WholeSign, Zodiac::Tropical);
        assert!(w.cusps.iter().all(|c| c % 30.0 == 0.0));
    }

    #[test]
    fn house_of_with_wrap() {
        let h = Houses {
            system: HouseSystem::Equal,
            cusps: std::array::from_fn(|i| (345.0 + 30.0 * i as f64) % 360.0),
        };
        // The 1st house runs from 345° to 15°.
        assert_eq!(h.house_of(345.0), 1);
        assert_eq!(h.house_of(359.9), 1);
        assert_eq!(h.house_of(0.0), 1);
        assert_eq!(h.house_of(14.99), 1);
        assert_eq!(h.house_of(15.0), 2);
        assert_eq!(h.house_of(344.99), 12);
        assert_eq!(h.house_of(-10.0), 1);
        let p = cast(HouseSystem::Placidus, Zodiac::Tropical);
        for i in 0..12 {
            assert_eq!(p.house_of(p.cusps[i]), i as u8 + 1);
            assert_eq!(p.house_of(p.cusps[i] + 0.001), i as u8 + 1);
        }
    }

    #[test]
    fn sidereal_shift() {
        let aya = Ayanamsa::Lahiri;
        for system in [HouseSystem::Equal, HouseSystem::Placidus, HouseSystem::Koch] {
            let t = cast(system, Zodiac::Tropical);
            let s = cast(system, Zodiac::Sidereal(aya));
            for i in 0..12 {
                let d = (t.cusps[i] - s.cusps[i]).rem_euclid(360.0);
                assert!((d - aya.at(TT)).abs() < 1e-9, "{system} cusp {}", i + 1);
            }
        }
        // Whole Sign: the sign of the sidereal Ascendant, not the tropical one's.
        let t = cast(HouseSystem::Equal, Zodiac::Tropical).cusps[0];
        let s = cast(HouseSystem::Equal, Zodiac::Sidereal(aya)).cusps[0];
        let w = cast(HouseSystem::WholeSign, Zodiac::Sidereal(aya));
        assert_eq!(w.cusps[0], (s / 30.0).floor() * 30.0);
        assert!(aya.at(TT) > 20.0 && t != s);
        assert_eq!(w.cusps[0] % 30.0, 0.0);
    }

    #[test]
    fn sidereal_whole_sign_crosses_a_sign() {
        // Over a day the Ascendant passes every sign: find a moment where the tropical and
        // sidereal signs differ, and check the sidereal one is the sidereal Ascendant's.
        let mut differed = false;
        for k in 0..48 {
            let tt = TT + f64::from(k) / 48.0;
            let z = Zodiac::Sidereal(Ayanamsa::FaganBradley);
            let (t, s) = (
                houses(tt, 40.0, 0.0, HouseSystem::WholeSign, Zodiac::Tropical).unwrap(),
                houses(tt, 40.0, 0.0, HouseSystem::WholeSign, z).unwrap(),
            );
            let sid_asc = houses(tt, 40.0, 0.0, HouseSystem::Equal, z).unwrap().cusps[0];
            assert_eq!(s.cusps[0], (sid_asc / 30.0).floor() * 30.0);
            differed |= t.cusps[0] != s.cusps[0];
        }
        assert!(differed);
    }

    #[test]
    fn polar_errors() {
        for system in [HouseSystem::Placidus, HouseSystem::Koch] {
            assert!(houses(TT, 66.0, 0.0, system, Zodiac::Tropical).is_ok());
            for lat in [67.0, -67.0] {
                let e = houses(TT, lat, 0.0, system, Zodiac::Tropical).unwrap_err();
                let Error::Polar {
                    latitude, limit, ..
                } = e
                else {
                    panic!("{e:?}")
                };
                assert_eq!(latitude, lat);
                assert!((limit - 66.56).abs() < 0.01);
                assert_eq!(
                    e.to_string(),
                    format!(
                        "horae: {system} houses aren't defined at latitude {lat}°, beyond 66.56°"
                    )
                );
            }
        }
        for system in [HouseSystem::Equal, HouseSystem::WholeSign] {
            let h = houses(TT, 80.0, 0.0, system, Zodiac::Tropical).unwrap();
            assert!(h.cusps.iter().all(|c| (0.0..360.0).contains(c)));
        }
    }

    #[test]
    fn bad_coordinates() {
        for (lat, lon) in [(91.0, 0.0), (0.0, 181.0), (f64::NAN, 0.0)] {
            let e = houses(TT, lat, lon, HouseSystem::Koch, Zodiac::Tropical).unwrap_err();
            assert!(matches!(
                e,
                Error::Horae(crate::Error::InvalidCoordinates(..))
            ));
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn zodiac_serde_round_trips() {
        for z in [
            Zodiac::Tropical,
            Zodiac::Sidereal(Ayanamsa::FaganBradley),
            Zodiac::Sidereal(Ayanamsa::Lahiri),
        ] {
            let json = serde_json::to_string(&z).unwrap();
            assert_eq!(serde_json::from_str::<Zodiac>(&json).unwrap(), z);
        }
        let h = cast(HouseSystem::Placidus, Zodiac::Tropical);
        let back: Houses = serde_json::from_str(&serde_json::to_string(&h).unwrap()).unwrap();
        assert_eq!(back, h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrometry::source::Builtin;

    const TT: f64 = 2461328.25;

    fn pos(point: Point, longitude: f64, speed: f64) -> Position {
        Position {
            point,
            longitude,
            speed,
        }
    }

    fn flat(orb: f64) -> impl Fn(Point, Point, Kind) -> Option<f64> {
        move |_, _, _| Some(orb)
    }

    #[test]
    fn wraps_at_zero() {
        let p = [pos(Point::Sun, 359.0, 0.0), pos(Point::Moon, 1.0, 0.0)];
        let got = aspects(&p, &[Kind::Conjunction], flat(3.0));
        assert_eq!(got.len(), 1);
        assert!((got[0].orb - 2.0).abs() < 1e-9 && (got[0].angle - 2.0).abs() < 1e-9);
        // A sextile across the wrap: 350° and 50°.
        let p = [pos(Point::Sun, 350.0, 0.0), pos(Point::Moon, 50.0, 0.0)];
        let got = aspects(&p, &[Kind::Sextile], flat(0.5));
        assert_eq!(got.len(), 1);
        assert!(got[0].orb < 1e-9);
    }

    #[test]
    fn pair_override_is_tighter() {
        let p = [pos(Point::Sun, 0.0, 0.0), pos(Point::Moon, 94.0, 0.0)];
        assert_eq!(aspects(&p, &[Kind::Square], flat(5.0)).len(), 1);
        let tight = |a, b, _| {
            if (a, b) == (Point::Sun, Point::Moon) {
                Some(3.0)
            } else {
                Some(5.0)
            }
        };
        assert!(aspects(&p, &[Kind::Square], tight).is_empty());
    }

    #[test]
    fn none_skips() {
        let p = [
            pos(Point::Sun, 0.0, 0.0),
            pos(Point::Moon, 90.0, 0.0),
            pos(Point::Mars, 180.0, 0.0),
        ];
        let orb = |a, _, k| {
            if a == Point::Sun || k == Kind::Opposition {
                None
            } else {
                Some(1.0)
            }
        };
        // Only the Moon and Mars square remains: the Sun's pairs and the opposition skipped.
        let got = aspects(&p, &Kind::ALL, orb);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].points, [Point::Moon, Point::Mars]);
        assert_eq!(got[0].kind, Kind::Square);
    }

    #[test]
    fn applying_and_separating() {
        // The Moon, faster, approaching a conjunction with the Sun from behind.
        let p = [pos(Point::Sun, 10.0, 1.0), pos(Point::Moon, 8.0, 13.0)];
        assert!(aspects(&p, &[Kind::Conjunction], flat(3.0))[0].applying);
        // Past it.
        let p = [pos(Point::Sun, 10.0, 1.0), pos(Point::Moon, 12.0, 13.0)];
        assert!(!aspects(&p, &[Kind::Conjunction], flat(3.0))[0].applying);
        // Moving apart across the wrap.
        let p = [pos(Point::Sun, 359.0, 1.0), pos(Point::Moon, 358.0, -1.0)];
        assert!(!aspects(&p, &[Kind::Conjunction], flat(3.0))[0].applying);
        // A fast angle (about 360°/day) overtaking a planet, and having passed it.
        let p = [
            pos(Point::Ascendant, 99.0, 360.0),
            pos(Point::Sun, 100.0, 1.0),
        ];
        assert!(aspects(&p, &[Kind::Conjunction], flat(3.0))[0].applying);
        let p = [
            pos(Point::Ascendant, 101.0, 360.0),
            pos(Point::Sun, 100.0, 1.0),
        ];
        assert!(!aspects(&p, &[Kind::Conjunction], flat(3.0))[0].applying);
        // Stationary and exact.
        let p = [pos(Point::Sun, 10.0, 0.0), pos(Point::Moon, 70.0, 0.0)];
        assert!(!aspects(&p, &[Kind::Sextile], flat(1.0))[0].applying);
    }

    #[test]
    fn fixed_pairs_skipped() {
        use Point::*;
        let p = [
            pos(Ascendant, 10.0, 0.0),
            pos(Descendant, 190.0, 0.0),
            pos(Midheaven, 280.0, 0.0),
            pos(ImumCoeli, 100.0, 0.0),
            pos(NorthNode, 50.0, 0.0),
            pos(SouthNode, 230.0, 0.0),
            pos(MeanNorthNode, 60.0, 0.0),
            pos(MeanSouthNode, 240.0, 0.0),
        ];
        for a in aspects(&p, &[Kind::Opposition], flat(0.1)) {
            assert!(!Point::fixed_pair(a.points[0], a.points[1]), "{a:?}");
        }
        // Reversed order is skipped too.
        let p = [pos(Descendant, 190.0, 0.0), pos(Ascendant, 10.0, 0.0)];
        assert!(aspects(&p, &[Kind::Opposition], flat(1.0)).is_empty());
        let p = [pos(SouthNode, 230.0, 0.0), pos(NorthNode, 50.0, 0.0)];
        assert!(aspects(&p, &[Kind::Opposition], flat(1.0)).is_empty());
        // Other pairs, exactly opposite, still count.
        let p = [pos(Ascendant, 10.0, 0.0), pos(ImumCoeli, 190.0, 0.0)];
        assert_eq!(aspects(&p, &[Kind::Opposition], flat(1.0)).len(), 1);
    }

    #[test]
    fn overlapping_kinds_both_returned() {
        // 42.5° apart: 2.5° from the semi-square and from the novile.
        let p = [pos(Point::Sun, 0.0, 0.0), pos(Point::Moon, 42.5, 0.0)];
        let got = aspects(&p, &[Kind::SemiSquare, Kind::Novile], flat(3.0));
        assert_eq!(got.len(), 2);
        assert!(got.iter().any(|a| a.kind == Kind::SemiSquare));
        assert!(got.iter().any(|a| a.kind == Kind::Novile));
    }

    #[test]
    fn sorted_closest_first() {
        let p = [
            pos(Point::Sun, 0.0, 0.0),
            pos(Point::Moon, 63.0, 0.0),
            pos(Point::Mars, 120.5, 0.0),
            pos(Point::Venus, 1.0, 0.0),
        ];
        let got = aspects(&p, &Kind::ALL, flat(4.0));
        assert!(got.len() >= 3);
        assert!(got.windows(2).all(|w| w[0].orb <= w[1].orb));
    }

    #[test]
    fn exact_has_zero_orb() {
        let p = [pos(Point::Sun, 10.0, 1.0), pos(Point::Jupiter, 130.0, 0.1)];
        let got = aspects(&p, &[Kind::Trine], flat(0.0));
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].orb, 0.0);
        assert_eq!(got[0].angle, 120.0);
    }

    #[test]
    fn positions_match_position_all() {
        let all = crate::Position::all(&Builtin, TT).unwrap();
        let got = positions(&Builtin, TT, 43.07, -89.4, &Point::ALL[..7]).unwrap();
        for (g, a) in got.iter().zip(&all) {
            assert_eq!(g.longitude.to_bits(), a.longitude.to_bits());
            assert_eq!(g.speed.to_bits(), a.speed.to_bits());
        }
        let got = positions(
            &Builtin,
            TT,
            43.07,
            -89.4,
            &[Point::SouthNode, Point::NorthNode],
        )
        .unwrap();
        for (g, base) in got
            .iter()
            .zip([crate::Point::SouthNode, crate::Point::NorthNode])
        {
            let a = all[base.index()];
            assert_eq!(g.longitude.to_bits(), a.longitude.to_bits());
            assert_eq!(g.speed.to_bits(), a.speed.to_bits());
        }
    }

    #[test]
    fn angles_positions() {
        use Point::*;
        let (lat, lon) = (43.07, -89.4);
        let got = positions(
            &Builtin,
            TT,
            lat,
            lon,
            &[Descendant, Ascendant, ImumCoeli, Midheaven],
        )
        .unwrap();
        let want = angles(TT, lat, lon);
        let [dsc, asc, ic, mc] = got[..] else {
            panic!()
        };
        assert_eq!(asc.longitude, want.ascendant);
        assert_eq!(mc.longitude, want.midheaven);
        assert!((diff(dsc.longitude, asc.longitude).abs() - 180.0).abs() < 1e-9);
        assert!((diff(ic.longitude, mc.longitude).abs() - 180.0).abs() < 1e-9);
        assert!((0.0..360.0).contains(&dsc.longitude) && (0.0..360.0).contains(&ic.longitude));
        for p in got {
            assert!((300.0..1000.0).contains(&p.speed), "{p:?}");
        }
        assert_eq!(asc.speed, dsc.speed);
        assert_eq!(mc.speed, ic.speed);
    }

    #[test]
    fn angles_need_no_ephemeris() {
        use astrometry::source::Body;
        use astrometry::vector::Vec3;
        struct Never;
        impl Source for Never {
            fn state(&self, _: Body, _: f64) -> Result<(Vec3, Vec3), EphemerisError> {
                panic!("touched the ephemeris")
            }
        }
        let got = positions(
            &Never,
            TT,
            10.0,
            20.0,
            &[
                Point::Midheaven,
                Point::MeanNorthNode,
                Point::MeanSouthNode,
                Point::Lilith,
            ],
        )
        .unwrap();
        assert_eq!(got.len(), 4);
        // Nor do their declinations.
        let points = [Point::Midheaven, Point::MeanSouthNode, Point::Lilith];
        assert_eq!(
            declinations(&Never, TT, 10.0, 20.0, &points).unwrap().len(),
            3
        );
    }

    #[test]
    fn mean_points() {
        use Point::*;
        let got = positions(
            &Builtin,
            TT,
            43.07,
            -89.4,
            &[MeanNorthNode, MeanSouthNode, Lilith],
        )
        .unwrap();
        let [north, south, lilith] = got[..] else {
            panic!()
        };
        assert_eq!(north.longitude, mean_node(TT));
        assert_eq!(lilith.longitude, mean_apogee(TT));
        assert!((diff(south.longitude, north.longitude).abs() - 180.0).abs() < 1e-9);
        assert!((0.0..360.0).contains(&south.longitude));
        assert_eq!(north.speed, mean_node_speed(TT));
        assert_eq!(south.speed, north.speed);
        assert_eq!(lilith.speed, mean_apogee_speed(TT));
        assert!((north.speed + 0.053).abs() < 0.002, "{north:?}");
        assert!((lilith.speed - 0.111).abs() < 0.002, "{lilith:?}");
    }

    #[test]
    fn order_preserved() {
        use Point::*;
        let want = [Midheaven, Moon, SouthNode, Sun, Ascendant];
        let got = positions(&Builtin, TT, 43.0, -89.0, &want).unwrap();
        assert_eq!(got.iter().map(|p| p.point).collect::<Vec<_>>(), want);
    }

    #[cfg(feature = "outer-planets")]
    #[test]
    fn the_outer_planets_are_the_ephemerides_own() {
        use astrometry::apparent::apparent;
        use astrometry::derived::speed;
        let points = [Point::Pluto, Point::Sun, Point::Uranus, Point::Neptune];
        let bodies = [Body::Pluto, Body::Sun, Body::Uranus, Body::Neptune];
        let got = positions(&Builtin, TT, 43.07, -89.4, &points).unwrap();
        assert_eq!(got.iter().map(|p| p.point).collect::<Vec<_>>(), points);
        for (g, body) in got.iter().zip(bodies) {
            assert!(g.point.available());
            let lon = apparent(&Builtin, body, TT).unwrap().longitude;
            assert_eq!(g.longitude.to_bits(), lon.to_bits());
            let spd = speed(&Builtin, body, TT).unwrap();
            assert_eq!(g.speed.to_bits(), spd.to_bits());
        }
        // The three planets are in three places.
        assert!(got[0].longitude != got[2].longitude && got[2].longitude != got[3].longitude);
    }

    #[test]
    fn errors() {
        #[cfg(not(feature = "outer-planets"))]
        {
            let e = positions(
                &Builtin,
                TT,
                0.0,
                0.0,
                &[Point::Sun, Point::Uranus, Point::Pluto],
            )
            .unwrap_err();
            assert!(matches!(e, Error::Unavailable(Point::Uranus)));
            assert_eq!(e.to_string(), "horae: Uranus isn't available in this build");
        }
        let e = positions(&Builtin, TT, 91.0, 0.0, &[Point::Sun]).unwrap_err();
        assert!(matches!(
            e,
            Error::Horae(crate::Error::InvalidCoordinates(..))
        ));
    }

    #[test]
    fn conversions() {
        for p in crate::Point::ALL {
            assert_eq!(Point::from(p).name(), p.name());
            assert_eq!(Point::from(p).symbol(), p.symbol());
        }
        for k in crate::aspect::Kind::ALL {
            assert_eq!(Kind::from(k).name(), k.name());
            assert_eq!(Kind::from(k).angle(), k.angle());
            assert_eq!(Kind::from(k).symbol(), k.symbol());
        }
    }
}

#[cfg(test)]
mod declination_tests {
    use super::*;
    use astrometry::houses::ramc_and_obliquity;
    use astrometry::source::Builtin;

    const TT: f64 = 2461328.25;

    fn dec(point: Point, declination: f64, speed: f64) -> Declination {
        Declination {
            point,
            declination,
            speed,
            out_of_bounds: false,
        }
    }

    fn flat(orb: f64) -> impl Fn(Point, Point, ParallelKind) -> Option<f64> {
        move |_, _, _| Some(orb)
    }

    #[test]
    fn parallel_and_contraparallel() {
        let d = [
            dec(Point::Sun, 10.0, 0.0),
            dec(Point::Mars, 10.4, 0.0),
            dec(Point::Venus, -10.3, 0.0),
            dec(Point::Moon, 25.0, 0.0),
        ];
        let got = parallels(&d, &ParallelKind::ALL, flat(0.5));
        let seen: Vec<_> = got.iter().map(|p| (p.points, p.kind)).collect();
        // Closest first: Mars and Venus 0.1° from a contraparallel, the Sun and Venus 0.3°, and
        // the Sun and Mars 0.4° from a parallel. The Moon is in neither.
        assert_eq!(
            seen,
            [
                ([Point::Mars, Point::Venus], ParallelKind::Contraparallel),
                ([Point::Sun, Point::Venus], ParallelKind::Contraparallel),
                ([Point::Sun, Point::Mars], ParallelKind::Parallel),
            ]
        );
        for (p, want) in got.iter().zip([0.1, 0.3, 0.4]) {
            assert!((p.orb - want).abs() < 1e-12, "{} vs {want}", p.orb);
        }
        // Only the kinds asked for.
        let only = parallels(&d, &[ParallelKind::Parallel], flat(0.5));
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].points, [Point::Sun, Point::Mars]);
        // The limit is inclusive (the values are exact in binary).
        let exact = [dec(Point::Sun, 8.0, 0.0), dec(Point::Mars, 8.5, 0.0)];
        assert_eq!(
            parallels(&exact, &[ParallelKind::Parallel], flat(0.5)).len(),
            1
        );
        assert!(parallels(&exact, &[ParallelKind::Parallel], flat(0.49)).is_empty());
    }

    #[test]
    fn pair_orb_and_none() {
        let d = [
            dec(Point::Sun, 10.0, 0.0),
            dec(Point::Mars, 10.4, 0.0),
            dec(Point::Venus, 10.2, 0.0),
        ];
        // The Sun and Mars get a tighter orb than the rest; Venus and Mars are skipped.
        let orb = |a: Point, b: Point, _: ParallelKind| match (a, b) {
            (Point::Sun, Point::Mars) => Some(0.3),
            (Point::Mars, Point::Venus) => None,
            _ => Some(0.5),
        };
        let got = parallels(&d, &[ParallelKind::Parallel], orb);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].points, [Point::Sun, Point::Venus]);
        // A kind can be skipped without the other.
        let kind_orb = |_: Point, _: Point, k: ParallelKind| {
            (k == ParallelKind::Contraparallel).then_some(30.0)
        };
        let got = parallels(&d, &ParallelKind::ALL, kind_orb);
        // All three pairs sum to about 20°: within 30°, but only as contraparallels.
        assert_eq!(got.len(), 3);
        assert!(got.iter().all(|p| p.kind == ParallelKind::Contraparallel));
    }

    #[test]
    fn fixed_pairs_skipped() {
        use Point::*;
        // Each opposite pair is exactly in contraparallel; none is reported, in either order.
        let d = [
            dec(Ascendant, 12.0, 0.0),
            dec(Descendant, -12.0, 0.0),
            dec(Midheaven, 5.0, 0.0),
            dec(ImumCoeli, -5.0, 0.0),
            dec(NorthNode, 3.0, 0.0),
            dec(SouthNode, -3.0, 0.0),
            dec(MeanNorthNode, 4.0, 0.0),
            dec(MeanSouthNode, -4.0, 0.0),
        ];
        assert!(parallels(&d, &ParallelKind::ALL, flat(0.5)).is_empty());
        let mut r = d;
        r.reverse();
        assert!(parallels(&r, &ParallelKind::ALL, flat(0.5)).is_empty());
        // The true and mean nodes aren't each other's pair.
        let nodes = [dec(NorthNode, 3.0, 0.0), dec(MeanSouthNode, -3.1, 0.0)];
        assert_eq!(parallels(&nodes, &ParallelKind::ALL, flat(0.5)).len(), 1);
    }

    #[test]
    fn applying_and_separating() {
        let kind = [ParallelKind::Parallel];
        // 0.5° apart; the second falls 1°/day, or rises, toward or from the first.
        let apply = [dec(Point::Sun, 10.0, 0.0), dec(Point::Mars, 10.5, -1.0)];
        let sep = [dec(Point::Sun, 10.0, 0.0), dec(Point::Mars, 10.5, 1.0)];
        assert!(parallels(&apply, &kind, flat(1.0))[0].applying);
        assert!(!parallels(&sep, &kind, flat(1.0))[0].applying);
        // Exact and still: not applying.
        let still = [dec(Point::Sun, 10.0, 0.0), dec(Point::Mars, 10.0, 0.0)];
        assert!(!parallels(&still, &kind, flat(1.0))[0].applying);
        // For a contraparallel the sum is what shrinks: 10 and −10.5, the second rising.
        let con = [dec(Point::Sun, 10.0, 0.0), dec(Point::Mars, -10.5, 1.0)];
        let con_kind = [ParallelKind::Contraparallel];
        assert!(parallels(&con, &con_kind, flat(1.0))[0].applying);
        let con = [dec(Point::Sun, 10.0, 0.0), dec(Point::Mars, -10.5, -1.0)];
        assert!(!parallels(&con, &con_kind, flat(1.0))[0].applying);
    }

    #[test]
    fn sorted_and_stable() {
        let d = [
            dec(Point::Sun, 10.0, 0.0),
            dec(Point::Moon, 10.3, 0.0),
            dec(Point::Mars, 10.1, 0.0),
            dec(Point::Venus, 10.3, 0.0),
        ];
        let got = parallels(&d, &[ParallelKind::Parallel], flat(1.0));
        let orbs: Vec<f64> = got.iter().map(|p| p.orb).collect();
        assert!(orbs.windows(2).all(|w| w[0] <= w[1]), "{orbs:?}");
        // Moon and Venus are exact; ties keep the order of the pairs (Sun–Mars 0.1 and
        // Moon–Mars 0.2 and Mars–Venus 0.2 both).
        assert_eq!(got[0].points, [Point::Moon, Point::Venus]);
        let tied: Vec<_> = got
            .iter()
            .filter(|p| (p.orb - 0.2).abs() < 1e-9)
            .map(|p| p.points)
            .collect();
        assert_eq!(
            tied,
            [[Point::Moon, Point::Mars], [Point::Mars, Point::Venus]]
        );
    }

    // The Sun's declination in October is about −8°; the planets are the ephemeris' own.
    #[test]
    fn planets_are_the_ephemerides_own() {
        let got = declinations(&Builtin, TT, 43.07, -89.4, &[Point::Moon, Point::Sun]).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].point, Point::Moon);
        assert_eq!(got[1].point, Point::Sun);
        assert!((-9.0..-7.0).contains(&got[1].declination), "{got:?}");
        assert!(got[1].speed < 0.0 && got[1].speed > -0.5);
        assert!(!got[1].out_of_bounds);
        for d in &got {
            let b = d.point.body().unwrap();
            assert_eq!(
                d.declination,
                astrometry::derived::declination(&Builtin, b, TT).unwrap()
            );
            assert_eq!(
                d.speed,
                astrometry::derived::declination_speed(&Builtin, b, TT).unwrap()
            );
        }
    }

    #[test]
    fn order_preserved() {
        use Point::*;
        let asked = [
            Ascendant,
            Saturn,
            Lilith,
            Sun,
            MeanSouthNode,
            Moon,
            NorthNode,
        ];
        let got = declinations(&Builtin, TT, 10.0, 20.0, &asked).unwrap();
        let points: Vec<Point> = got.iter().map(|d| d.point).collect();
        assert_eq!(points, asked);
        // A point asked twice is given twice.
        assert_eq!(
            declinations(&Builtin, TT, 10.0, 20.0, &[Sun, Sun])
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn opposites_are_opposite() {
        use Point::*;
        let pairs = [
            (Ascendant, Descendant),
            (Midheaven, ImumCoeli),
            (NorthNode, SouthNode),
            (MeanNorthNode, MeanSouthNode),
        ];
        let all: Vec<Point> = pairs.iter().flat_map(|&(a, b)| [a, b]).collect();
        let got = declinations(&Builtin, TT, 43.07, -89.4, &all).unwrap();
        for (i, _) in pairs.iter().enumerate() {
            let (a, b) = (got[2 * i], got[2 * i + 1]);
            assert_eq!(a.declination, -b.declination, "{:?}", a.point);
            assert_eq!(a.speed, -b.speed, "{:?}", a.point);
            assert_eq!(a.out_of_bounds, b.out_of_bounds);
            assert!(a.declination != 0.0 && a.speed != 0.0);
        }
    }

    // The nodes and angles are on the ecliptic: their declinations are their longitudes'.
    #[test]
    fn ecliptic_points_follow_their_longitudes() {
        use Point::*;
        let (lat, lon) = (43.07, -89.4);
        let eps = ramc_and_obliquity(TT, lon).1;
        let all = [NorthNode, MeanNorthNode, Ascendant, Midheaven, Descendant];
        let d = declinations(&Builtin, TT, lat, lon, &all).unwrap();
        let p = positions(&Builtin, TT, lat, lon, &all).unwrap();
        for (d, p) in d.iter().zip(&p) {
            let want = ecliptic_declination(p.longitude, 0.0, eps);
            assert!(
                (d.declination - want).abs() < 1e-9,
                "{:?}: {} vs {want}",
                d.point,
                d.declination
            );
            // A speed in declination is a speed in longitude scaled by the slope of the
            // ecliptic to the equator, at most 1 (when the ecliptic runs along the equator).
            assert!(
                d.speed.abs() <= p.speed.abs() * 1.0001 + 1e-9,
                "{d:?} {p:?}"
            );
        }
    }

    #[test]
    fn lilith_has_the_apogees_latitude() {
        let eps = ramc_and_obliquity(TT, 0.0).1;
        let d = declinations(&Builtin, TT, 0.0, 0.0, &[Point::Lilith]).unwrap()[0];
        let want = ecliptic_declination(mean_apogee(TT), mean_apogee_latitude(TT), eps);
        assert_eq!(d.declination, want);
        assert!(
            mean_apogee_latitude(TT).abs() > 0.1,
            "a latitude of its own"
        );
        assert!(d.speed.abs() > 0.0 && d.speed.abs() < 1.0);
    }

    // Lilith's latitude takes it past the obliquity: bisect for the date where it crosses,
    // and take the flag a hair either side.
    #[test]
    fn out_of_bounds_either_side_of_the_obliquity() {
        let lilith = |t: f64| declinations(&Builtin, t, 0.0, 0.0, &[Point::Lilith]).unwrap()[0];
        let margin = |t: f64| {
            let d = lilith(t);
            d.declination.abs() - ramc_and_obliquity(t, 0.0).1.to_degrees()
        };
        // Scan two of the apogee's 8.85-year cycles, every three days, for a crossing of the
        // bound in either direction, then bisect to it.
        let outside = |t: f64| margin(t) > 0.0;
        let mut t = TT;
        while outside(t) == outside(t + 3.0) {
            t += 3.0;
            assert!(t < TT + 6500.0, "no crossing");
        }
        let (mut lo, mut hi) = (t, t + 3.0);
        let before = outside(lo);
        for _ in 0..60 {
            let mid = (lo + hi) / 2.0;
            if outside(mid) == before {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        // A hair either side, a millionth of a day: Lilith moves less than a degree a day.
        let (early, late) = (lilith(lo - 1e-6), lilith(hi + 1e-6));
        assert_eq!(early.out_of_bounds, before, "{early:?}");
        assert_eq!(late.out_of_bounds, !before, "{late:?}");
        let eps = ramc_and_obliquity(hi, 0.0).1.to_degrees();
        assert!((early.declination.abs() - eps).abs() < 1e-4);
        assert!((late.declination.abs() - eps).abs() < 1e-4);
    }

    #[test]
    fn the_sun_is_never_out_of_bounds() {
        // Around a solstice the Sun's declination reaches the obliquity, no more.
        for k in 0..14 {
            let t = 2461212.0 + f64::from(k) * 0.25; // the solstice of 2026-06-21, about
            let d = declinations(&Builtin, t, 0.0, 0.0, &[Point::Sun]).unwrap()[0];
            assert!(!d.out_of_bounds, "{d:?}");
            assert!(d.declination > 23.4, "{d:?}");
        }
    }

    #[test]
    fn needs_no_ephemeris() {
        use astrometry::source::Body;
        use astrometry::vector::Vec3;
        struct Never;
        impl Source for Never {
            fn state(&self, _: Body, _: f64) -> Result<(Vec3, Vec3), EphemerisError> {
                panic!("touched the ephemeris")
            }
        }
        use Point::*;
        let got = declinations(
            &Never,
            TT,
            10.0,
            20.0,
            &[
                Ascendant,
                Midheaven,
                Descendant,
                ImumCoeli,
                MeanNorthNode,
                MeanSouthNode,
                Lilith,
            ],
        )
        .unwrap();
        assert_eq!(got.len(), 7);
    }

    #[test]
    fn errors() {
        #[cfg(not(feature = "outer-planets"))]
        {
            let e = declinations(
                &Builtin,
                TT,
                0.0,
                0.0,
                &[Point::Sun, Point::Neptune, Point::Pluto],
            )
            .unwrap_err();
            assert!(matches!(e, Error::Unavailable(Point::Neptune)));
        }
        let e = declinations(&Builtin, TT, 0.0, 181.0, &[Point::Sun]).unwrap_err();
        assert!(matches!(
            e,
            Error::Horae(crate::Error::InvalidCoordinates(..))
        ));
        // The coordinates are checked first.
        #[cfg(not(feature = "outer-planets"))]
        {
            let e = declinations(&Builtin, TT, 99.0, 0.0, &[Point::Pluto]).unwrap_err();
            assert!(matches!(e, Error::Horae(_)));
        }
    }

    #[test]
    fn parallel_kinds() {
        assert_eq!(ParallelKind::Parallel.symbol(), "∥");
        assert_eq!(ParallelKind::Contraparallel.symbol(), "⋕");
        assert_eq!(
            "contraparallel".parse::<ParallelKind>(),
            Ok(ParallelKind::Contraparallel)
        );
    }
}
