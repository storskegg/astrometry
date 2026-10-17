//! VSOP87E (Bretagnon and Francou, 1988): barycentric rectangular coordinates of the Sun and
//! planets in the dynamical ecliptic and equinox of J2000, as series of
//! T^α · A cos(B + C·T), T in Julian millennia of TDB from J2000. The series are the authors'
//! files, embedded as IMCCE distributes them.

use std::sync::LazyLock;

use crate::vector::Vec3;

/// One body's series: by coordinate (x, y, z), by power of T, the terms (A, B, C).
#[derive(Debug)]
pub struct Series([Vec<Vec<(f64, f64, f64)>>; 3]);

impl Series {
    pub fn parse(text: &str) -> Series {
        let mut s: [Vec<Vec<(f64, f64, f64)>>; 3] = Default::default();
        let (mut coord, mut power) = (0usize, 0usize);
        for line in text.lines() {
            if line.trim_start().starts_with("VSOP87") {
                let f: Vec<&str> = line.split_whitespace().collect();
                let at = |key: &str| f.iter().position(|w| *w == key).map(|i| f[i + 1]);
                coord = at("VARIABLE")
                    .and_then(|v| v.parse::<usize>().ok())
                    .expect("VARIABLE")
                    - 1;
                power = f
                    .iter()
                    .find_map(|w| w.strip_prefix("*T**"))
                    .and_then(|p| p.parse().ok())
                    .expect("*T**");
                while s[coord].len() <= power {
                    s[coord].push(Vec::new());
                }
                continue;
            }
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 3 {
                continue;
            }
            let n = f.len();
            let num = |i: usize| f[i].parse::<f64>().expect("a number");
            s[coord][power].push((num(n - 3), num(n - 2), num(n - 1)));
        }
        Series(s)
    }

    /// Position (au) and velocity (au per Julian millennium) at `t` Julian millennia of TDB
    /// from J2000.
    pub fn at(&self, t: f64) -> (Vec3, Vec3) {
        let mut pos = [0.0; 3];
        let mut vel = [0.0; 3];
        for (c, powers) in self.0.iter().enumerate() {
            for (alpha, terms) in powers.iter().enumerate() {
                let (mut sum, mut dsum) = (0.0, 0.0);
                for &(a, b, freq) in terms {
                    let (s, cos) = (b + freq * t).sin_cos();
                    sum += a * cos;
                    dsum -= a * freq * s;
                }
                let tp = t.powi(alpha as i32);
                pos[c] += tp * sum;
                vel[c] += tp * dsum;
                if alpha > 0 {
                    vel[c] += alpha as f64 * t.powi(alpha as i32 - 1) * sum;
                }
            }
        }
        (pos, vel)
    }
}

macro_rules! series {
    ($name:ident, $file:literal) => {
        pub static $name: LazyLock<Series> = LazyLock::new(|| {
            Series::parse(include_str!(concat!("../data/vsop87/VSOP87E.", $file)))
        });
    };
}

series!(SUN, "sun");
series!(MERCURY, "mer");
series!(VENUS, "ven");
series!(EARTH, "ear");
series!(MARS, "mar");
series!(JUPITER, "jup");
series!(SATURN, "sat");
