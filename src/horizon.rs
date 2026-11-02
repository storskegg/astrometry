//! The Sun against a place's horizon: its altitude, and when it crosses an altitude.

use crate::apparent::direction;
use crate::precession::gcrs_to_true_equator;
use crate::sidereal::gast;
use crate::source::{Body, Source};
use crate::spk;
use crate::time::ut1_from_tt;

/// The altitude of the Sun's centre at sunrise and sunset, radians: the upper edge on the
/// horizon, allowing for refraction (−0°50′), as the Go version has it.
pub const SUNRISE: f64 = -50.0 / 60.0 * std::f64::consts::PI / 180.0;

/// The Sun's local apparent hour angle (wrapped to (−π, π]) and declination, radians, from the
/// apparent direction alone.
fn hour_angle_and_declination(
    src: &dyn Source,
    body: Body,
    tt: f64,
    east: f64,
) -> Result<(f64, f64), spk::Error> {
    let p = gcrs_to_true_equator(tt).apply(direction(src, body, tt)?.0);
    let (ra, dec) = (p[1].atan2(p[0]), p[2].clamp(-1.0, 1.0).asin());
    let h = gast(ut1_from_tt(tt), tt) + east - ra;
    Ok((h.sin().atan2(h.cos()), dec))
}

/// The local apparent hour angle of `body` at TT Julian date `tt`, seen from east longitude
/// `east` (radians): how far west of the meridian it is, wrapped to (−π, π], radians.
pub fn hour_angle(src: &dyn Source, body: Body, tt: f64, east: f64) -> Result<f64, spk::Error> {
    Ok(hour_angle_and_declination(src, body, tt, east)?.0)
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
    let (h, dec) = hour_angle_and_declination(src, body, tt, east)?;
    Ok((lat.sin() * dec.sin() + lat.cos() * dec.cos() * h.cos()).asin())
}

/// The times between `from` and `to` at which `f` changes sign, each with whether it rose
/// through zero. `f` is sampled every ten minutes on a fixed grid, whole multiples of the step
/// from JD 0, and each change narrowed by bisection to under a second; so an event is bracketed
/// and bisected the same whatever the window it's found in.
fn sign_changes(
    from: f64,
    to: f64,
    f: impl Fn(f64) -> Result<f64, spk::Error>,
) -> Result<Vec<(f64, bool)>, spk::Error> {
    const STEP: f64 = 10.0 / 1440.0;
    let first = (from / STEP).floor();
    let at = |k: f64| k * STEP;
    let mut out = Vec::new();
    let (mut prev_t, mut prev) = (at(first), f(at(first))?);
    let mut k = first + 1.0;
    loop {
        let t = at(k);
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
            let found = (lo + hi) / 2.0;
            if (from..=to).contains(&found) {
                out.push((found, prev < 0.0));
            }
        }
        if t > to {
            break;
        }
        (prev_t, prev) = (t, cur);
        k += 1.0;
    }
    Ok(out)
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
/// sampled every ten minutes on a fixed grid, each crossing narrowed by bisection to under a
/// second, as the Go version finds sunrise and sunset. A crossing is the same whichever window
/// finds it.
pub fn sun_crossings(
    src: &dyn Source,
    from: f64,
    to: f64,
    lat: f64,
    east: f64,
    altitude_: f64,
) -> Result<Vec<Crossing>, spk::Error> {
    let found = sign_changes(from, to, |tt| {
        Ok(altitude(src, Body::Sun, tt, lat, east)? - altitude_)
    })?;
    Ok(found
        .into_iter()
        .map(|(tt, rising)| Crossing { tt, rising })
        .collect())
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
/// `east` (radians): the sine of its hour angle sampled and narrowed as the crossings are.
/// Latitude doesn't enter.
pub fn sun_transits(
    src: &dyn Source,
    from: f64,
    to: f64,
    east: f64,
) -> Result<Vec<Transit>, spk::Error> {
    // The sine rises through zero at the upper transit, where the hour angle is 0.
    let found = sign_changes(from, to, |tt| {
        Ok(hour_angle(src, Body::Sun, tt, east)?.sin())
    })?;
    let mut out = Vec::with_capacity(found.len());
    for (tt, _) in found {
        out.push(Transit {
            tt,
            upper: hour_angle(src, Body::Sun, tt, east)?.cos() > 0.0,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Builtin;

    // The same event comes out bit for bit whatever the window that finds it.
    #[test]
    fn windows_agree() {
        let (lat, east) = (43.0731f64.to_radians(), (-89.4012f64).to_radians());
        let base = 2461328.0;
        let windows = [
            (base, base + 3.0),
            (base + 0.13, base + 2.9),
            (base - 0.77, base + 3.41),
        ];
        let found: Vec<_> = windows
            .iter()
            .map(|&(a, b)| {
                let c = sun_crossings(&Builtin, a, b, lat, east, SUNRISE).unwrap();
                let t = sun_transits(&Builtin, a, b, east).unwrap();
                (c, t)
            })
            .collect();
        let (c0, t0) = &found[1];
        assert!(c0.len() >= 4 && t0.len() >= 4, "{c0:?} {t0:?}");
        for (c, t) in &found {
            for x in c0 {
                assert!(c.contains(x), "{x:?} in {c:?}");
            }
            for x in t0 {
                assert!(t.contains(x), "{x:?} in {t:?}");
            }
        }
    }

    #[test]
    fn events_stay_in_the_window() {
        let (lat, east) = (43.0731f64.to_radians(), (-89.4012f64).to_radians());
        let (a, b) = (2461328.3, 2461329.9);
        for x in sun_crossings(&Builtin, a, b, lat, east, SUNRISE).unwrap() {
            assert!((a..=b).contains(&x.tt), "{x:?}");
        }
        for x in sun_transits(&Builtin, a, b, east).unwrap() {
            assert!((a..=b).contains(&x.tt), "{x:?}");
        }
    }
}
