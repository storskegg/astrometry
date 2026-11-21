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
        // A NaN date is "not covered" too.
        if r.is_nan() || r < 0.0 || r >= self.records as f64 {
            return None;
        }
        let r = r as usize;
        let radius = self.interval / 2.0;
        let mid = self.init + r as f64 * self.interval + radius;
        let x = (et - mid) / radius;
        let n = self.n;
        let block = self.block(r / self.per_block)?;
        let at = r % self.per_block;
        let rec = block.get(at * 3 * n..(at + 1) * 3 * n)?;
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

    /// Block `b`'s coefficients, decoded now if it isn't among the last few used; none if the
    /// block is damaged (so a corrupt file is "not covered", not a panic).
    fn block(&self, b: usize) -> Option<Arc<[f64]>> {
        // Nothing here leaves the cache half-written, so a poisoned lock is safe to reuse.
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(i) = cache.iter().position(|(k, _)| *k == b) {
            cache[..=i].rotate_right(1);
        } else {
            let coef = self.decode_block(b).ok()?;
            cache.insert(0, (b, coef.into()));
            cache.truncate(CACHED);
        }
        Some(Arc::clone(&cache[0].1))
    }

    /// Inflates block `b` and reads its varints.
    fn decode_block(&self, b: usize) -> Result<Vec<f64>, String> {
        let fail = |what: &str| {
            format!(
                "slice: segment {}/{} block {b}: {what}",
                self.target, self.center
            )
        };
        let &(offset, len) = self.blocks.get(b).ok_or_else(|| fail("no such block"))?;
        let records = b
            .checked_mul(self.per_block)
            .and_then(|first| self.records.checked_sub(first))
            .map(|left| left.min(self.per_block))
            .ok_or_else(|| fail("no such block"))?;
        // `parse` bounded these, so the product can't overflow.
        let want = records * 3 * self.n;
        let raw = offset
            .checked_add(len)
            .and_then(|end| self.bytes.get().get(offset..end))
            .ok_or_else(|| fail("past the end of the file"))?;
        // A varint is at most 10 bytes, so a block of `want` coefficients inflates to no more
        // than that; reading one byte beyond it catches a stream that overruns (a deflate bomb).
        let cap = want.saturating_mul(10).saturating_add(1) as u64;
        let mut stream = Vec::new();
        DeflateDecoder::new(raw)
            .take(cap)
            .read_to_end(&mut stream)
            .map_err(|e| fail(&e.to_string()))?;
        if stream.len() as u64 >= cap {
            return Err(fail("inflates to more than its records need"));
        }
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
            if coef.len() == want {
                return Err(fail("more coefficients than its records need"));
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

// Bounds on a head's counts, wide of the real files' (at most 14 coefficients, 36,525 records
// and 92 records per block) but small enough that nothing absurd is allocated or multiplied.
/// Coefficients per coordinate: a Chebyshev polynomial of degree 1,023 is far past any ephemeris.
const MAX_N: usize = 1024;
/// Records in a segment: 1,000,000 even of a day's span is 2,700 years.
const MAX_RECORDS: usize = 1_000_000;
/// Records per block.
const MAX_PER_BLOCK: usize = 65_536;
/// Coefficients in one decoded block (8 MiB of f64), the most a block may need.
const MAX_BLOCK_COEF: usize = 1 << 20;
/// A block table entry's size: offset (u64) and length (u32).
const ENTRY_LEN: usize = 8 + 4;

/// A fixed-size array from a slice of that size, as an error not a panic.
fn word<const N: usize>(s: &[u8]) -> Result<[u8; N], String> {
    s.try_into().map_err(|_| "slice: truncated".to_string())
}

/// Reads the heads and block tables, and checks the blocks lie within the file.
fn parse(bytes: &[u8]) -> Result<Vec<Head>, String> {
    let at = std::cell::Cell::new(0usize);
    let take = |n: usize| -> Result<&[u8], String> {
        let s = bytes
            .get(at.get()..at.get().checked_add(n).ok_or("slice: truncated")?)
            .ok_or("slice: truncated")?;
        at.set(at.get() + n);
        Ok(s)
    };
    let u32_ = |s: &[u8]| -> Result<usize, String> { Ok(u32::from_le_bytes(word(s)?) as usize) };
    let f64_ = |s: &[u8]| -> Result<f64, String> { Ok(f64::from_le_bytes(word(s)?)) };
    if take(8)? != b"ASTRDE02" {
        return Err("slice: not a DE slice".into());
    }
    let count = u32_(take(4)?)?;
    let mut heads = Vec::new();
    for _ in 0..count {
        let target = i32::from_le_bytes(word(take(4)?)?);
        let center = i32::from_le_bytes(word(take(4)?)?);
        let init = f64_(take(8)?)?;
        let interval = f64_(take(8)?)?;
        let n = u32_(take(4)?)?;
        let records = u32_(take(4)?)?;
        let step = f64_(take(8)?)?;
        let per_block = u32_(take(4)?)?;
        let count = u32_(take(4)?)?;
        let bad = |what: &str| format!("slice: segment {target}/{center}: {what}");
        // A zero, negative, infinite or NaN span or step can't be a Chebyshev record's.
        if !init.is_finite() || !interval.is_finite() || interval <= 0.0 {
            return Err(bad("bad start or interval"));
        }
        if !step.is_finite() || step <= 0.0 {
            return Err(bad("bad step"));
        }
        if n == 0 || n > MAX_N {
            return Err(bad("bad coefficient count"));
        }
        if records == 0 || records > MAX_RECORDS {
            return Err(bad("bad record count"));
        }
        if per_block == 0
            || per_block > MAX_PER_BLOCK
            || per_block.saturating_mul(3 * n) > MAX_BLOCK_COEF
        {
            return Err(bad("bad records per block"));
        }
        if count != records.div_ceil(per_block) {
            return Err(bad("bad block count"));
        }
        // The table must fit in what's left of the file, checked before it's allocated.
        if count > bytes.len().saturating_sub(at.get()) / ENTRY_LEN {
            return Err("slice: truncated".into());
        }
        let mut blocks = Vec::with_capacity(count);
        for _ in 0..count {
            let offset = usize::try_from(u64::from_le_bytes(word(take(8)?)?))
                .map_err(|_| "slice: truncated".to_string())?;
            let len = u32_(take(4)?)?;
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
        // The same 1800–2200, to within a record (the Moon's are shorter than theirs).
        let (start, end) = segs[0].span();
        let (moon_start, moon_end) = moon(&built()).span();
        assert!(start <= moon_start + 1.0 && start >= moon_start - 32.0);
        assert!((end - moon_end).abs() < 64.0, "{end} {moon_end}");
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

    const MAIN: &[u8] = include_bytes!("../data/de440-1800-2200.bin");
    // Offsets in the first segment's head, after the magic and the segment count.
    const COUNT: usize = 8;
    const INIT: usize = 20;
    const INTERVAL: usize = 28;
    const N: usize = 36;
    const RECORDS: usize = 40;
    const STEP: usize = 44;
    const PER_BLOCK: usize = 52;
    const BLOCKS: usize = 56;
    const TABLE: usize = 60;

    /// Parses `bytes` and queries every segment at a spread of dates: an error or a `None` is
    /// fine, a panic isn't.
    fn survives(bytes: &[u8]) {
        let Ok(segs) = decode(bytes) else { return };
        for s in &segs {
            let (a, b) = s.span();
            for k in 0..=20 {
                let _ = s.state(a + (b - a) * f64::from(k) / 20.0);
            }
            let _ = s.state(f64::NAN);
        }
    }

    fn put(bytes: &mut [u8], at: usize, with: &[u8]) {
        bytes[at..at + with.len()].copy_from_slice(with);
    }

    #[test]
    fn a_truncated_file_is_an_error_not_a_panic() {
        let first = parse(MAIN).unwrap()[0].blocks[0];
        let mut cuts = vec![0, 1, 7, 8, 9, 11, 12, 13, INIT + 3, BLOCKS + 2, TABLE];
        cuts.extend([
            TABLE + 5,
            TABLE + 12,
            first.0 - 1,
            first.0,
            first.0 + first.1 / 2,
        ]);
        cuts.extend((1..=50).map(|k| MAIN.len() * k / 51));
        cuts.push(MAIN.len() - 1);
        for cut in cuts {
            let bytes = &MAIN[..cut];
            // The tables claim blocks the shortened file hasn't got.
            assert!(parse(bytes).is_err(), "cut at {cut} parsed");
            survives(bytes);
        }
        assert!(parse(MAIN).is_ok());
    }

    #[test]
    fn huge_or_nonsense_counts_are_errors() {
        for at in [COUNT, N, RECORDS, PER_BLOCK, BLOCKS] {
            let mut bytes = MAIN.to_vec();
            put(&mut bytes, at, &u32::MAX.to_le_bytes());
            assert!(parse(&bytes).is_err(), "huge count at {at}");
            survives(&bytes);
        }
        for at in [N, RECORDS, PER_BLOCK, BLOCKS] {
            let mut bytes = MAIN.to_vec();
            put(&mut bytes, at, &0u32.to_le_bytes());
            assert!(parse(&bytes).is_err(), "zero at {at}");
        }
        // A count the bounds allow but the file can't back.
        let mut bytes = MAIN.to_vec();
        put(&mut bytes, COUNT, &1_000u32.to_le_bytes());
        assert!(parse(&bytes).is_err());
        // A first segment whose table is longer than what's left.
        assert!(parse(&MAIN[..TABLE + 24]).is_err());
    }

    #[test]
    fn a_bad_step_or_interval_is_an_error() {
        for at in [INIT, INTERVAL, STEP] {
            for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let mut bytes = MAIN.to_vec();
                put(&mut bytes, at, &bad.to_le_bytes());
                assert!(parse(&bytes).is_err(), "{bad} at {at}");
            }
        }
        for at in [INTERVAL, STEP] {
            for bad in [0.0f64, -1.0] {
                let mut bytes = MAIN.to_vec();
                put(&mut bytes, at, &bad.to_le_bytes());
                assert!(parse(&bytes).is_err(), "{bad} at {at}");
            }
        }
    }

    #[test]
    fn a_wrong_magic_or_a_block_past_the_end_is_an_error() {
        let mut bytes = MAIN.to_vec();
        put(&mut bytes, TABLE, &(MAIN.len() as u64).to_le_bytes());
        assert!(parse(&bytes).is_err());
        put(&mut bytes, TABLE, &u64::MAX.to_le_bytes());
        assert!(parse(&bytes).is_err());
        let mut bytes = MAIN.to_vec();
        put(&mut bytes, TABLE + 8, &u32::MAX.to_le_bytes());
        assert!(parse(&bytes).is_err());
        let mut bytes = MAIN.to_vec();
        put(&mut bytes, 0, b"ASTRDE01");
        assert!(parse(&bytes).is_err());
    }

    #[test]
    fn a_corrupt_block_is_none_when_queried() {
        let heads = parse(MAIN).unwrap();
        let head = heads.iter().find(|h| h.target == 301).unwrap();
        let (offset, len) = head.blocks[0];
        let mut bytes = MAIN.to_vec();
        bytes[offset..offset + len].fill(0xff);
        let segs = decode(&bytes).unwrap();
        let (start, _) = moon(&segs).span();
        assert!(moon(&segs).state(start + 1.0).is_none());
        // The next block is untouched.
        let block_days = head.per_block as f64 * head.interval / 86400.0;
        assert!(moon(&segs).state(start + 1.5 * block_days).is_some());
        // Zeros aren't a deflate stream either.
        bytes[offset..offset + len].fill(0);
        let segs = decode(&bytes).unwrap();
        let _ = moon(&segs).state(start + 1.0);
    }

    #[test]
    fn flipped_bytes_inside_blocks_never_panic() {
        let heads = parse(MAIN).unwrap();
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for head in &heads {
            for _ in 0..4 {
                let b = (next() % head.blocks.len() as u64) as usize;
                let (offset, len) = head.blocks[b];
                let mut bytes = MAIN.to_vec();
                for _ in 0..8 {
                    let at = offset + (next() % len as u64) as usize;
                    bytes[at] ^= (next() & 0xff) as u8 | 1;
                }
                let segs = decode(&bytes).unwrap();
                let s = segs.iter().find(|s| s.target == head.target).unwrap();
                let first = (b * head.per_block) as f64 * head.interval / 86400.0;
                let _ = s.state(s.span().0 + first + 0.5);
            }
        }
    }

    #[test]
    fn a_block_of_the_wrong_size_is_an_error() {
        // A head with one more coefficient per coordinate than its blocks hold, then one fewer.
        let n = u32::from_le_bytes(MAIN[N..N + 4].try_into().unwrap());
        for wrong in [n + 1, n - 1] {
            let mut bytes = MAIN.to_vec();
            put(&mut bytes, N, &wrong.to_le_bytes());
            let segs = decode(&bytes).unwrap();
            assert!(segs[0].decode_block(0).is_err());
            assert!(segs[0].decode_block(1_000_000).is_err());
            let (start, _) = segs[0].span();
            assert!(segs[0].state(start + 1.0).is_none());
        }
    }
}
