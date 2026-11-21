//! The seasonal hours: the daylight, from sunrise to sunset, and the night, from sunset to the
//! next sunrise, are each divided into twelve equal hours, which are longer by day in summer
//! and by night in winter. The Egyptian day begins at sunrise.

use astrometry::horizon::{SUNRISE, altitude, sun_crossings};
use astrometry::source::{self, Source};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

use crate::civil::{CivilDate, civil_date_of};
use crate::{Error, check_coordinates, time};

ids_enum!(
    /// The half of the day: its daylight or its night.
    Half,
    [
        /// From sunrise to sunset.
        Day => ("half.day", "hrw"),
        /// From sunset to the next sunrise.
        Night => ("half.night", "grḥ"),
    ]
);

impl Half {
    fn tag(self) -> &'static str {
        match self {
            Half::Day => "day",
            Half::Night => "night",
        }
    }
}

/// A seasonal hour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hour {
    pub half: Half,
    /// 1-12, of the daylight or of the night.
    pub n: u8,
    pub start: Timestamp,
    pub end: Timestamp,
}

impl Hour {
    /// Whether the hour holds `t`: from its start, until its end.
    pub fn contains(&self, t: Timestamp) -> bool {
        self.start <= t && t < self.end
    }

    /// A stable identifier, such as `hour.day.01` or `hour.night.12`.
    pub fn id(&self) -> String {
        format!("hour.{}.{:02}", self.half.tag(), self.n)
    }
}

/// Why a place has no hours on a day, as can happen near the poles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Polar {
    /// The Sun stays up.
    Day,
    /// The Sun stays down.
    Night,
}

/// An Egyptian day, from one sunrise to the next, and its 24 hours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EgyptianDay {
    pub sunrise: Timestamp,
    pub sunset: Timestamp,
    pub next_sunrise: Timestamp,
    /// The twelve hours of the daylight and then the twelve of the night.
    pub hours: [Hour; 24],
}

impl EgyptianDay {
    /// The hour holding `t`, if the day does.
    pub fn hour_at(&self, t: Timestamp) -> Option<&Hour> {
        self.hours.iter().find(|h| h.contains(t))
    }

    /// The end of the hour holding `t`, when the next begins: `None` before sunrise and from
    /// the next sunrise on.
    pub fn next_change(&self, t: Timestamp) -> Option<Timestamp> {
        self.hour_at(t).map(|h| h.end)
    }
}

/// A day, in hours if the Sun rises and sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
// Plain data for the app's bindings, so the large variant isn't boxed.
#[allow(clippy::large_enum_variant)]
pub enum Day {
    Hours(EgyptianDay),
    Polar(Polar),
}

/// The Egyptian day holding `t`, from the sunrise at or before `t` to the next, at latitude
/// `lat` and longitude `lon` in degrees (north and east positive).
pub fn day(src: &dyn Source, t: &Zoned, lat: f64, lon: f64) -> Result<Day, Error> {
    check_coordinates(lat, lon)?;
    let tt = time::tt(t.timestamp());
    let (phi, east) = (lat.to_radians(), lon.to_radians());
    let events = sun_crossings(src, tt - 1.5, tt + 1.5, phi, east, SUNRISE)?;
    // Compared as the whole seconds the hours are given in, so that `t` equal to the sunrise
    // returned is in its day, as `civil_date_at` has it.
    let now = t.timestamp();
    let rise = events
        .iter()
        .rev()
        .find(|e| e.rising && time::timestamp(e.tt).is_ok_and(|r| r <= now))
        .map(|e| e.tt);
    let set = rise.and_then(|r| events.iter().find(|e| !e.rising && e.tt > r).map(|e| e.tt));
    let next = set.and_then(|s| events.iter().find(|e| e.rising && e.tt > s).map(|e| e.tt));
    let (Some(rise), Some(set), Some(next)) = (rise, set, next) else {
        let up = altitude(src, source::Body::Sun, tt, phi, east)? > SUNRISE;
        return Ok(Day::Polar(if up { Polar::Day } else { Polar::Night }));
    };
    // The 25 boundaries of the 24 hours, sunset and the next sunrise exact.
    let (by_day, by_night) = ((set - rise) / 12.0, (next - set) / 12.0);
    let mut bounds = [Timestamp::UNIX_EPOCH; 25];
    for (i, b) in bounds.iter_mut().enumerate() {
        let tt = match i {
            0..12 => rise + i as f64 * by_day,
            12..24 => set + (i - 12) as f64 * by_night,
            _ => next,
        };
        *b = time::timestamp(tt)?;
    }
    let hours = std::array::from_fn(|n| Hour {
        half: if n < 12 { Half::Day } else { Half::Night },
        n: (n % 12) as u8 + 1,
        start: bounds[n],
        end: bounds[n + 1],
    });
    Ok(Day::Hours(EgyptianDay {
        sunrise: bounds[0],
        sunset: bounds[12],
        next_sunrise: bounds[24],
        hours,
    }))
}

/// The Egyptian date at `t` at latitude `lat` and longitude `lon` (degrees, north and east
/// positive): that of the Gregorian date of `t` in its own time zone, unless `t` is before
/// that date's sunrise there, and then the day before's. Where the Sun doesn't rise that day,
/// the day begins at 06:00 local mean solar time, 06:00 UT less `lon`/15 hours.
pub fn civil_date_at(src: &dyn Source, t: &Zoned, lat: f64, lon: f64) -> Result<CivilDate, Error> {
    let date = t.date();
    let today = civil_date_of(date);
    let start = civil_day_start(src, &today, t.time_zone(), lat, lon)?;
    if t.timestamp() >= start.timestamp() {
        Ok(today)
    } else {
        let yesterday = date.yesterday().map_err(|_| Error::OutOfRange)?;
        Ok(civil_date_of(yesterday))
    }
}

/// The sunrise that begins the Egyptian day `d`, on the morning of its Gregorian date in
/// `tz`, at latitude `lat` and longitude `lon` (degrees, north and east positive). Where the
/// Sun doesn't rise that day: 06:00 local mean solar time, 06:00 UT less `lon`/15 hours.
pub fn civil_day_start(
    src: &dyn Source,
    d: &CivilDate,
    tz: &TimeZone,
    lat: f64,
    lon: f64,
) -> Result<Zoned, Error> {
    check_coordinates(lat, lon)?;
    let date = d.gregorian();
    let begin = date.to_zoned(tz.clone()).map_err(|_| Error::OutOfRange)?;
    let end = date
        .tomorrow()
        .and_then(|n| n.to_zoned(tz.clone()))
        .map_err(|_| Error::OutOfRange)?;
    let (from, to) = (begin.timestamp(), end.timestamp());
    let events = sun_crossings(
        src,
        time::tt(from) - 0.1,
        time::tt(to) + 0.1,
        lat.to_radians(),
        lon.to_radians(),
        SUNRISE,
    )?;
    for e in events.iter().filter(|e| e.rising) {
        let at = time::timestamp(e.tt)?;
        if from <= at && at < to {
            return Ok(at.to_zoned(tz.clone()));
        }
    }
    let midnight_ut = date
        .at(0, 0, 0, 0)
        .to_zoned(TimeZone::UTC)
        .map_err(|_| Error::OutOfRange)?
        .timestamp()
        .as_second();
    let secs = midnight_ut + 6 * 3600 - (lon / 15.0 * 3600.0).round() as i64;
    let at = Timestamp::from_second(secs).map_err(|_| Error::OutOfRange)?;
    Ok(at.to_zoned(tz.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrometry::source::Builtin;
    use jiff::SignedDuration;

    const LUXOR: (f64, f64) = (25.6995, 32.6391);

    fn at(s: &str) -> Zoned {
        s.parse().unwrap()
    }

    fn hours_of(d: Day) -> EgyptianDay {
        match d {
            Day::Hours(d) => d,
            Day::Polar(p) => panic!("polar: {p:?}"),
        }
    }

    #[test]
    fn the_hours_tile_the_day() {
        for s in [
            "2026-03-20T12:00[Africa/Cairo]",
            "2026-06-21T12:00[Africa/Cairo]",
            "2026-12-21T12:00[Africa/Cairo]",
        ] {
            let d = hours_of(day(&Builtin, &at(s), LUXOR.0, LUXOR.1).unwrap());
            assert_eq!(d.hours[0].start, d.sunrise);
            assert_eq!(d.hours[11].end, d.sunset);
            assert_eq!(d.hours[12].start, d.sunset);
            assert_eq!(d.hours[23].end, d.next_sunrise);
            for w in d.hours.windows(2) {
                assert_eq!(w[0].end, w[1].start);
            }
            let by_day: i64 = d.hours[..12]
                .iter()
                .map(|h| h.end.as_second() - h.start.as_second())
                .sum();
            assert_eq!(by_day, d.sunset.as_second() - d.sunrise.as_second());
            let by_night: i64 = d.hours[12..]
                .iter()
                .map(|h| h.end.as_second() - h.start.as_second())
                .sum();
            assert_eq!(by_night, d.next_sunrise.as_second() - d.sunset.as_second());
            // Daylight and night, of 24 hours together.
            assert_eq!(
                by_day + by_night,
                d.next_sunrise.as_second() - d.sunrise.as_second()
            );
            assert_eq!(d.hours[0].id(), "hour.day.01");
            assert_eq!(d.hours[23].id(), "hour.night.12");
        }
    }

    // The summer's hours of daylight are longer than the winter's.
    #[test]
    fn hours_are_seasonal() {
        let hour = |s: &str| {
            let d = hours_of(day(&Builtin, &at(s), LUXOR.0, LUXOR.1).unwrap());
            d.hours[0].end.as_second() - d.hours[0].start.as_second()
        };
        let (summer, winter) = (
            hour("2026-06-21T12:00[Africa/Cairo]"),
            hour("2026-12-21T12:00[Africa/Cairo]"),
        );
        assert!(summer > winter + 300, "{summer} {winter}");
    }

    // The planetary day of horae begins at the same sunrise.
    #[test]
    fn sunrise_is_horaes() {
        for s in [
            "2026-03-20T12:00[Africa/Cairo]",
            "2026-11-21T03:00[Africa/Cairo]",
            "2026-11-21T23:00[Africa/Cairo]",
            "2027-07-01T12:00[Africa/Cairo]",
        ] {
            let t = at(s);
            let mine = hours_of(day(&Builtin, &t, LUXOR.0, LUXOR.1).unwrap());
            let theirs = horae::day(&Builtin, &t, LUXOR.0, LUXOR.1).unwrap();
            let theirs = theirs.planetary().unwrap();
            assert_eq!(mine.sunrise, theirs.sunrise.timestamp(), "{s}");
            assert_eq!(mine.sunset, theirs.sunset.timestamp(), "{s}");
            assert_eq!(mine.next_sunrise, theirs.next_sunrise.timestamp(), "{s}");
            assert!(mine.hour_at(t.timestamp()).is_some());
            assert_eq!(
                mine.hour_at(t.timestamp()).map(|h| h.end),
                mine.next_change(t.timestamp())
            );
        }
    }

    #[test]
    fn before_sunrise_is_the_day_before() {
        let t = at("2026-11-21T03:00[Africa/Cairo]");
        let d = hours_of(day(&Builtin, &t, LUXOR.0, LUXOR.1).unwrap());
        // In the previous day's night.
        assert!(d.sunset < t.timestamp() && t.timestamp() < d.next_sunrise);
        assert_eq!(d.hour_at(t.timestamp()).unwrap().half, Half::Night);
    }

    #[test]
    fn the_poles() {
        let june = at("2026-06-21T12:00[UTC]");
        let december = at("2026-12-21T12:00[UTC]");
        assert_eq!(
            day(&Builtin, &june, 80.0, 0.0).unwrap(),
            Day::Polar(Polar::Day)
        );
        assert_eq!(
            day(&Builtin, &december, 80.0, 0.0).unwrap(),
            Day::Polar(Polar::Night)
        );
        assert!(day(&Builtin, &june, 91.0, 0.0).is_err());
        assert!(day(&Builtin, &june, 0.0, 181.0).is_err());
    }

    #[test]
    fn polar_days_begin_at_six_local_mean_solar_time() {
        let t = at("2026-06-21T12:00[UTC]");
        let d = civil_date_of(t.date());
        let z = civil_day_start(&Builtin, &d, &TimeZone::UTC, 80.0, 30.0).unwrap();
        // 06:00 UT less two hours.
        assert_eq!(z.timestamp(), at("2026-06-21T04:00[UTC]").timestamp());
        // Before it, the previous date's.
        let before = at("2026-06-21T03:59[UTC]");
        assert_eq!(
            civil_date_at(&Builtin, &before, 80.0, 30.0).unwrap(),
            civil_date_of(t.date().yesterday().unwrap())
        );
        assert_eq!(civil_date_at(&Builtin, &z, 80.0, 30.0).unwrap(), d);
    }

    #[test]
    fn the_civil_date_turns_at_sunrise() {
        let tz = TimeZone::get("Africa/Cairo").unwrap();
        let noon = at("2026-11-21T12:00[Africa/Cairo]");
        let rise = hours_of(day(&Builtin, &noon, LUXOR.0, LUXOR.1).unwrap()).sunrise;
        let today = civil_date_of(noon.date());
        let yesterday = civil_date_of(noon.date().yesterday().unwrap());
        let before = (rise - SignedDuration::from_secs(1)).to_zoned(tz.clone());
        let after = rise.to_zoned(tz.clone());
        assert_eq!(
            civil_date_at(&Builtin, &before, LUXOR.0, LUXOR.1).unwrap(),
            yesterday
        );
        assert_eq!(
            civil_date_at(&Builtin, &after, LUXOR.0, LUXOR.1).unwrap(),
            today
        );
        // `day` turns at the same second.
        let at_rise = hours_of(day(&Builtin, &after, LUXOR.0, LUXOR.1).unwrap());
        assert_eq!(at_rise.sunrise, rise);
        assert_eq!(at_rise.hour_at(rise).map(|h| (h.half, h.n)), Some((Half::Day, 1)));
        let just_before = hours_of(day(&Builtin, &before, LUXOR.0, LUXOR.1).unwrap());
        assert_eq!(just_before.next_sunrise, rise);
        // Just before midnight is still the same Egyptian day as noon.
        let late = at("2026-11-21T23:59[Africa/Cairo]");
        assert_eq!(
            civil_date_at(&Builtin, &late, LUXOR.0, LUXOR.1).unwrap(),
            today
        );
        let start = civil_day_start(&Builtin, &today, &tz, LUXOR.0, LUXOR.1).unwrap();
        assert_eq!(start.timestamp(), rise);
    }
}
