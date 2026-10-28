//! The Resh times and the elemental tides against the planetary day they divide.

use horae::resh::{self, Resh, Tattva};
use horae::{Builtin, Day, hours};
use jiff::civil::date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Zoned};

const MADISON: (f64, f64) = (43.0731, -89.4012);
const TROMSO: (f64, f64) = (69.65, 18.96);

fn chicago() -> TimeZone {
    TimeZone::get("America/Chicago").unwrap()
}

fn at(y: i16, m: i8, d: i8, h: i8, tz: TimeZone) -> Zoned {
    date(y, m, d).at(h, 0, 0, 0).to_zoned(tz).unwrap()
}

fn madison_day(t: &Zoned) -> Day {
    hours::day(&Builtin, t, MADISON.0, MADISON.1).unwrap()
}

#[test]
fn tides_of_a_real_day() {
    let day = madison_day(&at(2026, 10, 15, 13, chicago()));
    let day = day.planetary().unwrap();
    let tides = resh::tides(day);
    assert_eq!(tides.len(), 60);
    assert_eq!(tides.iter().filter(|d| !d.night).count(), 30);
    assert_eq!(tides[0].start, day.sunrise);
    assert_eq!(tides[29].end, day.sunset);
    assert_eq!(tides[59].end, day.next_sunrise);
    for w in tides.windows(2) {
        assert_eq!(w[0].end, w[1].start);
    }
    for (i, d) in tides.iter().enumerate() {
        assert_eq!(usize::from(d.number), i % 30 + 1);
        assert_eq!(d.tattva, Tattva::ALL[i % 5]);
        assert_eq!(d.part, (d.number - 1) / 5 + 1);
        assert_eq!(d.night, i >= 30);
    }
    // Every tenth boundary is an hour's.
    for p in 0..12 {
        assert_eq!(tides[5 * p].start, day.hours[2 * p].start);
        assert_eq!(tides[5 * p + 4].end, day.hours[2 * p + 1].end);
    }
}

#[test]
fn tide_at_agrees_with_tides() {
    let day = madison_day(&at(2026, 10, 15, 13, chicago()));
    let planetary = day.planetary().unwrap();
    let tides = resh::tides(planetary);
    for d in &tides {
        let mid = d.start.timestamp() + SignedDuration::from_secs(1);
        assert_eq!(resh::tide_at(&day, mid).as_ref(), Some(d));
        assert_eq!(resh::tide_at(&day, d.start.timestamp()).as_ref(), Some(d));
    }
    let before = planetary.sunrise.timestamp() - SignedDuration::from_secs(1);
    assert_eq!(resh::tide_at(&day, before), None);
    assert_eq!(
        resh::tide_at(&day, planetary.next_sunrise.timestamp()),
        None
    );
}

#[test]
fn a_day_of_resh_times() {
    let day = madison_day(&at(2026, 10, 15, 13, chicago()));
    let day = day.planetary().unwrap();
    let hour = SignedDuration::from_hours(1);
    let found = resh::times(
        &Builtin,
        &(&day.sunrise - hour),
        &(&day.next_sunrise - hour),
        MADISON.0,
        MADISON.1,
    )
    .unwrap();
    let kinds: Vec<Resh> = found.iter().map(|r| r.resh).collect();
    assert_eq!(
        kinds,
        [Resh::Dawn, Resh::Noon, Resh::Sunset, Resh::Midnight]
    );
    // Found over another window, so the bisection can fall a second either side.
    let near =
        |a: &Zoned, b: &Zoned| (a.timestamp().as_second() - b.timestamp().as_second()).abs() <= 1;
    assert!(near(&found[0].at, &day.sunrise), "{}", found[0].at);
    assert!(near(&found[2].at, &day.sunset), "{}", found[2].at);
    // Noon between sunrise and sunset, Midnight between sunset and the next sunrise.
    assert!(day.sunrise < found[1].at && found[1].at < day.sunset);
    assert!(day.sunset < found[3].at && found[3].at < day.next_sunrise);
    assert!(
        found
            .iter()
            .all(|r| r.at.time_zone() == day.sunrise.time_zone())
    );
}

#[test]
fn the_window_is_half_open() {
    let (from, to) = (
        at(2026, 10, 15, 0, chicago()),
        at(2026, 10, 16, 0, chicago()),
    );
    let found = resh::times(&Builtin, &from, &to, MADISON.0, MADISON.1).unwrap();
    assert_eq!(found.len(), 4);
    assert!(found.windows(2).all(|w| w[0].at < w[1].at));
    assert!(found.iter().all(|r| from <= r.at && r.at < to));
    assert!(
        resh::times(&Builtin, &to, &from, MADISON.0, MADISON.1)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn midnight_falls_in_the_night() {
    let day = madison_day(&at(2026, 10, 15, 13, chicago()));
    let day = day.planetary().unwrap();
    let minute = SignedDuration::from_mins(1);
    let found = resh::times(
        &Builtin,
        &(&day.sunset + minute),
        &(&day.next_sunrise - minute),
        MADISON.0,
        MADISON.1,
    )
    .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].resh, Resh::Midnight);
}

#[test]
fn next_time_steps_through_a_day() {
    let mut t = at(2026, 10, 15, 1, chicago());
    let mut seen = Vec::new();
    for _ in 0..8 {
        let next = resh::next_time(&Builtin, &t, MADISON.0, MADISON.1)
            .unwrap()
            .expect("a Resh time within two days");
        assert!(next.at > t);
        seen.push(next.resh);
        t = next.at;
    }
    use Resh::*;
    assert_eq!(
        seen,
        [Dawn, Noon, Sunset, Midnight, Dawn, Noon, Sunset, Midnight]
    );
    // Strictly after: at a Resh time itself, the next one.
    let again = resh::next_time(&Builtin, &t, MADISON.0, MADISON.1)
        .unwrap()
        .unwrap();
    assert_eq!(again.resh, Dawn);
}

#[test]
fn polar_day_has_noon_and_midnight_only() {
    let t = at(2026, 6, 21, 12, TimeZone::UTC);
    let day = hours::day(&Builtin, &t, TROMSO.0, TROMSO.1).unwrap();
    assert!(day.planetary().is_none());
    assert_eq!(resh::tide_at(&day, t.timestamp()), None);
    let from = at(2026, 6, 21, 0, TimeZone::UTC);
    let to = at(2026, 6, 22, 0, TimeZone::UTC);
    let found = resh::times(&Builtin, &from, &to, TROMSO.0, TROMSO.1).unwrap();
    let kinds: Vec<Resh> = found.iter().map(|r| r.resh).collect();
    assert_eq!(kinds, [Resh::Noon, Resh::Midnight]);
}

#[test]
fn invalid_coordinates() {
    let t = at(2026, 10, 15, 13, chicago());
    assert!(resh::times(&Builtin, &t, &t, 91.0, 0.0).is_err());
    assert!(resh::next_time(&Builtin, &t, 0.0, f64::NAN).is_err());
}

#[cfg(feature = "serde")]
#[test]
fn serializes() {
    let day = madison_day(&at(2026, 10, 15, 13, chicago()));
    let tide = resh::tides(day.planetary().unwrap()).remove(31);
    let v = serde_json::to_value(&tide).unwrap();
    assert_eq!(v["tattva"], "Air");
    assert_eq!(v["part"], 1);
    assert_eq!(v["number"], 2);
    assert_eq!(v["night"], true);
    let back: horae::Tide = serde_json::from_value(v).unwrap();
    assert_eq!(back, tide);
}
