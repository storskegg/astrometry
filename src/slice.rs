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
use std::sync::OnceLock;

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
        self.state_after(jd, 0.0)
    }

    /// As [`Segment::state`], `secs` seconds after `jd`, the two kept apart until they're
    /// seconds past J2000 (see [`crate::source::Source::state_after`]).
    pub fn state_after(&self, jd: f64, secs: f64) -> Option<([f64; 3], [f64; 3])> {
        let et = (jd - 2451545.0) * 86400.0 + secs;
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
        Some(crate::cheb::with_polynomials(x, n, |t, dt| {
            let (mut pos, mut vel) = ([0.0; 3], [0.0; 3]);
            for c in 0..3 {
                let coef = &rec[c * n..(c + 1) * n];
                pos[c] = crate::cheb::sum(coef, t);
                vel[c] = crate::cheb::sum(coef, dt) / radius;
            }
            (pos, vel)
        }))
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

static SLICE: OnceLock<Vec<Segment>> = OnceLock::new();

/// Installs the slice from its bytes, `data/de440-1800-2200.bin`, for a build without the
/// `embedded` feature (a browser's, which fetches them after it starts). Once per process: an
/// error if it's installed already, as it always is with `embedded`, or if the bytes aren't a
/// slice.
pub fn install(bytes: &[u8]) -> Result<(), String> {
    if SLICE.get().is_some() || cfg!(feature = "embedded") {
        return Err("slice: installed already".into());
    }
    SLICE
        .set(decode(bytes)?)
        .map_err(|_| "slice: installed already".into())
}

/// Whether the slice is there: always with `embedded`, otherwise once [`install`]ed.
pub fn installed() -> bool {
    cfg!(feature = "embedded") || SLICE.get().is_some()
}

fn slice() -> &'static [Segment] {
    #[cfg(feature = "embedded")]
    return SLICE.get_or_init(|| {
        decode(include_bytes!("../data/de440-1800-2200.bin")).expect("the built-in slice")
    });
    #[cfg(not(feature = "embedded"))]
    SLICE.get().expect(
        "astrometry: the DE440 slice isn't installed (built without `embedded`: call \
         astrometry::slice::install first)",
    )
}

/// The built-in slice's segment for `target` relative to `center`. Panics if the slice isn't
/// [`install`]ed in a build without `embedded`, rather than fall back to the theories unseen.
pub fn segment(target: i32, center: i32) -> Option<&'static Segment> {
    slice()
        .iter()
        .find(|s| s.target == target && s.center == center)
}

#[cfg(all(test, not(feature = "embedded")))]
mod tests {
    #[test]
    fn install_from_bytes() {
        assert!(!super::installed());
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/de440-1800-2200.bin");
        let bytes = std::fs::read(path).unwrap();
        assert!(super::install(b"not a slice").is_err());
        super::install(&bytes).unwrap();
        assert!(super::installed() && super::install(&bytes).is_err());
        assert!(super::segment(301, 3).is_some());
    }
}
