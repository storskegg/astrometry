//! Sirius against the Swiss Ephemeris (JPL mode, `tests/data/swisseph/stars.tsv`), which was
//! given this crate's own catalogue row: the reduction in the ICRS axes, the apparent right
//! ascension, declination, longitude and latitude of date, and the rising and setting at Luxor
//! and Heliopolis. The heliacal risings in the file are for later use and not tested here.

use std::path::PathBuf;

use astrometry::source::Builtin;
use astrometry::stars::{apparent, crossings, equatorial, sirius};
use astrometry::time::{DAY, delta_t, ut1_from_tt};

fn rows(kind: &str) -> Vec<Vec<String>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/swisseph/stars.tsv");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| l.split('\t').map(str::to_string).collect::<Vec<_>>())
        .filter(|f| f[0] == kind)
        .collect()
}

/// The angle between two directions given as (longitude, latitude) in degrees, milliarcseconds.
fn separation_mas(a: (f64, f64), b: (f64, f64)) -> f64 {
    let dlon = (a.0 - b.0 + 540.0).rem_euclid(360.0) - 180.0;
    let dlat = a.1 - b.1;
    // Small angles: the east-west difference shrinks with the cosine of the latitude.
    let x = dlon * ((a.1 + b.1) / 2.0).to_radians().cos();
    x.hypot(dlat) * 3600.0 * 1000.0
}

/// The worst difference (mas, at which JD) and the count, over the rows of `kind` in `jpl` mode.
/// The rows in the Swiss Ephemeris' own mode (near 1300 BCE, where DE440 does not reach, and our
/// analytical theories stand in for it) are printed, not counted. `ours` gives the equatorial
/// and the ecliptic position at a TT Julian date; `columns` is how many of those the file has
/// (2: the equatorial one, 4: both).
fn compare(
    kind: &str,
    ours: impl Fn(f64) -> ((f64, f64), (f64, f64)),
    columns: usize,
) -> ((f64, f64), usize) {
    let mut worst = (0.0f64, 0.0f64);
    let mut n = 0;
    for f in rows(kind) {
        let (mode, jd): (&str, f64) = (&f[1], f[2].parse().unwrap());
        let v: Vec<f64> = f[3..3 + columns]
            .iter()
            .map(|s| s.parse().unwrap())
            .collect();
        let (eq, ecl) = ours(jd);
        let d_eq = separation_mas(eq, (v[0], v[1]));
        let d_ecl = if columns == 4 {
            separation_mas(ecl, (v[2], v[3]))
        } else {
            0.0
        };
        if mode == "jpl" {
            n += 1;
            if d_eq.max(d_ecl) > worst.0 {
                worst = (d_eq.max(d_ecl), jd);
            }
        } else {
            println!(
                "{kind} {mode} JD {jd:.1} (1300 BCE): {d_eq:.3} mas (ecliptic {d_ecl:.3} mas)"
            );
        }
    }
    (worst, n)
}

fn positions(jd: f64) -> ((f64, f64), (f64, f64)) {
    let star = sirius();
    let eq = equatorial(&Builtin, &star, jd).unwrap_or((f64::NAN, f64::NAN));
    let a = apparent(&Builtin, &star, jd).unwrap();
    (eq, (a.longitude, a.latitude))
}

// The reduction from the catalogue row alone, in the ICRS axes (no precession or nutation):
// space motion with the radial velocity, parallax, light deflection and aberration.
#[test]
fn reduction_against_swiss_ephemeris() {
    let ours = |jd: f64| {
        let p = apparent(&Builtin, &sirius(), jd).unwrap().direction;
        let ra = p[1].atan2(p[0]).to_degrees().rem_euclid(360.0);
        ((ra, p[2].asin().to_degrees()), (0.0, 0.0))
    };
    let ((worst, at), n) = compare("icrs", ours, 2);
    println!("worst over {n} instants in 1800-2200: {worst:.5} mas at JD {at:.3}");
    assert!(n >= 200, "{n} instants");
    // Measured 0.0004 mas.
    assert!(worst < 0.01, "{worst:.5} mas at JD {at:.3}");
}

// Of date: the right ascension and declination on the true equator and equinox, and the
// ecliptic longitude and latitude, within 2.5 mas (measured 2.40 mas, at 1805). The gap is the
// models, not the reduction (above, which agrees to 0.0004 mas): the Swiss Ephemeris sets
// Vondrak 2011 precession and IAU 2000B nutation by default, we IAU 2006 and IAU 2000A_R06.
// The precession part is smooth, zero near 2000 and up to 1.75 mas at 1800; nutation adds
// 0.3-1.4 mas. Every body shows it (see declinations.rs).
#[test]
fn positions_of_date_against_swiss_ephemeris() {
    let ((worst, at), n) = compare("apparent", positions, 4);
    println!("worst over {n} instants in 1800-2200: {worst:.4} mas at JD {at:.3}");
    assert!(n >= 200, "{n} instants");
    assert!(worst < 2.5, "{worst:.4} mas at JD {at:.3}");
}

/// Our rising or setting of Sirius, as a UT1 Julian date, after UT Julian date `start`.
fn ours(site: (f64, f64), rise: bool, start: f64) -> f64 {
    let (lat, east) = (site.0.to_radians(), site.1.to_radians());
    let from = start + delta_t(start) / DAY;
    let found = crossings(&Builtin, &sirius(), from, from + 1.5, lat, east, 0.0).unwrap();
    let x = found
        .iter()
        .find(|x| x.rising == rise)
        .unwrap_or_else(|| panic!("no crossing after {start}: {found:?}"));
    ut1_from_tt(x.tt)
}

#[test]
fn risings_and_settings_against_swiss_ephemeris() {
    // Latitude and east longitude, degrees.
    let sites = [
        ("Luxor", (25.6995, 32.6391)),
        ("Heliopolis", (30.1293, 31.3075)),
    ];
    let (mut worst, mut at) = (0.0f64, String::new());
    let mut n = 0;
    for kind in ["rise", "set"] {
        for f in rows(kind) {
            let site = sites.iter().find(|s| s.0 == f[1]).expect("a known site").1;
            let (start, expect): (f64, f64) = (f[2].parse().unwrap(), f[3].parse().unwrap());
            let got = ours(site, kind == "rise", start);
            let d = (got - expect).abs() * DAY;
            n += 1;
            if d > worst {
                (worst, at) = (d, format!("{kind} at {} after UT {start}", f[1]));
            }
        }
    }
    println!("worst over {n} risings and settings in 1800-2200: {worst:.3} s ({at})");
    assert!(n >= 80, "{n} events");
    assert!(worst < 2.0, "{worst:.3} s: {at}");
}
