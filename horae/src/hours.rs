//! The planetary days and hours. The planetary day runs from sunrise to sunrise, so before
//! sunrise it's still the previous day's. Its daylight and its night are each divided into
//! twelve equal hours, ruled in Chaldean order from the day's ruler in the first.

use std::fmt;

use astrometry::horizon::{SUNRISE, altitude, sun_crossings};
use astrometry::source::{self, Source};
use astrometry::time::DAY;
use jiff::civil::Weekday;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

use crate::zodiac::Planet;
use crate::{Error, check_coordinates, time};

/// The planets in Chaldean order, slowest first, which rule the hours in turn.
pub const CHALDEAN: [Planet; 7] = [
    Planet::Saturn,
    Planet::Jupiter,
    Planet::Mars,
    Planet::Sun,
    Planet::Venus,
    Planet::Mercury,
    Planet::Moon,
];

/// The planet of a weekday: the Sun's Sunday, the Moon's Monday, and so on.
pub fn weekday_ruler(w: Weekday) -> Planet {
    [
        Planet::Sun,
        Planet::Moon,
        Planet::Mars,
        Planet::Mercury,
        Planet::Jupiter,
        Planet::Venus,
        Planet::Saturn,
    ][w.to_sunday_zero_offset() as usize]
}

/// The ruler of the planetary hour `n` (0–23, the night's from 12) of a day ruled by `day`.
pub fn hour_ruler(day: Planet, n: usize) -> Planet {
    let first = CHALDEAN
        .iter()
        .position(|p| *p == day)
        .expect("every planet is in the Chaldean order");
    CHALDEAN[(first + n) % 7]
}

/// A planetary hour.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hour {
    pub ruler: Planet,
    /// 1–12, of the day or of the night.
    pub number: u8,
    pub night: bool,
    pub start: Zoned,
    pub end: Zoned,
}

impl Hour {
    /// Whether the hour holds `t`: from its start, until its end.
    pub fn contains(&self, t: Timestamp) -> bool {
        self.start.timestamp() <= t && t < self.end.timestamp()
    }
}

/// Why a place has no planetary hours on a date, as can happen near the poles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Polar {
    NoSunrise,
    NoSunset,
}

impl fmt::Display for Polar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Polar::NoSunrise => "the Sun doesn't rise here on this date",
            Polar::NoSunset => "the Sun doesn't set here on this date",
        })
    }
}

/// A planetary day, from one sunrise to the next, and its 24 hours.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlanetaryDay {
    /// The planet of the weekday of its sunrise.
    pub ruler: Planet,
    /// Where, degrees, north and east positive.
    pub latitude: f64,
    pub longitude: f64,
    /// Sunrise begins it; sunset and the next sunrise end its daylight and its night.
    pub sunrise: Zoned,
    pub sunset: Zoned,
    pub next_sunrise: Zoned,
    /// The twelve hours of the daylight and then the twelve of the night.
    pub hours: Vec<Hour>,
}

impl PlanetaryDay {
    /// The hour holding `t`, if the day does.
    pub fn hour_at(&self, t: Timestamp) -> Option<&Hour> {
        self.hours.iter().find(|h| h.contains(t))
    }

    /// The day after: from the next sunrise.
    pub fn next(&self, src: &dyn Source) -> Result<Day, Error> {
        // A minute into it, clear of the rounding of sunrise to the second.
        let tt = time::tt(self.next_sunrise.timestamp()) + 60.0 / DAY;
        day_at(
            src,
            tt,
            self.next_sunrise.time_zone(),
            self.next_sunrise.weekday(),
            self.latitude,
            self.longitude,
        )
    }
}

/// A day, planetary if the Sun rises and sets.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Day {
    Planetary(PlanetaryDay),
    /// No sunrise or no sunset: no hours, and the day takes the calendar weekday's ruler.
    Polar {
        ruler: Planet,
        polar: Polar,
    },
}

impl Day {
    pub fn ruler(&self) -> Planet {
        match self {
            Day::Planetary(d) => d.ruler,
            Day::Polar { ruler, .. } => *ruler,
        }
    }

    pub fn planetary(&self) -> Option<&PlanetaryDay> {
        match self {
            Day::Planetary(d) => Some(d),
            Day::Polar { .. } => None,
        }
    }

    /// The hour holding `t`, if any.
    pub fn hour_at(&self, t: Timestamp) -> Option<&Hour> {
        self.planetary()?.hour_at(t)
    }
}

/// The planetary day holding `t`, at latitude `lat` and longitude `lon` in degrees (north and
/// east positive). Its times are in `t`'s zone.
pub fn day(src: &dyn Source, t: &Zoned, lat: f64, lon: f64) -> Result<Day, Error> {
    check_coordinates(lat, lon)?;
    day_at(
        src,
        time::tt(t.timestamp()),
        t.time_zone(),
        t.weekday(),
        lat,
        lon,
    )
}

/// The hours of `days` in turn, from the planetary ones.
pub fn hours(days: &[Day]) -> impl Iterator<Item = &Hour> {
    days.iter()
        .filter_map(Day::planetary)
        .flat_map(|d| d.hours.iter())
}

/// The first hour of `days` to begin after `t`.
pub fn next_hour(days: &[Day], t: Timestamp) -> Option<&Hour> {
    hours(days).find(|h| h.start.timestamp() > t)
}

/// The first hour of `days` ruled by `ruler` to begin after `t`: seven hours on, unless a day
/// boundary breaks the sequence, so `days` should run at least to the day after `t`'s.
pub fn next_hour_of(days: &[Day], ruler: Planet, t: Timestamp) -> Option<&Hour> {
    hours(days).find(|h| h.ruler == ruler && h.start.timestamp() > t)
}

/// The day holding TT Julian date `tt`; a polar one takes the ruler of `weekday`.
fn day_at(
    src: &dyn Source,
    tt: f64,
    tz: &TimeZone,
    weekday: Weekday,
    lat: f64,
    lon: f64,
) -> Result<Day, Error> {
    let (phi, east) = (lat.to_radians(), lon.to_radians());
    let events = sun_crossings(src, tt - 1.5, tt + 1.5, phi, east, SUNRISE)?;
    let rise = events
        .iter()
        .rev()
        .find(|e| e.rising && e.tt <= tt)
        .map(|e| e.tt);
    let set = rise.and_then(|r| events.iter().find(|e| !e.rising && e.tt > r).map(|e| e.tt));
    let next = set.and_then(|s| events.iter().find(|e| e.rising && e.tt > s).map(|e| e.tt));
    let (Some(rise), Some(set), Some(next)) = (rise, set, next) else {
        let up = altitude(src, source::Body::Sun, tt, phi, east)? > SUNRISE;
        return Ok(Day::Polar {
            ruler: weekday_ruler(weekday),
            polar: if up {
                Polar::NoSunset
            } else {
                Polar::NoSunrise
            },
        });
    };
    let sunrise = time::zoned(rise, tz);
    let ruler = weekday_ruler(sunrise.weekday());
    // The 25 boundaries of the 24 hours, sunset and the next sunrise exact.
    let (day, night) = ((set - rise) / 12.0, (next - set) / 12.0);
    let bounds: Vec<f64> = (0..12)
        .map(|i| rise + i as f64 * day)
        .chain((0..12).map(|i| set + i as f64 * night))
        .chain([next])
        .collect();
    let hours = (0..24)
        .map(|n| Hour {
            ruler: hour_ruler(ruler, n),
            number: (n % 12) as u8 + 1,
            night: n >= 12,
            start: time::zoned(bounds[n], tz),
            end: time::zoned(bounds[n + 1], tz),
        })
        .collect();
    Ok(Day::Planetary(PlanetaryDay {
        ruler,
        latitude: lat,
        longitude: lon,
        sunrise,
        sunset: time::zoned(set, tz),
        next_sunrise: time::zoned(next, tz),
        hours,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use Planet::*;

    #[test]
    fn weekday_rulers() {
        assert_eq!(weekday_ruler(Weekday::Sunday), Sun);
        assert_eq!(weekday_ruler(Weekday::Thursday), Jupiter);
        assert_eq!(weekday_ruler(Weekday::Saturday), Saturn);
    }

    // The ruler of the first hour of the next day follows on from the 24th: the Chaldean
    // order, stepped 24 times, gives the weekdays' order.
    #[test]
    fn hours_lead_into_the_next_day() {
        let days = [
            Weekday::Sunday,
            Weekday::Monday,
            Weekday::Tuesday,
            Weekday::Wednesday,
            Weekday::Thursday,
            Weekday::Friday,
            Weekday::Saturday,
        ];
        for (i, w) in days.into_iter().enumerate() {
            let ruler = weekday_ruler(w);
            assert_eq!(hour_ruler(ruler, 0), ruler);
            assert_eq!(
                hour_ruler(ruler, 24),
                weekday_ruler(days[(i + 1) % 7]),
                "after {w:?}"
            );
        }
    }
}
