//! The Ptolemaic aspects, with Lilly's moieties for orbs.

use astrometry::derived::diff;

use crate::zodiac::{Planet, Point};

named_enum!(
    /// One of the Ptolemaic aspects.
    Kind, "aspect", [
        Conjunction => "Conjunction", Sextile => "Sextile", Square => "Square",
        Trine => "Trine", Opposition => "Opposition",
    ]
);

impl Kind {
    /// The aspect's symbol, such as □.
    pub fn symbol(self) -> &'static str {
        ["☌", "⚹", "□", "△", "☍"][self as usize]
    }

    /// Its angle, degrees.
    pub fn angle(self) -> f64 {
        [0.0, 60.0, 90.0, 120.0, 180.0][self as usize]
    }
}

/// An aspect between two points.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Aspect {
    /// The two points, in that order.
    pub points: [Point; 2],
    pub kind: Kind,
    /// How far the points are from the exact aspect, degrees.
    pub orb: f64,
    /// Becoming exact, rather than separating.
    pub applying: bool,
}

/// Half the planet's orb, degrees, after Lilly. Two planets are in aspect within the sum of
/// their moieties.
pub fn moiety(p: Planet) -> f64 {
    match p {
        Planet::Sun => 7.5,
        Planet::Moon => 6.0,
        Planet::Mercury | Planet::Venus => 3.5,
        Planet::Mars => 3.75,
        Planet::Jupiter | Planet::Saturn => 4.5,
    }
}

/// The angular distance between two longitudes, degrees, 0–180.
pub fn angle(a: f64, b: f64) -> f64 {
    diff(a, b).abs()
}

/// The aspects among the planets at longitudes `now` (degrees, by point index), and their
/// conjunctions with the nodes within the planet's moiety, using the longitudes a little
/// `later` to tell applying from separating. Closest first.
pub fn aspects(now: &[f64; 9], later: &[f64; 9]) -> Vec<Aspect> {
    let mut out = Vec::new();
    let mut try_ = |a: Point, b: Point, kinds: &[Kind], orb: f64| {
        for &k in kinds {
            let o = (angle(now[a.index()], now[b.index()]) - k.angle()).abs();
            if o <= orb {
                let o2 = (angle(later[a.index()], later[b.index()]) - k.angle()).abs();
                out.push(Aspect {
                    points: [a, b],
                    kind: k,
                    orb: o,
                    applying: o2 < o,
                });
            }
        }
    };
    for (i, &a) in Planet::ALL.iter().enumerate() {
        for &b in &Planet::ALL[i + 1..] {
            try_(a.into(), b.into(), &Kind::ALL, moiety(a) + moiety(b));
        }
        for n in [Point::NorthNode, Point::SouthNode] {
            try_(a.into(), n, &[Kind::Conjunction], moiety(a));
        }
    }
    out.sort_by(|x, y| x.orb.total_cmp(&y.orb));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use Point::*;

    #[test]
    fn aspects_by_orb() {
        let mut now = [0.0; 9];
        let mut later = [0.0; 9];
        // Sun at 0°, Moon at 95° and moving away from the square; Mars at 245°, trine the Sun
        // within 5° and closing; Saturn opposite Mars by 11°, beyond their 8°15′; Venus on the
        // North Node at 30°.
        let mut set = |p: Point, n: f64, l: f64| {
            now[p.index()] = n;
            later[p.index()] = l;
        };
        set(Sun, 0.0, 0.0);
        set(Moon, 95.0, 96.0);
        set(Mercury, 300.0, 300.0);
        set(Venus, 30.0, 30.0);
        set(Mars, 245.0, 244.0);
        set(Jupiter, 200.0, 200.0);
        set(Saturn, 76.0, 76.0);
        set(NorthNode, 31.0, 31.0);
        set(SouthNode, 211.0, 211.0);
        let got = aspects(&now, &later);
        let find = |a: Point, b: Point| got.iter().find(|x| x.points == [a, b]);
        for (a, b, kind, orb, applying) in [
            (Sun, Moon, Kind::Square, 5.0, false),
            (Sun, Mars, Kind::Trine, 5.0, true),
            (Venus, NorthNode, Kind::Conjunction, 1.0, false),
        ] {
            let x = find(a, b).unwrap_or_else(|| panic!("{a} and {b}"));
            assert!(
                x.kind == kind && (x.orb - orb).abs() < 1e-9 && x.applying == applying,
                "{x:?}"
            );
        }
        assert!(
            find(Mars, Saturn).is_none(),
            "Mars and Saturn 11° from opposition"
        );
        // The nodes make only conjunctions: Saturn is sextile the North Node within 1°, which
        // doesn't count.
        assert!(find(Saturn, NorthNode).is_none());
    }
}
