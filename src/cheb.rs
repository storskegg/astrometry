//! Chebyshev series, as JPL's ephemerides give positions: the polynomials and their
//! derivatives at a point, once, for the slice and the SPK reader alike.

/// The most terms held on the stack; a longer series (none in DE440, whose most is 15) is
/// held on the heap.
const STACK: usize = 32;

/// Calls `f` with the Chebyshev polynomials T₀…Tₙ₋₁ at `x` (in −1…1) and their derivatives
/// with respect to `x`, without allocating for a series of up to 32 terms.
pub(crate) fn with_polynomials<R>(x: f64, n: usize, f: impl FnOnce(&[f64], &[f64]) -> R) -> R {
    if n <= STACK {
        let (mut t, mut dt) = ([0.0; STACK], [0.0; STACK]);
        fill(x, &mut t[..n], &mut dt[..n]);
        f(&t[..n], &dt[..n])
    } else {
        let (mut t, mut dt) = (vec![0.0; n], vec![0.0; n]);
        fill(x, &mut t, &mut dt);
        f(&t, &dt)
    }
}

fn fill(x: f64, t: &mut [f64], dt: &mut [f64]) {
    let n = t.len();
    if n == 0 {
        return;
    }
    t[0] = 1.0;
    if n > 1 {
        t[1] = x;
        dt[1] = 1.0;
    }
    for k in 2..n {
        t[k] = 2.0 * x * t[k - 1] - t[k - 2];
        dt[k] = 2.0 * t[k - 1] + 2.0 * x * dt[k - 1] - dt[k - 2];
    }
}

/// A series' value: its coefficients against the polynomials.
pub(crate) fn sum(coef: &[f64], t: &[f64]) -> f64 {
    coef.iter().zip(t).map(|(a, b)| a * b).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T₂(x) = 2x² − 1, T₃(x) = 4x³ − 3x, and their derivatives; on the heap as on the stack.
    #[test]
    fn polynomials() {
        let x = 0.3;
        for n in [4, 40] {
            with_polynomials(x, n, |t, dt| {
                assert_eq!(t.len(), n);
                assert!((t[2] - (2.0 * x * x - 1.0)).abs() < 1e-15);
                assert!((t[3] - (4.0 * x * x * x - 3.0 * x)).abs() < 1e-15);
                assert!((dt[2] - 4.0 * x).abs() < 1e-15);
                assert!((dt[3] - (12.0 * x * x - 3.0)).abs() < 1e-15);
            });
        }
        with_polynomials(x, 0, |t, dt| assert!(t.is_empty() && dt.is_empty()));
    }
}
