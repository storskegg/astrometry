//! The Moon: its phase, the lunations around a moment, and its course to the end of its sign.

use astrometry::derived::{diff, moon_light};
use astrometry::events::{full_moons, new_moons};
use astrometry::source::Source;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

use crate::aspect::Kind;
use crate::chart::Zodiac;
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
///
/// The sign is the tropical one; for a sidereal zodiac see [`course_in`]. [`crate::Sky`] stays
/// tropical.
pub fn course(src: &dyn Source, tt: f64, tz: &TimeZone) -> Result<Course, EphemerisError> {
    course_with(src, tt, tz, Zodiac::Tropical)
}

/// [`course`] in a chosen zodiac: [`Zodiac::Tropical`] gives `course`'s result exactly.
///
/// In a sidereal zodiac, `leaves_sign` is when the Moon's sidereal longitude (the apparent
/// tropical longitude less the ayanamsa at that moment, see [`crate::chart::positions_in`]) reaches
/// the next multiple of 30°. The aspects are the same Ptolemaic ones perfected before then:
/// their times don't depend on the zodiac, only the end of the window moves, so the Moon is
/// void of course by the chosen zodiac's sign.
pub fn course_in(
    src: &dyn Source,
    tt: f64,
    tz: &TimeZone,
    zodiac: Zodiac,
) -> Result<Course, EphemerisError> {
    course_with(src, tt, tz, zodiac)
}

fn course_with(
    src: &dyn Source,
    tt: f64,
    tz: &TimeZone,
    zodiac: Zodiac,
) -> Result<Course, EphemerisError> {
    // What is taken from the Moon's tropical longitude at a moment, to give it in the zodiac.
    let offset = |t: f64| match zodiac {
        Zodiac::Tropical => 0.0,
        Zodiac::Sidereal(a) => a.at(t),
    };
    const STEP: f64 = 1.0 / 24.0;
    const MAX_DAYS: f64 = 3.0; // the Moon crosses a sign in under 2.7 days
    let moon = Planet::Moon.index();
    let now = longitudes(src, tt)?;
    let here = match zodiac {
        Zodiac::Tropical => now[moon],
        Zodiac::Sidereal(_) => (now[moon] - offset(tt)).rem_euclid(360.0),
    };
    let cusp = ((here / 30.0).floor() + 1.0) * 30.0;
    let past_cusp = |l: &[f64; 7], t: f64| diff(l[moon] - offset(t), cusp);
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
        |(p, _, angle): (Planet, Kind, f64), l: &[f64; 7]| diff(l[moon] - l[p.index()], angle);
    let narrow = |mut lo: f64,
                  mut hi: f64,
                  f: &dyn Fn(&[f64; 7], f64) -> f64|
     -> Result<f64, EphemerisError> {
        while hi - lo > 30.0 / 86400.0 {
            let mid = (lo + hi) / 2.0;
            if f(&longitudes(src, mid)?, mid) < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Ok((lo + hi) / 2.0)
    };
    let (mut prev_t, mut prev_cusp) = (tt, past_cusp(&now, tt));
    let mut prev: Vec<f64> = targets.iter().map(|&t| past_exact(t, &now)).collect();
    let mut found: Vec<(Planet, Kind, f64)> = Vec::new();
    let mut left = None;
    let mut t = tt + STEP;
    while t <= tt + MAX_DAYS {
        let cur = longitudes(src, t)?;
        let c = past_cusp(&cur, t);
        if prev_cusp < 0.0 && c >= 0.0 {
            left = Some(narrow(prev_t, t, &past_cusp)?);
        }
        for (i, &tg) in targets.iter().enumerate() {
            let v = past_exact(tg, &cur);
            // A crossing from below, not the jump at ±180° opposite it.
            if prev[i] < 0.0 && v >= 0.0 && v - prev[i] < 90.0 {
                let at = narrow(prev_t, t, &|l, _| past_exact(tg, l))?;
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

    use crate::chart::{Ayanamsa, Point, positions_in};
    use astrometry::source::Builtin;

    const START: f64 = 2461328.25;
    const SIDEREALS: [Zodiac; 2] = [
        Zodiac::Sidereal(Ayanamsa::FaganBradley),
        Zodiac::Sidereal(Ayanamsa::Lahiri),
    ];

    fn moon_in(tt: f64, zodiac: Zodiac) -> f64 {
        positions_in(&Builtin, tt, 0.0, 0.0, &[Point::Moon], zodiac).unwrap()[0].longitude
    }

    #[test]
    fn tropical_course_is_unchanged() {
        let tz = TimeZone::UTC;
        for i in 0..(60 * 24 / 7) {
            let tt = START + f64::from(i) * 7.0 / 24.0;
            assert_eq!(
                course(&Builtin, tt, &tz).unwrap(),
                course_in(&Builtin, tt, &tz, Zodiac::Tropical).unwrap(),
                "{tt}"
            );
        }
    }

    #[test]
    fn sidereal_course_ends_at_the_sidereal_cusp() {
        let tz = TimeZone::UTC;
        for zodiac in SIDEREALS {
            for i in 0..8 {
                let tt = START + f64::from(i) * 1.7;
                let c = course_in(&Builtin, tt, &tz, zodiac).unwrap();
                let end = time::tt(c.leaves_sign.timestamp());
                assert!(end > tt && end - tt < 3.0, "{zodiac:?} {tt}");
                let at = moon_in(end, zodiac);
                let off = (at / 30.0).round() * 30.0 - at;
                // The search narrows to within 30 s; the Moon moves 1.5e-4° a second.
                assert!(off.abs() < 5e-3, "{zodiac:?} {tt}: {at}");
                let before = (moon_in(tt, zodiac) / 30.0).floor();
                let just = (moon_in(end - 1.0 / 1440.0, zodiac) / 30.0).floor();
                assert_eq!(before, just, "{zodiac:?} {tt}: still in the first sign");
                for a in &c.aspects {
                    assert!(a.exact.timestamp() < c.leaves_sign.timestamp());
                }
            }
        }
    }

    #[test]
    fn the_zodiac_decides_void_of_course() {
        let tz = TimeZone::UTC;
        let z = Zodiac::Sidereal(Ayanamsa::FaganBradley);
        // Found by searching half-daily from START: void in one zodiac and not the other.
        for (tt, tropical_void) in [(2461329.75, true), (2461331.75, false)] {
            let t = course(&Builtin, tt, &tz).unwrap();
            let s = course_in(&Builtin, tt, &tz, z).unwrap();
            assert_eq!(t.is_void_of_course(), tropical_void, "{tt}");
            assert_eq!(s.is_void_of_course(), !tropical_void, "{tt}");
            assert_ne!(t.leaves_sign, s.leaves_sign);
        }
    }
}
