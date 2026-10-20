//! The vocabulary: the elements, the seven planets, the points (the planets and the lunar
//! nodes), and the signs.
//!
//! Each is a small enum that writes and reads as its name.

use std::ops::Add;

named_enum!(
    /// One of the four classical elements.
    Element, "element", [Fire => "Fire", Air => "Air", Water => "Water", Earth => "Earth"]
);

impl Element {
    /// The element's alchemical symbol: 🜂 Fire, 🜁 Air, 🜄 Water, 🜃 Earth.
    pub fn symbol(self) -> &'static str {
        ["🜂", "🜁", "🜄", "🜃"][self as usize]
    }
}

named_enum!(
    /// One of the seven classical planets: those that rule signs, days and hours, and have
    /// dignities and aspects.
    Planet, "planet", [
        Sun => "Sun", Moon => "Moon", Mercury => "Mercury", Venus => "Venus", Mars => "Mars",
        Jupiter => "Jupiter", Saturn => "Saturn",
    ]
);

impl Planet {
    /// The planet's symbol, such as ☉ for the Sun.
    pub fn symbol(self) -> &'static str {
        Point::from(self).symbol()
    }

    /// The planet's two-letter abbreviation, such as "Su".
    pub fn abbreviation(self) -> &'static str {
        Point::from(self).abbreviation()
    }
}

named_enum!(
    /// A point with a place in the zodiac: one of the seven planets, or a lunar node. The
    /// planets come first, in [`Planet::ALL`] order, so a planet's index is the same as its
    /// point's.
    Point, "point", [
        Sun => "Sun", Moon => "Moon", Mercury => "Mercury", Venus => "Venus", Mars => "Mars",
        Jupiter => "Jupiter", Saturn => "Saturn",
        /// Caput Draconis, the Dragon's Head: the Moon's ascending node.
        NorthNode => "North Node",
        /// Cauda Draconis, the Dragon's Tail: the descending node, opposite.
        SouthNode => "South Node",
    ]
);

impl Point {
    /// The planet, if this is one; `None` for the nodes.
    pub fn planet(self) -> Option<Planet> {
        Planet::from_index(self.index())
    }

    /// Whether this is one of the lunar nodes.
    pub fn is_node(self) -> bool {
        matches!(self, Point::NorthNode | Point::SouthNode)
    }

    /// The point's symbol, such as ☉ for the Sun and ☊ for the North Node.
    pub fn symbol(self) -> &'static str {
        ["☉", "☽", "☿", "♀", "♂", "♃", "♄", "☊", "☋"][self as usize]
    }

    /// The point's two-letter abbreviation, such as "Su" or "NN".
    pub fn abbreviation(self) -> &'static str {
        ["Su", "Mo", "Me", "Ve", "Ma", "Ju", "Sa", "NN", "SN"][self as usize]
    }

    /// The traditional Latin name of a node, "Caput Draconis" or "Cauda Draconis"; a
    /// planet's name otherwise.
    pub fn latin_name(self) -> &'static str {
        match self {
            Point::NorthNode => "Caput Draconis",
            Point::SouthNode => "Cauda Draconis",
            _ => self.name(),
        }
    }
}

impl From<Planet> for Point {
    fn from(p: Planet) -> Point {
        Point::ALL[p.index()]
    }
}

named_enum!(
    /// One of the twelve signs, in zodiacal order from Aries.
    Sign, "sign", [
        Aries => "Aries", Taurus => "Taurus", Gemini => "Gemini", Cancer => "Cancer",
        Leo => "Leo", Virgo => "Virgo", Libra => "Libra", Scorpio => "Scorpio",
        Sagittarius => "Sagittarius", Capricorn => "Capricorn", Aquarius => "Aquarius",
        Pisces => "Pisces",
    ]
);

/// The sign `n` places after this one in zodiacal order, wrapping past Pisces. Negative `n`
/// counts backwards.
impl Add<i32> for Sign {
    type Output = Sign;

    fn add(self, n: i32) -> Sign {
        Sign::ALL[(self as i32 + n).rem_euclid(12) as usize]
    }
}

impl Sign {
    /// The sign a longitude falls in, degrees from 0° Aries, any value.
    pub fn of(longitude: f64) -> Sign {
        Sign::ALL[(longitude.rem_euclid(360.0) / 30.0) as usize % 12]
    }

    /// The following sign in zodiacal order.
    pub fn next(self) -> Sign {
        self + 1
    }

    /// The sign's element (its triplicity). The elements repeat in zodiacal order: Fire,
    /// Earth, Air, Water.
    pub fn element(self) -> Element {
        [Element::Fire, Element::Earth, Element::Air, Element::Water][self as usize % 4]
    }

    /// The sign's symbol, such as "♈" for Aries.
    pub fn symbol(self) -> &'static str {
        [
            "♈", "♉", "♊", "♋", "♌", "♍", "♎", "♏", "♐", "♑", "♒", "♓",
        ][self as usize]
    }

    /// The sign's two-letter abbreviation, such as "Ar".
    pub fn abbreviation(self) -> &'static str {
        [
            "Ar", "Ta", "Ge", "Cn", "Le", "Vi", "Li", "Sc", "Sg", "Cp", "Aq", "Pi",
        ][self as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Sign::*;

    #[test]
    fn lists() {
        assert_eq!(Element::ALL.len(), 4);
        assert_eq!(Planet::ALL.len(), 7);
        assert_eq!(Point::ALL.len(), 9);
        assert_eq!(Sign::ALL.len(), 12);
        for (i, s) in Sign::ALL.into_iter().enumerate() {
            assert_eq!(s.index(), i);
            assert_eq!(Sign::from_index(i), Some(s));
        }
        assert_eq!(Element::from_index(4), None);
        assert_eq!(Planet::from_index(7), None);
        assert_eq!(Point::from_index(9), None);
        assert_eq!(Sign::from_index(12), None);
    }

    // A planet and its point share an index and a name; the nodes are no planets.
    #[test]
    fn planets_and_points() {
        for p in Planet::ALL {
            let pt = Point::from(p);
            assert_eq!(
                (pt.index(), pt.name(), pt.planet()),
                (p.index(), p.name(), Some(p))
            );
            assert!(!pt.is_node());
        }
        for n in [Point::NorthNode, Point::SouthNode] {
            assert!(n.is_node() && n.planet().is_none());
        }
        assert_eq!(Point::NorthNode.latin_name(), "Caput Draconis");
        assert_eq!(Point::SouthNode.latin_name(), "Cauda Draconis");
    }

    #[test]
    fn sign_add() {
        let tests = [
            (Cancer, 0, Cancer),
            (Cancer, 1, Leo),
            (Cancer, 11, Gemini),
            (Pisces, 1, Aries),
            (Aries, -1, Pisces),
            (Aries, 24, Aries),
            (Aries, -25, Pisces),
        ];
        for (from, n, want) in tests {
            assert_eq!(from + n, want, "{from} + {n}");
        }
    }

    // Adding is consistent with repeated next, and n and n±12 agree.
    #[test]
    fn sign_add_properties() {
        for s in Sign::ALL {
            let mut next = s;
            for n in 0..30 {
                assert_eq!(s + n, next, "{s} + {n}");
                assert!(
                    s + n == s + (n - 12) && s + -n == s + (12 - n),
                    "{s} at {n}"
                );
                assert_eq!(s + n + -n, s);
                next = next.next();
            }
        }
    }

    #[test]
    fn sign_of_longitude() {
        let tests = [
            (0.0, Aries),
            (29.9999, Aries),
            (30.0, Taurus),
            (359.99, Pisces),
            (360.0, Aries),
            (-0.5, Pisces),
            (725.0, Aries),
        ];
        for (lon, want) in tests {
            assert_eq!(Sign::of(lon), want, "{lon}");
        }
    }

    #[test]
    fn sign_element() {
        let want = [
            (Element::Fire, [Aries, Leo, Sagittarius]),
            (Element::Earth, [Taurus, Virgo, Capricorn]),
            (Element::Air, [Gemini, Libra, Aquarius]),
            (Element::Water, [Cancer, Scorpio, Pisces]),
        ];
        for (e, signs) in want {
            for s in signs {
                assert_eq!(s.element(), e, "{s}");
            }
        }
    }

    #[test]
    fn symbols() {
        let signs: Vec<char> = "♈♉♊♋♌♍♎♏♐♑♒♓".chars().collect();
        for (i, s) in Sign::ALL.into_iter().enumerate() {
            assert_eq!(s.symbol(), signs[i].to_string());
        }
        assert_eq!(Planet::Saturn.symbol(), "♄");
        assert_eq!(Point::SouthNode.symbol(), "☋");
        assert_eq!(Planet::Mercury.abbreviation(), "Me");
    }

    #[test]
    fn parse_round_trip() {
        for s in Sign::ALL {
            assert_eq!(s.name().parse(), Ok(s));
        }
        for p in Point::ALL {
            assert_eq!(p.name().parse(), Ok(p));
        }
        for e in Element::ALL {
            assert_eq!(e.name().parse(), Ok(e));
        }
    }

    #[test]
    fn parse_lenient() {
        assert_eq!("north_node".parse(), Ok(Point::NorthNode));
        assert_eq!(" sagittarius ".parse(), Ok(Sagittarius));
    }

    #[test]
    fn parse_unknown() {
        let err = "Ophiuchus".parse::<Sign>().unwrap_err();
        assert_eq!(err.to_string(), r#"horae: unknown sign "Ophiuchus""#);
        assert!("".parse::<Element>().is_err());
        assert!("Pluto".parse::<Point>().is_err());
        // The nodes are points, not planets.
        assert!("North Node".parse::<Planet>().is_err());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn json() {
        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        struct Payload {
            sign: Sign,
            point: Point,
            element: Element,
        }
        let p = Payload {
            sign: Capricorn,
            point: Point::SouthNode,
            element: Element::Water,
        };
        let b = serde_json::to_string(&p).unwrap();
        assert_eq!(
            b,
            r#"{"sign":"Capricorn","point":"South Node","element":"Water"}"#
        );
        assert_eq!(serde_json::from_str::<Payload>(&b).unwrap(), p);
        assert!(serde_json::from_str::<Sign>(r#""Ophiuchus""#).is_err());
    }
}
