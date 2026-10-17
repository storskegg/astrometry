//! ELP/MPP02 (Chapront and Francou, 2003): the Moon's geocentric position, a port of the
//! authors' Fortran (ELPMPP02.for) with the constants fitted to DE405 (their `icor = 1`). The
//! series are the authors' six files, embedded as distributed.
//!
//! The result is in the inertial mean ecliptic and equinox of J2000, km and km/day.

use std::f64::consts::PI;
use std::sync::LazyLock;

use crate::vector::Vec3;

const RAD: f64 = 648000.0 / PI;
const DEG: f64 = PI / 180.0;

fn dms(d: f64, m: f64, s: f64) -> f64 {
    (d + m / 60.0 + s / 3600.0) * DEG
}

/// The constants and arguments of INITIAL.
struct Constants {
    w: [[f64; 5]; 3],
    del: [[f64; 5]; 4],
    p: [[f64; 5]; 8],
    zeta: [f64; 5],
    delnu: f64,
    dele: f64,
    delg: f64,
    delnp: f64,
    delep: f64,
    dtasm: f64,
    am: f64,
    pq: ([f64; 5], [f64; 5]),
}

fn constants() -> Constants {
    let bp = [
        [
            0.311079095,
            -0.4482398e-2,
            -0.110248500e-2,
            0.1056062e-2,
            0.50928e-4,
        ],
        [
            -0.103837907,
            0.6682870e-3,
            -0.129807200e-2,
            -0.1780280e-3,
            -0.37342e-4,
        ],
    ];
    let dprec = -0.29965; // IAU 2000A
    let am = 0.074801329;
    let alpha = 0.002571881;
    let dtasm = (2.0 * alpha) / (3.0 * am);
    let xa = (2.0 * alpha) / 3.0;
    // The corrections of the fit to DE405.
    let (dw1_0, dw2_0, dw3_0, deart_0, dperi) = (-0.07008, 0.20794, -0.07215, -0.00033, -0.00749);
    let (dw1_1, dgam, de, deart_1, dep) = (-0.35106, 0.00085, -0.00006, 0.00732, 0.00224);
    let (dw2_1, dw3_1, dw1_2) = (0.08017, -0.04317, -0.03743);

    let mut w = [
        [
            dms(218.0, 18.0, 59.95571 + dw1_0),
            (1732559343.73604 + dw1_1) / RAD,
            (-6.8084 + dw1_2) / RAD,
            0.66040e-2 / RAD,
            -0.31690e-4 / RAD,
        ],
        [
            dms(83.0, 21.0, 11.67475 + dw2_0),
            (14643420.3171 + dw2_1) / RAD,
            -38.2631 / RAD,
            -0.45047e-1 / RAD,
            0.21301e-3 / RAD,
        ],
        [
            dms(125.0, 2.0, 40.39816 + dw3_0),
            (-6967919.5383 + dw3_1) / RAD,
            6.3590 / RAD,
            0.76250e-2 / RAD,
            -0.35860e-4 / RAD,
        ],
    ];
    let eart = [
        dms(100.0, 27.0, 59.13885 + deart_0),
        (129597742.29300 + deart_1) / RAD,
        -0.020200 / RAD,
        0.90000e-5 / RAD,
        0.15000e-6 / RAD,
    ];
    let peri = [
        dms(102.0, 56.0, 14.45766 + dperi),
        1161.24342 / RAD,
        0.529265 / RAD,
        -0.11814e-3 / RAD,
        0.11379e-4 / RAD,
    ];
    // icor = 1.
    w[0][3] -= 0.00018865 / RAD;
    w[0][4] -= 0.00001024 / RAD;
    w[1][2] += 0.00470602 / RAD;
    w[1][3] -= 0.00025213 / RAD;
    w[2][2] -= 0.00261070 / RAD;
    w[2][3] -= 0.00010712 / RAD;

    let x2 = w[1][1] / w[0][1];
    let x3 = w[2][1] / w[0][1];
    let y2 = am * bp[0][0] + xa * bp[0][4];
    let y3 = am * bp[1][0] + xa * bp[1][4];
    let (d21, d22, d23, d24, d25) = (
        x2 - y2,
        w[0][1] * bp[0][1],
        w[0][1] * bp[0][2],
        w[0][1] * bp[0][3],
        y2 / am,
    );
    let (d31, d32, d33, d34, d35) = (
        x3 - y3,
        w[0][1] * bp[1][1],
        w[0][1] * bp[1][2],
        w[0][1] * bp[1][3],
        y3 / am,
    );
    let cw2_1 = d21 * dw1_1 + d25 * deart_1 + d22 * dgam + d23 * de + d24 * dep;
    let cw3_1 = d31 * dw1_1 + d35 * deart_1 + d32 * dgam + d33 * de + d34 * dep;
    w[1][1] += cw2_1 / RAD;
    w[2][1] += cw3_1 / RAD;

    let mut del = [[0.0; 5]; 4];
    for i in 0..5 {
        del[0][i] = w[0][i] - eart[i]; // D
        del[1][i] = w[0][i] - w[2][i]; // F
        del[2][i] = w[0][i] - w[1][i]; // l
        del[3][i] = eart[i] - peri[i]; // l'
    }
    del[0][0] += PI;

    let mut p = [[0.0; 5]; 8];
    let p0 = [
        dms(252.0, 15.0, 3.216919),
        dms(181.0, 58.0, 44.758419),
        dms(100.0, 27.0, 59.138850),
        dms(355.0, 26.0, 3.642778),
        dms(34.0, 21.0, 5.379392),
        dms(50.0, 4.0, 38.902495),
        dms(314.0, 3.0, 4.354234),
        dms(304.0, 20.0, 56.808371),
    ];
    let p1 = [
        538101628.66888,
        210664136.45777,
        129597742.29300,
        68905077.65936,
        10925660.57335,
        4399609.33632,
        1542482.57845,
        786547.89700,
    ];
    for i in 0..8 {
        p[i][0] = p0[i];
        p[i][1] = p1[i] / RAD;
    }
    let zeta = [
        w[0][0],
        w[0][1] + (5029.0966 + dprec) / RAD,
        w[0][2],
        w[0][3],
        w[0][4],
    ];

    Constants {
        delnu: (0.55604 + dw1_1) / RAD / w[0][1],
        dele: (0.01789 + de) / RAD,
        delg: (-0.08066 + dgam) / RAD,
        delnp: (-0.06424 + deart_1) / RAD / w[0][1],
        delep: (-0.12879 + dep) / RAD,
        w,
        del,
        p,
        zeta,
        dtasm,
        am,
        pq: (
            [
                0.10180391e-04,
                0.47020439e-06,
                -0.5417367e-09,
                -0.2507948e-11,
                0.463486e-14,
            ],
            [
                -0.113469002e-03,
                0.12372674e-06,
                0.1265417e-08,
                -0.1371808e-11,
                -0.320334e-14,
            ],
        ),
    }
}

/// A term: amplitude, then the argument's polynomial in t (centuries), and its power of t.
struct Term {
    amp: f64,
    arg: [f64; 5],
    power: i32,
}

struct Elp {
    c: Constants,
    /// By coordinate: longitude, latitude, distance.
    terms: [Vec<Term>; 3],
}

/// A fixed-column field of a Fortran record, as a number (D exponents read as E).
fn col(line: &str, from: usize, width: usize) -> f64 {
    let s = line
        .get(from..(from + width).min(line.len()))
        .unwrap_or("")
        .trim()
        .replace(['D', 'd'], "E");
    if s.is_empty() {
        0.0
    } else {
        s.parse()
            .unwrap_or_else(|_| panic!("bad field {s:?} in {line:?}"))
    }
}

fn load() -> Elp {
    let c = constants();
    let main = [
        include_str!("../data/elpmpp02/ELP_MAIN.S1"),
        include_str!("../data/elpmpp02/ELP_MAIN.S2"),
        include_str!("../data/elpmpp02/ELP_MAIN.S3"),
    ];
    let pert = [
        include_str!("../data/elpmpp02/ELP_PERT.S1"),
        include_str!("../data/elpmpp02/ELP_PERT.S2"),
        include_str!("../data/elpmpp02/ELP_PERT.S3"),
    ];
    let mut terms: [Vec<Term>; 3] = Default::default();
    for (iv, text) in main.iter().enumerate() {
        // format (4i3, 2x, f13.5, 5f12.2), after a header.
        for line in text.lines().skip(1).filter(|l| !l.trim().is_empty()) {
            let ilu = [0, 3, 6, 9].map(|o| col(line, o, 3));
            let mut a = col(line, 14, 13);
            let b: [f64; 5] = std::array::from_fn(|k| col(line, 27 + 12 * k, 12));
            let tgv = b[0] + c.dtasm * b[4];
            if iv == 2 {
                a -= 2.0 * a * c.delnu / 3.0;
            }
            let amp = a
                + tgv * (c.delnp - c.am * c.delnu)
                + b[1] * c.delg
                + b[2] * c.dele
                + b[3] * c.delep;
            let mut arg = [0.0; 5];
            for (k, x) in arg.iter_mut().enumerate() {
                *x = (0..4).map(|i| ilu[i] * c.del[i][k]).sum();
            }
            if iv == 2 {
                arg[0] += PI / 2.0;
            }
            terms[iv].push(Term { amp, arg, power: 0 });
        }
    }
    for (iv, text) in pert.iter().enumerate() {
        let mut power = 0;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            if line.starts_with("PERTURBATIONS") {
                // format (25x, 2i10): the count, then the power of t.
                power = col(line, 35, 10) as i32;
                continue;
            }
            // format (i5, 2d20.13, 16i3)
            let s = col(line, 5, 20);
            let cc = col(line, 25, 20);
            let ifi: [f64; 16] = std::array::from_fn(|k| col(line, 45 + 3 * k, 3));
            let amp = (cc * cc + s * s).sqrt();
            let mut pha = cc.atan2(s);
            if pha < 0.0 {
                pha += 2.0 * PI;
            }
            let mut arg = [0.0; 5];
            for (k, x) in arg.iter_mut().enumerate() {
                let mut v = if k == 0 { pha } else { 0.0 };
                v += (0..4).map(|i| ifi[i] * c.del[i][k]).sum::<f64>();
                v += (4..12).map(|i| ifi[i] * c.p[i - 4][k]).sum::<f64>();
                v += ifi[12] * c.zeta[k];
                *x = v;
            }
            terms[iv].push(Term { amp, arg, power });
        }
    }
    Elp { c, terms }
}

static ELP: LazyLock<Elp> = LazyLock::new(load);

/// The Moon's geocentric position (km) and velocity (km/day) at `tj` days of TDB from J2000,
/// in the inertial mean ecliptic and equinox of J2000: EVALUATE.
pub fn moon(tj: f64) -> (Vec3, Vec3) {
    const A405: f64 = 384747.9613701725;
    const AELP: f64 = 384747.980674318;
    const SC: f64 = 36525.0;
    let e = &*ELP;
    let t1 = tj / SC;
    let t = [1.0, t1, t1 * t1, t1 * t1 * t1, t1 * t1 * t1 * t1];
    let mut v = [0.0; 6];
    for iv in 0..3 {
        for term in &e.terms[iv] {
            let mut y = term.arg[0];
            let mut yp = 0.0;
            for k in 1..5 {
                y += term.arg[k] * t[k];
                yp += k as f64 * term.arg[k] * t[k - 1];
            }
            let it = term.power as usize;
            let xp = if it != 0 {
                it as f64 * term.amp * t[it - 1]
            } else {
                0.0
            };
            let (sy, cy) = y.sin_cos();
            v[iv] += term.amp * t[it] * sy;
            v[iv + 3] += xp * sy + term.amp * t[it] * yp * cy;
        }
    }
    let w = &e.c.w;
    v[0] = v[0] / RAD + w[0][0] + w[0][1] * t[1] + w[0][2] * t[2] + w[0][3] * t[3] + w[0][4] * t[4];
    v[1] /= RAD;
    v[2] *= A405 / AELP;
    v[3] =
        v[3] / RAD + w[0][1] + 2.0 * w[0][2] * t[1] + 3.0 * w[0][3] * t[2] + 4.0 * w[0][4] * t[3];
    v[4] /= RAD;

    let (slamb, clamb) = v[0].sin_cos();
    let (sbeta, cbeta) = v[1].sin_cos();
    let cw = v[2] * cbeta;
    let sw = v[2] * sbeta;
    let (x1, x2, x3) = (cw * clamb, cw * slamb, sw);
    let xp1 = (v[5] * cbeta - v[4] * sw) * clamb - v[3] * x2;
    let xp2 = (v[5] * cbeta - v[4] * sw) * slamb + v[3] * x1;
    let xp3 = v[5] * sbeta + v[4] * cw;

    let (p, q) = &e.c.pq;
    let pw = (p[0] + p[1] * t[1] + p[2] * t[2] + p[3] * t[3] + p[4] * t[4]) * t[1];
    let qw = (q[0] + q[1] * t[1] + q[2] * t[2] + q[3] * t[3] + q[4] * t[4]) * t[1];
    let ra = 2.0 * (1.0 - pw * pw - qw * qw).sqrt();
    let pwqw = 2.0 * pw * qw;
    let pw2 = 1.0 - 2.0 * pw * pw;
    let qw2 = 1.0 - 2.0 * qw * qw;
    let pwra = pw * ra;
    let qwra = qw * ra;
    let pos = [
        pw2 * x1 + pwqw * x2 + pwra * x3,
        pwqw * x1 + qw2 * x2 - qwra * x3,
        -pwra * x1 + qwra * x2 + (pw2 + qw2 - 1.0) * x3,
    ];
    let ppw =
        p[0] + (2.0 * p[1] + 3.0 * p[2] * t[1] + 4.0 * p[3] * t[2] + 5.0 * p[4] * t[3]) * t[1];
    let qpw =
        q[0] + (2.0 * q[1] + 3.0 * q[2] * t[1] + 4.0 * q[3] * t[2] + 5.0 * q[4] * t[3]) * t[1];
    let ppw2 = -4.0 * pw * ppw;
    let qpw2 = -4.0 * qw * qpw;
    let ppwqpw = 2.0 * (ppw * qw + pw * qpw);
    let rap = (ppw2 + qpw2) / ra;
    let ppwra = ppw * ra + pw * rap;
    let qpwra = qpw * ra + qw * rap;
    let vel = [
        (pw2 * xp1 + pwqw * xp2 + pwra * xp3 + ppw2 * x1 + ppwqpw * x2 + ppwra * x3) / SC,
        (pwqw * xp1 + qw2 * xp2 - qwra * xp3 + ppwqpw * x1 + qpw2 * x2 - qpwra * x3) / SC,
        (-pwra * xp1 + qwra * xp2 + (pw2 + qw2 - 1.0) * xp3 - ppwra * x1
            + qpwra * x2
            + (ppw2 + qpw2) * x3)
            / SC,
    ];
    (pos, vel)
}
