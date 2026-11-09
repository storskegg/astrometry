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

use std::fmt;

use astrometry::derived::{diff, mean_apogee, mean_apogee_speed, mean_node, mean_node_speed};
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
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Horae(e) => e.fmt(f),
            Error::Unavailable(p) => write!(f, "horae: {} isn't available in this build", p.name()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Horae(e) => Some(e),
            Error::Unavailable(_) => None,
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
