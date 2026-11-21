//! The DE440 reference fixture, `tests/data/de440/reference.bin` (`cargo xtask fixtures de440`
//! writes it from the full de440.bsp): what the tests compare the built-in slice and the
//! analytical theories against, so that they don't need the 120 MB file.
//!
//! Format `ASTRREF1`, all little-endian, uncompressed:
//!
//! - the magic `ASTRREF1`, then `n_instants`, `n_segments` and `n_bodies` (u32 each);
//! - the instants: `n_instants` f64, Julian dates. They are the TT instants of the analytic
//!   tests (`2378496.5 + 0.37 + i * 146097 / 2000`, i = 0..2000), and DE440's segments are
//!   read at the same numbers as TDB dates;
//! - the geometric section: for each segment of the built-in slice, then of the outer file,
//!   its target and centre (i32 each), then for each instant the position x, y, z (f64, km) of
//!   the target about the centre, from the full DE440;
//! - the apparent section: for each body, its id (u32: the `Body` enum's discriminant, Sun 0,
//!   Moon 1, Mercury 2, Venus 3, Earth 4, Mars 5, Jupiter 6, Saturn 7, Uranus 8, Neptune 9,
//!   Pluto 10), then for each instant the ecliptic longitude and latitude (f64, degrees) of
//!   the body's apparent position from the full DE440, as `apparent::apparent` gives it.

use std::path::Path;

use astrometry::apparent::apparent;
use astrometry::source::{Body, Jpl};
use astrometry::spk::Spk;

use crate::de440::{OUTER, SEGMENTS};
use crate::{Result, root};

const BODIES: [Body; 10] = [
    Body::Sun,
    Body::Moon,
    Body::Mercury,
    Body::Venus,
    Body::Mars,
    Body::Jupiter,
    Body::Saturn,
    Body::Uranus,
    Body::Neptune,
    Body::Pluto,
];

/// The instants: 2,000 spread evenly over 1800–2200, a little off whole days.
fn instants() -> Vec<f64> {
    (0..2000)
        .map(|i| 2378496.5 + 0.37 + i as f64 * 146097.0 / 2000.0)
        .collect()
}

/// Writes the fixture from the de440.bsp at `bsp`.
pub fn write(bsp: &Path) -> Result {
    let err = |e: &dyn std::fmt::Display| format!("{}: {e}", bsp.display());
    let spk = Spk::open(bsp).map_err(|e| err(&e))?;
    let jpl = Jpl::open(bsp).map_err(|e| err(&e))?;
    let instants = instants();
    let segments: Vec<(i32, i32)> = SEGMENTS
        .iter()
        .chain(OUTER.iter())
        .map(|&(t, c, _)| (t, c))
        .collect();

    let mut out = Vec::new();
    out.extend(b"ASTRREF1");
    out.extend((instants.len() as u32).to_le_bytes());
    out.extend((segments.len() as u32).to_le_bytes());
    out.extend((BODIES.len() as u32).to_le_bytes());
    for jd in &instants {
        out.extend(jd.to_le_bytes());
    }
    for &(target, center) in &segments {
        out.extend(target.to_le_bytes());
        out.extend(center.to_le_bytes());
        for &jd in &instants {
            let (p, _) = spk.state(target, center, jd).map_err(|e| err(&e))?;
            for c in p {
                out.extend(c.to_le_bytes());
            }
        }
    }
    for body in BODIES {
        out.extend((body as u32).to_le_bytes());
        for &tt in &instants {
            let a = apparent(&jpl, body, tt).map_err(|e| err(&e))?;
            out.extend(a.longitude.to_le_bytes());
            out.extend(a.latitude.to_le_bytes());
        }
    }
    let file = root().join("tests/data/de440/reference.bin");
    std::fs::create_dir_all(file.parent().expect("a file path")).map_err(|e| e.to_string())?;
    std::fs::write(&file, &out).map_err(|e| e.to_string())?;
    println!(
        "wrote {} ({} bytes)",
        file.strip_prefix(root()).unwrap().display(),
        out.len()
    );
    Ok(())
}
