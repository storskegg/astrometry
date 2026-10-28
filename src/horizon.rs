//! The Sun against a place's horizon: its altitude, and when it crosses an altitude.

use crate::apparent::apparent;
use crate::precession::gcrs_to_true_equator;
use crate::sidereal::gast;
use crate::source::{Body, Source};
use crate::spk;
use crate::time::ut1_from_tt;

/// The altitude of the Sun's centre at sunrise and sunset, radians: the upper edge on the
/// horizon, allowing for refraction (−0°50′), as the Go version has it.
pub const SUNRISE: f64 = -50.0 / 60.0 * std::f64::consts::PI / 180.0;

/// The local apparent hour angle of `body` at TT Julian date `tt`, seen from east longitude
/// `east` (radians): how far west of the meridian it is, wrapped to (−π, π], radians.
pub fn hour_angle(src: &dyn Source, body: Body, tt: f64, east: f64) -> Result<f64, spk::Error> {
    let p = gcrs_to_true_equator(tt).apply(apparent(src, body, tt)?.direction);
    let ra = p[1].atan2(p[0]);
    let h = gast(ut1_from_tt(tt), tt) + east - ra;
    Ok(h.sin().atan2(h.cos()))
}

/// The geocentric apparent altitude of `body` at TT Julian date `tt`, seen from latitude `lat`
/// and east longitude `east` (radians), radians.
pub fn altitude(
    src: &dyn Source,
    body: Body,
    tt: f64,
    lat: f64,
    east: f64,
) -> Result<f64, spk::Error> {
    let p = gcrs_to_true_equator(tt).apply(apparent(src, body, tt)?.direction);
    let dec = p[2].clamp(-1.0, 1.0).asin();
    let h = hour_angle(src, body, tt, east)?;
    Ok((lat.sin() * dec.sin() + lat.cos() * dec.cos() * h.cos()).asin())
}

/// A crossing of an altitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    /// TT Julian date.
    pub tt: f64,
    /// Rising through it, rather than setting.
    pub rising: bool,
}

/// The Sun's crossings of `altitude` between TT Julian dates `from` and `to`: its altitude
/// sampled every ten minutes, each crossing narrowed by bisection to under a second, as the Go
/// version finds sunrise and sunset.
pub fn sun_crossings(
    src: &dyn Source,
    from: f64,
    to: f64,
    lat: f64,
    east: f64,
    altitude_: f64,
) -> Result<Vec<Crossing>, spk::Error> {
    const STEP: f64 = 10.0 / 1440.0;
    let f = |tt: f64| -> Result<f64, spk::Error> {
        Ok(altitude(src, Body::Sun, tt, lat, east)? - altitude_)
    };
    let mut out = Vec::new();
    let (mut prev_t, mut prev) = (from, f(from)?);
    let mut t = from + STEP;
    while t <= to {
        let cur = f(t)?;
        if (prev < 0.0) != (cur < 0.0) {
            let (mut lo, mut hi) = (prev_t, t);
            while hi - lo > 0.5 / 86400.0 {
                let mid = (lo + hi) / 2.0;
                if (f(mid)? < 0.0) == (prev < 0.0) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            out.push(Crossing {
                tt: (lo + hi) / 2.0,
                rising: prev < 0.0,
            });
        }
        (prev_t, prev) = (t, cur);
        t += STEP;
    }
    Ok(out)
}

/// A transit of the meridian.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transit {
    /// TT Julian date.
    pub tt: f64,
    /// Across the upper meridian, rather than the lower.
    pub upper: bool,
}

/// The Sun's transits of the meridian between TT Julian dates `from` and `to`, at east longitude
/// `east` (radians): the sine of its hour angle sampled every ten minutes, each sign change
/// narrowed by bisection to under a second. Latitude doesn't enter.
pub fn sun_transits(
    src: &dyn Source,
    from: f64,
    to: f64,
    east: f64,
) -> Result<Vec<Transit>, spk::Error> {
    const STEP: f64 = 10.0 / 1440.0;
    let f =
        |tt: f64| -> Result<f64, spk::Error> { Ok(hour_angle(src, Body::Sun, tt, east)?.sin()) };
    let mut out = Vec::new();
    let (mut prev_t, mut prev) = (from, f(from)?);
    let mut t = from + STEP;
    while t <= to {
        let cur = f(t)?;
        if (prev < 0.0) != (cur < 0.0) {
            let (mut lo, mut hi) = (prev_t, t);
            while hi - lo > 0.5 / 86400.0 {
                let mid = (lo + hi) / 2.0;
                if (f(mid)? < 0.0) == (prev < 0.0) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let tt = (lo + hi) / 2.0;
            out.push(Transit {
                tt,
                upper: hour_angle(src, Body::Sun, tt, east)?.cos() > 0.0,
            });
        }
        (prev_t, prev) = (t, cur);
        t += STEP;
    }
    Ok(out)
}
