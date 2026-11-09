//! Declinations against the Swiss Ephemeris (JPL mode, `tests/data/swisseph/declinations.tsv`).

use std::collections::HashMap;
use std::path::PathBuf;

use astrometry::derived::{
    declination, declination_speed, ecliptic_declination, mean_apogee, mean_apogee_latitude,
    mean_node, true_node,
};
use astrometry::houses::ramc_and_obliquity;
use astrometry::source::{Body, Builtin};

fn rows() -> Vec<(String, f64, f64, f64)> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/swisseph/declinations.tsv");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (
                f[0].to_string(),
                f[1].parse().unwrap(),
                f[2].parse().unwrap(),
                f[3].parse().unwrap(),
            )
        })
        .collect()
}

fn body(name: &str) -> Option<Body> {
    Some(match name {
        "Sun" => Body::Sun,
        "Moon" => Body::Moon,
        "Mercury" => Body::Mercury,
        "Venus" => Body::Venus,
        "Mars" => Body::Mars,
        "Jupiter" => Body::Jupiter,
        "Saturn" => Body::Saturn,
        #[cfg(feature = "outer-planets")]
        "Uranus" => Body::Uranus,
        #[cfg(feature = "outer-planets")]
        "Neptune" => Body::Neptune,
        #[cfg(feature = "outer-planets")]
        "Pluto" => Body::Pluto,
        _ => return None,
    })
}

#[test]
fn declinations_against_swiss_ephemeris() {
    // Worst differences, arcseconds (arcseconds per day for speeds), by group.
    let mut worst: HashMap<&str, f64> = HashMap::new();
    let mut note = |k: &'static str, v: f64| {
        let e = worst.entry(k).or_insert(0.0);
        *e = e.max(v.abs() * 3600.0);
    };
    for (name, jd, dec, spd) in rows() {
        let eps = ramc_and_obliquity(jd, 0.0).1;
        match name.as_str() {
            // Ecliptic latitude 0: the node's longitude on the equator by the true obliquity.
            "MeanNode" => note(
                "mean node",
                ecliptic_declination(mean_node(jd), 0.0, eps) - dec,
            ),
            "TrueNode" => note(
                "true node",
                ecliptic_declination(true_node(&Builtin, jd).unwrap(), 0.0, eps) - dec,
            ),
            "MeanApogee" => note(
                "mean apogee",
                ecliptic_declination(mean_apogee(jd), mean_apogee_latitude(jd), eps) - dec,
            ),
            name => {
                let Some(b) = body(name) else {
                    assert!(matches!(name, "Uranus" | "Neptune" | "Pluto"), "{name}");
                    continue;
                };
                let (k, ks) = if matches!(name, "Uranus" | "Neptune" | "Pluto") {
                    ("outer planets", "outer planets speed")
                } else {
                    ("planets", "planets speed")
                };
                note(k, declination(&Builtin, b, jd).unwrap() - dec);
                note(ks, declination_speed(&Builtin, b, jd).unwrap() - spd);
            }
        }
    }
    let mut keys: Vec<_> = worst.iter().collect();
    keys.sort_by(|a, b| a.0.cmp(b.0));
    for (k, v) in keys {
        println!("{k}: {v:.5}″");
    }
    // Limits are 1.5 times what was measured: the planets 0.0019″ (their speeds 0.108″/day,
    // the Moon's, from differences over 0.01 day against the Swiss Ephemeris' own), the outer
    // planets 0.0018″ and 0.0007″/day. The true node is the Swiss Ephemeris' to 0.005″. Its
    // nodes have latitude 0, as ours do, so their declinations follow from longitude alone;
    // the mean node differs by the 0.3″ in longitude that its expression has from Meeus's, and
    // the mean apogee (which the Swiss Ephemeris gives its latitude, up to 5.14°) by its 1.7″
    // in longitude.
    for (k, limit) in [
        ("planets", 0.0029),
        ("planets speed", 0.163),
        ("true node", 0.0071),
        ("mean node", 0.126),
        ("mean apogee", 0.589),
    ] {
        assert!(worst[k] < limit, "{k}: {:.5}″", worst[k]);
    }
    #[cfg(feature = "outer-planets")]
    for (k, limit) in [("outer planets", 0.0028), ("outer planets speed", 0.0011)] {
        assert!(worst[k] < limit, "{k}: {:.5}″", worst[k]);
    }
}

// The mean apogee's latitude is the Swiss Ephemeris', from the same projection.
#[test]
fn mean_apogee_latitude_against_swiss_ephemeris() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/swisseph/positions.tsv");
    let text = std::fs::read_to_string(path).unwrap();
    let (mut worst, mut n) = (0.0f64, 0);
    for f in text.lines().map(|l| l.split('\t').collect::<Vec<_>>()) {
        if f[0] == "jpl" && f[1] == "MeanApogee" {
            let (jd, lat): (f64, f64) = (f[2].parse().unwrap(), f[4].parse().unwrap());
            worst = worst.max((mean_apogee_latitude(jd) - lat).abs() * 3600.0);
            n += 1;
        }
    }
    println!("mean apogee latitude: {n} rows, worst {worst:.5}″");
    assert!(n > 100);
    // Measured 0.081″, the difference of the mean elements' conventions.
    assert!(worst < 0.122, "{worst}″");
}
