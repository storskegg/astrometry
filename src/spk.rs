//! JPL SPK files: the DAF container and its Chebyshev segments (types 2 and 3), as JPL's DE
//! ephemerides are distributed (de440.bsp and the like).

use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;

/// A segment's summary.
#[derive(Clone, Copy, Debug)]
struct Segment {
    /// TDB seconds past J2000.
    start: f64,
    end: f64,
    target: i32,
    center: i32,
    kind: i32,
    /// Its first and last 8-byte words, 1-based.
    begin: usize,
    finish: usize,
    /// From its trailer: the first record's start, each record's span and size, the count.
    init: f64,
    interval: f64,
    record_size: usize,
    records: usize,
}

/// A run of a type 2 segment's records: each record's coefficients, x then y then z, `n`
/// each, the first record starting at `init` (TDB seconds past J2000), each `interval`
/// seconds long.
#[derive(Clone, Debug)]
pub struct Records {
    pub target: i32,
    pub center: i32,
    pub init: f64,
    pub interval: f64,
    pub n: usize,
    pub records: Vec<Vec<f64>>,
}

/// An SPK file, read on demand.
#[derive(Debug)]
pub struct Spk {
    file: Mutex<File>,
    segments: Vec<Segment>,
    /// The last record read, by segment index and record number.
    cache: Mutex<Option<(usize, usize, Vec<f64>)>>,
}

/// An SPK file that couldn't be read, or a request it can't answer.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Format(String),
    NoSegment { target: i32, center: i32, jd: f64 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "spk: {e}"),
            Error::Format(why) => write!(f, "spk: {why}"),
            Error::NoSegment { target, center, jd } => {
                write!(
                    f,
                    "spk: no segment for {target} relative to {center} at JD {jd}"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::Io(e)
    }
}

const RECORD: usize = 1024;

fn f64_at(b: &[u8], i: usize) -> f64 {
    f64::from_le_bytes(b[i..i + 8].try_into().expect("eight bytes"))
}

fn i32_at(b: &[u8], i: usize) -> i32 {
    i32::from_le_bytes(b[i..i + 4].try_into().expect("four bytes"))
}

impl Spk {
    pub fn open(path: impl AsRef<Path>) -> Result<Spk, Error> {
        let mut file = File::open(path)?;
        let mut rec = vec![0u8; RECORD];
        file.read_exact(&mut rec)?;
        if &rec[..8] != b"DAF/SPK " {
            return Err(Error::Format("not an SPK file".into()));
        }
        if &rec[88..96] != b"LTL-IEEE" {
            return Err(Error::Format("not little-endian IEEE".into()));
        }
        let (nd, ni) = (i32_at(&rec, 8) as usize, i32_at(&rec, 12) as usize);
        if (nd, ni) != (2, 6) {
            return Err(Error::Format(format!("ND {nd}, NI {ni}; want 2 and 6")));
        }
        let mut next = i32_at(&rec, 76) as usize;
        let summary_doubles = nd + ni.div_ceil(2);
        let mut segments = Vec::new();
        while next != 0 {
            file.seek(SeekFrom::Start(((next - 1) * RECORD) as u64))?;
            file.read_exact(&mut rec)?;
            let count = f64_at(&rec, 16) as usize;
            for i in 0..count {
                let o = 24 + i * summary_doubles * 8;
                let ints = |k: usize| i32_at(&rec, o + 16 + 4 * k);
                segments.push(Segment {
                    start: f64_at(&rec, o),
                    end: f64_at(&rec, o + 8),
                    target: ints(0),
                    center: ints(1),
                    kind: ints(3),
                    begin: ints(4) as usize,
                    finish: ints(5) as usize,
                    init: 0.0,
                    interval: 0.0,
                    record_size: 0,
                    records: 0,
                });
            }
            next = f64_at(&rec, 0) as usize;
        }
        let spk = Spk {
            file: Mutex::new(file),
            segments,
            cache: Mutex::new(None),
        };
        let mut segments = spk.segments.clone();
        for s in &mut segments {
            if !matches!(s.kind, 2 | 3) {
                continue;
            }
            let t = spk.words(s.finish - 3, 4)?;
            (s.init, s.interval, s.record_size, s.records) =
                (t[0], t[1], t[2] as usize, t[3] as usize);
        }
        Ok(Spk { segments, ..spk })
    }

    /// `n` words from word `at` (1-based).
    fn words(&self, at: usize, n: usize) -> Result<Vec<f64>, Error> {
        let mut b = vec![0u8; n * 8];
        let mut file = self.file.lock().expect("not poisoned");
        file.seek(SeekFrom::Start(((at - 1) * 8) as u64))?;
        file.read_exact(&mut b)?;
        Ok((0..n).map(|i| f64_at(&b, i * 8)).collect())
    }

    /// The position (km) and velocity (km/s) of `target` relative to `center` at TDB Julian
    /// date `jd`, in the file's frame (ICRF for the DE ephemerides).
    pub fn state(&self, target: i32, center: i32, jd: f64) -> Result<([f64; 3], [f64; 3]), Error> {
        self.state_after(target, center, jd, 0.0)
    }

    /// As [`Spk::state`], `secs` seconds after `jd`, the two kept apart until they're seconds
    /// past J2000 (see [`crate::source::Source::state_after`]).
    pub fn state_after(
        &self,
        target: i32,
        center: i32,
        jd: f64,
        secs: f64,
    ) -> Result<([f64; 3], [f64; 3]), Error> {
        let et = (jd - 2451545.0) * 86400.0 + secs;
        let (i, s) = self
            .segments
            .iter()
            .enumerate()
            .find(|(_, s)| s.target == target && s.center == center && s.start <= et && et <= s.end)
            .ok_or(Error::NoSegment { target, center, jd })?;
        if !matches!(s.kind, 2 | 3) {
            return Err(Error::Format(format!(
                "segment type {} isn't Chebyshev",
                s.kind
            )));
        }
        let r = (((et - s.init) / s.interval).floor() as usize).min(s.records - 1);
        let mut cache = self.cache.lock().expect("not poisoned");
        let hit = matches!(&*cache, Some((ci, cr, _)) if *ci == i && *cr == r);
        if !hit {
            let words = self.words(s.begin + r * s.record_size, s.record_size)?;
            *cache = Some((i, r, words));
        }
        let rec = &cache.as_ref().expect("filled").2;
        let (mid, radius) = (rec[0], rec[1]);
        let components = if s.kind == 2 { 3 } else { 6 };
        let n = (s.record_size - 2) / components;
        let x = (et - mid) / radius;
        Ok(crate::cheb::with_polynomials(x, n, |t, dt| {
            let (mut pos, mut vel) = ([0.0; 3], [0.0; 3]);
            for c in 0..3 {
                let coef = &rec[2 + c * n..2 + (c + 1) * n];
                pos[c] = crate::cheb::sum(coef, t);
                vel[c] = if s.kind == 3 {
                    crate::cheb::sum(&rec[2 + (3 + c) * n..2 + (4 + c) * n], t)
                } else {
                    crate::cheb::sum(coef, dt) / radius
                };
            }
            (pos, vel)
        }))
    }

    /// The Chebyshev records of `target` relative to `center` that cover TDB Julian dates
    /// `from` to `to`, from one type 2 segment, for re-encoding.
    pub fn records(&self, target: i32, center: i32, from: f64, to: f64) -> Result<Records, Error> {
        let (et0, et1) = ((from - 2451545.0) * 86400.0, (to - 2451545.0) * 86400.0);
        let s = self
            .segments
            .iter()
            .find(|s| s.target == target && s.center == center && s.start <= et0 && et1 <= s.end)
            .ok_or(Error::NoSegment {
                target,
                center,
                jd: from,
            })?;
        if s.kind != 2 {
            return Err(Error::Format(format!("segment type {}, want 2", s.kind)));
        }
        let first = ((et0 - s.init) / s.interval).floor() as usize;
        let last = (((et1 - s.init) / s.interval).ceil() as usize).min(s.records);
        let n = (s.record_size - 2) / 3;
        let mut records = Vec::with_capacity(last - first);
        for r in first..last {
            let words = self.words(s.begin + r * s.record_size, s.record_size)?;
            let (mid, radius) = (words[0], words[1]);
            let start = s.init + r as f64 * s.interval;
            if (mid - (start + s.interval / 2.0)).abs() > 1e-6
                || (radius - s.interval / 2.0).abs() > 1e-6
            {
                return Err(Error::Format(format!("record {r} isn't evenly spaced")));
            }
            records.push(words[2..].to_vec());
        }
        Ok(Records {
            target,
            center,
            init: s.init + first as f64 * s.interval,
            interval: s.interval,
            n,
            records,
        })
    }

    /// The span the file covers for `target` relative to `center`, as TDB Julian dates.
    pub fn span(&self, target: i32, center: i32) -> Option<(f64, f64)> {
        let s: Vec<&Segment> = self
            .segments
            .iter()
            .filter(|s| s.target == target && s.center == center)
            .collect();
        let start = s.iter().map(|s| s.start).fold(f64::INFINITY, f64::min);
        let end = s.iter().map(|s| s.end).fold(f64::NEG_INFINITY, f64::max);
        (!s.is_empty()).then(|| (2451545.0 + start / 86400.0, 2451545.0 + end / 86400.0))
    }
}
