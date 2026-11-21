//! `cargo xtask de440` and `cargo xtask outer`: cut the crate's built-in slice of DE440 (see
//! its `slice` module for the format), and the outer planets' file, from a downloaded
//! de440.bsp: `ASTROMETRY_DE440`, or `local/jpl/de440.bsp`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use astrometry::spk::Spk;
use flate2::Compression;
use flate2::write::DeflateEncoder;

use crate::{Result, root};

/// The segments the Sun, Moon and classical planets need, and each one's step, km: 1 cm for
/// the Moon about the Earth–Moon barycentre, 1 m for the rest. Mercury's and Venus's offsets
/// from their barycentres (199/1, 299/2) are zero in DE440, and the Earth's from the Earth–Moon
/// barycentre (399/3) is the Moon's divided by the Earth/Moon mass ratio, to 5 picometres; so
/// none of the three is kept.
pub const SEGMENTS: [(i32, i32, f64); 8] = [
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
pub const OUTER: [(i32, i32, f64); 3] = [(7, 0, 1e-3), (8, 0, 1e-3), (9, 0, 1e-3)];

/// 1800-01-01 and 2200-01-01, TDB Julian dates.
const FROM: f64 = 2378496.5;
const TO: f64 = 2524593.5;

const DE440_URL: &str = "https://ssd.jpl.nasa.gov/ftp/eph/planets/bsp/de440.bsp";

/// SHA-256 of de440.bsp as JPL serves it, and of the two files cut from it (the deflate level
/// is explicit, `Compression::best()`, and flate2 is pinned in Cargo.lock, so the cut is
/// reproducible). If JPL changes the file, update all three.
const DE440_SHA256: &str = "a4ce9bf9b3282becc9f4b2ac3cebe03a2ae7599981aabd7265fd8482fff7c4b5";
const SLICE_SHA256: &str = "68e5bd49234e959d3be8946cbc9361d4d2e1b85578b2919b16c091384da1c324";
const OUTER_SHA256: &str = "29e651b2c6ab2a9c63071532c135a8d0442fdcb4e2b12460c8eab656c254e805";
const SLICE_FILE: &str = "data/de440-1800-2200.bin";
const OUTER_FILE: &str = "data/de440-outer-1800-2200.bin";

/// Where `cargo xtask fixtures de440` keeps the download while it works.
pub fn download_path() -> PathBuf {
    root().join("local/jpl/de440.bsp")
}

/// Downloads de440.bsp to `path`, unless it's there: beside it, and renamed once complete, so
/// that an interrupted download never passes for the real file.
fn download(path: &Path) -> Result {
    if path.exists() {
        return Ok(());
    }
    println!("==> download de440.bsp");
    std::fs::create_dir_all(path.parent().expect("a file path"))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let part = path.with_extension("bsp.part");
    let status = Command::new("curl")
        .args(["-sS", "--fail", "-L", "-o"])
        .arg(&part)
        .arg(DE440_URL)
        .status()
        .map_err(|e| format!("curl: {e}"))?;
    if status.success() {
        std::fs::rename(&part, path).map_err(|e| format!("{}: {e}", path.display()))
    } else {
        let _ = std::fs::remove_file(&part);
        Err(format!("download de440.bsp: {DE440_URL}: {status}"))
    }
}

/// Deletes the download, and a partial one, and the directories if they're empty.
fn delete(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("bsp.part"));
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir(dir);
        if let Some(local) = dir.parent() {
            let _ = std::fs::remove_dir(local);
        }
    }
}

/// The SHA-256 of a file, by `shasum -a 256`.
fn sha256(path: &Path) -> std::result::Result<String, String> {
    let out = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .map_err(|e| format!("shasum: {e}"))?;
    if !out.status.success() {
        return Err(format!("shasum {}: {}", path.display(), out.status));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text.split_whitespace().next().unwrap_or("").to_string())
}

/// `cargo xtask de440`: from `ASTROMETRY_DE440`, or `local/jpl/de440.bsp`.
pub fn run() -> Result {
    cut(&env_path(), &SEGMENTS, SLICE_FILE)
}

/// `cargo xtask outer`: the same, for the file behind the `outer-planets` feature.
pub fn outer() -> Result {
    cut(&env_path(), &OUTER, OUTER_FILE)
}

fn env_path() -> PathBuf {
    std::env::var_os("ASTROMETRY_DE440")
        .map(PathBuf::from)
        .unwrap_or_else(download_path)
}

/// `cargo xtask fixtures de440`: downloads de440.bsp, rebuilds both files of the slice from
/// it, writes the reference fixture, and runs the whole test suite against the full file;
/// then deletes the download, whatever happened.
pub fn pipeline() -> Result {
    let path = download_path();
    let result = (|| {
        download(&path)?;
        let hash = sha256(&path)?;
        println!("de440.bsp SHA-256: {hash}");
        let known = hash == DE440_SHA256;
        if !known {
            println!(
                "NOTE: de440.bsp's SHA-256 differs from the recorded one: JPL changed the file. \
                 Going on; update the constants in xtask/src/de440.rs and the README."
            );
        }
        cut(&path, &SEGMENTS, SLICE_FILE)?;
        cut(&path, &OUTER, OUTER_FILE)?;
        if known {
            // The same input must give the same bytes.
            for (file, want) in [(SLICE_FILE, SLICE_SHA256), (OUTER_FILE, OUTER_SHA256)] {
                let got = sha256(&root().join(file))?;
                if got != want {
                    return Err(format!(
                        "{file}: rebuilt from the recorded de440.bsp, its SHA-256 is {got}, \
                         not the recorded {want}"
                    ));
                }
            }
        }
        crate::reference::write(&path)?;
        verify(&path)
    })();
    delete(&path);
    result
}

/// From a clean, every test with the full file, so that both it and the fixture are tested.
fn verify(path: &Path) -> Result {
    for (args, de440) in [
        (&["clean"][..], false),
        (
            &["test", "--workspace", "--all-features", "--locked"][..],
            true,
        ),
    ] {
        println!("==> cargo {}", args.join(" "));
        let mut cmd = Command::new("cargo");
        cmd.args(args).current_dir(root());
        if de440 {
            cmd.env("ASTROMETRY_DE440", path);
        }
        let status = cmd.status().map_err(|e| format!("cargo: {e}"))?;
        if !status.success() {
            return Err(format!("verify: cargo {}: {status}", args.join(" ")));
        }
    }
    Ok(())
}

fn cut(path: &Path, segments: &[(i32, i32, f64)], file: &str) -> Result {
    let spk = Spk::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
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
