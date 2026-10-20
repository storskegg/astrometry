//! The angles of a chart: the Ascendant, the degree of the ecliptic rising in the east, and
//! the Midheaven, the degree culminating on the meridian. Their opposites are the Descendant
//! and the Imum Coeli.

use crate::nutation::nutation;
use crate::precession::mean_obliquity;
use crate::sidereal::gast;
use crate::time::ut1_from_tt;

/// The Ascendant and Midheaven, degrees, 0–360 from 0° Aries.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Angles {
    pub ascendant: f64,
    pub midheaven: f64,
}

/// The angles at TT Julian date `tt`, at latitude `lat` and longitude `lon`, degrees, north
/// and east positive: from the apparent sidereal time and the true obliquity of the ecliptic.
/// Within the polar circles, where some of the ecliptic never rises, the Ascendant is the
/// formula's, which there can lie west of the Midheaven.
pub fn angles(tt: f64, lat: f64, lon: f64) -> Angles {
    let obliquity = mean_obliquity(tt) + nutation(tt).1;
    let ramc = gast(ut1_from_tt(tt), tt) + lon.to_radians();
    from_ramc(ramc, obliquity, lat.to_radians())
}

/// The angles from the right ascension of the Midheaven `ramc`, the obliquity of the ecliptic
/// and the latitude, all radians.
pub fn from_ramc(ramc: f64, obliquity: f64, lat: f64) -> Angles {
    let (sin_r, cos_r) = ramc.sin_cos();
    let (sin_e, cos_e) = obliquity.sin_cos();
    let midheaven = sin_r.atan2(cos_r * cos_e);
    let ascendant = cos_r.atan2(-(sin_r * cos_e + lat.tan() * sin_e));
    Angles {
        ascendant: ascendant.to_degrees().rem_euclid(360.0),
        midheaven: midheaven.to_degrees().rem_euclid(360.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OBLIQUITY: f64 = 23.4392911_f64;

    fn at(ramc: f64, lat: f64) -> Angles {
        from_ramc(ramc.to_radians(), OBLIQUITY.to_radians(), lat.to_radians())
    }

    // At the equator, with 0° Aries culminating, 0° Cancer rises; with 0° Cancer culminating,
    // 0° Libra.
    #[test]
    fn at_the_equator() {
        let a = at(0.0, 0.0);
        assert!(
            (a.midheaven - 0.0).abs() < 1e-9 && (a.ascendant - 90.0).abs() < 1e-9,
            "{a:?}"
        );
        let a = at(90.0, 0.0);
        assert!(
            (a.midheaven - 90.0).abs() < 1e-9 && (a.ascendant - 180.0).abs() < 1e-9,
            "{a:?}"
        );
    }

    // Outside the polar circles the Ascendant is always east of the Midheaven: within the next
    // half of the zodiac.
    #[test]
    fn ascendant_east_of_midheaven() {
        for lat in [-60.0, -33.9, 0.0, 43.1, 64.1] {
            for ramc in (0..360).step_by(5) {
                let a = at(ramc as f64, lat);
                let east = (a.ascendant - a.midheaven).rem_euclid(360.0);
                assert!(east > 0.0 && east < 180.0, "{lat}° at RAMC {ramc}°: {a:?}");
            }
        }
    }

    // In the north, signs of long ascension (Cancer to Sagittarius) rise slowly, and those of
    // short ascension (Capricorn to Gemini) quickly: an hour of sidereal time raises fewer
    // degrees of the one than of the other.
    #[test]
    fn long_and_short_ascension() {
        let rising = |ramc: f64| {
            let d = at(ramc + 15.0, 43.1).ascendant - at(ramc, 43.1).ascendant;
            d.rem_euclid(360.0)
        };
        // With RAMC 270° Aries rises; with 90°, Libra.
        assert!(
            rising(270.0) > 2.0 * rising(90.0),
            "{} vs {}",
            rising(270.0),
            rising(90.0)
        );
    }
}
