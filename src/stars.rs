//! Apparent positions of fixed stars, and their rising and setting.
//!
//! The chain is the one the bodies have in [`crate::apparent`], with a star's own steps first:
//!
//! 1. **Space motion**, rigorously, from the catalogue epoch to the date: the star's
//!    barycentric position vector moves in a straight line at its space velocity, made of the
//!    proper motion and the radial velocity, so the proper motion's own change (the perspective
//!    acceleration) comes out of it. Explanatory Supplement to the Astronomical Almanac
//!    (3rd ed., 2013, §7.2), Kaplan, USNO Circular 179 (2005, ch. 7).
//! 2. **Parallax**: the Earth's barycentric position is subtracted from the star's, so the
//!    direction is the geocentric one (Explanatory Supplement §7.2; Kaplan ch. 7).
//! 3. **Light deflection** by the Sun, and **annual aberration**, relativistically, as
//!    [`crate::apparent`] has them.
//! 4. **Precession** (IAU 2006) and **nutation** (IAU 2000A_R06), to the true equator and
//!    equinox of date, and from there the ecliptic of date.
//!
//! Positions are good anywhere the [`Source`] covers (the built-in one, from 1800 to 2200
//! from DE440, beyond that from the analytical theories); all the reduction takes from it is
//! the Earth's and the Sun's position and the Earth's velocity. Against the Swiss Ephemeris,
//! fed the same catalogue rows, the reduction agrees within 0.0005 mas over 1800–2200 in the
//! ICRS axes, and positions of date within 2.4 mas, which is the two programs' precession and
//! nutation models (see `tests/stars.rs`). The catalogue's own errors, in the proper motion and
//! the parallax, are a good deal larger, and grow with the years from the catalogue epoch.
//!
//! The catalogue, `data/stars.tsv`, has Sirius (HIP 32349) for now.

use std::f64::consts::{PI, TAU};
use std::sync::OnceLock;

use crate::apparent::{Apparent, C, aberrate, deflect};
use crate::horizon::{Crossing, altitude_of, hour_angle_and_declination_of, illinois};
use crate::nutation::ARCSEC;
use crate::precession::{gcrs_to_ecliptic_of_date, gcrs_to_true_equator};
use crate::source::{AU, Body, Source};
use crate::spk;
use crate::time::{DAY, tdb_minus_tt};
use crate::vector::{Vec3, add, norm, scale, sub, unit};

/// A star's astrometry at the catalogue epoch, ICRS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Star {
    /// Hipparcos number.
    pub hip: u32,
    /// Right ascension at the epoch, degrees.
    pub ra: f64,
    /// Declination at the epoch, degrees.
    pub dec: f64,
    /// Proper motion in right ascension times cos δ, mas/yr.
    pub pm_ra: f64,
    /// Proper motion in declination, mas/yr.
    pub pm_dec: f64,
    /// Parallax, mas. At zero or less the star is taken to be infinitely far.
    pub parallax: f64,
    /// Radial velocity, km/s, positive receding.
    pub radial_velocity: f64,
    /// The catalogue epoch as a TT Julian date (Hipparcos: J1991.25 = 2448349.0625).
    pub epoch: f64,
}

const DATA: &str = include_str!("../data/stars.tsv");

/// The catalogue, parsed once.
fn catalogue() -> &'static [Star] {
    static STARS: OnceLock<Vec<Star>> = OnceLock::new();
    STARS.get_or_init(|| {
        DATA.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            // The first line left is the header.
            .skip(1)
            .map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                let num = |i: usize| -> f64 {
                    f.get(i)
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or_else(|| panic!("data/stars.tsv, column {i}: {l:?}"))
                };
                Star {
                    hip: num(0) as u32,
                    ra: num(1),
                    dec: num(2),
                    pm_ra: num(3),
                    pm_dec: num(4),
                    parallax: num(5),
                    radial_velocity: num(6),
                    epoch: num(7),
                }
            })
            .collect()
    })
}

/// The star with Hipparcos number `hip`, if the catalogue has it.
pub fn by_hip(hip: u32) -> Option<Star> {
    catalogue().iter().find(|s| s.hip == hip).copied()
}

/// Sirius, α Canis Majoris (HIP 32349).
pub fn sirius() -> Star {
    by_hip(32349).expect("Sirius is in data/stars.tsv")
}

/// The apparent direction of `star` in GCRS axes (a unit vector) and its distance, au, at TT
/// Julian date `tt`.
fn direction(src: &dyn Source, star: &Star, tt: f64) -> Result<(Vec3, f64), spk::Error> {
    let (ra, dec) = (star.ra.to_radians(), star.dec.to_radians());
    let (sa, ca) = ra.sin_cos();
    let (sd, cd) = dec.sin_cos();
    // The unit vector to the star at the epoch, and east and north at it.
    let r0 = [cd * ca, cd * sa, sd];
    let east = [-sa, ca, 0.0];
    let north = [-ca * sd, -sa * sd, cd];
    // Everything below is the position in units of the star's distance at the epoch, so that
    // a star with no parallax needs no distance. `plx` is the parallax in radians: 1/d, per au.
    let plx = star.parallax.max(0.0) * 1e-3 * ARCSEC;
    // The space velocity, in those units, per day: the proper motion across, and the radial
    // velocity (au/day) over the distance along.
    let mas_per_day = 1e-3 * ARCSEC / 365.25;
    let mu = add(
        scale(star.pm_ra * mas_per_day, east),
        scale(star.pm_dec * mas_per_day, north),
    );
    let rv = star.radial_velocity * DAY / AU;
    let v = add(mu, scale(rv * plx, r0));
    // The barycentric position at the date: a straight line.
    let s = add(r0, scale(tt - star.epoch, v));

    // TDB as seconds from `tt`, not added to it (see `Source::state_after`).
    let tdb_tt = tdb_minus_tt(tt);
    let (earth, earth_v) = src.state_after(Body::Earth, tt, tdb_tt)?;
    let (sun, _) = src.state_after(Body::Sun, tt, tdb_tt)?;

    // Parallax: the geocentric direction.
    let u = sub(s, scale(plx, earth));
    let mut p = unit(u);
    // Light deflection by the Sun: q is from the Sun to the star, e from the Sun to the Earth.
    let q = unit(sub(s, scale(plx, sun)));
    let e = sub(earth, sun);
    p = deflect(p, q, unit(e), norm(e));
    p = aberrate(p, scale(1.0 / C, earth_v));
    let distance = if star.parallax > 0.0 {
        norm(u) / plx
    } else {
        f64::INFINITY
    };
    Ok((p, distance))
}

/// The apparent geocentric position of `star` at TT Julian date `tt`, with the ecliptic
/// longitude and latitude of date from the true equinox, as for the bodies. The distance is in
/// au, `f64::INFINITY` when the parallax is zero or less.
pub fn apparent(src: &dyn Source, star: &Star, tt: f64) -> Result<Apparent, spk::Error> {
    let (p, distance) = direction(src, star, tt)?;
    let ecl = gcrs_to_ecliptic_of_date(tt).apply(p);
    Ok(Apparent {
        longitude: ecl[1].atan2(ecl[0]).to_degrees().rem_euclid(360.0),
        latitude: ecl[2].clamp(-1.0, 1.0).asin().to_degrees(),
        distance,
        direction: p,
    })
}

/// The apparent right ascension (0–360) and declination of `star`, degrees, at TT Julian date
/// `tt`, on the true equator and equinox of date.
pub fn equatorial(src: &dyn Source, star: &Star, tt: f64) -> Result<(f64, f64), spk::Error> {
    let p = gcrs_to_true_equator(tt).apply(direction(src, star, tt)?.0);
    Ok((
        p[1].atan2(p[0]).to_degrees().rem_euclid(360.0),
        p[2].clamp(-1.0, 1.0).asin().to_degrees(),
    ))
}

/// The geocentric apparent altitude of `star` at TT Julian date `tt`, seen from latitude `lat`
/// and east longitude `east` (radians), radians; no refraction.
pub fn altitude(
    src: &dyn Source,
    star: &Star,
    tt: f64,
    lat: f64,
    east: f64,
) -> Result<f64, spk::Error> {
    let (h, dec) = hour_angle_and_declination_of(direction(src, star, tt)?.0, tt, east);
    Ok(altitude_of(h, dec, lat))
}

/// The star's hour angle at `tt`, wrapped to (−π, π].
fn hour_angle(src: &dyn Source, star: &Star, tt: f64, east: f64) -> Result<f64, spk::Error> {
    Ok(hour_angle_and_declination_of(direction(src, star, tt)?.0, tt, east).0)
}

/// The star's transits of the meridian between TT Julian dates `from` and `to`, upper
/// (`true`) and lower, in order. The star's hour angle runs at the sidereal rate to a few
/// parts in 10⁷ (its proper motion and the precession of the equinox are slower still), so
/// each transit is predicted from the hour angle at `from` and corrected by Newton's steps to
/// a fraction of a millisecond, which does not depend on the window.
fn transits(
    src: &dyn Source,
    star: &Star,
    from: f64,
    to: f64,
    east: f64,
) -> Result<Vec<(f64, bool)>, spk::Error> {
    // Radians per day: a turn per sidereal day (the Earth Rotation Angle's rate, IERS
    // Conventions 2010 eq. 5.15).
    const RATE: f64 = TAU * 1.002_737_811_911_354_5;
    let wrap = |x: f64| x.sin().atan2(x.cos());
    let half = PI / RATE;
    let upper0 = from - hour_angle(src, star, from, east)? / RATE;
    let kmin = ((from - upper0) / half).floor() as i64 - 1;
    let kmax = ((to - upper0) / half).ceil() as i64 + 1;
    let mut out = Vec::new();
    for k in kmin..=kmax {
        let upper = k.rem_euclid(2) == 0;
        let mut tt = upper0 + k as f64 * half;
        for _ in 0..10 {
            let h = hour_angle(src, star, tt, east)?;
            let step = wrap(if upper { h } else { h - PI }) / RATE;
            tt -= step;
            if step.abs() < 2e-9 {
                break;
            }
        }
        if (from..=to).contains(&tt) {
            out.push((tt, upper));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(out)
}

/// The star's crossings of `altitude` (radians) between TT Julian dates `from` and `to`, seen
/// from latitude `lat` and east longitude `east` (radians), no refraction. As for the Sun
/// ([`crate::horizon::sun_crossings`]): the altitude is least at the lower transit and greatest
/// at the upper, so a crossing lies between two transits where the altitude changes sides of
/// `altitude`, and is narrowed there by regula falsi to under a second. A star that stays above
/// or below `altitude` all day has none.
pub fn crossings(
    src: &dyn Source,
    star: &Star,
    from: f64,
    to: f64,
    lat: f64,
    east: f64,
    altitude_: f64,
) -> Result<Vec<Crossing>, spk::Error> {
    let f = |tt: f64| Ok(altitude(src, star, tt, lat, east)? - altitude_);
    // Wide enough that every bracket of a crossing in the window has both its transits.
    let mut out = Vec::new();
    let mut prev: Option<(f64, f64)> = None;
    for (t, _) in transits(src, star, from - 1.0, to + 1.0, east)? {
        let cur = (t, f(t)?);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Builtin;

    #[test]
    fn catalogue_has_sirius() {
        let s = sirius();
        assert_eq!(s.hip, 32349);
        assert!((s.parallax - 379.21).abs() < 1e-9 && s.epoch == 2448349.0625);
        assert!(by_hip(1).is_none());
    }

    // Sirius in 2000: right ascension 6h45m, declination −16°43′, as the almanacs have it, and
    // its distance 2.64 pc.
    #[test]
    fn sirius_in_2000() {
        let (ra, dec) = equatorial(&Builtin, &sirius(), 2451545.0).unwrap();
        assert!(
            (ra - 101.2866).abs() < 0.01 && (dec - -16.7161).abs() < 0.01,
            "{ra} {dec}"
        );
        let a = apparent(&Builtin, &sirius(), 2451545.0).unwrap();
        assert!(
            (a.distance / 206264.806 - 2.637).abs() < 0.001,
            "{}",
            a.distance
        );
    }

    // No parallax: infinitely far, and still a position.
    #[test]
    fn no_parallax_is_infinitely_far() {
        let star = Star {
            parallax: 0.0,
            ..sirius()
        };
        let a = apparent(&Builtin, &star, 2460000.5).unwrap();
        assert!(a.distance.is_infinite() && a.longitude.is_finite() && a.latitude.is_finite());
        let b = apparent(
            &Builtin,
            &Star {
                parallax: -3.0,
                ..star
            },
            2460000.5,
        )
        .unwrap();
        assert_eq!(a, b);
    }

    // Sirius, at 25.7° N, rises and sets once a day, and the altitude is zero there.
    #[test]
    fn crossings_sit_on_the_horizon() {
        let (lat, east) = (25.6995f64.to_radians(), 32.6391f64.to_radians());
        let (a, b) = (2461000.0, 2461004.0);
        let found = crossings(&Builtin, &sirius(), a, b, lat, east, 0.0).unwrap();
        assert_eq!(found.len(), 8, "{found:?}");
        for (k, x) in found.iter().enumerate() {
            let h = altitude(&Builtin, &sirius(), x.tt, lat, east).unwrap();
            assert!(h.abs() < 1e-6, "{x:?} at {h:e}");
            if k > 0 {
                assert_ne!(x.rising, found[k - 1].rising);
            }
        }
        // The same crossing, whatever the window.
        let w = crossings(&Builtin, &sirius(), a + 0.37, b - 0.9, lat, east, 0.0).unwrap();
        assert!(w.len() >= 4);
        for x in &w {
            assert!(
                found
                    .iter()
                    .any(|y| (y.tt - x.tt).abs() < 1e-8 && y.rising == x.rising)
            );
        }
    }

    // Sirius never rises at 80° N: no crossings of the horizon.
    #[test]
    fn circumpolar_has_none() {
        let (lat, east) = (80f64.to_radians(), 0.0);
        let found = crossings(&Builtin, &sirius(), 2461000.0, 2461003.0, lat, east, 0.0).unwrap();
        assert!(found.is_empty(), "{found:?}");
    }
}
