//! Speeds and nodes against the Swiss Ephemeris (JPL mode).

use std::path::PathBuf;

use astrometry::derived::{
    diff, mean_apogee, mean_apogee_speed, mean_node, mean_node_speed, speed, true_node,
};
use astrometry::source::{Body, Builtin};

fn rows() -> Vec<Vec<String>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/swisseph/positions.tsv");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| l.starts_with("jpl\t"))
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

fn body(name: &str) -> Option<Body> {
    Some(match name {
        "Sun" => Body::Sun,
        "Moon" => Body::Moon,
        "Mercury" => Body::Mercury,
        "Venus" => Body::Venus,
        "Mars" => Body::Mars,
        "Jupiter" => Body::Jupiter,
        "Saturn" => Body::Saturn,
        _ => return None,
    })
}

#[test]
fn speeds_and_nodes_against_swiss_ephemeris() {
    let mut speeds = 0.0f64;
    let (mut tn, mut mn, mut ma) = (0.0f64, 0.0f64, 0.0f64);
    let (mut mns, mut mas) = (0.0f64, 0.0f64);
    for r in rows() {
        let jd: f64 = r[2].parse().unwrap();
        let (lon, spd): (f64, f64) = (r[3].parse().unwrap(), r[6].parse().unwrap());
        match r[1].as_str() {
            "TrueNode" => tn = tn.max(diff(true_node(&Builtin, jd).unwrap(), lon).abs() * 3600.0),
            "MeanNode" => {
                mn = mn.max(diff(mean_node(jd), lon).abs() * 3600.0);
                mns = mns.max((mean_node_speed(jd) - spd).abs() * 3600.0);
            }
            "MeanApogee" => {
                ma = ma.max(diff(mean_apogee(jd), lon).abs() * 3600.0);
                mas = mas.max((mean_apogee_speed(jd) - spd).abs() * 3600.0);
            }
            name => {
                let b = body(name).unwrap();
                let got = speed(&Builtin, b, jd).unwrap();
                // Relative to the body's typical daily motion.
                speeds = speeds.max((got - spd).abs() * 3600.0);
            }
        }
    }
    println!(
        "speeds within {speeds:.4}″/day; true node {tn:.3}″; mean node {mn:.3}″, mean apogee {ma:.3}″; mean speeds {mns:.4}″/day, {mas:.4}″/day"
    );
    assert!(speeds < 0.5, "speeds {speeds:.4}″/day");
    assert!(tn < 0.05, "true node {tn:.3}″");
    // The mean node is a convention: the Swiss Ephemeris' expression differs from Meeus's.
    assert!(mn < 0.3, "mean node {mn:.3}″");
    // The mean apogee is the Swiss Ephemeris' too, projected onto the ecliptic; the remainder is
    // a difference of conventions in the mean elements.
    assert!(ma < 1.7, "mean apogee {ma:.3}″");
    assert!(mns < 0.001, "mean node speed {mns:.4}″/day");
    assert!(mas < 0.007, "mean apogee speed {mas:.4}″/day");
}

fn data(file: &str) -> Vec<Vec<String>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(file);
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

/// Greenwich apparent sidereal time against Horizons' local apparent sidereal time at 0° E:
/// the Earth's rotation, UT1 included, by era.
#[test]
fn sidereal_against_horizons() {
    use astrometry::sidereal::gast;
    use astrometry::time::ut1_from_tt;
    let mut worst = [0.0f64; 2];
    for r in data("horizons/sidereal.tsv") {
        let tt: f64 = r[0].parse().unwrap();
        let want: f64 = r[1].parse().unwrap();
        let got = gast(ut1_from_tt(tt), tt).to_degrees() / 15.0;
        let d = ((got - want + 36.0).rem_euclid(24.0) - 12.0).abs() * 3600.0;
        let future = tt > 2461682.5; // after Bulletin A's last prediction of ΔT
        worst[usize::from(future)] = worst[usize::from(future)].max(d);
    }
    println!(
        "sidereal time against Horizons: observed era {:.4} s, after {:.3} s",
        worst[0], worst[1]
    );
    // UT1 interpolated where it's observed or predicted; held at its last prediction after, as
    // Horizons holds it (the two editions of Bulletin A differing, as they're fetched apart).
    assert!(
        worst[0] < 0.05 && worst[1] < 0.2,
        "{:.4} s, {:.3} s",
        worst[0],
        worst[1]
    );
}

/// Sunrise and sunset against Horizons, which gives them to the minute.
#[test]
fn rise_set_against_horizons() {
    use astrometry::horizon::{SUNRISE, sun_crossings};
    use astrometry::time::{DAY, civil_from_tt, julian_date, tt_from_civil};
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut worst = 0.0f64;
    let rows = data("horizons/rise-set.tsv");
    for r in rows.iter().filter(|r| r[2] != "transit") {
        let (lon, lat) = r[1].split_once(',').unwrap();
        let (lon, lat): (f64, f64) = (lon.parse().unwrap(), lat.parse().unwrap());
        // "2026-Oct-15 12:12:00"
        let (date, clock) = r[3].split_once(' ').unwrap();
        let d: Vec<&str> = date.split('-').collect();
        let m = months.iter().position(|x| *x == d[1]).unwrap() as u8 + 1;
        let c: Vec<u8> = clock.split(':').map(|x| x.parse().unwrap()).collect();
        let want = julian_date(
            d[0].parse().unwrap(),
            m,
            d[2].parse().unwrap(),
            c[0],
            c[1],
            0.0,
        );
        let tt = tt_from_civil(want);
        let found = sun_crossings(
            &Builtin,
            tt - 0.25,
            tt + 0.25,
            lat.to_radians(),
            lon.to_radians(),
            SUNRISE,
        )
        .unwrap();
        let near = found
            .iter()
            .filter(|x| x.rising == (r[2] == "rise"))
            .min_by(|a, b| (a.tt - tt).abs().total_cmp(&(b.tt - tt).abs()));
        let got = civil_from_tt(near.expect("a crossing").tt);
        // Seconds from the start of Horizons' minute.
        let off = (got - want) * DAY;
        // Horizons gives the first minute after the crossing; ours falls in the minute before,
        // to within a second (its refraction model against a fixed −0°50′).
        assert!(
            (-61.0..1.0).contains(&off),
            "{} {} {}: {off:+.1} s",
            r[0],
            r[2],
            r[3]
        );
        worst = worst.max(off.abs());
    }
    println!(
        "rise and set: within {worst:.1} s of the start of Horizons' minute ({} events)",
        rows.iter().filter(|r| r[2] != "transit").count()
    );

    // As many crossings as Horizons finds in each window, polar days and nights included.
    let windows = [
        (
            "Madison",
            -89.4012f64,
            43.0731f64,
            (2026, 10, 14),
            (2026, 10, 17),
        ),
        (
            "Springfield",
            -89.64371,
            39.80172,
            (2026, 10, 14),
            (2026, 10, 17),
        ),
        ("Honolulu", -157.85833, 21.30694, (2026, 7, 1), (2026, 7, 4)),
        ("Utqiagvik", -156.7886, 71.2906, (2026, 5, 8), (2026, 5, 14)),
        (
            "Utqiagvik",
            -156.7886,
            71.2906,
            (2026, 11, 14),
            (2026, 11, 20),
        ),
        ("Madison", -89.4012, 43.0731, (1850, 3, 19), (1850, 3, 22)),
        ("Madison", -89.4012, 43.0731, (2150, 12, 20), (2150, 12, 23)),
    ];
    for (name, lon, lat, (y0, m0, d0), (y1, m1, d1)) in windows {
        let (from, to) = (
            julian_date(y0, m0, d0, 0, 0, 0.0),
            julian_date(y1, m1, d1, 0, 0, 0.0),
        );
        let month = &months[m0 as usize - 1];
        let want = rows
            .iter()
            .filter(|r| {
                r[0] == name && r[2] != "transit" && r[3].starts_with(&format!("{y0}-{month}"))
            })
            .count();
        let got = sun_crossings(
            &Builtin,
            tt_from_civil(from),
            tt_from_civil(to),
            lat.to_radians(),
            lon.to_radians(),
            SUNRISE,
        )
        .unwrap()
        .len();
        assert_eq!(got, want, "{name} {y0}-{m0}-{d0}");
    }
}

/// Horizons' time `r` ("2026-Oct-15 12:12:00") as a Julian date, UT.
fn horizons_time(r: &str) -> f64 {
    use astrometry::time::julian_date;
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let (date, clock) = r.split_once(' ').unwrap();
    let d: Vec<&str> = date.split('-').collect();
    let m = months.iter().position(|x| *x == d[1]).unwrap() as u8 + 1;
    let c: Vec<u8> = clock.split(':').map(|x| x.parse().unwrap()).collect();
    julian_date(
        d[0].parse().unwrap(),
        m,
        d[2].parse().unwrap(),
        c[0],
        c[1],
        0.0,
    )
}

/// The Sun's upper transits against Horizons, which gives them to the minute.
#[test]
fn upper_transits_against_horizons() {
    use astrometry::horizon::sun_transits;
    use astrometry::time::{DAY, civil_from_tt, tt_from_civil};
    let mut worst = (f64::MAX, f64::MIN);
    let rows = data("horizons/rise-set.tsv");
    let rows: Vec<_> = rows.iter().filter(|r| r[2] == "transit").collect();
    assert!(rows.len() > 20, "{} transits", rows.len());
    for r in &rows {
        let (lon, _) = r[1].split_once(',').unwrap();
        let lon: f64 = lon.parse().unwrap();
        let want = horizons_time(&r[3]);
        let tt = tt_from_civil(want);
        let found = sun_transits(&Builtin, tt - 0.25, tt + 0.25, lon.to_radians()).unwrap();
        let near = found
            .iter()
            .filter(|x| x.upper)
            .min_by(|a, b| (a.tt - tt).abs().total_cmp(&(b.tt - tt).abs()));
        let off = (civil_from_tt(near.expect("a transit").tt) - want) * DAY;
        // Horizons gives the first minute after the transit; ours falls in the minute before.
        assert!(
            (-61.0..1.0).contains(&off),
            "{} {}: {off:+.1} s",
            r[0],
            r[3]
        );
        worst = (worst.0.min(off), worst.1.max(off));
    }
    println!(
        "upper transits: {:+.1} to {:+.1} s from the start of Horizons' minute ({} events)",
        worst.0,
        worst.1,
        rows.len()
    );
}

/// At a lower transit the hour angle is π, to well under a second of time, and it falls about
/// midway between the upper ones.
#[test]
fn lower_transits() {
    use astrometry::horizon::{hour_angle, sun_transits};
    use astrometry::time::{DAY, julian_date, tt_from_civil};
    let from = tt_from_civil(julian_date(2026, 10, 14, 0, 0, 0.0));
    for lon in [-89.4012f64, 18.96, 139.69, -157.86] {
        let east = lon.to_radians();
        let all = sun_transits(&Builtin, from, from + 6.0, east).unwrap();
        assert!(all.windows(2).all(|w| w[0].upper != w[1].upper), "{all:?}");
        let lower: Vec<_> = all.iter().filter(|x| !x.upper).collect();
        assert!(lower.len() >= 5, "{lon}: {all:?}");
        for x in &lower {
            let h = hour_angle(&Builtin, Body::Sun, x.tt, east).unwrap();
            // The Earth turns 2π a day: a second of time is 7.3e-5 rad.
            let off = (h.abs() - std::f64::consts::PI) * DAY / std::f64::consts::TAU;
            assert!(off.abs() < 0.5, "{lon}: {off:.3} s");
        }
        let upper: Vec<_> = all.iter().filter(|x| x.upper).collect();
        for l in &lower {
            let (Some(a), Some(b)) = (
                upper.iter().rev().find(|u| u.tt < l.tt),
                upper.iter().find(|u| u.tt > l.tt),
            ) else {
                continue;
            };
            // The equation of time moves the transits by a few seconds a day.
            let off = (l.tt - (a.tt + b.tt) / 2.0) * DAY;
            assert!(off.abs() < 30.0, "{lon}: {off:.1} s from the midpoint");
        }
    }
}

/// New and full moons, and the Moon's ingresses, against the Swiss Ephemeris' (JPL mode).
#[test]
fn events_against_swiss_ephemeris() {
    use astrometry::events::{full_moons, new_moons, passes};
    let rows: Vec<Vec<String>> = {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/swisseph/positions.tsv");
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .filter(|l| l.starts_with("event\t"))
            .map(|l| l.split('\t').map(str::to_string).collect())
            .collect()
    };
    let mut worst = 0.0f64;
    for r in &rows {
        let want: f64 = r[2].parse().unwrap();
        let (from, to) = (want - 1.0, want + 1.0);
        let found = match r[1].as_str() {
            "new" => new_moons(&Builtin, from, to).unwrap(),
            "full" => full_moons(&Builtin, from, to).unwrap(),
            ingress => {
                let k: f64 = ingress.trim_start_matches("ingress").parse().unwrap();
                passes(&Builtin, Body::Moon, 30.0 * k, from, to, 0.25).unwrap()
            }
        };
        assert_eq!(found.len(), 1, "{} near {want}: {found:?}", r[1]);
        worst = worst.max((found[0] - want).abs() * 86400.0);
    }
    println!(
        "{} events within {worst:.3} s of the Swiss Ephemeris'",
        rows.len()
    );
    assert!(worst < 0.5, "{worst:.3} s");
}
