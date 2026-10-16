//! Nutation: IAU 2000A with the IAU 2006 adjustments (IAU 2000A_R06), from the IERS
//! Conventions (2010) tables 5.3a and 5.3b, with the fundamental arguments of the IERS
//! Conventions (2003).

use std::sync::LazyLock;

use crate::time::centuries;

/// Arcseconds in a radian.
pub const ARCSEC: f64 = std::f64::consts::PI / (180.0 * 3600.0);

/// Arcseconds in a full turn.
const TURN: f64 = 1_296_000.0;

/// A series' term: microarcseconds for sin and cos of the argument, the argument's 14
/// multipliers, and whether it's multiplied by t.
struct Term {
    sin: f64,
    cos: f64,
    args: [i8; 14],
    t: bool,
}

/// A table's terms; `sin_first` says which coefficient its first column holds.
fn parse(text: &str, sin_first: bool) -> Vec<Term> {
    let mut t = false;
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with("j = 1") {
            t = true;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() != 17 || f[0].parse::<u32>().is_err() {
            continue;
        }
        let (Ok(a), Ok(b)) = (f[1].parse::<f64>(), f[2].parse::<f64>()) else {
            continue;
        };
        let mut args = [0i8; 14];
        for (x, s) in args.iter_mut().zip(&f[3..]) {
            *x = s.parse().expect("multiplier");
        }
        let (sin, cos) = if sin_first { (a, b) } else { (b, a) };
        out.push(Term { sin, cos, args, t });
    }
    out
}

/// Nutation in longitude: A_i sin + A"_i cos (table 5.3a).
static LONGITUDE: LazyLock<Vec<Term>> =
    LazyLock::new(|| parse(include_str!("../data/iers-tab5.3a.txt"), true));
/// Nutation in obliquity: B"_i sin, B_i cos, listed in that order (table 5.3b).
static OBLIQUITY: LazyLock<Vec<Term>> =
    LazyLock::new(|| parse(include_str!("../data/iers-tab5.3b.txt"), true));

/// The 14 fundamental arguments at `t` Julian centuries of TT since J2000, radians: the
/// Delaunay arguments l, l′, F, D, Ω, the planets' mean longitudes Mercury to Neptune, and the
/// general precession in longitude.
fn arguments(t: f64) -> [f64; 14] {
    let arcsec =
        |c: [f64; 5]| ((c[0] + t * (c[1] + t * (c[2] + t * (c[3] + t * c[4])))) % TURN) * ARCSEC;
    [
        arcsec([
            485868.249036,
            1717915923.2178,
            31.8792,
            0.051635,
            -0.00024470,
        ]),
        arcsec([
            1287104.793048,
            129596581.0481,
            -0.5532,
            0.000136,
            -0.00001149,
        ]),
        arcsec([
            335779.526232,
            1739527262.8478,
            -12.7512,
            -0.001037,
            0.00000417,
        ]),
        arcsec([
            1072260.703692,
            1602961601.2090,
            -6.3706,
            0.006593,
            -0.00003169,
        ]),
        arcsec([450160.398036, -6962890.5431, 7.4722, 0.007702, -0.00005939]),
        4.402608842 + 2608.7903141574 * t,
        3.176146697 + 1021.3285546211 * t,
        1.753470314 + 628.3075849991 * t,
        6.203480913 + 334.0612426700 * t,
        0.599546497 + 52.9690962641 * t,
        0.874016757 + 21.3299104960 * t,
        5.481293872 + 7.4781598567 * t,
        5.311886287 + 3.8133035638 * t,
        (0.024381750 + 0.00000538691 * t) * t,
    ]
}

fn sum(terms: &[Term], args: &[f64; 14], t: f64) -> f64 {
    terms
        .iter()
        .map(|term| {
            let arg: f64 = term
                .args
                .iter()
                .zip(args)
                .map(|(&k, a)| f64::from(k) * a)
                .sum();
            let (s, c) = arg.sin_cos();
            let v = term.sin * s + term.cos * c;
            if term.t { v * t } else { v }
        })
        .sum::<f64>()
        * 1e-6
        * ARCSEC
}

/// Nutation in longitude and in obliquity, radians, at TT Julian date `jd`.
pub fn nutation(jd: f64) -> (f64, f64) {
    let t = centuries(jd);
    let args = arguments(t);
    (sum(&LONGITUDE, &args, t), sum(&OBLIQUITY, &args, t))
}

#[cfg(test)]
// SOFA's test values, quoted as it gives them.
#[allow(clippy::excessive_precision)]
mod tests {
    use super::*;

    #[test]
    fn tables() {
        assert_eq!(LONGITUDE.len(), 1320 + 38);
        assert_eq!(OBLIQUITY.len(), 1037 + 19);
        // The leading terms: the 18.6-year term in Ω.
        assert_eq!(
            (LONGITUDE[0].sin, LONGITUDE[0].cos),
            (-17206424.18, 3338.60)
        );
        assert_eq!((OBLIQUITY[0].sin, OBLIQUITY[0].cos), (1537.70, 9205233.10));
    }

    // SOFA's iauNut06a test value: at 2400000.5 + 53736.0, Δψ = -0.9630912025820308797e-5 rad,
    // Δε = 0.4063238496887249798e-4 rad.
    #[test]
    fn against_sofa() {
        let (dpsi, deps) = nutation(2400000.5 + 53736.0);
        assert!((dpsi - -0.9630912025820308797e-5).abs() < 1e-12, "{dpsi:e}");
        assert!((deps - 0.4063238496887249798e-4).abs() < 1e-12, "{deps:e}");
    }
}
