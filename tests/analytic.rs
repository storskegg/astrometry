//! E3's gate (plan 12): the analytical source (VSOP87E, ELP/MPP02) against DE440, over
//! 1800–2200.

use std::path::PathBuf;

use astrometry::apparent::apparent;
use astrometry::source::{Analytic, Body, Jpl, Source};
use astrometry::vector::{norm, sub};

fn de440() -> Option<Jpl> {
    if let Some(path) = std::env::var_os("ASTROMETRY_DE440").filter(|p| !p.is_empty()) {
        return Some(Jpl::open(PathBuf::from(path)).expect("ASTROMETRY_DE440"));
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/jpl/de440.bsp");
    if !path.exists() {
        eprintln!("skipped: no DE440 at {}", path.display());
        return None;
    }
    Some(Jpl::open(&path).unwrap())
}

const BODIES: [Body; 7] = [
    Body::Sun,
    Body::Moon,
    Body::Mercury,
    Body::Venus,
    Body::Mars,
    Body::Jupiter,
    Body::Saturn,
];

/// 2,000 TT instants spread evenly over 1800–2200, a little off whole days.
fn instants() -> impl Iterator<Item = f64> {
    (0..2000).map(|i| 2378496.5 + 0.37 + i as f64 * 146097.0 / 2000.0)
}

#[test]
fn apparent_against_de440() {
    let Some(jpl) = de440() else { return };
    let ana = Analytic;
    for b in BODIES {
        let (mut lon, mut lat) = (0.0f64, 0.0f64);
        for tt in instants() {
            let a = apparent(&ana, b, tt).unwrap();
            let j = apparent(&jpl, b, tt).unwrap();
            let dl = ((a.longitude - j.longitude + 540.0).rem_euclid(360.0) - 180.0)
                * 3600.0
                * j.latitude.to_radians().cos();
            lon = lon.max(dl.abs());
            lat = lat.max(((a.latitude - j.latitude) * 3600.0).abs());
        }
        println!("{b:?}: longitude {lon:.4}″, latitude {lat:.4}″");
        // As measured when E3 began, as a floor: VSOP87 (fitted to DE200) drifts against
        // DE440, most for Mars to Saturn, and ELP/MPP02 (fitted to DE405) a little.
        let (max_lon, max_lat) = match b {
            Body::Sun => (0.07, 0.03),
            Body::Moon => (0.15, 0.04),
            Body::Mercury | Body::Venus => (0.13, 0.08),
            Body::Mars => (1.1, 0.06),
            Body::Jupiter => (0.75, 0.11),
            Body::Saturn => (0.45, 0.04),
            Body::Earth => unreachable!(),
        };
        assert!(
            lon < max_lon && lat < max_lat,
            "{b:?}: longitude {lon:.4}″, latitude {lat:.4}″"
        );
    }
}

#[test]
#[ignore]
fn raw_vectors_against_de440() {
    let Some(jpl) = de440() else { return };
    let ana = Analytic;
    for b in [
        Body::Sun,
        Body::Earth,
        Body::Moon,
        Body::Mars,
        Body::Jupiter,
        Body::Saturn,
    ] {
        let mut worst = 0.0f64;
        for tt in instants().step_by(20) {
            let (a, _) = ana.state(b, tt).unwrap();
            let (j, _) = jpl.state(b, tt).unwrap();
            let (ae, _) = ana.state(Body::Earth, tt).unwrap();
            let (je, _) = jpl.state(Body::Earth, tt).unwrap();
            let rel = if b == Body::Earth {
                norm(sub(a, j))
            } else {
                norm(sub(sub(a, ae), sub(j, je))) / norm(sub(j, je)) * 206264.806
            };
            worst = worst.max(rel);
        }
        println!(
            "{b:?}: {worst:.6} ({})",
            if b == Body::Earth {
                "au"
            } else {
                "″ geocentric"
            }
        );
    }
}
