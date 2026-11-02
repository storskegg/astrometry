//! The Sun against a place's horizon: its altitude, and when it crosses an altitude.

use crate::apparent::direction;
use crate::precession::gcrs_to_true_equator;
use crate::sidereal::gast;
use crate::source::{Body, Source};
use crate::spk;
use crate::time::{DAY, delta_t, ut1_from_tt};

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

/// The zero of `f` between `a` and `b`, where `fa` and `fb` (its values there) differ in sign,
/// by regula falsi with the Illinois modification, until a step is under half a second.
fn illinois(
    f: impl Fn(f64) -> Result<f64, spk::Error>,
    (mut a, mut fa): (f64, f64),
    (mut b, mut fb): (f64, f64),
) -> Result<f64, spk::Error> {
    // Which end the last estimate replaced: -1 for `a`, 1 for `b`.
    let mut side = 0;
    let mut last = f64::NAN;
    let mut c = (a + b) / 2.0;
    for _ in 0..60 {
        c = (fa * b - fb * a) / (fa - fb);
        if (c - last).abs() < 0.5 / DAY {
            break;
        }
        last = c;
        let fc = f(c)?;
        if fc == 0.0 {
            break;
        }
        if (fc < 0.0) == (fb < 0.0) {
            (b, fb) = (c, fc);
            if side == 1 {
                fa /= 2.0;
            }
            side = 1;
        } else {
            (a, fa) = (c, fc);
            if side == -1 {
                fb /= 2.0;
            }
            side = -1;
        }
    }
    Ok(c)
}

/// A crossing of an altitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    /// TT Julian date.
    pub tt: f64,
    /// Rising through it, rather than setting.
    pub rising: bool,
}

/// The Sun's crossings of `altitude` between TT Julian dates `from` and `to`. The Sun's altitude
/// is least at the lower transit and greatest at the upper, so a crossing lies between two
/// transits where the altitude changes sides of `altitude`, and is narrowed there by regula
/// falsi to under a second. The transits fix the brackets, so a crossing is the same whichever
/// window finds it.
pub fn sun_crossings(
    src: &dyn Source,
    from: f64,
    to: f64,
    lat: f64,
    east: f64,
    altitude_: f64,
) -> Result<Vec<Crossing>, spk::Error> {
    let f = |tt: f64| Ok(altitude(src, Body::Sun, tt, lat, east)? - altitude_);
    // Wide enough that every bracket of a crossing in the window has both its transits.
    let transits = sun_transits(src, from - 1.0, to + 1.0, east)?;
    let mut out = Vec::new();
    let mut prev: Option<(f64, f64)> = None;
    for x in transits {
        let cur = (x.tt, f(x.tt)?);
        if let Some(p) = prev
            && (p.1 < 0.0) != (cur.1 < 0.0)
        {
            let tt = illinois(f, p, cur)?;
            if (from..=to).contains(&tt) {
                out.push(Crossing {
                    tt,
                    rising: p.1 < 0.0,
                });
            }
        }
        prev = Some(cur);
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
/// `east` (radians). Each is predicted from local mean noon or midnight of its date and
/// corrected by Newton's steps on the hour angle until a step is under half a second, so a
/// transit is found the same whatever the window. Latitude doesn't enter.
pub fn sun_transits(
    src: &dyn Source,
    from: f64,
    to: f64,
    east: f64,
) -> Result<Vec<Transit>, spk::Error> {
    use std::f64::consts::{PI, TAU};
    // The hour angle's rate, radians per day: a turn per solar day, to a few parts in 10⁴.
    const RATE: f64 = TAU;
    let wrap = |x: f64| x.sin().atan2(x.cos());
    // Local mean noon is at UT1 JD n − east/2π, midnight half a day before; the Sun is within
    // a quarter of an hour of either.
    let offset = east / TAU;
    let first = (from + offset).floor() as i64 - 1;
    let last = (to + offset).ceil() as i64 + 1;
    let mut out = Vec::new();
    for n in first..=last {
        for upper in [true, false] {
            let ut1 = n as f64 - offset - if upper { 0.0 } else { 0.5 };
            let mut tt = ut1 + delta_t(ut1) / DAY;
            if tt < from - 0.05 || tt > to + 0.05 {
                continue;
            }
            for _ in 0..8 {
                let h = hour_angle(src, Body::Sun, tt, east)?;
                let step = wrap(if upper { h } else { h - PI }) / RATE;
                tt -= step;
                if step.abs() < 0.5 / DAY {
                    break;
                }
            }
            if (from..=to).contains(&tt) {
                out.push(Transit { tt, upper });
            }
        }
    }
    out.sort_by(|a, b| a.tt.total_cmp(&b.tt));
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

    // Each transit has the Sun on the meridian to half a second (3.6e-5 rad of hour angle), the
    // upper and lower alternating.
    #[test]
    fn transits_sit_on_the_meridian() {
        let east = (-89.4012f64).to_radians();
        let base = 2461328.0;
        let found = sun_transits(&Builtin, base, base + 6.0, east).unwrap();
        assert_eq!(found.len(), 12, "{found:?}");
        for (k, x) in found.iter().enumerate() {
            let h = hour_angle(&Builtin, Body::Sun, x.tt, east).unwrap();
            let off = if x.upper {
                h
            } else {
                h.abs() - std::f64::consts::PI
            };
            assert!(off.abs() < 3.6e-5, "{x:?} off by {off:e}");
            if k > 0 {
                assert_ne!(x.upper, found[k - 1].upper, "{found:?}");
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
