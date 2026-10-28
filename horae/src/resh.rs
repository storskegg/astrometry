//! The Resh times and the elemental tides, the sibling of the planetary hours. Resh is four
//! moments of the Sun's day: Dawn at sunrise, Noon at its upper meridian transit, Sunset, and
//! Midnight at its lower transit. Noon and Midnight are the transits, not the midpoints of the
//! day and the night, so they drift a little with the equation of time; and they come on polar
//! dates too, when the Sun neither rises nor sets.
//!
//! The tides divide the planetary day: the daylight from sunrise to sunset in six equal parts,
//! and the night from sunset to the next sunrise in six more, each part (two planetary hours)
//! in five equal tides ordered Spirit, Air, Fire, Water, Earth. That is 30 tides by day and 30
//! by night, and every tenth boundary is an hour's. A polar day has none.

use astrometry::horizon::{SUNRISE, sun_crossings, sun_transits};
use astrometry::source::Source;
use astrometry::time::DAY;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp, Zoned};

use crate::hours::{Day, PlanetaryDay};
use crate::{Error, check_coordinates, time};

named_enum!(
    /// The four moments of the Sun's day.
    Resh, "Resh time", [
        Dawn => "Dawn",
        Noon => "Noon",
        Sunset => "Sunset",
        Midnight => "Midnight",
    ]
);

named_enum!(
    /// The five tattvas, which take the tides in turn.
    Tattva, "tattva", [
        Spirit => "Spirit",
        Air => "Air",
        Fire => "Fire",
        Water => "Water",
        Earth => "Earth",
    ]
);

/// A Resh time.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReshTime {
    pub resh: Resh,
    pub at: Zoned,
}

/// An elemental tide.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tide {
    pub tattva: Tattva,
    /// 1–6, the part of the day or of the night.
    pub part: u8,
    /// 1–30, of the day or of the night.
    pub number: u8,
    pub night: bool,
    pub start: Zoned,
    pub end: Zoned,
}

impl Tide {
    /// Whether the tide holds `t`: from its start, until its end.
    pub fn contains(&self, t: Timestamp) -> bool {
        self.start.timestamp() <= t && t < self.end.timestamp()
    }
}

const SECOND: i128 = 1_000_000_000;

/// The moment `ns` nanoseconds after the epoch, rounded to the second, in `tz`.
fn rounded(ns: i128, tz: &TimeZone) -> Zoned {
    let secs = (ns + SECOND / 2).div_euclid(SECOND);
    Timestamp::from_second(secs as i64)
        .expect("in range")
        .to_zoned(tz.clone())
}

/// The 60 tides of a planetary day: the daylight's 30 and then the night's. Each of the twelve
/// parts runs across two hours, so its ends are the hours' exactly; its five tides are equal to
/// the nanosecond, their inner boundaries then rounded to the second.
pub fn tides(day: &PlanetaryDay) -> Vec<Tide> {
    let tz = day.sunrise.time_zone();
    let mut out = Vec::with_capacity(60);
    for part in 0..12 {
        let (start, end) = (&day.hours[2 * part].start, &day.hours[2 * part + 1].end);
        let (s, e) = (
            start.timestamp().as_nanosecond(),
            end.timestamp().as_nanosecond(),
        );
        let bound = |k: i128| match k {
            0 => start.clone(),
            5 => end.clone(),
            _ => rounded(s + (e - s) * k / 5, tz),
        };
        let mut from = bound(0);
        for k in 0..5 {
            let to = bound(k as i128 + 1);
            let number = (part % 6) as u8 * 5 + k + 1;
            out.push(Tide {
                tattva: Tattva::ALL[k as usize],
                part: (part % 6) as u8 + 1,
                number,
                night: part >= 6,
                start: from,
                end: to.clone(),
            });
            from = to;
        }
    }
    out
}

/// The tide holding `t`, if the day is planetary and holds it.
pub fn tide_at(day: &Day, t: Timestamp) -> Option<Tide> {
    tides(day.planetary()?).into_iter().find(|d| d.contains(t))
}

/// Every Resh time from `from` until `to`, in order, in `from`'s zone, at latitude `lat` and
/// longitude `lon` in degrees (north and east positive). Dawn and Sunset are found as the
/// planetary day finds sunrise and sunset, so a place without either on a date has none.
pub fn times(
    src: &dyn Source,
    from: &Zoned,
    to: &Zoned,
    lat: f64,
    lon: f64,
) -> Result<Vec<ReshTime>, Error> {
    check_coordinates(lat, lon)?;
    let (phi, east) = (lat.to_radians(), lon.to_radians());
    // A little beyond each end, for the rounding of the times to the second.
    let (a, b) = (
        time::tt(from.timestamp()) - 2.0 / DAY,
        time::tt(to.timestamp()) + 2.0 / DAY,
    );
    let tz = from.time_zone();
    let mut out = Vec::new();
    for c in sun_crossings(src, a, b, phi, east, SUNRISE)? {
        let resh = if c.rising { Resh::Dawn } else { Resh::Sunset };
        out.push(ReshTime {
            resh,
            at: time::zoned(c.tt, tz),
        });
    }
    for x in sun_transits(src, a, b, east)? {
        let resh = if x.upper { Resh::Noon } else { Resh::Midnight };
        out.push(ReshTime {
            resh,
            at: time::zoned(x.tt, tz),
        });
    }
    out.retain(|r| from.timestamp() <= r.at.timestamp() && r.at.timestamp() < to.timestamp());
    out.sort_by_key(|r| r.at.timestamp());
    Ok(out)
}

/// The first Resh time strictly after `t`, looking two days on.
pub fn next_time(
    src: &dyn Source,
    t: &Zoned,
    lat: f64,
    lon: f64,
) -> Result<Option<ReshTime>, Error> {
    let to = t
        .checked_add(SignedDuration::from_hours(48))
        .expect("in range");
    Ok(times(src, t, &to, lat, lon)?
        .into_iter()
        .find(|r| r.at.timestamp() > t.timestamp()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Planet;
    use crate::hours::Hour;
    use jiff::civil::date;

    /// A day in UTC with irregular hours (whole seconds, as the real ones are) from 06:00:00.
    fn fake_day() -> PlanetaryDay {
        let tz = TimeZone::UTC;
        let t0 = date(2026, 10, 15)
            .at(6, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap();
        let mut at = t0.timestamp().as_second();
        let mut bounds = vec![at];
        for n in 0..24 {
            at += if n < 12 { 3_001 + n } else { 4_103 + n };
            bounds.push(at);
        }
        let z = |s: i64| Timestamp::from_second(s).unwrap().to_zoned(tz.clone());
        let hours = (0..24usize)
            .map(|n| Hour {
                ruler: Planet::Sun,
                number: (n % 12) as u8 + 1,
                night: n >= 12,
                start: z(bounds[n]),
                end: z(bounds[n + 1]),
            })
            .collect();
        PlanetaryDay {
            ruler: Planet::Sun,
            latitude: 0.0,
            longitude: 0.0,
            sunrise: z(bounds[0]),
            sunset: z(bounds[12]),
            next_sunrise: z(bounds[24]),
            hours,
        }
    }

    #[test]
    fn tides_tile_the_day() {
        let day = fake_day();
        let tides = tides(&day);
        assert_eq!(tides.len(), 60);
        assert_eq!(tides[0].start, day.sunrise);
        assert_eq!(tides[29].end, day.sunset);
        assert_eq!(tides[30].start, day.sunset);
        assert_eq!(tides[59].end, day.next_sunrise);
        for w in tides.windows(2) {
            assert_eq!(w[0].end, w[1].start);
        }
        for (i, d) in tides.iter().enumerate() {
            assert!(d.start < d.end, "{d:?}");
            assert_eq!(d.night, i >= 30);
            assert_eq!(usize::from(d.number), i % 30 + 1);
            assert_eq!(d.tattva, Tattva::ALL[i % 5]);
            assert_eq!(usize::from(d.part), i % 30 / 5 + 1);
            assert_eq!(d.part, (d.number - 1) / 5 + 1);
        }
    }

    // Every tenth boundary is an hour's, and each part's five tides are equal to a second.
    #[test]
    fn tides_meet_the_hours() {
        let day = fake_day();
        let tides = tides(&day);
        for p in 0..12 {
            assert_eq!(tides[5 * p].start, day.hours[2 * p].start);
            assert_eq!(tides[5 * p + 4].end, day.hours[2 * p + 1].end);
            let len = |d: &Tide| d.end.timestamp().as_second() - d.start.timestamp().as_second();
            let part: Vec<i64> = tides[5 * p..5 * p + 5].iter().map(len).collect();
            let (lo, hi) = (part.iter().min().unwrap(), part.iter().max().unwrap());
            assert!(hi - lo <= 1, "part {p}: {part:?}");
        }
    }

    #[test]
    fn tide_holds_its_start_not_its_end() {
        let day = fake_day();
        let tides = tides(&day);
        let d = &tides[7];
        assert!(d.contains(d.start.timestamp()));
        assert!(!d.contains(d.end.timestamp()));
        assert!(tides[8].contains(d.end.timestamp()));
    }

    #[test]
    fn rounding_to_the_second() {
        let tz = TimeZone::UTC;
        assert_eq!(
            rounded(10 * SECOND + 499_999_999, &tz)
                .timestamp()
                .as_second(),
            10
        );
        assert_eq!(
            rounded(10 * SECOND + 500_000_000, &tz)
                .timestamp()
                .as_second(),
            11
        );
    }

    #[test]
    fn names_and_orders() {
        assert_eq!(
            Resh::ALL.map(Resh::name),
            ["Dawn", "Noon", "Sunset", "Midnight"]
        );
        assert_eq!(
            Tattva::ALL.map(Tattva::name),
            ["Spirit", "Air", "Fire", "Water", "Earth"]
        );
        assert_eq!("midnight".parse::<Resh>(), Ok(Resh::Midnight));
        assert!("Dusk".parse::<Resh>().is_err());
    }
}
