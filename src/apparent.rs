//! Apparent geocentric positions: from barycentric states to the direction a geocentric
//! observer sees, then to the ecliptic of date.
//!
//! The chain, as JPL Horizons and the Swiss Ephemeris have it: light time; the gravitational
//! deflection of light by the Sun; annual aberration, relativistically; precession (IAU 2006)
//! and nutation (IAU 2000A_R06) to the true equinox and ecliptic of date.

use crate::precession;
use crate::source::{Body, Source};
use crate::spk;
use crate::time::{DAY, tdb_minus_tt};
use crate::vector::{Vec3, add, dot, norm, scale, sub, unit};

/// The speed of light, au/day.
pub const C: f64 = 299_792.458 * DAY / crate::source::AU;

/// An apparent position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Apparent {
    /// Ecliptic longitude of date from the true equinox, degrees, 0–360.
    pub longitude: f64,
    /// Ecliptic latitude of date, degrees.
    pub latitude: f64,
    /// The light-time distance, au.
    pub distance: f64,
    /// The apparent direction in GCRS axes, a unit vector.
    pub direction: Vec3,
}

/// The Sun's gravitational parameter GM☉, au³/day² (DE440; Park et al. 2021, AJ 161, 105).
const GM_SUN: f64 = 2.959_122_082_841_195e-4;

/// The Sun's Schwarzschild radius 2μ/c² = 2 GM☉ / c², au (c in au/day, from `C`).
const SCHWARZSCHILD_SUN: f64 = 2.0 * GM_SUN / (C * C);

/// Light deflection by the Sun's gravity.
///
/// `p` is the unit geocentric direction to the body, `q` the unit vector from the Sun to the
/// body, `e` the unit vector from the Sun to the observer and `em` the observer's distance from
/// the Sun, au. Explanatory Supplement to the Astronomical Almanac (3rd ed., 2013, §7.2), Kaplan,
/// USNO Circular 179 (2005, §7.2), IERS Conventions (2010) ch. 11:
/// p₁ = p + (2μ/c² E) / (1 + q·e) × [(p·q) e − (e·p) q].
pub(crate) fn deflect(p: Vec3, q: Vec3, e: Vec3, em: f64) -> Vec3 {
    let qe = dot(q, e);
    // The formula blows up as 1 + q·e → 0, a body directly behind the Sun. Below 1e-9 (the
    // body within about 9 arcseconds of the Sun's centre, well inside its disc) there is no
    // deflection at all: the body is hidden, and the position is meaningless anyway.
    if 1.0 + qe < 1e-9 {
        return p;
    }
    let g = SCHWARZSCHILD_SUN / em / (1.0 + qe);
    let d = sub(scale(dot(p, q), e), scale(dot(e, p), q));
    unit(add(p, scale(g, d)))
}

/// Relativistic annual aberration.
///
/// `p` is the unit direction to the body, `v` the observer's barycentric velocity in units of c.
/// Explanatory Supplement (3rd ed., §7.2, eq. 7.40), Kaplan, USNO Circular 179:
/// with 1/β = √(1 − v·v), p′ = [p/β + (1 + (p·v)/(1 + 1/β)) v] / (1 + p·v), normalised.
pub(crate) fn aberrate(p: Vec3, v: Vec3) -> Vec3 {
    let pv = dot(p, v);
    let ib = (1.0 - dot(v, v)).sqrt();
    let w = 1.0 + pv / (1.0 + ib);
    unit(scale(1.0 / (1.0 + pv), add(scale(ib, p), scale(w, v))))
}

/// The apparent direction of `body` in GCRS axes (a unit vector) and its light-time distance,
/// au, at TT Julian date `tt`.
pub(crate) fn direction(src: &dyn Source, body: Body, tt: f64) -> Result<(Vec3, f64), spk::Error> {
    // TDB, and the light time, as seconds from `tt`, not added to it (see
    // `Source::state_after`).
    let tdb_tt = tdb_minus_tt(tt);
    let (earth, earth_v) = src.state_after(Body::Earth, tt, tdb_tt)?;
    let (sun, _) = src.state_after(Body::Sun, tt, tdb_tt)?;

    // Light time: where the body was when the light now arriving left it.
    let mut tau = 0.0;
    let mut at = earth;
    for _ in 0..4 {
        at = src.state_after(body, tt, tdb_tt - tau * DAY)?.0;
        tau = norm(sub(at, earth)) / C;
    }
    let u = sub(at, earth);
    let distance = norm(u);
    let mut p = unit(u);

    if body != Body::Sun {
        let (sun_then, _) = src.state_after(Body::Sun, tt, tdb_tt - tau * DAY)?;
        let q = unit(sub(at, sun_then));
        let e = sub(earth, sun);
        p = deflect(p, q, unit(e), norm(e));
    }
    p = aberrate(p, scale(1.0 / C, earth_v));
    Ok((p, distance))
}

/// The apparent geocentric position of `body` at TT Julian date `tt`.
pub fn apparent(src: &dyn Source, body: Body, tt: f64) -> Result<Apparent, spk::Error> {
    let (p, distance) = direction(src, body, tt)?;
    let ecl = precession::gcrs_to_ecliptic_of_date(tt).apply(p);
    let longitude = ecl[1].atan2(ecl[0]).to_degrees().rem_euclid(360.0);
    let latitude = ecl[2].clamp(-1.0, 1.0).asin().to_degrees();
    Ok(Apparent {
        longitude,
        latitude,
        distance,
        direction: p,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Builtin;

    // The Moon's apparent longitude, sampled every 43 s, is smooth: its second differences
    // keep to the curve's, without the steps a light time added to the date makes as the date
    // rounds (up to 0.0007″ before it was kept apart).
    #[test]
    fn moon_is_smooth() {
        let (jd, h) = (2501825.033912, 0.0005);
        let lon = |t: f64| apparent(&Builtin, Body::Moon, t).unwrap().longitude * 3600.0;
        let d2: Vec<f64> = (-10..10)
            .map(|k| jd + f64::from(k) * h)
            .map(|t| lon(t + h) - 2.0 * lon(t) + lon(t - h))
            .collect();
        let mean = d2.iter().sum::<f64>() / d2.len() as f64;
        let worst = d2.iter().map(|d| (d - mean).abs()).fold(0.0, f64::max);
        assert!(worst < 1e-4, "second differences part by {worst:.6}″");
    }
}
