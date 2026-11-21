//! The analytical source (VSOP87E, ELP/MPP02) against DE440, over
//! 1800–2200.
#![cfg(feature = "theories")]

use std::path::PathBuf;

mod common;

use astrometry::apparent::apparent;
use astrometry::source::{Analytic, Body, Builtin, Jpl, Source};
use astrometry::vector::{norm, sub};
use common::Reference;

/// DE440, from `ASTROMETRY_DE440` if that's set.
fn de440() -> Option<Jpl> {
    let Some(path) = std::env::var_os("ASTROMETRY_DE440").filter(|p| !p.is_empty()) else {
        eprintln!("skipped: ASTROMETRY_DE440 isn't set");
        return None;
    };
    Some(Jpl::open(PathBuf::from(path)).expect("ASTROMETRY_DE440"))
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
    let reference = Reference::load();
    let ana = Analytic;
    for b in BODIES {
        let (mut lon, mut lat) = (0.0f64, 0.0f64);
        for (&tt, j) in reference.instants.iter().zip(reference.apparent(b)) {
            let a = apparent(&ana, b, tt).unwrap();
            let dl = ((a.longitude - j[0] + 540.0).rem_euclid(360.0) - 180.0)
                * 3600.0
                * j[1].to_radians().cos();
            lon = lon.max(dl.abs());
            lat = lat.max(((a.latitude - j[1]) * 3600.0).abs());
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
            Body::Earth | Body::Uranus | Body::Neptune | Body::Pluto => unreachable!(),
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

/// The built-in slice against DE440 itself (the reference fixture): the rounding of its coefficients only.
#[test]
fn builtin_against_de440() {
    let reference = Reference::load();
    let mut worst = 0.0f64;
    for b in BODIES {
        for (&tt, j) in reference.instants.iter().zip(reference.apparent(b)) {
            let a = apparent(&Builtin, b, tt).unwrap();
            let d = ((a.longitude - j[0] + 540.0).rem_euclid(360.0) - 180.0)
                .abs()
                .max((a.latitude - j[1]).abs())
                * 3600.0;
            worst = worst.max(d);
        }
    }
    println!("built-in slice against DE440: {worst:.6}″");
    assert!(worst < 0.0005, "{worst:.6}″");
}
