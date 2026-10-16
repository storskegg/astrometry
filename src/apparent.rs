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
use crate::vector::{Vec3, add, cross, dot, norm, scale, sub, unit};

/// The speed of light, au/day.
pub const C: f64 = 299_792.458 * DAY / crate::source::AU;

/// The Sun's Schwarzschild radius, au (2GM/c²).
const SRS: f64 = 1.97412574336e-8;

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

/// The light-deflected direction `p` of a body at `q` (unit, from the Sun) seen from the
/// observer at `e` (unit, from the Sun) at distance `em` au from the Sun: SOFA's `iauLd` for
/// the Sun.
fn deflect(p: Vec3, q: Vec3, e: Vec3, em: f64) -> Vec3 {
    let w = SRS / em / (dot(q, e) + 1.0).max(1e-9);
    add(p, scale(w, cross(p, cross(e, q))))
}

/// The direction `p` (unit) aberrated by the observer's barycentric velocity `v` (units of c)
/// at distance `s` au from the Sun: SOFA's `iauAb`.
fn aberrate(p: Vec3, v: Vec3, s: f64) -> Vec3 {
    let bm1 = (1.0 - dot(v, v)).sqrt();
    let pdv = dot(p, v);
    let w1 = 1.0 + pdv / (1.0 + bm1);
    let w2 = SRS / s;
    unit([0, 1, 2].map(|i| p[i] * bm1 + w1 * v[i] + w2 * (v[i] - pdv * p[i])))
}

/// The apparent geocentric position of `body` at TT Julian date `tt`.
pub fn apparent(src: &dyn Source, body: Body, tt: f64) -> Result<Apparent, spk::Error> {
    let tdb = tt + tdb_minus_tt(tt) / DAY;
    let (earth, earth_v) = src.state(Body::Earth, tdb)?;
    let (sun, _) = src.state(Body::Sun, tdb)?;

    // Light time: where the body was when the light now arriving left it.
    let mut tau = 0.0;
    let mut at = earth;
    for _ in 0..4 {
        at = src.state(body, tdb - tau)?.0;
        tau = norm(sub(at, earth)) / C;
    }
    let u = sub(at, earth);
    let distance = norm(u);
    let mut p = unit(u);

    if body != Body::Sun {
        let (sun_then, _) = src.state(Body::Sun, tdb - tau)?;
        let q = unit(sub(at, sun_then));
        let e = sub(earth, sun);
        p = deflect(p, q, unit(e), norm(e));
    }
    p = aberrate(p, scale(1.0 / C, earth_v), norm(sub(earth, sun)));

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
