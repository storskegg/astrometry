//! The built-in slice against DE440's own segments (the reference fixture's geometric
//! section): the rounding of its coefficients only.
#![cfg(feature = "embedded")]

mod common;

use astrometry::slice;
use common::Reference;

#[test]
fn slice_segments_against_de440() {
    let r = Reference::load();
    let mut failed = Vec::new();
    for (target, center, positions) in &r.segments {
        let found = slice::segment(*target, *center);
        #[cfg(feature = "outer-planets")]
        let found = found.or_else(|| slice::outer_segment(*target, *center));
        let Some(seg) = found else {
            // Only the outer planets' file may be missing, without its feature.
            assert!((7..=9).contains(target), "no segment {target}/{center}");
            println!("{target}/{center}: skipped (no `outer-planets`)");
            continue;
        };
        let mut worst = 0.0f64;
        for (jd, p) in r.instants.iter().zip(positions) {
            let (s, _) = seg.state(*jd).expect("covered");
            let d = ((s[0] - p[0]).powi(2) + (s[1] - p[1]).powi(2) + (s[2] - p[2]).powi(2)).sqrt();
            worst = worst.max(d);
        }
        // n·step/2 with n ≤ 14: the rounding's guaranteed bound (each coefficient is rounded by
        // at most half the step, and |T_k| ≤ 1). Step 1 m, so 7 m; 1 cm for the Moon about the
        // Earth–Moon barycentre, so 7 cm. This guards the format; accuracy is guarded by
        // `builtin_against_de440`.
        let limit = if (*target, *center) == (301, 3) {
            7e-5
        } else {
            7e-3
        };
        println!(
            "{target}/{center}: worst {:.3} m (limit {} m)",
            worst * 1e3,
            limit * 1e3
        );
        if worst >= limit {
            failed.push(format!("{target}/{center}: {:.3} m", worst * 1e3));
        }
    }
    assert!(failed.is_empty(), "over the limit: {failed:?}");
}
