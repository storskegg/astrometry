//! `cargo xtask de440`: cuts the crate's built-in slice of DE440 (see its
//! `slice` module for the format) from a downloaded de440.bsp: `ASTROMETRY_DE440`, or
//! `target/jpl/de440.bsp`.

use std::io::Write;

use astrometry::spk::Spk;
use flate2::Compression;
use flate2::write::DeflateEncoder;

use crate::{Result, root};

/// The segments the Sun, Moon and classical planets need, and each one's step, km: 1 cm for
/// the Moon about the Earth–Moon barycentre, 1 m for the rest. Mercury's and Venus's offsets
/// from their barycentres (199/1, 299/2) are zero in DE440, and the Earth's from the Earth–Moon
/// barycentre (399/3) is the Moon's divided by the Earth/Moon mass ratio, to 5 picometres; so
/// none of the three is kept.
const SEGMENTS: [(i32, i32, f64); 8] = [
    (10, 0, 1e-3),
    (1, 0, 1e-3),
    (2, 0, 1e-3),
    (3, 0, 1e-3),
    (301, 3, 1e-5),
    (4, 0, 1e-3),
    (5, 0, 1e-3),
    (6, 0, 1e-3),
];

/// 1800-01-01 and 2200-01-01, TDB Julian dates.
const FROM: f64 = 2378496.5;
const TO: f64 = 2524593.5;

pub fn run() -> Result {
    let path = std::env::var_os("ASTROMETRY_DE440")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root().join("target/jpl/de440.bsp"));
    let spk = Spk::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut head = Vec::new();
    let mut body = Vec::new();
    head.extend(b"ASTRDE01");
    head.extend((SEGMENTS.len() as u32).to_le_bytes());
    for (target, center, step) in SEGMENTS {
        let r = spk
            .records(target, center, FROM, TO)
            .map_err(|e| e.to_string())?;
        let mut stream = Vec::new();
        let mut worst = 0.0f64;
        for rec in &r.records {
            for &c in rec {
                let q = (c / step).round() as i64;
                worst = worst.max((q as f64 * step - c).abs());
                let mut v = ((q << 1) ^ (q >> 63)) as u64;
                loop {
                    let byte = (v & 0x7f) as u8;
                    v >>= 7;
                    if v == 0 {
                        stream.push(byte);
                        break;
                    }
                    stream.push(byte | 0x80);
                }
            }
        }
        head.extend(target.to_le_bytes());
        head.extend(center.to_le_bytes());
        head.extend(r.init.to_le_bytes());
        head.extend(r.interval.to_le_bytes());
        head.extend((r.n as u32).to_le_bytes());
        head.extend((r.records.len() as u32).to_le_bytes());
        head.extend(step.to_le_bytes());
        head.extend((stream.len() as u64).to_le_bytes());
        println!(
            "{target:>3}/{center}: {} records of {} coefficients, {:.1} days each; {} bytes; worst rounding {worst:.2e} km",
            r.records.len(),
            r.n,
            r.interval / 86400.0,
            stream.len()
        );
        body.extend(stream);
    }
    head.extend(body);
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
    enc.write_all(&head).map_err(|e| e.to_string())?;
    let out = enc.finish().map_err(|e| e.to_string())?;
    let file = root().join("data/de440-1800-2200.bin");
    std::fs::write(&file, &out).map_err(|e| e.to_string())?;
    println!(
        "wrote {} ({} bytes, {} before deflating)",
        file.strip_prefix(root()).unwrap().display(),
        out.len(),
        head.len()
    );
    Ok(())
}
