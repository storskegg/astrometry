//! Stations: the moments a planet stands still in apparent longitude and turns, retrograde or
//! direct. The Sun and Moon never do; nor, seen from it, the Earth.

use crate::apparent::apparent;
use crate::derived::speed;
use crate::source::{Body, Source};
use crate::spk;

/// A station.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Station {
    /// When, TT Julian date.
    pub tt: f64,
    /// Whether it turns retrograde here; else direct.
    pub retrograde: bool,
    /// Where it stands, apparent ecliptic longitude, degrees.
    pub longitude: f64,
}

/// The longest `body` goes between stations, days, with room to spare: its synodic period
/// less its shortest retrograde. None for the Sun, Moon and Earth, which never station.
pub fn longest_between(body: Body) -> Option<f64> {
    match body {
        Body::Sun | Body::Moon | Body::Earth => None,
        Body::Mercury => Some(120.0), // direct some 95 days
        Body::Venus => Some(580.0),   // 545
        Body::Mars => Some(760.0),    // 720
        Body::Jupiter => Some(300.0), // 280
        Body::Saturn => Some(260.0),  // 240
    }
}

/// How far apart a planet's speed is sampled, days: a quarter of its briefest motion between
/// stations, its retrograde (Mercury's some 20 days, Venus's 40, Mars's 58, Jupiter's 118,
/// Saturn's 136), so that no two stations fall between samples.
fn step(body: Body) -> f64 {
    match body {
        Body::Mercury => 5.0,
        Body::Venus => 10.0,
        Body::Mars => 14.0,
        Body::Jupiter => 29.0,
        Body::Saturn => 34.0,
        Body::Sun | Body::Moon | Body::Earth => f64::INFINITY,
    }
}

/// How finely stations are narrowed, days: about 9 s. The speeds are differences over
/// ±0.01 day, so finer would be no truer.
const TOLERANCE: f64 = 1e-4;

/// `body`'s stations between TT Julian dates `from` and `to`, in order: its speed in
/// longitude sampled, and each change of sign narrowed by bisection.
pub fn stations(
    src: &dyn Source,
    body: Body,
    from: f64,
    to: f64,
) -> Result<Vec<Station>, spk::Error> {
    if longest_between(body).is_none() {
        return Ok(Vec::new());
    }
    let v = |tt| speed(src, body, tt);
    let mut out = Vec::new();
    let (mut t, mut prev) = (from, v(from)?);
    while t < to {
        let u = (t + step(body)).min(to);
        let cur = v(u)?;
        if (prev < 0.0) != (cur < 0.0) {
            let (mut lo, mut hi) = (t, u);
            while hi - lo > TOLERANCE {
                let mid = (lo + hi) / 2.0;
                if (v(mid)? < 0.0) == (prev < 0.0) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let tt = (lo + hi) / 2.0;
            out.push(Station {
                tt,
                // Falling through zero, it turns retrograde.
                retrograde: cur < 0.0,
                longitude: apparent(src, body, tt)?.longitude,
            });
        }
        (t, prev) = (u, cur);
    }
    Ok(out)
}

/// `body`'s last station at or before TT Julian date `tt`; none for the Sun, Moon and Earth.
pub fn previous_station(
    src: &dyn Source,
    body: Body,
    tt: f64,
) -> Result<Option<Station>, spk::Error> {
    let Some(span) = longest_between(body) else {
        return Ok(None);
    };
    Ok(stations(src, body, tt - span, tt)?.pop())
}

/// `body`'s first station after TT Julian date `tt`; none for the Sun, Moon and Earth.
pub fn next_station(src: &dyn Source, body: Body, tt: f64) -> Result<Option<Station>, spk::Error> {
    let Some(span) = longest_between(body) else {
        return Ok(None);
    };
    Ok(stations(src, body, tt, tt + span)?.into_iter().next())
}
