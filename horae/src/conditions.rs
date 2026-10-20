//! Planets' conditions: essential dignities by sign, and closeness to the Sun.

use crate::zodiac::{Planet, Sign};

named_enum!(
    /// A planet's essential dignity or debility by sign, under the traditional rulerships.
    Dignity, "dignity", [
        /// In a sign it rules.
        Domicile => "Domicile",
        Exaltation => "Exaltation",
        /// Opposite a sign it rules.
        Detriment => "Detriment",
        /// Opposite its exaltation.
        Fall => "Fall",
        /// None of these.
        Peregrine => "Peregrine",
    ]
);

/// Each sign's traditional ruler, Aries first.
const RULERS: [Planet; 12] = [
    Planet::Mars,
    Planet::Venus,
    Planet::Mercury,
    Planet::Moon,
    Planet::Sun,
    Planet::Mercury,
    Planet::Venus,
    Planet::Mars,
    Planet::Jupiter,
    Planet::Saturn,
    Planet::Saturn,
    Planet::Jupiter,
];

/// The sign's traditional ruler.
pub fn ruler(s: Sign) -> Planet {
    RULERS[s.index()]
}

/// The sign of the planet's exaltation.
pub fn exaltation(p: Planet) -> Sign {
    match p {
        Planet::Sun => Sign::Aries,
        Planet::Moon => Sign::Taurus,
        Planet::Mercury => Sign::Virgo,
        Planet::Venus => Sign::Pisces,
        Planet::Mars => Sign::Capricorn,
        Planet::Jupiter => Sign::Cancer,
        Planet::Saturn => Sign::Libra,
    }
}

/// `p`'s dignities and debilities in `s`, or Peregrine. Mercury in Virgo is both in its
/// domicile and exalted.
pub fn dignities(p: Planet, s: Sign) -> Vec<Dignity> {
    let mut ds = Vec::new();
    if ruler(s) == p {
        ds.push(Dignity::Domicile);
    }
    if exaltation(p) == s {
        ds.push(Dignity::Exaltation);
    }
    if ruler(s + 6) == p {
        ds.push(Dignity::Detriment);
    }
    if exaltation(p) + 6 == s {
        ds.push(Dignity::Fall);
    }
    if ds.is_empty() {
        ds.push(Dignity::Peregrine);
    }
    ds
}

named_enum!(
    /// A planet's closeness to the Sun in longitude, when close.
    Solar, "solar", [
        /// Within 17′: "in the heart of the Sun", strengthened.
        Cazimi => "Cazimi",
        /// Within 8°30′: burnt up, weakened.
        Combust => "Combust",
        /// Within 17°: hidden, somewhat weakened.
        UnderTheBeams => "Under the beams",
    ]
);

/// The closeness to the Sun at `deg` degrees from it; none beyond 17°.
pub fn solar(deg: f64) -> Option<Solar> {
    if deg <= 17.0 / 60.0 {
        Some(Solar::Cazimi)
    } else if deg <= 8.5 {
        Some(Solar::Combust)
    } else if deg <= 17.0 {
        Some(Solar::UnderTheBeams)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Planet::*;

    #[test]
    fn dignities_by_sign() {
        use Dignity::*;
        let tests = [
            (Mercury, Sign::Virgo, vec![Domicile, Exaltation]),
            (Mercury, Sign::Pisces, vec![Detriment, Fall]),
            (Moon, Sign::Cancer, vec![Domicile]),
            (Moon, Sign::Scorpio, vec![Fall]),
            (Mars, Sign::Capricorn, vec![Exaltation]),
            (Venus, Sign::Aries, vec![Detriment]),
            (Sun, Sign::Aquarius, vec![Detriment]),
            (Jupiter, Sign::Leo, vec![Peregrine]),
            (Saturn, Sign::Aquarius, vec![Domicile]),
        ];
        for (p, s, want) in tests {
            assert_eq!(dignities(p, s), want, "{p} in {s}");
        }
    }

    #[test]
    fn closeness_to_the_sun() {
        let tests = [
            (0.2, Some(Solar::Cazimi)),
            (0.29, Some(Solar::Combust)),
            (8.5, Some(Solar::Combust)),
            (8.6, Some(Solar::UnderTheBeams)),
            (17.0, Some(Solar::UnderTheBeams)),
            (17.1, None),
        ];
        for (deg, want) in tests {
            assert_eq!(solar(deg), want, "{deg}°");
        }
    }
}
