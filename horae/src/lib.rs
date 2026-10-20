//! Traditional astrology of a moment at a place, on the `astrometry` ephemeris: the seven
//! planets and the lunar nodes in the tropical zodiac, their dignities, their closeness to the
//! Sun and their aspects, the Moon's phase, lunations and course, and the planetary days and
//! hours.
//!
//! Positions are apparent geocentric ecliptic longitudes of date, from JPL's DE440, built in
//! for 1800–2200. The nodes are the true (osculating) nodes.
//!
//! Each part can be had alone, at the cost of only what it needs ([`Position::at`],
//! [`day`], [`moon::appearance`], [`moon::lunations`], [`moon::course`],
//! [`stations::next_station`]), or all together in a [`Sky`].
//!
//! Begun as the astrology of `geomancer`; Apache-2.0, beside astrometry in its repository. The
//! stations, the angles and the Moon's light are astrometry's, named and zoned here.

/// Implements `Display` and `FromStr` for an enum from its labels, with `ALL`, `index`,
/// `from_index` and `name`, and with the `serde` feature, serde as its name. Parsing ignores
/// case and spacing.
macro_rules! named_enum {
    ($(#[$meta:meta])* $name:ident, $kind:literal, [$($(#[$vmeta:meta])* $variant:ident => $label:literal),+ $(,)?]) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $($(#[$vmeta])* $variant),+
        }

        impl $name {
            /// Every member, in order.
            pub const ALL: [$name; [$($label),+].len()] = [$($name::$variant),+];

            const LABELS: [&'static str; [$($label),+].len()] = [$($label),+];

            /// The member at `index` in [`Self::ALL`], if there is one.
            pub fn from_index(index: usize) -> Option<Self> {
                Self::ALL.get(index).copied()
            }

            /// The position in [`Self::ALL`].
            pub fn index(self) -> usize {
                self as usize
            }

            /// The name, such as "North Node".
            pub fn name(self) -> &'static str {
                Self::LABELS[self as usize]
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.pad(self.name())
            }
        }

        /// Parses a name, ignoring case and spacing.
        impl std::str::FromStr for $name {
            type Err = $crate::UnknownName;

            fn from_str(s: &str) -> Result<Self, $crate::UnknownName> {
                $crate::names::index(&Self::LABELS, s)
                    .and_then(Self::from_index)
                    .ok_or_else(|| $crate::UnknownName { kind: $kind, input: s.to_string() })
            }
        }

        #[cfg(feature = "serde")]
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.name())
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = <String as serde::Deserialize>::deserialize(d)?;
                s.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

pub mod angles;
pub mod aspect;
pub mod conditions;
pub mod hours;
pub mod moon;
mod names;
pub mod position;
mod sky;
pub mod stations;
pub mod time;
pub mod zodiac;

use std::fmt;

pub use angles::{Angles, angles};
pub use aspect::{Aspect, Kind, aspects};
pub use astrometry::source::{Builtin, Source};
pub use conditions::{Dignity, Solar, dignities, solar};
pub use hours::{Day, Hour, PlanetaryDay, Polar, day};
pub use moon::Phase;
pub use position::Position;
pub use sky::{Body, Moon, Sky};
pub use stations::{Direction, Station};
pub use zodiac::{Element, Planet, Point, Sign};

/// The ephemeris's error: a date outside a JPL file's span, or a file that can't be read.
/// The built-in ephemeris doesn't fail.
pub use astrometry::spk::Error as EphemerisError;

/// What can go wrong.
#[derive(Debug)]
pub enum Error {
    /// Coordinates outside the Earth: latitude and longitude, degrees.
    InvalidCoordinates(f64, f64),
    Ephemeris(EphemerisError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidCoordinates(lat, lon) => {
                write!(f, "horae: invalid coordinates {lat}, {lon}")
            }
            Error::Ephemeris(e) => write!(f, "horae: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Ephemeris(e) => Some(e),
            Error::InvalidCoordinates(..) => None,
        }
    }
}

impl From<EphemerisError> for Error {
    fn from(e: EphemerisError) -> Error {
        Error::Ephemeris(e)
    }
}

/// Checks latitude `lat` and longitude `lon`, degrees, north and east positive.
pub(crate) fn check_coordinates(lat: f64, lon: f64) -> Result<(), Error> {
    if lat.abs() > 90.0 || lon.abs() > 180.0 || lat.is_nan() || lon.is_nan() {
        return Err(Error::InvalidCoordinates(lat, lon));
    }
    Ok(())
}

/// A name that doesn't match any member of an enum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownName {
    kind: &'static str,
    input: String,
}

impl fmt::Display for UnknownName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "horae: unknown {} {:?}", self.kind, self.input)
    }
}

impl std::error::Error for UnknownName {}
