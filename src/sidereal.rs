//! Sidereal time: the Earth Rotation Angle, and Greenwich mean and apparent sidereal time
//! (IAU 2006), as SOFA's `iauEra00`, `iauGmst06` and `iauGst06a` have them, the equation of
//! the equinoxes' complementary terms to their two largest.

use std::f64::consts::TAU;

use crate::nutation::{ARCSEC, nutation};
use crate::precession::mean_obliquity;
use crate::time::{J2000, centuries};

/// The Earth Rotation Angle at UT1 Julian date `ut1`, radians, 0–2π.
pub fn earth_rotation_angle(ut1: f64) -> f64 {
    let d = ut1 - J2000;
    (TAU * (d.fract() + 0.7790572732640 + 0.00273781191135448 * d)).rem_euclid(TAU)
}

/// Greenwich mean sidereal time, radians, at UT1 `ut1` and TT `tt` (Julian dates).
pub fn gmst(ut1: f64, tt: f64) -> f64 {
    let t = centuries(tt);
    let poly = 0.014506
        + t * (4612.156534
            + t * (1.3915817 + t * (-0.00000044 + t * (-0.000029956 + t * -0.0000000368))));
    (earth_rotation_angle(ut1) + poly * ARCSEC).rem_euclid(TAU)
}

/// Greenwich apparent sidereal time, radians, at UT1 `ut1` and TT `tt`.
pub fn gast(ut1: f64, tt: f64) -> f64 {
    let t = centuries(tt);
    // The Moon's mean node, for the complementary terms.
    let om = ((450160.398036
        + t * (-6962890.5431 + t * (7.4722 + t * (0.007702 + t * -0.00005939))))
        % 1_296_000.0)
        * ARCSEC;
    let ee = nutation(tt).0 * mean_obliquity(tt).cos()
        + (0.00264096 * om.sin() + 0.00006352 * (2.0 * om).sin()) * ARCSEC;
    (gmst(ut1, tt) + ee).rem_euclid(TAU)
}

#[cfg(test)]
mod tests {
    use super::*;

    // SOFA's test values: iauEra00(2400000.5, 54388.0), iauGmst06 and iauGst06a at
    // (2400000.5, 53736.0) for both UT1 and TT.
    #[test]
    #[allow(clippy::excessive_precision)]
    fn against_sofa() {
        assert!((earth_rotation_angle(2400000.5 + 54388.0) - 0.4022837240028158102).abs() < 1e-12);
        let jd = 2400000.5 + 53736.0;
        assert!(
            (gmst(jd, jd) - 1.754174971870091203).abs() < 1e-12,
            "{}",
            gmst(jd, jd)
        );
        // The two complementary terms leave out a few µas.
        assert!(
            (gast(jd, jd) - 1.754166137675019159).abs() < 1e-10,
            "{}",
            gast(jd, jd)
        );
    }
}
