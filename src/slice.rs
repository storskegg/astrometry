//! The built-in slice of JPL's DE440: the Chebyshev records of the segments the Sun, Moon and
//! classical planets need, for 1800–2200, re-encoded by this project (`cargo xtask de440`):
//! each coefficient rounded to a fixed step (1 cm for the Moon about the Earth–Moon barycentre,
//! 1 m for the rest), written as zigzag varints, and deflated. The Earth about the Earth–Moon
//! barycentre isn't kept: it's the Moon's offset divided by the Earth/Moon mass ratio.
//!
//! Derived from DE440 (Park, Folkner, Williams and Boggs, 2021) as JPL and NAIF distribute it;
//! modified, as NAIF's rules for SPICE kernels put it, by this project, and validated against
//! the original by the crate's tests.
//!
//! The file: `ASTRDE01`, the segment count (u32), then for each segment its target and centre
//! (i32), the first record's start and each record's span (f64, TDB seconds past J2000), the
//! coefficients per coordinate and the record count (u32), the step (f64, km), and the length
//! of its varints (u64); then the varints, segment after segment. All little-endian, and
//! deflated as a whole.

use std::io::Read;
use std::sync::LazyLock;

use flate2::read::DeflateDecoder;

/// A decoded segment.
#[derive(Debug)]
pub struct Segment {
    pub target: i32,
    pub center: i32,
    init: f64,
    interval: f64,
    n: usize,
    /// Record after record, x, y, z coefficients, km.
    coef: Vec<f64>,
    records: usize,
}

impl Segment {
    /// The TDB Julian dates it covers.
    pub fn span(&self) -> (f64, f64) {
        let start = 2451545.0 + self.init / 86400.0;
        (start, start + self.records as f64 * self.interval / 86400.0)
    }

    /// Position (km) and velocity (km/s) at TDB Julian date `jd`, if it's covered.
    pub fn state(&self, jd: f64) -> Option<([f64; 3], [f64; 3])> {
        let et = (jd - 2451545.0) * 86400.0;
        let r = ((et - self.init) / self.interval).floor();
        if r < 0.0 || r as usize >= self.records {
            return None;
        }
        let r = r as usize;
        let radius = self.interval / 2.0;
        let mid = self.init + r as f64 * self.interval + radius;
        let x = (et - mid) / radius;
        let n = self.n;
        let rec = &self.coef[r * 3 * n..(r + 1) * 3 * n];
        let (mut t, mut dt) = (vec![0.0; n], vec![0.0; n]);
        t[0] = 1.0;
        if n > 1 {
            t[1] = x;
            dt[1] = 1.0;
        }
        for k in 2..n {
            t[k] = 2.0 * x * t[k - 1] - t[k - 2];
            dt[k] = 2.0 * t[k - 1] + 2.0 * x * dt[k - 1] - dt[k - 2];
        }
        let (mut pos, mut vel) = ([0.0; 3], [0.0; 3]);
        for c in 0..3 {
            let coef = &rec[c * n..(c + 1) * n];
            pos[c] = coef.iter().zip(&t).map(|(a, b)| a * b).sum();
            vel[c] = coef.iter().zip(&dt).map(|(a, b)| a * b).sum::<f64>() / radius;
        }
        Some((pos, vel))
    }
}

/// Reads the slice's bytes, after inflating them.
pub fn decode(deflated: &[u8]) -> Result<Vec<Segment>, String> {
    let mut b = Vec::new();
    DeflateDecoder::new(deflated)
        .read_to_end(&mut b)
        .map_err(|e| e.to_string())?;
    let mut at = 0usize;
    let mut take = |n: usize| -> Result<&[u8], String> {
        let s = b.get(at..at + n).ok_or("slice: truncated")?;
        at += n;
        Ok(s)
    };
    if take(8)? != b"ASTRDE01" {
        return Err("slice: not a DE slice".into());
    }
    let u32_ = |s: &[u8]| u32::from_le_bytes(s.try_into().expect("four")) as usize;
    let count = u32_(take(4)?);
    let mut heads = Vec::with_capacity(count);
    for _ in 0..count {
        let target = i32::from_le_bytes(take(4)?.try_into().expect("four"));
        let center = i32::from_le_bytes(take(4)?.try_into().expect("four"));
        let init = f64::from_le_bytes(take(8)?.try_into().expect("eight"));
        let interval = f64::from_le_bytes(take(8)?.try_into().expect("eight"));
        let n = u32_(take(4)?);
        let records = u32_(take(4)?);
        let step = f64::from_le_bytes(take(8)?.try_into().expect("eight"));
        let len = u64::from_le_bytes(take(8)?.try_into().expect("eight")) as usize;
        heads.push((target, center, init, interval, n, records, step, len));
    }
    let mut out = Vec::with_capacity(count);
    for (target, center, init, interval, n, records, step, len) in heads {
        let stream = take(len)?;
        let mut coef = Vec::with_capacity(records * 3 * n);
        let mut i = 0;
        while i < stream.len() {
            let (mut v, mut shift) = (0u64, 0);
            loop {
                let byte = stream[i];
                i += 1;
                v |= u64::from(byte & 0x7f) << shift;
                if byte & 0x80 == 0 {
                    break;
                }
                shift += 7;
            }
            let q = (v >> 1) as i64 ^ -((v & 1) as i64);
            coef.push(q as f64 * step);
        }
        if coef.len() != records * 3 * n {
            return Err(format!(
                "slice: segment {target}/{center} has {} coefficients, want {}",
                coef.len(),
                records * 3 * n
            ));
        }
        out.push(Segment {
            target,
            center,
            init,
            interval,
            n,
            coef,
            records,
        });
    }
    Ok(out)
}

static SLICE: LazyLock<Vec<Segment>> = LazyLock::new(|| {
    decode(include_bytes!("../data/de440-1800-2200.bin")).expect("the built-in slice")
});

/// The built-in slice's segment for `target` relative to `center`.
pub fn segment(target: i32, center: i32) -> Option<&'static Segment> {
    SLICE
        .iter()
        .find(|s| s.target == target && s.center == center)
}
