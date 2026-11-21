//! The DE440 reference fixture, `tests/data/de440/reference.bin` (format `ASTRREF1`, described
//! in `xtask/src/reference.rs` and the README).
#![allow(dead_code)]

use astrometry::source::Body;

pub struct Reference {
    pub instants: Vec<f64>,
    /// Target, centre and the position (km) at each instant.
    pub segments: Vec<(i32, i32, Vec<[f64; 3]>)>,
    /// Body id (`Body as u32`), and the longitude and latitude (degrees) at each instant.
    pub apparent: Vec<(u32, Vec<[f64; 2]>)>,
}

impl Reference {
    pub fn load() -> Reference {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/de440/reference.bin"
        );
        let b = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert!(b.starts_with(b"ASTRREF1"), "{path}: not an ASTRREF1 file");
        let mut at = 8;
        let mut take = |n: usize| {
            at += n;
            &b[at - n..at]
        };
        let u32_ = |s: &[u8]| u32::from_le_bytes(s.try_into().unwrap());
        let f64_ = |s: &[u8]| f64::from_le_bytes(s.try_into().unwrap());
        let (n, segs, bodies) = (u32_(take(4)), u32_(take(4)), u32_(take(4)));
        let instants = (0..n).map(|_| f64_(take(8))).collect();
        let segments = (0..segs)
            .map(|_| {
                let (t, c) = (u32_(take(4)) as i32, u32_(take(4)) as i32);
                let v = (0..n).map(|_| [(); 3].map(|_| f64_(take(8)))).collect();
                (t, c, v)
            })
            .collect();
        let apparent = (0..bodies)
            .map(|_| {
                let id = u32_(take(4));
                (id, (0..n).map(|_| [(); 2].map(|_| f64_(take(8)))).collect())
            })
            .collect();
        assert_eq!(at, b.len(), "{path}: trailing bytes");
        Reference {
            instants,
            segments,
            apparent,
        }
    }

    /// The body's longitude and latitude at each instant.
    pub fn apparent(&self, body: Body) -> &[[f64; 2]] {
        &self
            .apparent
            .iter()
            .find(|(id, _)| *id == body as u32)
            .unwrap_or_else(|| panic!("no {body:?} in the reference"))
            .1
    }
}
