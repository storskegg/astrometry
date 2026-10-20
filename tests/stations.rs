//! Stations, the Moon's light, and the angles, against the almanacs and their own geometry.

use astrometry::angles::angles;
use astrometry::derived::{moon_light, speed};
use astrometry::events::{full_moons, new_moons};
use astrometry::source::{Body, Builtin};
use astrometry::stations::{next_station, previous_station, stations};
use astrometry::time::{calendar, civil_from_tt, julian_date, tt_from_civil};

fn tt(y: i32, m: u8, d: u8) -> f64 {
    tt_from_civil(julian_date(y, m, d, 0, 0, 0.0))
}

/// The UTC date of TT Julian date `tt`.
fn date(tt: f64) -> (i32, u8, u8) {
    let (y, m, d, _) = calendar(civil_from_tt(tt));
    (y, m, d)
}

// Mercury's stations in 2026, on the dates the almanacs give (UTC).
#[test]
fn mercury_in_2026() {
    let got: Vec<_> = stations(&Builtin, Body::Mercury, tt(2026, 1, 1), tt(2027, 1, 1))
        .unwrap()
        .into_iter()
        .map(|s| (s.retrograde, date(s.tt)))
        .collect();
    assert_eq!(
        got,
        [
            (true, (2026, 2, 26)),
            (false, (2026, 3, 20)),
            (true, (2026, 6, 29)),
            (false, (2026, 7, 23)),
            (true, (2026, 10, 24)),
            (false, (2026, 11, 13)),
        ]
    );
}

// Venus retrograde from 3 October to 14 November 2026: the stations either side of 15 October,
// where it stands still; and none for the Sun.
#[test]
fn either_side() {
    let now = tt(2026, 10, 15);
    let before = previous_station(&Builtin, Body::Venus, now)
        .unwrap()
        .unwrap();
    let after = next_station(&Builtin, Body::Venus, now).unwrap().unwrap();
    assert!(
        before.retrograde && date(before.tt) == (2026, 10, 3),
        "{before:?}"
    );
    assert!(
        !after.retrograde && date(after.tt) == (2026, 11, 14),
        "{after:?}"
    );
    assert!(speed(&Builtin, Body::Venus, after.tt).unwrap().abs() < 1e-3);
    assert!(next_station(&Builtin, Body::Sun, now).unwrap().is_none());
    assert!(
        stations(&Builtin, Body::Moon, now, now + 30.0)
            .unwrap()
            .is_empty()
    );
}

// The Moon dark at a new Moon and full at a full one (as the lunations find them), half lit
// at the quarter between, and its light growing as it waxes.
#[test]
fn moon_light_through_a_month() {
    let from = tt(2026, 10, 1);
    let new = new_moons(&Builtin, from, from + 30.0).unwrap()[0];
    let full = full_moons(&Builtin, new, new + 30.0).unwrap()[0];
    let (e, lit) = moon_light(&Builtin, new).unwrap();
    assert!(!(0.5..359.5).contains(&e) && lit < 0.001, "new: {e} {lit}");
    let (e, lit) = moon_light(&Builtin, full).unwrap();
    assert!((e - 180.0).abs() < 0.5 && lit > 0.995, "full: {e} {lit}");
    let (e, lit) = moon_light(&Builtin, (new + full) / 2.0).unwrap();
    assert!(
        (60.0..120.0).contains(&e) && (0.3..0.7).contains(&lit),
        "quarter: {e} {lit}"
    );
    let (_, l1) = moon_light(&Builtin, new + 3.0).unwrap();
    let (_, l2) = moon_light(&Builtin, new + 9.0).unwrap();
    assert!(l1 < l2, "{l1} {l2}");
}

// The Ascendant and Midheaven 90° of each other's opposites at the equator's equinox moment,
// and always one sign's turn apart in the day: a quick sanity of the moved formula.
#[test]
fn angles_turn_with_the_day() {
    let t = tt(2026, 3, 20);
    let a = angles(t, 43.07, -89.40);
    let b = angles(t + 1.0 / 24.0, 43.07, -89.40);
    // An hour turns the Midheaven by about 15° of right ascension: 13–17° of longitude.
    let turned = (b.midheaven - a.midheaven).rem_euclid(360.0);
    assert!((12.0..18.0).contains(&turned), "{turned}");
    let east = (a.ascendant - a.midheaven).rem_euclid(360.0);
    assert!(east > 0.0 && east < 180.0, "{a:?}");
}
