//! Sources of positions: barycentric states of the Sun, the Moon, the Earth and the planets.

use std::path::Path;

use crate::spk::{self, Spk};
use crate::vector::{Vec3, add, scale};

/// A body an ephemeris gives. Mars to Saturn are their systems' barycentres, as the JPL DE
/// ephemerides give them and the Swiss Ephemeris uses them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Body {
    Sun,
    Moon,
    Mercury,
    Venus,
    Earth,
    Mars,
    Jupiter,
    Saturn,
}

/// The astronomical unit, km.
pub const AU: f64 = 149_597_870.7;

/// Barycentric positions (au) and velocities (au/day) in ICRF axes, at a TDB Julian date.
pub trait Source: Send + Sync {
    fn state(&self, body: Body, tdb: f64) -> Result<(Vec3, Vec3), spk::Error>;
}

/// A JPL DE ephemeris, from an SPK file such as de440.bsp.
#[derive(Debug)]
pub struct Jpl {
    spk: Spk,
}

impl Jpl {
    pub fn open(path: impl AsRef<Path>) -> Result<Jpl, spk::Error> {
        Ok(Jpl {
            spk: Spk::open(path)?,
        })
    }

    /// The span the file covers, as TDB Julian dates.
    pub fn span(&self) -> Option<(f64, f64)> {
        self.spk.span(301, 3)
    }

    /// A segment's state in au and au/day.
    fn part(&self, target: i32, center: i32, tdb: f64) -> Result<(Vec3, Vec3), spk::Error> {
        let (p, v) = self.spk.state(target, center, tdb)?;
        Ok((scale(1.0 / AU, p), scale(86400.0 / AU, v)))
    }
}

impl Source for Jpl {
    fn state(&self, body: Body, tdb: f64) -> Result<(Vec3, Vec3), spk::Error> {
        let chain: &[(i32, i32)] = match body {
            Body::Sun => &[(10, 0)],
            Body::Mercury => &[(1, 0), (199, 1)],
            Body::Venus => &[(2, 0), (299, 2)],
            Body::Earth => &[(3, 0), (399, 3)],
            Body::Moon => &[(3, 0), (301, 3)],
            Body::Mars => &[(4, 0)],
            Body::Jupiter => &[(5, 0)],
            Body::Saturn => &[(6, 0)],
        };
        let mut state = ([0.0; 3], [0.0; 3]);
        for &(target, center) in chain {
            let (p, v) = self.part(target, center, tdb)?;
            state = (add(state.0, p), add(state.1, v));
        }
        Ok(state)
    }
}
