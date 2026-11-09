//! Quantities derived from positions: speeds in longitude, the lunar nodes, and how the Moon
//! is lit.

use crate::apparent::apparent;
use crate::nutation::nutation;
use crate::precession::gcrs_to_ecliptic_of_date;
use crate::source::{AU, Body, Source};
use crate::spk;
use crate::time::{centuries, tdb_minus_tt};
use crate::vector::{cross, sub};

/// The step of the symmetric differences, days.
pub const STEP: f64 = 0.01;

/// The signed difference `a − b` of two longitudes, degrees, in −180..180.
pub fn diff(a: f64, b: f64) -> f64 {
    (a - b + 540.0).rem_euclid(360.0) - 180.0
}

/// The speed of `body`'s apparent ecliptic longitude at TT Julian date `tt`, degrees per day;
/// negative when retrograde.
pub fn speed(src: &dyn Source, body: Body, tt: f64) -> Result<f64, spk::Error> {
    let a = apparent(src, body, tt - STEP)?.longitude;
    let b = apparent(src, body, tt + STEP)?.longitude;
    Ok(diff(b, a) / (2.0 * STEP))
}

/// The true node: the ascending node of the Moon's osculating geocentric orbit on the ecliptic
/// of date, from the true equinox, degrees.
pub fn true_node(src: &dyn Source, tt: f64) -> Result<f64, spk::Error> {
    let tdb_tt = tdb_minus_tt(tt);
    let (mp, mv) = src.state_after(Body::Moon, tt, tdb_tt)?;
    let (ep, ev) = src.state_after(Body::Earth, tt, tdb_tt)?;
    let m = gcrs_to_ecliptic_of_date(tt);
    let h = cross(m.apply(sub(mp, ep)), m.apply(sub(mv, ev)));
    Ok(h[0].atan2(-h[1]).to_degrees().rem_euclid(360.0))
}

/// The mean node, from the true equinox, degrees: Meeus's polynomial (Astronomical
/// Algorithms, eq. 47.7, after Chapront), plus the nutation in longitude.
pub fn mean_node(tt: f64) -> f64 {
    let t = centuries(tt);
    let omega = 125.0445479 - 1934.1362891 * t + 0.0020754 * t * t + t.powi(3) / 467441.0
        - t.powi(4) / 60616000.0;
    (omega + nutation(tt).0.to_degrees()).rem_euclid(360.0)
}

/// The mean inclination of the Moon's orbit to the ecliptic, degrees.
const MEAN_INCLINATION: f64 = 5.1453964;

/// The mean lunar apogee ("Black Moon Lilith"), from the true equinox, degrees: the apogee
/// projected onto the ecliptic of date, as the Swiss Ephemeris gives it. The apogee on the orbit
/// is Meeus's mean perigee (Astronomical Algorithms, eq. 47.7) plus 180°; its angle from the
/// mean node is projected onto the ecliptic through [`MEAN_INCLINATION`], and the nutation in
/// longitude is added.
pub fn mean_apogee(tt: f64) -> f64 {
    let t = centuries(tt);
    let perigee = 83.3532465 + 4069.0137287 * t - 0.0103200 * t * t - t.powi(3) / 80053.0
        + t.powi(4) / 18999000.0;
    let omega = 125.0445479 - 1934.1362891 * t + 0.0020754 * t * t + t.powi(3) / 467441.0
        - t.powi(4) / 60616000.0;
    let u = (perigee + 180.0 - omega).to_radians();
    let projected = (u.sin() * MEAN_INCLINATION.to_radians().cos())
        .atan2(u.cos())
        .to_degrees();
    (omega + projected + nutation(tt).0.to_degrees()).rem_euclid(360.0)
}

/// The speed of the mean node, degrees per day (central differences over [`STEP`]).
pub fn mean_node_speed(tt: f64) -> f64 {
    diff(mean_node(tt + STEP), mean_node(tt - STEP)) / (2.0 * STEP)
}

/// The speed of the mean apogee, degrees per day (central differences over [`STEP`]).
pub fn mean_apogee_speed(tt: f64) -> f64 {
    diff(mean_apogee(tt + STEP), mean_apogee(tt - STEP)) / (2.0 * STEP)
}

/// The speed of the true node, degrees per day.
pub fn true_node_speed(src: &dyn Source, tt: f64) -> Result<f64, spk::Error> {
    Ok(diff(true_node(src, tt + STEP)?, true_node(src, tt - STEP)?) / (2.0 * STEP))
}

/// How the Moon is lit at TT Julian date `tt`: its elongation, the Moon's apparent longitude
/// less the Sun's, degrees 0–360; and the fraction of its disc lit, 0 to 1. The phase angle,
/// Sun–Moon–Earth, from the elongation and the distances (Meeus 48.3), ignoring the Moon's
/// latitude.
pub fn moon_light(src: &dyn Source, tt: f64) -> Result<(f64, f64), spk::Error> {
    let sun = apparent(src, Body::Sun, tt)?;
    let moon = apparent(src, Body::Moon, tt)?;
    let elongation = (moon.longitude - sun.longitude).rem_euclid(360.0);
    let (sun_km, moon_km) = (sun.distance * AU, moon.distance * AU);
    let psi = diff(moon.longitude, sun.longitude).abs().to_radians();
    let i = (sun_km * psi.sin()).atan2(moon_km - sun_km * psi.cos());
    Ok((elongation, (1.0 + i.cos()) / 2.0))
}
