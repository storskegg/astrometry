//! Precession: IAU 2006 (Capitaine, Wallace and Chapront 2003), as the Fukushima–Williams
//! angles, which carry the frame bias between the GCRS and the J2000 mean equator too.

use crate::nutation::{ARCSEC, nutation};
use crate::time::centuries;
use crate::vector::Mat3;

/// The Fukushima–Williams angles at TT Julian date `jd`, radians: γ̄, φ̄, ψ̄ and the mean
/// obliquity ε_A.
pub fn angles(jd: f64) -> (f64, f64, f64, f64) {
    let t = centuries(jd);
    let poly = |c: [f64; 6]| {
        (c[0] + t * (c[1] + t * (c[2] + t * (c[3] + t * (c[4] + t * c[5]))))) * ARCSEC
    };
    (
        poly([
            -0.052928,
            10.556378,
            0.4932044,
            -0.00031238,
            -0.000002788,
            0.0000000260,
        ]),
        poly([
            84381.412819,
            -46.811016,
            0.0511268,
            0.00053289,
            -0.000000440,
            -0.0000000176,
        ]),
        poly([
            -0.041775,
            5038.481484,
            1.5584175,
            -0.00018522,
            -0.000026452,
            -0.0000000148,
        ]),
        mean_obliquity(jd),
    )
}

/// The mean obliquity of the ecliptic, IAU 2006, radians.
pub fn mean_obliquity(jd: f64) -> f64 {
    let t = centuries(jd);
    (84381.406
        + t * (-46.836769
            + t * (-0.0001831 + t * (0.00200340 + t * (-0.000000576 + t * -0.0000000434)))))
        * ARCSEC
}

/// The rotation from GCRS axes to the ecliptic of date with the true equinox: x towards the
/// true equinox, z towards the ecliptic's pole.
pub fn gcrs_to_ecliptic_of_date(jd: f64) -> Mat3 {
    let (gamma, phi, psi, _) = angles(jd);
    let (dpsi, _) = nutation(jd);
    Mat3::rz(gamma)
        .then(Mat3::rx(phi))
        .then(Mat3::rz(-(psi + dpsi)))
}

/// The rotation from GCRS axes to the true equator and equinox of date.
pub fn gcrs_to_true_equator(jd: f64) -> Mat3 {
    let (gamma, phi, psi, eps) = angles(jd);
    let (dpsi, deps) = nutation(jd);
    Mat3::rz(gamma)
        .then(Mat3::rx(phi))
        .then(Mat3::rz(-(psi + dpsi)))
        .then(Mat3::rx(-(eps + deps)))
}

#[cfg(test)]
// SOFA's test values, quoted as it gives them.
#[allow(clippy::excessive_precision)]
mod tests {
    use super::*;

    // SOFA's iauPfw06 test values at 2400000.5 + 50123.9999.
    #[test]
    fn against_sofa() {
        let (g, p, s, e) = angles(2400000.5 + 50123.9999);
        assert!((g - -0.2243387670997995690e-5).abs() < 1e-16, "{g:e}");
        assert!((p - 0.4091014602391312808).abs() < 1e-12, "{p}");
        assert!((s - -0.9501954178013031895e-3).abs() < 1e-14, "{s:e}");
        assert!((e - 0.4091014316587367491).abs() < 1e-12, "{e}");
    }
}
