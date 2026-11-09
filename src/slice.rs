//! The built-in slice of JPL's DE440: the Chebyshev records of the segments the Sun, Moon and
//! classical planets need, for 1800–2200, re-encoded by this project (`cargo xtask de440`):
//! each coefficient rounded to a fixed step (1 cm for the Moon about the Earth–Moon barycentre,
//! 1 m for the rest), written as zigzag varints, and deflated in blocks of about a year's
//! records, so only the blocks in use are ever inflated. The Earth about the Earth–Moon
//! barycentre isn't kept: it's the Moon's offset divided by the Earth/Moon mass ratio.
//!
//! Derived from DE440 (Park, Folkner, Williams and Boggs, 2021) as JPL and NAIF distribute it;
//! modified, as NAIF's rules for SPICE kernels put it, by this project, and validated against
//! the original by the crate's tests.
//!
//! The file: `ASTRDE02`, the segment count (u32), then for each segment its head and its block
//! table. The head: target and centre (i32), the first record's start and each record's span
//! (f64, TDB seconds past J2000), the coefficients per coordinate and the record count (u32),
//! the step (f64, km), the records per block and the block count (u32). The table: for each
//! block, its byte offset in the file (u64) and its length (u32). Then the blocks, each
//! deflated on its own: the zigzag varints of its records' coefficients, record after record,
//! x, y, z. All little-endian; the file as a whole isn't deflated.
//!
//! With the `outer-planets` feature there's a second file of the same format and span,
//! `data/de440-outer-1800-2200.bin` (`cargo xtask outer`): Uranus, Neptune and Pluto's
//! barycentres (7, 8, 9) about the solar-system barycentre, rounded to the same 1 m.

use std::io::Read;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use flate2::read::DeflateDecoder;

/// Blocks kept decoded per segment, the most recently used.
const CACHED: usize = 4;

/// The file's bytes: the compiled-in ones, or a copy shared by the segments read from them.
#[derive(Clone, Debug)]
enum Bytes {
    #[cfg(feature = "embedded")]
    Static(&'static [u8]),
    Shared(Arc<[u8]>),
}

impl Bytes {
    fn get(&self) -> &[u8] {
        match self {
            #[cfg(feature = "embedded")]
            Bytes::Static(b) => b,
            Bytes::Shared(b) => b,
        }
    }
}

/// A segment: its head and block table are parsed, its blocks decoded as they're needed.
#[derive(Debug)]
pub struct Segment {
    pub target: i32,
    pub center: i32,
    init: f64,
    interval: f64,
    n: usize,
    records: usize,
    step: f64,
    per_block: usize,
    /// Each block's offset and length in `bytes`.
    blocks: Vec<(usize, usize)>,
    bytes: Bytes,
    /// Decoded blocks, the most recently used first: record after record, x, y, z
    /// coefficients, km.
    cache: Mutex<Vec<(usize, Arc<[f64]>)>>,
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
        let block = self.block(r / self.per_block);
        let at = r % self.per_block;
        let rec = &block[at * 3 * n..(at + 1) * 3 * n];
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

    /// Block `b`'s coefficients, decoded now if it isn't among the last few used.
    fn block(&self, b: usize) -> Arc<[f64]> {
        // Nothing here leaves the cache half-written, so a poisoned lock is safe to reuse.
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(i) = cache.iter().position(|(k, _)| *k == b) {
            cache[..=i].rotate_right(1);
        } else {
            let coef = self.decode_block(b).expect("the built-in slice");
            cache.insert(0, (b, coef.into()));
            cache.truncate(CACHED);
        }
        Arc::clone(&cache[0].1)
    }

    /// Inflates block `b` and reads its varints.
    fn decode_block(&self, b: usize) -> Result<Vec<f64>, String> {
        let (offset, len) = self.blocks[b];
        let records = self.per_block.min(self.records - b * self.per_block);
        let want = records * 3 * self.n;
        let fail = |what: &str| {
            format!(
                "slice: segment {}/{} block {b}: {what}",
                self.target, self.center
            )
        };
        let mut stream = Vec::new();
        DeflateDecoder::new(&self.bytes.get()[offset..offset + len])
            .read_to_end(&mut stream)
            .map_err(|e| fail(&e.to_string()))?;
        let mut coef = Vec::with_capacity(want);
        let mut i = 0;
        while i < stream.len() {
            let (mut v, mut shift) = (0u64, 0);
            loop {
                let byte = *stream.get(i).ok_or_else(|| fail("truncated varint"))?;
                i += 1;
                v |= u64::from(byte & 0x7f) << shift;
                if byte & 0x80 == 0 {
                    break;
                }
                shift += 7;
                if shift > 63 {
                    return Err(fail("varint too long"));
                }
            }
            let q = (v >> 1) as i64 ^ -((v & 1) as i64);
            coef.push(q as f64 * self.step);
        }
        if coef.len() != want {
            return Err(fail(&format!("{} coefficients, want {want}", coef.len())));
        }
        Ok(coef)
    }
}

/// A segment's head and block table, as the file has them.
struct Head {
    target: i32,
    center: i32,
    init: f64,
    interval: f64,
    n: usize,
    records: usize,
    step: f64,
    per_block: usize,
    blocks: Vec<(usize, usize)>,
}

/// Reads the heads and block tables, and checks the blocks lie within the file.
fn parse(bytes: &[u8]) -> Result<Vec<Head>, String> {
    let mut at = 0usize;
    let mut take = |n: usize| -> Result<&[u8], String> {
        let s = bytes
            .get(at..at.checked_add(n).ok_or("slice: truncated")?)
            .ok_or("slice: truncated")?;
        at += n;
        Ok(s)
    };
    if take(8)? != b"ASTRDE02" {
        return Err("slice: not a DE slice".into());
    }
    let u32_ = |s: &[u8]| u32::from_le_bytes(s.try_into().expect("four")) as usize;
    let f64_ = |s: &[u8]| f64::from_le_bytes(s.try_into().expect("eight"));
    let count = u32_(take(4)?);
    let mut heads = Vec::new();
    for _ in 0..count {
        let target = i32::from_le_bytes(take(4)?.try_into().expect("four"));
        let center = i32::from_le_bytes(take(4)?.try_into().expect("four"));
        let init = f64_(take(8)?);
        let interval = f64_(take(8)?);
        let n = u32_(take(4)?);
        let records = u32_(take(4)?);
        let step = f64_(take(8)?);
        let per_block = u32_(take(4)?);
        let count = u32_(take(4)?);
        if per_block == 0 || count != records.div_ceil(per_block) {
            return Err(format!(
                "slice: segment {target}/{center} has a bad block count"
            ));
        }
        let mut blocks = Vec::new();
        for _ in 0..count {
            let offset = u64::from_le_bytes(take(8)?.try_into().expect("eight")) as usize;
            let len = u32_(take(4)?);
            if offset.checked_add(len).is_none_or(|end| end > bytes.len()) {
                return Err("slice: truncated".into());
            }
            blocks.push((offset, len));
        }
        heads.push(Head {
            target,
            center,
            init,
            interval,
            n,
            records,
            step,
            per_block,
            blocks,
        });
    }
    Ok(heads)
}

/// The slice's segments over its bytes: nothing is inflated yet.
fn segments(heads: Vec<Head>, bytes: &Bytes) -> Vec<Segment> {
    heads
        .into_iter()
        .map(|h| Segment {
            target: h.target,
            center: h.center,
            init: h.init,
            interval: h.interval,
            n: h.n,
            records: h.records,
            step: h.step,
            per_block: h.per_block,
            blocks: h.blocks,
            bytes: bytes.clone(),
            cache: Mutex::new(Vec::new()),
        })
        .collect()
}

/// Reads a slice's bytes into segments of their own, on a copy of them. Only the heads are
/// read now; each block is inflated when it's first needed.
pub fn decode(bytes: &[u8]) -> Result<Vec<Segment>, String> {
    let heads = parse(bytes)?;
    Ok(segments(heads, &Bytes::Shared(Arc::from(bytes))))
}

static SLICE: OnceLock<Vec<Segment>> = OnceLock::new();

/// Installs the slice from its bytes, `data/de440-1800-2200.bin`, for a build without the
/// `embedded` feature (a browser's, which fetches them after it starts). Once per process: an
/// error if it's installed already, as it always is with `embedded`, or if the bytes aren't a
/// slice. It keeps a copy of the bytes.
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
        let bytes: &'static [u8] = include_bytes!("../data/de440-1800-2200.bin");
        segments(
            parse(bytes).expect("the built-in slice"),
            &Bytes::Static(bytes),
        )
    });
    #[cfg(not(feature = "embedded"))]
    SLICE.get().expect(
        "astrometry: the DE440 slice isn't installed (built without `embedded`: call \
         astrometry::slice::install first)",
    )
}

/// The outer planets' segments, Uranus, Neptune and Pluto about the solar-system barycentre,
/// from their own file: the same format and span as the main slice.
#[cfg(feature = "outer-planets")]
static OUTER: OnceLock<Vec<Segment>> = OnceLock::new();

/// Installs the outer planets' file, `data/de440-outer-1800-2200.bin`, for a build with
/// `outer-planets` but without `embedded`, as [`install`] does the main slice. Once per
/// process: an error if it's installed already, as it always is with `embedded`, or if the
/// bytes aren't a slice. It keeps a copy of the bytes.
#[cfg(feature = "outer-planets")]
pub fn install_outer(bytes: &[u8]) -> Result<(), String> {
    if OUTER.get().is_some() || cfg!(feature = "embedded") {
        return Err("slice: the outer planets are installed already".into());
    }
    OUTER
        .set(decode(bytes)?)
        .map_err(|_| "slice: the outer planets are installed already".into())
}

/// Whether the outer planets' file is there: always with `embedded`, otherwise once
/// [`install_outer`]ed.
#[cfg(feature = "outer-planets")]
pub fn outer_installed() -> bool {
    cfg!(feature = "embedded") || OUTER.get().is_some()
}

/// The outer planets' segment for `target` relative to `center`; none if there isn't one, or
/// in a build without `embedded` before [`install_outer`] (so that the planets are then "not
/// covered" errors, not panics).
#[cfg(feature = "outer-planets")]
pub fn outer_segment(target: i32, center: i32) -> Option<&'static Segment> {
    #[cfg(feature = "embedded")]
    let segs = OUTER.get_or_init(|| {
        let bytes: &'static [u8] = include_bytes!("../data/de440-outer-1800-2200.bin");
        segments(
            parse(bytes).expect("the built-in outer slice"),
            &Bytes::Static(bytes),
        )
    });
    #[cfg(not(feature = "embedded"))]
    let segs = OUTER.get()?;
    segs.iter()
        .find(|s| s.target == target && s.center == center)
}

/// The built-in slice's segment for `target` relative to `center`. Panics if the slice isn't
/// [`install`]ed in a build without `embedded`, rather than fall back to the theories unseen.
pub fn segment(target: i32, center: i32) -> Option<&'static Segment> {
    slice()
        .iter()
        .find(|s| s.target == target && s.center == center)
}

#[cfg(all(test, not(feature = "embedded")))]
mod install_tests {
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

    #[cfg(feature = "outer-planets")]
    #[test]
    fn install_outer_from_bytes() {
        assert!(!super::outer_installed() && super::outer_segment(7, 0).is_none());
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/de440-outer-1800-2200.bin"
        );
        let bytes = std::fs::read(path).unwrap();
        assert!(super::install_outer(b"not a slice").is_err());
        super::install_outer(&bytes).unwrap();
        assert!(super::outer_installed() && super::install_outer(&bytes).is_err());
        assert!(super::outer_segment(7, 0).is_some() && super::outer_segment(301, 3).is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fresh segments over the file, apart from the process-wide ones.
    fn built() -> Vec<Segment> {
        decode(include_bytes!("../data/de440-1800-2200.bin")).unwrap()
    }

    fn moon(segs: &[Segment]) -> &Segment {
        segs.iter().find(|s| s.target == 301).unwrap()
    }

    #[test]
    fn every_block_decodes_to_the_coefficients_the_head_counts() {
        for s in built() {
            let mut total = 0;
            for b in 0..s.blocks.len() {
                total += s.decode_block(b).unwrap().len();
            }
            assert_eq!(total, s.records * 3 * s.n, "{}/{}", s.target, s.center);
        }
    }

    #[test]
    fn a_damaged_block_is_an_error() {
        let mut bytes = include_bytes!("../data/de440-1800-2200.bin").to_vec();
        let (offset, len) = parse(&bytes).unwrap()[0].blocks[3];
        bytes[offset..offset + len].fill(0xff);
        let segs = decode(&bytes).unwrap();
        assert!(segs[0].decode_block(3).is_err());
        assert!(segs[0].decode_block(2).is_ok());
        assert!(parse(&bytes[..bytes.len() / 2]).is_err());
    }

    #[test]
    fn moon_is_finite_and_continuous_across_blocks() {
        let segs = built();
        let moon = moon(&segs);
        let (start, end) = moon.span();
        let boundary = start + moon.per_block as f64 * moon.interval / 86400.0;
        for jd in [
            start,
            start + 1.0,
            boundary - 0.5,
            boundary + 0.5,
            2451545.0,
            2461000.5,
            end - 1e-6,
        ] {
            let (p, v) = moon.state(jd).unwrap();
            assert!(p.iter().chain(&v).all(|x| x.is_finite()), "{jd}");
        }
        assert!(moon.state(start - 1.0).is_none() && moon.state(end + 1.0).is_none());
        // Across the boundary the position moves by about the velocity times the gap.
        let gap = 1e-4;
        let (a, va) = moon.state(boundary - gap / 2.0).unwrap();
        let (b, _) = moon.state(boundary + gap / 2.0).unwrap();
        for c in 0..3 {
            let expect = va[c] * gap * 86400.0;
            assert!((b[c] - a[c] - expect).abs() < 1e-3, "{c}: {}", b[c] - a[c]);
        }
    }

    #[test]
    fn cache_keeps_the_last_four_blocks() {
        let segs = built();
        let moon = moon(&segs);
        let (start, _) = moon.span();
        let block_days = moon.per_block as f64 * moon.interval / 86400.0;
        for b in 0..50 {
            assert!(moon.state(start + (b as f64 + 0.5) * block_days).is_some());
            assert!(moon.cache.lock().unwrap().len() <= CACHED);
        }
        let cache = moon.cache.lock().unwrap();
        let held: Vec<usize> = cache.iter().map(|(k, _)| *k).collect();
        assert_eq!(held, [49, 48, 47, 46]);
    }

    #[cfg(all(feature = "embedded", feature = "outer-planets"))]
    #[test]
    fn the_outer_file_has_uranus_neptune_and_pluto_about_the_barycentre() {
        let segs = decode(include_bytes!("../data/de440-outer-1800-2200.bin")).unwrap();
        let targets: Vec<_> = segs.iter().map(|s| (s.target, s.center)).collect();
        assert_eq!(targets, [(7, 0), (8, 0), (9, 0)]);
        assert_eq!(segs[0].span(), moon(&built()).span());
        for t in 7..=9 {
            assert!(outer_segment(t, 0).is_some());
        }
    }

    #[cfg(feature = "embedded")]
    #[test]
    fn decode_gives_segments_that_agree_with_the_built_in_ones() {
        let segs = decode(include_bytes!("../data/de440-1800-2200.bin")).unwrap();
        assert!(decode(b"not a slice").is_err());
        for s in &segs {
            let built_in = segment(s.target, s.center).unwrap();
            let (start, end) = s.span();
            assert_eq!(s.span(), built_in.span());
            for k in 0..200 {
                let jd = start + (end - start) * f64::from(k) / 200.0;
                assert_eq!(s.state(jd), built_in.state(jd), "{}/{}", s.target, s.center);
            }
        }
    }
}
