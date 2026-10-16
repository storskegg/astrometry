//! E2's gate (plan 12): DE440 through the reduction chain, against JPL Horizons and the Swiss
//! Ephemeris.
//!
//! DE440 (120 MB) isn't in the repository: these tests read it from `ASTROMETRY_DE440`, or
//! `target/jpl/de440.bsp` in the workspace, and are skipped without it. CI downloads it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use astrometry::apparent::apparent;
use astrometry::source::{Body, Jpl};

fn de440() -> Option<Jpl> {
    let path = std::env::var_os("ASTROMETRY_DE440")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/jpl/de440.bsp")
        });
    if !path.exists() {
        eprintln!("skipped: no DE440 at {}", path.display());
        return None;
    }
    Some(Jpl::open(&path).unwrap())
}

fn rows(file: &str) -> Vec<Vec<String>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(file);
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

fn body(name: &str) -> Option<Body> {
    Some(match name {
        "Sun" => Body::Sun,
        "Moon" => Body::Moon,
        "Mercury" => Body::Mercury,
        "Venus" => Body::Venus,
        "MarsBarycenter" => Body::Mars,
        "JupiterBarycenter" => Body::Jupiter,
        "SaturnBarycenter" => Body::Saturn,
        _ => return None,
    })
}

/// The angle between two longitudes, arcseconds, on a great circle at `lat` degrees.
fn arcsec(a: f64, b: f64, lat: f64) -> f64 {
    ((a - b + 540.0).rem_euclid(360.0) - 180.0).abs() * 3600.0 * lat.to_radians().cos()
}

#[derive(Default, Debug)]
struct Worst {
    lon: f64,
    lat: f64,
}

/// The worst differences by body, before Horizons' switch to UTC (1962) and after.
fn compare(
    src: &Jpl,
    file: &str,
    select: impl Fn(&[String]) -> Option<(Body, f64, f64, f64)>,
) -> BTreeMap<(String, bool), Worst> {
    let mut worst: BTreeMap<(String, bool), Worst> = BTreeMap::new();
    for r in rows(file) {
        let Some((b, jd, lon, lat)) = select(&r) else {
            continue;
        };
        let got = apparent(src, b, jd).unwrap();
        let w = worst
            .entry((format!("{b:?}"), jd >= 2437684.0))
            .or_default();
        w.lon = w.lon.max(arcsec(got.longitude, lon, lat));
        w.lat = w.lat.max((got.latitude - lat).abs() * 3600.0);
    }
    for ((b, modern), w) in &worst {
        println!(
            "{file}: {b:8} {}: longitude {:.4}″, latitude {:.4}″",
            if *modern { "from 1962" } else { "to 1962  " },
            w.lon,
            w.lat
        );
    }
    worst
}

#[test]
fn against_horizons() {
    let Some(src) = de440() else { return };
    let worst = compare(&src, "horizons/positions.tsv", |r| {
        let b = body(&r[0])?;
        Some((
            b,
            r[1].parse().ok()?,
            r[2].parse().ok()?,
            r[3].parse().ok()?,
        ))
    });
    let (lon, lat) = worst
        .values()
        .fold((0.0f64, 0.0f64), |(a, b), w| (a.max(w.lon), b.max(w.lat)));
    println!("worst against Horizons: longitude {lon:.4}″, latitude {lat:.4}″");
    // Horizons' quantity 31 is the IAU76/80 ecliptic of date: corrected to the measured pole
    // from 1962 to the present (where it's 0.048″ from IAU 2006), and outside then the IAU76
    // precession's rate error, about 0.3″ a century, drifts it to ±0.5″ by 1800 and 2200.
    assert!(
        lon < 0.55 && lat < 0.06,
        "longitude {lon:.4}″, latitude {lat:.4}″"
    );
}

/// Against Horizons with its ecliptic taken out: each body's difference less the Sun's at the
/// same instant, which carries the IAU76/80 ecliptic's offset and drift.
#[test]
fn against_horizons_relative_to_the_sun() {
    let Some(src) = de440() else { return };
    let h = rows("horizons/positions.tsv");
    let sun: BTreeMap<String, (f64, f64)> = h
        .iter()
        .filter(|r| r[0] == "Sun")
        .map(|r| (r[1].clone(), (r[2].parse().unwrap(), r[3].parse().unwrap())))
        .collect();
    let d = |a: f64, b: f64| ((a - b + 540.0).rem_euclid(360.0) - 180.0) * 3600.0;
    let mut worst = 0.0f64;
    for r in &h {
        let Some(b) = body(&r[0]).filter(|b| *b != Body::Sun) else {
            continue;
        };
        let jd: f64 = r[1].parse().unwrap();
        let (lon, lat): (f64, f64) = (r[2].parse().unwrap(), r[3].parse().unwrap());
        let got = apparent(&src, b, jd).unwrap();
        let s = apparent(&src, Body::Sun, jd).unwrap();
        let rel = (d(got.longitude, lon) - d(s.longitude, sun[&r[1]].0)) * lat.to_radians().cos();
        worst = worst.max(rel.abs());
    }
    // The Moon's 0.017″ is the ecliptics' difference again, through its 5° of latitude.
    println!("worst against Horizons relative to the Sun: longitude {worst:.4}″");
    assert!(worst < 0.03, "{worst:.4}″");
}

#[test]
fn against_swiss_ephemeris() {
    let Some(src) = de440() else { return };
    let worst = compare(&src, "swisseph/positions.tsv", |r| {
        if r[0] != "jpl" {
            return None;
        }
        let b = match r[1].as_str() {
            "Mars" => Body::Mars,
            "Jupiter" => Body::Jupiter,
            "Saturn" => Body::Saturn,
            name => body(name)?,
        };
        Some((
            b,
            r[2].parse().ok()?,
            r[3].parse().ok()?,
            r[4].parse().ok()?,
        ))
    });
    let (lon, lat) = worst
        .values()
        .fold((0.0f64, 0.0f64), |(a, b), w| (a.max(w.lon), b.max(w.lat)));
    println!(
        "worst against the Swiss Ephemeris (JPL mode): longitude {lon:.4}″, latitude {lat:.4}″"
    );
    // The same DE440 and the same IAU 2006/2000A reduction.
    assert!(
        lon < 0.005 && lat < 0.005,
        "longitude {lon:.4}″, latitude {lat:.4}″"
    );
}
