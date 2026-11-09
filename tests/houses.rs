//! House cusps and ayanamsas against the Swiss Ephemeris (`tests/data/swisseph/houses.tsv`).

use std::collections::HashMap;
use std::path::PathBuf;

use astrometry::ayanamsa;
use astrometry::derived::diff;
use astrometry::houses::{equal, koch, placidus, polar_limit, whole_sign};

fn data(name: &str) -> Vec<Vec<String>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/swisseph")
        .join(name);
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

/// The largest difference, arcseconds, of each system's cusps from the Swiss Ephemeris'.
fn cusp_differences() -> HashMap<String, f64> {
    let mut worst: HashMap<String, f64> = HashMap::new();
    for r in data("houses.tsv").iter().filter(|r| r[0] == "house") {
        let ramc = r[2].parse::<f64>().unwrap().to_radians();
        let lat: f64 = r[3].parse().unwrap();
        let eps = r[4].parse::<f64>().unwrap().to_radians();
        // Past the polar limit the Swiss Ephemeris' Ascendant parts from the formula's (and
        // it returns Porphyry for Placidus and Koch, which the fixture omits); skip.
        if lat.abs() >= polar_limit(eps) {
            continue;
        }
        let want: Vec<f64> = r[5..17].iter().map(|x| x.parse().unwrap()).collect();
        let ours = match r[1].as_str() {
            "P" => placidus(ramc, eps, lat.to_radians()),
            "K" => koch(ramc, eps, lat.to_radians()),
            "E" | "W" => {
                let asc = astrometry::angles::from_ramc(ramc, eps, lat.to_radians()).ascendant;
                // An Ascendant within a microdegree of a sign's edge may fall either side of
                // it, by round-off: those rows (RAMC 90° and 270°) are skipped.
                if r[1] == "W" && (asc / 30.0 - (asc / 30.0).round()).abs() < 1e-8 {
                    continue;
                }
                Some(if r[1] == "E" {
                    equal(asc)
                } else {
                    whole_sign(asc)
                })
            }
            s => panic!("{s}"),
        };
        // The Swiss Ephemeris returns Porphyry houses past the limit (the fixture omits those
        // rows); we return none, and the rows that remain are inside.
        let Some(ours) = ours else {
            assert!(
                matches!(r[1].as_str(), "P" | "K") && lat.abs() >= polar_limit(eps) - 1e-9,
                "{r:?}"
            );
            continue;
        };
        for (n, (o, w)) in ours.iter().zip(&want).enumerate() {
            let d = diff(*o, *w).abs() * 3600.0;
            let e = worst.entry(r[1].clone()).or_insert(0.0);
            if d > *e {
                *e = d;
            }
            assert!(d < 1.0, "{r:?} cusp {}: {o} against {w}", n + 1);
        }
    }
    worst
}

#[test]
fn cusps_against_swiss_ephemeris() {
    let worst = cusp_differences();
    println!("{worst:?}");
    // Measured: Placidus 0.0029″ (the Swiss Ephemeris stops its iteration early), Koch and Equal
    // 1.8e-6″ (the fixture's nine decimals), Whole Sign exact. Limits at 1.5 times.
    for (system, limit) in [("P", 0.0043), ("K", 2.7e-6), ("E", 2.7e-6), ("W", 1e-9)] {
        assert!(worst[system] < limit, "{system}: {:.6}″", worst[system]);
    }
}

#[test]
fn polar_limit_for_placidus_and_koch() {
    let eps = 23.4392911f64.to_radians();
    assert!((polar_limit(eps) - 66.5607089).abs() < 1e-9);
    for ramc in (0..360).step_by(30) {
        let ramc = f64::from(ramc).to_radians();
        for lat in [66.0f64, -66.0] {
            let lat = lat.to_radians();
            assert!(placidus(ramc, eps, lat).is_some(), "Placidus at {lat}");
            assert!(koch(ramc, eps, lat).is_some(), "Koch at {lat}");
        }
        for lat in [67.0f64, -67.0, 80.0, 90.0] {
            let lat = lat.to_radians();
            assert!(placidus(ramc, eps, lat).is_none(), "Placidus at {lat}");
            assert!(koch(ramc, eps, lat).is_none(), "Koch at {lat}");
        }
        // Equal and Whole Sign need only the Ascendant, wherever that is.
        let asc = astrometry::angles::from_ramc(ramc, eps, 80f64.to_radians()).ascendant;
        assert!((equal(asc)[1] - (asc + 30.0).rem_euclid(360.0)).abs() < 1e-9);
        assert_eq!(whole_sign(asc)[0] % 30.0, 0.0);
    }
}

#[test]
fn equal_and_whole_sign_shapes() {
    let e = equal(350.5);
    assert!((e[1] - 20.5).abs() < 1e-12 && (e[11] - 320.5).abs() < 1e-12);
    let w = whole_sign(350.5);
    assert_eq!((w[0], w[1], w[11]), (330.0, 0.0, 300.0));
}

#[test]
fn angles_use_the_shared_ramc() {
    // `angles` is `from_ramc` on `ramc_and_obliquity`, exactly.
    let (tt, lat, lon) = (2_461_329.26, 43.1, -70.8);
    let (ramc, eps) = astrometry::houses::ramc_and_obliquity(tt, lon);
    assert_eq!(
        astrometry::angles::angles(tt, lat, lon),
        astrometry::angles::from_ramc(ramc, eps, lat.to_radians())
    );
}

#[test]
fn ayanamsas_against_swiss_ephemeris() {
    let (mut fb, mut la) = (0.0f64, 0.0f64);
    for r in data("houses.tsv").iter().filter(|r| r[0] == "ayanamsa") {
        let jd: f64 = r[2].parse().unwrap();
        let want: f64 = r[3].parse().unwrap();
        let (got, worst) = match r[1].as_str() {
            "FaganBradley" => (ayanamsa::fagan_bradley(jd), &mut fb),
            "Lahiri" => (ayanamsa::lahiri(jd), &mut la),
            s => panic!("{s}"),
        };
        *worst = worst.max((got - want).abs() * 3600.0);
    }
    println!("ayanamsa: Fagan-Bradley {fb:.4}″, Lahiri {la:.4}″");
    // Measured 0.0041″ and 0.0028″, with the limits at 1.5 times.
    assert!(fb < 0.0062, "Fagan-Bradley {fb}″");
    assert!(la < 0.0042, "Lahiri {la}″");
}

// The Swiss Ephemeris' sidereal Sun and Moon are its apparent tropical longitudes minus an
// ayanamsa that includes the nutation: ours does too.
#[test]
fn sidereal_longitudes_are_apparent_minus_ayanamsa() {
    let tropical: HashMap<(String, String), f64> = data("positions.tsv")
        .iter()
        .filter(|r| r[0] == "jpl" && (r[1] == "Sun" || r[1] == "Moon"))
        .map(|r| ((r[1].clone(), r[2].clone()), r[3].parse().unwrap()))
        .collect();
    let mut worst = 0.0f64;
    let mut n = 0;
    for r in data("houses.tsv").iter().filter(|r| r[0] == "sidereal") {
        let jd: f64 = r[3].parse().unwrap();
        let want: f64 = r[4].parse().unwrap();
        let Some(trop) = tropical.get(&(r[2].clone(), r[3].clone())) else {
            continue;
        };
        let a = match r[1].as_str() {
            "FaganBradley" => ayanamsa::fagan_bradley(jd),
            _ => ayanamsa::lahiri(jd),
        };
        worst = worst.max(diff(trop - a, want).abs() * 3600.0);
        n += 1;
    }
    println!("sidereal: {n} rows, worst {worst:.4}″");
    assert!(n >= 40, "{n} rows"); // Measured 0.0036″.
    assert!(worst < 0.0055, "{worst}″");
}
