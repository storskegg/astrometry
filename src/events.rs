//! When things happen: the moments an angle between bodies, or a body's longitude, passes a
//! value. Lunations, ingresses and exact aspects are all such crossings.

use crate::apparent::apparent;
use crate::derived::diff;
use crate::source::{Body, Source};
use crate::spk;

/// How finely crossings are narrowed, days: about 10 ms.
const TOLERANCE: f64 = 1e-7;

/// The TT Julian dates in `from`..`to` at which `f` (a signed angle, degrees, −180..180)
/// rises through zero, found by sampling every `step` days and bisecting. A jump of 180° or
/// more between samples is taken for the angle wrapping round, not a crossing, so `step` must
/// be short enough for the angle to move less than that.
pub fn crossings(
    mut f: impl FnMut(f64) -> Result<f64, spk::Error>,
    from: f64,
    to: f64,
    step: f64,
) -> Result<Vec<f64>, spk::Error> {
    let mut out = Vec::new();
    let (mut t, mut prev) = (from, f(from)?);
    while t < to {
        let u = (t + step).min(to);
        let cur = f(u)?;
        if prev < 0.0 && cur >= 0.0 && cur - prev < 180.0 {
            let (mut lo, mut hi) = (t, u);
            while hi - lo > TOLERANCE {
                let mid = (lo + hi) / 2.0;
                if f(mid)? < 0.0 { lo = mid } else { hi = mid }
            }
            out.push((lo + hi) / 2.0);
        }
        (t, prev) = (u, cur);
    }
    Ok(out)
}

/// The apparent elongation of `a` from `b`, degrees, −180..180: a's longitude less b's.
pub fn elongation(src: &dyn Source, a: Body, b: Body, tt: f64) -> Result<f64, spk::Error> {
    Ok(diff(
        apparent(src, a, tt)?.longitude,
        apparent(src, b, tt)?.longitude,
    ))
}

/// The moments in `from`..`to` when `a`'s longitude less `b`'s passes `angle` degrees going
/// forward: `angle` 0 for a conjunction, 180 for an opposition, and so on.
pub fn aspects(
    src: &dyn Source,
    a: Body,
    b: Body,
    angle: f64,
    from: f64,
    to: f64,
) -> Result<Vec<f64>, spk::Error> {
    crossings(
        |tt| Ok(diff(elongation(src, a, b, tt)?, angle)),
        from,
        to,
        0.5,
    )
}

/// New moons in `from`..`to`, TT Julian dates.
pub fn new_moons(src: &dyn Source, from: f64, to: f64) -> Result<Vec<f64>, spk::Error> {
    aspects(src, Body::Moon, Body::Sun, 0.0, from, to)
}

/// Full moons in `from`..`to`, TT Julian dates.
pub fn full_moons(src: &dyn Source, from: f64, to: f64) -> Result<Vec<f64>, spk::Error> {
    aspects(src, Body::Moon, Body::Sun, 180.0, from, to)
}

/// The moments in `from`..`to` when `body`'s longitude passes `longitude` degrees going
/// forward, sampled every `step` days.
pub fn passes(
    src: &dyn Source,
    body: Body,
    longitude: f64,
    from: f64,
    to: f64,
    step: f64,
) -> Result<Vec<f64>, spk::Error> {
    crossings(
        |tt| Ok(diff(apparent(src, body, tt)?.longitude, longitude)),
        from,
        to,
        step,
    )
}
