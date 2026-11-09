//! `cargo xtask de440` and `cargo xtask outer`: cut the crate's built-in slice of DE440 (see
//! its `slice` module for the format), and the outer planets' file, from a downloaded
//! de440.bsp: `ASTROMETRY_DE440`, or `target/jpl/de440.bsp`.

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

/// The outer planets' file: Uranus, Neptune and Pluto's barycentres about the solar-system
/// barycentre, to the same 1 m. Their own centres are within about 0.01″ of these, geocentrically.
const OUTER: [(i32, i32, f64); 3] = [(7, 0, 1e-3), (8, 0, 1e-3), (9, 0, 1e-3)];

/// 1800-01-01 and 2200-01-01, TDB Julian dates.
const FROM: f64 = 2378496.5;
const TO: f64 = 2524593.5;

pub fn run() -> Result {
    cut(&SEGMENTS, "data/de440-1800-2200.bin")
}

/// `cargo xtask outer`: the same, for the file behind the `outer-planets` feature.
pub fn outer() -> Result {
    cut(&OUTER, "data/de440-outer-1800-2200.bin")
}

fn cut(segments: &[(i32, i32, f64)], file: &str) -> Result {
    let path = std::env::var_os("ASTROMETRY_DE440")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root().join("target/jpl/de440.bsp"));
    let spk = Spk::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    // Per segment: its head, then the deflated blocks of about a year's records each.
    let mut heads = Vec::new();
    let mut tables: Vec<Vec<(u64, u32)>> = Vec::new();
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    let mut total = 0;
    for &(target, center, step) in segments {
        let r = spk
            .records(target, center, FROM, TO)
            .map_err(|e| e.to_string())?;
        let per_block = (365.25 * 86400.0 / r.interval).ceil() as usize;
        let mut worst = 0.0f64;
        let mut table = Vec::new();
        let mut bytes = 0;
        for chunk in r.records.chunks(per_block) {
            let mut stream = Vec::new();
            for rec in chunk {
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
            bytes += stream.len();
            let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
            enc.write_all(&stream).map_err(|e| e.to_string())?;
            let block = enc.finish().map_err(|e| e.to_string())?;
            table.push((0, block.len() as u32));
            blocks.push(block);
        }
        let mut head = Vec::new();
        head.extend(target.to_le_bytes());
        head.extend(center.to_le_bytes());
        head.extend(r.init.to_le_bytes());
        head.extend(r.interval.to_le_bytes());
        head.extend((r.n as u32).to_le_bytes());
        head.extend((r.records.len() as u32).to_le_bytes());
        head.extend(step.to_le_bytes());
        head.extend((per_block as u32).to_le_bytes());
        head.extend((table.len() as u32).to_le_bytes());
        println!(
            "{target:>3}/{center}: {} records of {} coefficients, {:.1} days each, {} to a block ({} blocks); {bytes} bytes; worst rounding {worst:.2e} km",
            r.records.len(),
            r.n,
            r.interval / 86400.0,
            per_block,
            table.len(),
        );
        total += table.len();
        heads.push(head);
        tables.push(table);
    }
    // The blocks follow the heads and tables; now their offsets are known.
    let mut at = (8 + 4 + heads.iter().map(Vec::len).sum::<usize>() + total * 12) as u64;
    for table in &mut tables {
        for (offset, len) in table.iter_mut() {
            *offset = at;
            at += u64::from(*len);
        }
    }
    let mut out = Vec::new();
    out.extend(b"ASTRDE02");
    out.extend((segments.len() as u32).to_le_bytes());
    for (head, table) in heads.iter().zip(&tables) {
        out.extend(head);
        for &(offset, len) in table {
            out.extend(offset.to_le_bytes());
            out.extend(len.to_le_bytes());
        }
    }
    for block in &blocks {
        out.extend(block);
    }
    let file = root().join(file);
    std::fs::write(&file, &out).map_err(|e| e.to_string())?;
    println!(
        "wrote {} ({} bytes in {total} blocks)",
        file.strip_prefix(root()).unwrap().display(),
        out.len(),
    );
    Ok(())
}
