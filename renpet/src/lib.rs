//! The ancient Egyptian calendar and hours, carried forward unbroken, on the `astrometry`
//! ephemeris.
//!
//! [`civil`] is the 365-day civil calendar (no leap days, so it drifts through the seasons
//! and returns every 1,461 years: the Sothic cycle), by pure arithmetic on Julian Day Numbers.
//! [`hours`] is the day of twelve seasonal hours of daylight and twelve of night, from the
//! Sun's rising and setting at a place.

use std::fmt;

/// Defines a fieldless enum with `ALL`, `id` and `translit`, and with the `serde` feature,
/// serde as its variant names.
macro_rules! ids_enum {
    ($(#[$meta:meta])* $name:ident, [$($(#[$vmeta:meta])* $variant:ident => ($id:literal, $translit:literal)),+ $(,)?]) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub enum $name {
            $($(#[$vmeta])* $variant),+
        }

        impl $name {
            /// Every member, in order.
            pub const ALL: [$name; [$($id),+].len()] = [$($name::$variant),+];

            /// A stable snake_case identifier, such as `season.akhet`.
            pub fn id(&self) -> &'static str {
                match self {
                    $($name::$variant => $id),+
                }
            }

            /// The Egyptological transliteration.
            pub fn translit(&self) -> &'static str {
                match self {
                    $($name::$variant => $translit),+
                }
            }
        }
    };
}

pub mod civil;
pub mod hours;
mod time;

pub use civil::{CivilDate, CivilDay, Epagomenal, Month, Season, SothicYear, civil_date_of, next};

pub use astrometry::source::{Builtin, Source};

/// What can go wrong.
#[derive(Debug)]
pub enum Error {
    /// A day of a month outside 1–30 (month, day).
    InvalidDay(Month, u8),
    /// A day of the year outside 1–365.
    InvalidDayOfYear(u16),
    /// A year of the Sothic cycle outside 1–1461.
    InvalidSothicYear(u16),
    /// A date beyond what a Gregorian date can hold (years -9999 to 9999), or a moment beyond
    /// what a timestamp can.
    OutOfRange,
    /// Coordinates outside the Earth: latitude and longitude, degrees.
    InvalidCoordinates(f64, f64),
    Ephemeris(astrometry::spk::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidDay(m, d) => {
                write!(f, "renpet: day {d} is not a day of {} (1-30)", m.id())
            }
            Error::InvalidDayOfYear(d) => write!(f, "renpet: day {d} of the year is not 1-365"),
            Error::InvalidSothicYear(y) => {
                write!(f, "renpet: year {y} of the Sothic cycle is not 1-1461")
            }
            Error::OutOfRange => f.write_str("renpet: beyond the dates that can be held"),
            Error::InvalidCoordinates(lat, lon) => {
                write!(f, "renpet: invalid coordinates {lat}, {lon}")
            }
            Error::Ephemeris(e) => write!(f, "renpet: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Ephemeris(e) => Some(e),
            _ => None,
        }
    }
}

impl From<astrometry::spk::Error> for Error {
    fn from(e: astrometry::spk::Error) -> Self {
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
