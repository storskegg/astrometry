//! The Moon: its phase, the lunations around a moment, and its course to the end of its sign.

use astrometry::derived::{diff, moon_light};
use astrometry::events::{full_moons, new_moons};
use astrometry::source::Source;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

use crate::aspect::Kind;
use crate::position::longitudes;
use crate::zodiac::Planet;
use crate::{EphemerisError, time};

named_enum!(
    /// One of the Moon's eight phases, each 45° of its elongation from the Sun, beginning at
    /// the conjunction, as astrologers divide them.
    Phase, "phase", [
        New => "New", Crescent => "Crescent", FirstQuarter => "First Quarter",
        Gibbous => "Gibbous", Full => "Full", Disseminating => "Disseminating",
        LastQuarter => "Last Quarter", Balsamic => "Balsamic",
    ]
);

impl Phase {
    /// The phase at an elongation, the Moon's longitude less the Sun's, degrees.
    pub fn of(elongation: f64) -> Phase {
        Phase::ALL[(elongation.rem_euclid(360.0) / 45.0) as usize % 8]
    }

    /// Whether the Moon grows in this phase: from New to Full.
    pub fn is_waxing(self) -> bool {
        (self as u8) < (Phase::Full as u8)
    }
}

/// How the Moon looks.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Appearance {
    pub phase: Phase,
    /// The Moon's longitude less the Sun's, degrees, 0–360.
    pub elongation: f64,
    /// The fraction of the disc lit, 0 to 1.
    pub illuminated: f64,
}

/// How the Moon looks at TT Julian date `tt`: its light, astrometry's, and its phase.
pub fn appearance(src: &dyn Source, tt: f64) -> Result<Appearance, EphemerisError> {
    let (elongation, illuminated) = moon_light(src, tt)?;
    Ok(Appearance {
        phase: Phase::of(elongation),
        elongation,
        illuminated,
    })
}

/// The New and Full Moons either side of a moment.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Lunations {
    pub previous_new: Zoned,
    pub previous_full: Zoned,
    pub next_new: Zoned,
    pub next_full: Zoned,
}

const SYNODIC_MONTH: f64 = 29.530588861;

/// A search for new or full moons between two TT Julian dates.
type Search = fn(&dyn Source, f64, f64) -> Result<Vec<f64>, EphemerisError>;

/// The lunations either side of TT Julian date `tt`, in `tz`.
pub fn lunations(src: &dyn Source, tt: f64, tz: &TimeZone) -> Result<Lunations, EphemerisError> {
    let around = |find: Search| -> Result<(Zoned, Zoned), EphemerisError> {
        let before = find(src, tt - SYNODIC_MONTH - 1.0, tt)?;
        let after = find(src, tt, tt + SYNODIC_MONTH + 1.0)?;
        let prev = *before
            .iter()
            .rfind(|&&x| x <= tt)
            .expect("a lunation in the month before");
        let next = *after
            .iter()
            .find(|&&x| x > tt)
            .expect("a lunation in the month after");
        Ok((time::zoned(prev, tz), time::zoned(next, tz)))
    };
    let (previous_new, next_new) = around(new_moons)?;
    let (previous_full, next_full) = around(full_moons)?;
    Ok(Lunations {
        previous_new,
        previous_full,
        next_new,
        next_full,
    })
}

/// The Moon's course to the end of its sign.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Course {
    /// When the Moon next enters a sign.
    pub leaves_sign: Zoned,
    /// The Ptolemaic aspects the Moon perfects with the planets before it leaves its sign, in
    /// order: none, and it's void of course.
    pub aspects: Vec<MoonAspect>,
}

impl Course {
    /// The first aspect the Moon perfects.
    pub fn next_aspect(&self) -> Option<&MoonAspect> {
        self.aspects.first()
    }

    /// The first aspect the Moon perfects after `t`: none, and it's void of course then.
    pub fn next_aspect_after(&self, t: Timestamp) -> Option<&MoonAspect> {
        self.aspects.iter().find(|a| a.exact.timestamp() > t)
    }

    pub fn is_void_of_course(&self) -> bool {
        self.aspects.is_empty()
    }
}

/// An aspect the Moon will perfect.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MoonAspect {
    pub planet: Planet,
    pub kind: Kind,
    pub exact: Zoned,
}

/// Follows the Moon from TT Julian date `tt` until it leaves its sign: when it does, and the
/// Ptolemaic aspects it perfects with the planets on the way. It samples hourly,
/// which the Moon's half-degree an hour makes safe, and narrows each event to under a minute.
/// The Moon is faster than every planet, so its distance past each exact aspect only grows:
/// each perfection is a crossing of zero from below.
pub fn course(src: &dyn Source, tt: f64, tz: &TimeZone) -> Result<Course, EphemerisError> {
    const STEP: f64 = 1.0 / 24.0;
    const MAX_DAYS: f64 = 3.0; // the Moon crosses a sign in under 2.7 days
    let moon = Planet::Moon.index();
    let now = longitudes(src, tt)?;
    let cusp = ((now[moon] / 30.0).floor() + 1.0) * 30.0;
    let past_cusp = |l: &[f64; 7]| diff(l[moon], cusp);
    let mut targets = Vec::new();
    for p in Planet::ALL.into_iter().filter(|p| *p != Planet::Moon) {
        for k in Kind::ALL {
            let a = k.angle();
            targets.push((p, k, a));
            if a != 0.0 && a != 180.0 {
                targets.push((p, k, -a));
            }
        }
    }
    let past_exact =
        |(p, _, offset): (Planet, Kind, f64), l: &[f64; 7]| diff(l[moon] - l[p.index()], offset);
    let narrow =
        |mut lo: f64, mut hi: f64, f: &dyn Fn(&[f64; 7]) -> f64| -> Result<f64, EphemerisError> {
            while hi - lo > 30.0 / 86400.0 {
                let mid = (lo + hi) / 2.0;
                if f(&longitudes(src, mid)?) < 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            Ok((lo + hi) / 2.0)
        };
    let (mut prev_t, mut prev_cusp) = (tt, past_cusp(&now));
    let mut prev: Vec<f64> = targets.iter().map(|&t| past_exact(t, &now)).collect();
    let mut found: Vec<(Planet, Kind, f64)> = Vec::new();
    let mut left = None;
    let mut t = tt + STEP;
    while t <= tt + MAX_DAYS {
        let cur = longitudes(src, t)?;
        let c = past_cusp(&cur);
        if prev_cusp < 0.0 && c >= 0.0 {
            left = Some(narrow(prev_t, t, &past_cusp)?);
        }
        for (i, &tg) in targets.iter().enumerate() {
            let v = past_exact(tg, &cur);
            // A crossing from below, not the jump at ±180° opposite it.
            if prev[i] < 0.0 && v >= 0.0 && v - prev[i] < 90.0 {
                let at = narrow(prev_t, t, &|l| past_exact(tg, l))?;
                if left.is_none_or(|l| at < l) {
                    found.push((tg.0, tg.1, at));
                }
            }
            prev[i] = v;
        }
        if left.is_some() {
            break;
        }
        (prev_t, prev_cusp) = (t, c);
        t += STEP;
    }
    found.sort_by(|a, b| a.2.total_cmp(&b.2));
    Ok(Course {
        leaves_sign: time::zoned(left.unwrap_or(tt + MAX_DAYS), tz),
        aspects: found
            .into_iter()
            .map(|(planet, kind, at)| MoonAspect {
                planet,
                kind,
                exact: time::zoned(at, tz),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases() {
        let tests = [
            (0.0, Phase::New),
            (44.9, Phase::New),
            (45.0, Phase::Crescent),
            (180.0, Phase::Full),
            (359.0, Phase::Balsamic),
            (-1.0, Phase::Balsamic),
        ];
        for (e, want) in tests {
            assert_eq!(Phase::of(e), want, "{e}°");
        }
        assert!(Phase::Gibbous.is_waxing() && !Phase::Full.is_waxing());
    }
}
