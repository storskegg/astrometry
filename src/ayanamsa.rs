//! Ayanamsas: the angle from the tropical zodiac's 0° (the equinox of date) to a sidereal
//! zodiac's 0° (a point fixed among the stars). It grows by the general precession in
//! longitude, about 50″ a year.
//!
//! Each value is a defining constant at its epoch, carried to the date by the IAU 2006
//! general precession in longitude p_A (Capitaine, Wallace and Chapront 2003, table 1; Hilton
//! et al. 2006). Both functions *include* the nutation in longitude, so that a sidereal
//! longitude is the apparent (true-equinox) tropical longitude minus the ayanamsa: that is
//! what the Swiss Ephemeris' sidereal positions come to (its ayanamsa, with nutation, differs
//! from the mean one by Δψ, as its sidereal Sun and Moon show).

use crate::nutation::{ARCSEC, nutation};
use crate::time::centuries;

/// The IAU 2006 general precession in longitude since J2000, arcseconds, at TT Julian date `jd`.
fn general_precession(jd: f64) -> f64 {
    let t = centuries(jd);
    t * (5028.796195 + t * (1.1054348 + t * (0.00007964 + t * (-0.000023857 + t * -0.0000000383))))
}

/// The mean ayanamsa, degrees, of a zodiac whose value was `value` degrees at TT Julian date
/// `epoch`.
fn mean(tt: f64, epoch: f64, value: f64) -> f64 {
    value + (general_precession(tt) - general_precession(epoch)) / 3600.0
}

/// Nutation in longitude, degrees.
fn nutation_deg(tt: f64) -> f64 {
    (nutation(tt).0 / ARCSEC) / 3600.0
}

/// Besselian 1950.0, the epoch of Fagan and Bradley's value.
const FAGAN_BRADLEY_EPOCH: f64 = 2_433_282.423_46;
/// Fagan and Bradley's synetic vernal point at 1950.0, as published: 24°02′31.36″ (Fagan and
/// Bradley, 1950; Fagan, *Astrological Origins*). Taken in practice 0.41″ lower, 24°02′30.95″,
/// which is the value the Swiss Ephemeris carries and ours matches to 0.01″: the sources
/// only give the arcsecond to two places and don't say which equinox, mean or true, it is
/// measured from.
const FAGAN_BRADLEY_AT_EPOCH: f64 = 24.0 + 2.0 / 60.0 + 30.95 / 3600.0;

/// 21 March 1956, 0h, the date the Calendar Reform Committee fixed the Indian zodiac at.
const LAHIRI_EPOCH: f64 = 2_435_553.5;
/// The Calendar Reform Committee's (Government of India, 1955) value for 21 March 1956:
/// 23°15′00″, measured from the true equinox of that date, so it includes the nutation then.
/// Taken in practice as 23°15′00.80″, as the Swiss Ephemeris carries it (it adds 0.66″ to the
/// round figure) and ours matches to 0.01″, for the same reason as Fagan-Bradley's.
const LAHIRI_AT_EPOCH: f64 = 23.0 + 15.0 / 60.0 + 0.7975 / 3600.0;

/// The Fagan-Bradley ayanamsa at TT Julian date `tt`, degrees, including the nutation in
/// longitude.
pub fn fagan_bradley(tt: f64) -> f64 {
    mean(tt, FAGAN_BRADLEY_EPOCH, FAGAN_BRADLEY_AT_EPOCH) + nutation_deg(tt)
}

/// The Lahiri (Chitrapaksha) ayanamsa at TT Julian date `tt`, degrees, including the nutation
/// in longitude.
pub fn lahiri(tt: f64) -> f64 {
    mean(
        tt,
        LAHIRI_EPOCH,
        LAHIRI_AT_EPOCH - nutation_deg(LAHIRI_EPOCH),
    ) + nutation_deg(tt)
}
