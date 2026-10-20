//! The sky of a moment, ported from geomancy's tests, where the expectations were checked
//! against JPL Horizons and the Swiss Ephemeris; and the hours a clock needs.

use astrometry::derived::diff;
use horae::aspect::angle;
use horae::hours::{self, next_hour, next_hour_of};
use horae::{Builtin, Day, Dignity, Kind, Phase, Planet, Point, Polar, Sign, Sky, Solar};
use jiff::Zoned;
use jiff::civil::date;
use jiff::tz::TimeZone;

use Planet::*;

const MADISON: (f64, f64) = (43.0731, -89.4012);

fn chicago() -> TimeZone {
    TimeZone::get("America/Chicago").unwrap()
}

fn at(y: i16, m: i8, d: i8, h: i8, min: i8, tz: TimeZone) -> Zoned {
    date(y, m, d).at(h, min, 0, 0).to_zoned(tz).unwrap()
}

fn madison(t: &Zoned) -> Sky {
    Sky::at(t, MADISON.0, MADISON.1).unwrap()
}

fn madison_day(t: &Zoned) -> Day {
    hours::day(&Builtin, t, MADISON.0, MADISON.1).unwrap()
}

/// Positions to the second against JPL Horizons at 2026-10-15 18:14 UT: within 0.1″
/// (Horizons' IAU76/80 ecliptic is 0.05″ from ours in this era).
#[test]
fn against_horizons() {
    let want = [
        (Sun, 202.4099271),
        (Moon, 260.7275509),
        (Mercury, 227.1042314),
        (Venus, 215.4692232),
        (Mars, 130.0552437),
        (Jupiter, 142.0661601),
        (Saturn, 10.4275590),
    ];
    let s = madison(&at(2026, 10, 15, 18, 14, TimeZone::UTC));
    for (p, lon) in want {
        let got = s.body(p).position.longitude;
        let d = diff(got, lon).abs() * 3600.0;
        assert!(d < 0.1, "{p}: {got:.7}°, Horizons {lon}° ({d:.3}″ off)");
    }
}

#[test]
fn bodies() {
    let s = madison(&at(2026, 10, 15, 13, 14, chicago()));
    let tests = [
        (
            Point::Sun,
            "22°24′",
            Sign::Libra,
            false,
            vec![Dignity::Fall],
            None,
        ),
        (
            Point::Moon,
            "20°43′",
            Sign::Sagittarius,
            false,
            vec![Dignity::Peregrine],
            None,
        ),
        (
            Point::Venus,
            "5°28′",
            Sign::Scorpio,
            true,
            vec![Dignity::Detriment],
            Some(Solar::UnderTheBeams),
        ),
        (
            Point::Saturn,
            "10°25′",
            Sign::Aries,
            true,
            vec![Dignity::Fall],
            None,
        ),
        // The true node: the Swiss Ephemeris has it at 328.1620° at this moment.
        (
            Point::NorthNode,
            "28°09′",
            Sign::Aquarius,
            true,
            vec![],
            None,
        ),
        (Point::SouthNode, "28°09′", Sign::Leo, true, vec![], None),
    ];
    for (p, minutes, sign, retrograde, dignities, solar) in tests {
        let b = s.body(p);
        let pos = &b.position;
        assert_eq!(pos.point, p);
        assert!(
            pos.to_string().starts_with(minutes) && pos.sign() == sign,
            "{p}: {pos}"
        );
        assert_eq!(
            (pos.is_retrograde(), &b.dignities, b.solar),
            (retrograde, &dignities, solar),
            "{p}"
        );
    }
}

// The nodes' motion is the true node's: the North Node a day on is where its speed says, and
// the South Node moves with it.
#[test]
fn nodes_move_as_their_speed_says() {
    let t = at(2026, 10, 15, 13, 14, chicago());
    let a = madison(&t);
    let b = madison(&t.checked_add(jiff::SignedDuration::from_hours(24)).unwrap());
    let (n0, n1) = (a.body(Point::NorthNode), b.body(Point::NorthNode));
    let moved = diff(n1.position.longitude, n0.position.longitude);
    let speed = (n0.position.speed + n1.position.speed) / 2.0;
    assert!(
        (moved - speed).abs() < 0.02,
        "moved {moved:.4}°, speed {speed:.4}°/day"
    );
    let s = a.body(Point::SouthNode).position;
    assert_eq!(s.speed, n0.position.speed);
    assert!((diff(s.longitude, n0.position.longitude).abs() - 180.0).abs() < 1e-9);
}

#[test]
fn planetary_day() {
    let tz = chicago();
    let day = |h, m| {
        let t = at(2026, 10, 15, h, m, tz.clone());
        let d = madison(&t).day;
        let hour = d.hour_at(t.timestamp()).cloned();
        (d, hour)
    };
    // Thursday, after sunrise: the day of Jupiter, whose hour is first.
    let (d, h) = day(13, 14);
    let h = h.unwrap();
    assert!(
        d.ruler() == Jupiter && h.ruler == Saturn && h.number == 7 && !h.night,
        "{d:?}"
    );
    // Sunrise and sunset against JPL Horizons, which gives the first minute after them: 12:12
    // and 23:15 UT.
    let pd = d.planetary().unwrap();
    for (got, (hh, mm)) in [(&pd.sunrise, (12, 12)), (&pd.sunset, (23, 15))] {
        let want = at(2026, 10, 15, hh, mm, TimeZone::UTC);
        let secs = want.timestamp().as_second() - got.timestamp().as_second();
        assert!((0..=61).contains(&secs), "{got}, Horizons {want}");
    }
    let first = day(7, 12).1.unwrap();
    assert!(
        first.ruler == Jupiter && first.number == 1 && !first.night,
        "{first:?}"
    );
    // Before sunrise it's still Wednesday, Mercury's day, in its night. Counting on from
    // Mercury in Chaldean order, the 11th night hour (the 23rd) is the Moon's and the 12th
    // Saturn's, which leads into Thursday's first, Jupiter's.
    let (d, h) = day(5, 30);
    let h = h.unwrap();
    assert!(
        d.ruler() == Mercury && h.night && h.ruler == Moon && h.number == 11,
        "{d:?}"
    );
    let h = day(6, 30).1.unwrap();
    assert!(h.night && h.number == 12 && h.ruler == Saturn, "{h:?}");
    // After sunset: Thursday's 12th day hour is Mercury's, so the 1st night hour is the
    // Moon's.
    let h = day(18, 30).1.unwrap();
    assert!(h.night && h.number == 1 && h.ruler == Moon, "{h:?}");
}

// Each day's 24 hours run end to end from sunrise, through sunset, to the next sunrise, each
// a twelfth of the daylight or the night, across the end of daylight saving on 1 November.
#[test]
fn the_hours_tile_the_day() {
    let d = madison_day(&at(2026, 10, 31, 23, 0, chicago()));
    let d = d.planetary().unwrap();
    assert_eq!(d.hours.len(), 24);
    assert_eq!(d.hours[0].start, d.sunrise);
    assert_eq!(d.hours[12].start, d.sunset);
    assert_eq!(d.hours[23].end, d.next_sunrise);
    for w in d.hours.windows(2) {
        assert_eq!(w[0].end, w[1].start);
    }
    let secs =
        |a: &Zoned, b: &Zoned| (b.timestamp().as_second() - a.timestamp().as_second()) as f64;
    let night = secs(&d.sunset, &d.next_sunrise) / 12.0;
    for h in &d.hours[12..] {
        let got = secs(&h.start, &h.end);
        assert!(
            (got - night).abs() <= 1.0,
            "a night hour lasts {got} s, want {night}"
        );
    }
}

// The next day begins where this one ends, its first hour the ruler of its weekday.
#[test]
fn the_next_day() {
    let today = madison_day(&at(2026, 10, 15, 13, 14, chicago()));
    let today = today.planetary().unwrap();
    let tomorrow = today.next(&Builtin).unwrap();
    let tomorrow = tomorrow.planetary().unwrap();
    assert_eq!(tomorrow.ruler, Venus, "Friday");
    let gap = tomorrow.sunrise.timestamp().as_second() - today.next_sunrise.timestamp().as_second();
    assert!(
        gap.abs() <= 1,
        "{} then {}",
        today.next_sunrise,
        tomorrow.sunrise
    );
    assert_eq!(tomorrow.hours[0].ruler, Venus);
}

// The next hour, and the next of the same planet: seven hours on, here across sunset.
#[test]
fn next_hours() {
    let t = at(2026, 10, 15, 13, 14, chicago());
    let today = madison_day(&t);
    let tomorrow = today.planetary().unwrap().next(&Builtin).unwrap();
    let days = [today, tomorrow];
    let now = days[0].hour_at(t.timestamp()).unwrap();
    assert_eq!((now.ruler, now.number, now.night), (Saturn, 7, false));
    let next = next_hour(&days, t.timestamp()).unwrap();
    assert_eq!(
        (next.ruler, next.number, next.start.clone()),
        (Jupiter, 8, now.end.clone())
    );
    let again = next_hour_of(&days, Saturn, t.timestamp()).unwrap();
    assert_eq!(
        (again.number, again.night),
        (2, true),
        "seven hours on: the 2nd of the night"
    );
    // Late in the night, the next of the same planet is in the next day.
    let late = at(2026, 10, 16, 5, 30, chicago());
    let h = days[0].hour_at(late.timestamp()).unwrap();
    let again = next_hour_of(&days, h.ruler, late.timestamp()).unwrap();
    assert!(
        !again.night && again.start > days[1].planetary().unwrap().sunrise,
        "{again:?}"
    );
}

// At Utqiaġvik the Sun doesn't set at midsummer or rise at midwinter: the day takes the
// weekday's ruler and has no hours.
#[test]
fn polar() {
    let ak = TimeZone::get("America/Anchorage").unwrap();
    for (t, ruler, why) in [
        (at(2026, 6, 21, 12, 0, ak.clone()), Sun, Polar::NoSunset), // a Sunday
        (at(2026, 12, 21, 12, 0, ak.clone()), Moon, Polar::NoSunrise), // a Monday
    ] {
        let d = Sky::at(&t, 71.2906, -156.7886).unwrap().day;
        assert_eq!(d, Day::Polar { ruler, polar: why }, "{t}");
        assert!(d.hour_at(t.timestamp()).is_none());
    }
}

// The lunations are the conjunction and opposition of the Sun and Moon, on either side of
// the moment.
#[test]
fn lunations() {
    let t = at(2026, 10, 15, 13, 14, chicago());
    let m = madison(&t).moon;
    let a = m.appearance;
    assert!(
        a.phase == Phase::Crescent && (a.illuminated - 0.238).abs() < 0.005 && a.phase.is_waxing(),
        "{a:?}"
    );
    let l = &m.lunations;
    let ts = |z: &Zoned| z.timestamp();
    assert!(ts(&l.previous_new) <= ts(&t) && ts(&l.previous_full) <= ts(&t));
    assert!(ts(&l.next_new) > ts(&t) && ts(&l.next_full) > ts(&t));
    let month = (ts(&l.next_new).as_second() - ts(&l.previous_new).as_second()) as f64 / 86400.0;
    assert!(
        (29.2..29.9).contains(&month),
        "new moons {month} days apart"
    );
    for (z, want) in [
        (&l.previous_new, 0.0),
        (&l.next_new, 0.0),
        (&l.previous_full, 180.0),
        (&l.next_full, 180.0),
    ] {
        let e = madison(z).moon.appearance.elongation;
        assert!(
            diff(e, want).abs() < 0.02,
            "elongation at {z} = {e:.3}°, want {want}°"
        );
    }
    assert_eq!(l.previous_new.time_zone(), t.time_zone());
}

// The Moon's course: its next aspect, exact when its distance from the planet is the
// aspect's angle, and void of course once it has made its last aspect in a sign.
#[test]
fn moon_course() {
    let s = madison(&at(2026, 10, 15, 18, 14, TimeZone::UTC));
    let c = &s.moon.course;
    let next = c.next_aspect().expect("an aspect");
    assert!(
        !c.is_void_of_course() && next.planet == Jupiter && next.kind == Kind::Trine,
        "{c:?}"
    );
    let exact = madison(&next.exact);
    let d = angle(
        exact.body(Moon).position.longitude,
        exact.body(Jupiter).position.longitude,
    );
    assert!(
        (d - 120.0).abs() < 0.02,
        "the Moon is {d:.3}° from Jupiter at {}",
        next.exact
    );
    // It enters Capricorn at the cusp.
    let leaves = &c.leaves_sign;
    let minute = jiff::SignedDuration::from_mins(1);
    let sign = |z: Zoned| madison(&z).body(Moon).position.sign();
    assert_eq!(
        (
            sign(leaves.checked_sub(minute).unwrap()),
            sign(leaves.checked_add(minute).unwrap())
        ),
        (Sign::Sagittarius, Sign::Capricorn),
        "leaves {leaves}"
    );
    // After its sextile to the Sun, it makes no more aspects in Sagittarius.
    let v = madison(&at(2026, 10, 16, 6, 0, TimeZone::UTC)).moon.course;
    let apart = (v.leaves_sign.timestamp().as_second() - leaves.timestamp().as_second()).abs();
    assert!(v.is_void_of_course() && apart <= 60, "{v:?}");
    // The same course, seen from earlier: its aspects in order, the last before then.
    assert!(c.aspects.windows(2).all(|w| w[0].exact <= w[1].exact));
    let last = c.aspects.last().unwrap();
    assert!(last.exact < v.leaves_sign && c.next_aspect_after(last.exact.timestamp()).is_none());
}

#[test]
fn aspects_found() {
    let s = madison(&at(2026, 10, 15, 13, 14, chicago()));
    assert!(
        s.aspects
            .iter()
            .any(|a| a.points == [Point::Mars, Point::Saturn] && a.kind == Kind::Trine),
        "{:?}",
        s.aspects
    );
}

#[cfg(feature = "serde")]
#[test]
fn json_round_trip() {
    let s = madison(&at(2026, 10, 15, 13, 14, chicago()));
    let b = serde_json::to_string(&s).unwrap();
    for want in [
        r#""point":"Venus""#,
        r#""dignities":["Detriment"]"#,
        r#""solar":"Under the beams""#,
        r#""phase":"Crescent""#,
    ] {
        assert!(b.contains(want), "JSON lacks {want}:\n{b}");
    }
    let back: Sky = serde_json::from_str(&b).unwrap();
    assert_eq!(serde_json::to_string(&back).unwrap(), b);
}

#[test]
fn invalid_coordinates() {
    assert!(Sky::at(&Zoned::now(), 91.0, 0.0).is_err());
    assert!(hours::day(&Builtin, &Zoned::now(), 0.0, f64::NAN).is_err());
}

// Mercury's stations in 2026, on the dates the almanacs give, and the turns either side of a
// moment agreeing with the planets' motion then.
#[test]
fn stations() {
    use horae::Direction::{Direct, Retrograde};
    use horae::stations::{next_station, previous_station, stations};
    let tt = |y, m, d| horae::time::tt(at(y, m, d, 0, 0, TimeZone::UTC).timestamp());
    let got: Vec<_> = stations(
        &Builtin,
        Mercury,
        tt(2026, 1, 1),
        tt(2027, 1, 1),
        &TimeZone::UTC,
    )
    .unwrap()
    .into_iter()
    .map(|s| (s.turns, s.at.date()))
    .collect();
    let want = [
        (Retrograde, date(2026, 2, 26)),
        (Direct, date(2026, 3, 20)),
        (Retrograde, date(2026, 6, 29)),
        (Direct, date(2026, 7, 23)),
        (Retrograde, date(2026, 10, 24)),
        (Direct, date(2026, 11, 13)),
    ];
    assert_eq!(got, want);
    // On 15 October, Venus is retrograde, since 3 October and until 14 November.
    let t = at(2026, 10, 15, 13, 14, chicago());
    let now = horae::time::tt(t.timestamp());
    let tz = TimeZone::UTC;
    let before = previous_station(&Builtin, Venus, now, &tz)
        .unwrap()
        .unwrap();
    let after = next_station(&Builtin, Venus, now, &tz).unwrap().unwrap();
    assert_eq!(
        (before.turns, before.at.date()),
        (Retrograde, date(2026, 10, 3))
    );
    assert_eq!((after.turns, after.at.date()), (Direct, date(2026, 11, 14)));
    // At a station, the planet stands still.
    let still = horae::Position::at(
        &Builtin,
        Point::Venus,
        horae::time::tt(after.at.timestamp()),
    )
    .unwrap();
    assert!(still.speed.abs() < 1e-3, "{still:?}");
    assert!(next_station(&Builtin, Sun, now, &tz).unwrap().is_none());
}

// At sunrise the Sun is on the eastern horizon, so the Ascendant is beside it: within a degree
// or two, the Sun being 50′ below the horizon then, by the almanacs' rule. At its culmination,
// halfway from sunrise to sunset, the Midheaven is beside it.
#[test]
fn angles_at_sunrise_and_noon() {
    let d = madison_day(&at(2026, 10, 15, 13, 14, chicago()));
    let d = d.planetary().unwrap();
    let tt = |z: &Zoned| horae::time::tt(z.timestamp());
    let sun = |tt: f64| {
        horae::Position::at(&Builtin, Point::Sun, tt)
            .unwrap()
            .longitude
    };
    let rise = tt(&d.sunrise);
    let a = horae::angles(rise, MADISON.0, MADISON.1);
    assert!(
        diff(a.ascendant, sun(rise)).abs() < 2.0,
        "Ascendant {a:?}, Sun {}",
        sun(rise)
    );
    let noon = (rise + tt(&d.sunset)) / 2.0;
    let a = horae::angles(noon, MADISON.0, MADISON.1);
    assert!(
        diff(a.midheaven, sun(noon)).abs() < 1.0,
        "Midheaven {a:?}, Sun {}",
        sun(noon)
    );
}
