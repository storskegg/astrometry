//! Time: Julian dates, the Gregorian calendar, and the time scales an ephemeris needs.
//!
//! - **TT**, Terrestrial Time, the timescale of the ephemeris arguments; **TDB** differs from it
//!   by under 2 ms, periodically.
//! - **UT1**, the Earth's rotation, for sidereal time and the horizon; **ΔT** is TT − UT1.
//! - **Civil time**, the time on clocks, as JPL Horizons reads it: UTC from 1962-01-19 12:00,
//!   with its leap seconds (and before 1972 its drifting offsets); UT1 before then, when clocks
//!   kept mean solar time. A clock time maps to TT exactly from 1962; before, through ΔT.
//!
//! Julian dates are `f64` days: about 20 µs of resolution in our era, far below anything an
//! ephemeris of the Sun, Moon and planets can show.

use std::sync::LazyLock;

/// The Julian date of J2000.0, 2000-01-01 12:00 TT.
pub const J2000: f64 = 2451545.0;

/// Days in a Julian century.
pub const CENTURY: f64 = 36525.0;

/// Seconds in a day.
pub const DAY: f64 = 86400.0;

/// The Julian date of JPL Horizons' switch from UT1 to UTC for clock times: 1962-01-19 12:00.
pub const UTC_FROM: f64 = 2437684.0;

/// TT − TAI, seconds.
const TT_TAI: f64 = 32.184;

/// The Julian date of a moment of the proleptic Gregorian calendar, `hour`, `minute` and
/// `second` included.
pub fn julian_date(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: f64) -> f64 {
    let (y, m) = if month <= 2 {
        (year - 1, i32::from(month) + 12)
    } else {
        (year, i32::from(month))
    };
    let a = y.div_euclid(100);
    let b = 2 - a + a.div_euclid(4);
    let days = (365.25 * f64::from(y + 4716)).floor()
        + (30.6001 * f64::from(m + 1)).floor()
        + f64::from(day)
        + f64::from(b)
        - 1524.5;
    days + (f64::from(hour) * 3600.0 + f64::from(minute) * 60.0 + second) / DAY
}

/// The proleptic Gregorian calendar date and time of a Julian date: year, month, day, and the
/// seconds since midnight.
pub fn calendar(jd: f64) -> (i32, u8, u8, f64) {
    // Meeus, Astronomical Algorithms, chapter 7.
    let z = (jd + 0.5).floor();
    let f = jd + 0.5 - z;
    let alpha = ((z - 1867216.25) / 36524.25).floor();
    let a = z + 1.0 + alpha - (alpha / 4.0).floor();
    let b = a + 1524.0;
    let c = ((b - 122.1) / 365.25).floor();
    let d = (365.25 * c).floor();
    let e = ((b - d) / 30.6001).floor();
    let day = b - d - (30.6001 * e).floor();
    let month = if e < 14.0 { e - 1.0 } else { e - 13.0 };
    let year = if month > 2.0 { c - 4716.0 } else { c - 4715.0 };
    (year as i32, month as u8, day as u8, f * DAY)
}

/// Julian centuries of TT (or TDB) since J2000.
pub fn centuries(jd: f64) -> f64 {
    (jd - J2000) / CENTURY
}

/// TDB − TT in seconds at TT Julian date `jd`: the series of USNO Circular 179 (eq. 2.6, after
/// Fairhead and Bretagnon), good to about 10 µs from 1600 to 2200.
pub fn tdb_minus_tt(jd: f64) -> f64 {
    let t = centuries(jd);
    0.001657 * (628.3076 * t + 6.2401).sin()
        + 0.000022 * (575.3385 * t + 4.2970).sin()
        + 0.000014 * (1256.6152 * t + 6.1969).sin()
        + 0.000005 * (606.9777 * t + 4.0212).sin()
        + 0.000005 * (52.9691 * t + 0.4444).sin()
        + 0.000002 * (21.3299 * t + 5.5431).sin()
        + 0.000010 * t * (628.3076 * t + 4.2490).sin()
}

/// UTC's offsets from TAI before whole leap seconds, as USNO's tai-utc.dat gives them: from the
/// Julian date, TAI − UTC = a + (MJD − mjd) × rate seconds.
const DRIFTING: [(f64, f64, f64, f64); 13] = [
    (2437300.5, 1.4228180, 37300.0, 0.001296),
    (2437512.5, 1.3728180, 37300.0, 0.001296),
    (2437665.5, 1.8458580, 37665.0, 0.0011232),
    (2438334.5, 1.9458580, 37665.0, 0.0011232),
    (2438395.5, 3.2401300, 38761.0, 0.001296),
    (2438486.5, 3.3401300, 38761.0, 0.001296),
    (2438639.5, 3.4401300, 38761.0, 0.001296),
    (2438761.5, 3.5401300, 38761.0, 0.001296),
    (2438820.5, 3.6401300, 38761.0, 0.001296),
    (2438942.5, 3.7401300, 38761.0, 0.001296),
    (2439004.5, 3.8401300, 38761.0, 0.001296),
    (2439126.5, 4.3131700, 39126.0, 0.002592),
    (2439887.5, 4.2131700, 39126.0, 0.002592),
];

/// The Julian date of 1972-01-01, when UTC began whole leap seconds.
const LEAP_FROM: f64 = 2441317.5;

/// The leap seconds from the IANA list: from each Julian date, TAI − UTC. Its expiry date is
/// kept for reference; after the last leap second, none are assumed (by the CGPM's 2022
/// resolution, none are expected after 2035).
struct LeapSeconds {
    steps: Vec<(f64, f64)>,
    #[allow(dead_code)]
    expires: f64,
}

static LEAP_SECONDS: LazyLock<LeapSeconds> = LazyLock::new(|| {
    // NTP seconds count from 1900-01-01 00:00 UTC.
    const NTP_EPOCH: f64 = 2415020.5;
    let mut steps = Vec::new();
    let mut expires = 0.0;
    for line in include_str!("../data/leap-seconds.list").lines() {
        if let Some(e) = line.strip_prefix("#@") {
            expires = NTP_EPOCH + e.trim().parse::<f64>().expect("expiry") / DAY;
        }
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let mut f = line.split_whitespace();
        let ntp: f64 = f.next().and_then(|s| s.parse().ok()).expect("NTP seconds");
        let offset: f64 = f.next().and_then(|s| s.parse().ok()).expect("TAI - UTC");
        steps.push((NTP_EPOCH + ntp / DAY, offset));
    }
    LeapSeconds { steps, expires }
});

/// TAI − UTC in seconds at UTC Julian date `jd`, from UTC's start in 1961; 0 before.
pub fn tai_minus_utc(jd: f64) -> f64 {
    if jd >= LEAP_FROM {
        let steps = &LEAP_SECONDS.steps;
        let i = steps.partition_point(|(from, _)| *from <= jd);
        return steps[i.saturating_sub(1)].1;
    }
    match DRIFTING.iter().rev().find(|(from, ..)| *from <= jd) {
        Some((_, a, mjd, rate)) => a + (jd - 2400000.5 - mjd) * rate,
        None => 0.0,
    }
}

/// The TT Julian date of a civil (clock) Julian date: UTC from 1962-01-19 12:00, UT1 before,
/// as JPL Horizons reads clock times.
pub fn tt_from_civil(jd: f64) -> f64 {
    if jd >= UTC_FROM {
        jd + (tai_minus_utc(jd) + TT_TAI) / DAY
    } else {
        jd + delta_t(jd) / DAY
    }
}

/// The civil (clock) Julian date of a TT Julian date: the inverse of [`tt_from_civil`].
pub fn civil_from_tt(tt: f64) -> f64 {
    let mut jd = tt - 69.0 / DAY;
    for _ in 0..4 {
        jd += tt - tt_from_civil(jd);
    }
    jd
}

/// The UT1 Julian date of a TT Julian date.
pub fn ut1_from_tt(tt: f64) -> f64 {
    let mut ut1 = tt - delta_t(tt) / DAY;
    ut1 = tt - delta_t(ut1) / DAY;
    tt - delta_t(ut1) / DAY
}

/// ΔT tables, as (UT1 Julian date, TT − UT1 seconds), in order.
struct Table(Vec<(f64, f64)>);

impl Table {
    fn parse(text: &str, to_tt: impl Fn(f64, f64) -> f64) -> Table {
        let rows = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let mut f = l.split('\t');
                let jd: f64 = f.next().and_then(|s| s.parse().ok()).expect("Julian date");
                let v: f64 = f
                    .next()
                    .and_then(|s| s.trim().parse().ok())
                    .expect("seconds");
                (jd, to_tt(jd, v))
            })
            .collect();
        Table(rows)
    }

    /// Linear interpolation, or `None` outside the table.
    fn at(&self, jd: f64) -> Option<f64> {
        let rows = &self.0;
        let i = rows.partition_point(|(t, _)| *t <= jd);
        if i == 0 || (i == rows.len() && jd > rows[rows.len() - 1].0) {
            return None;
        }
        if i == rows.len() {
            return Some(rows[i - 1].1);
        }
        let ((t0, v0), (t1, v1)) = (rows[i - 1], rows[i]);
        Some(v0 + (v1 - v0) * (jd - t0) / (t1 - t0))
    }

    fn first(&self) -> (f64, f64) {
        self.0[0]
    }

    fn last(&self) -> (f64, f64) {
        self.0[self.0.len() - 1]
    }
}

/// Horizons' ΔT to its switch to UTC: its TDB − UT1, less TDB − TT.
static HORIZONS: LazyLock<Table> = LazyLock::new(|| {
    Table::parse(
        include_str!("../data/horizons-tdb-ut1.tsv"),
        |jd, tdb_ut1| tdb_ut1 - tdb_minus_tt(jd + tdb_ut1 / DAY),
    )
});

/// IERS's observed ΔT from 1962: its UT1 − UTC, as TT − UT1.
static IERS: LazyLock<Table> = LazyLock::new(|| {
    Table::parse(include_str!("../data/iers-ut1-utc.tsv"), |jd, ut1_utc| {
        TT_TAI + tai_minus_utc(jd) - ut1_utc
    })
});

/// USNO's observed ΔT after the IERS series ends.
static USNO: LazyLock<Table> =
    LazyLock::new(|| Table::parse(include_str!("../data/usno-tt-ut1.tsv"), |_, tt_ut1| tt_ut1));

/// The IERS's Bulletin A after USNO's series: its rapid values, then its predictions a year
/// ahead, daily, as TT − UT1.
static BULLETIN_A: LazyLock<Table> = LazyLock::new(|| {
    Table::parse(
        include_str!("../data/iers-bulletin-a.tsv"),
        |jd, ut1_utc| TT_TAI + tai_minus_utc(jd) - ut1_utc,
    )
});

/// ΔT, TT − UT1, in seconds at UT1 Julian date `jd`:
///
/// - before 1800, Espenak and Meeus's polynomials (NASA, Five Millennium Canon of Solar
///   Eclipses, 2006), eased over the 18th century into Horizons' value for 1800;
/// - from 1800 to Horizons' switch to UTC in January 1962, Horizons' own values, monthly, so
///   clock times before then mean what they mean to Horizons;
/// - from then, observed values: IERS's (EOP 20 C04), monthly, then USNO's, which run a
///   little later, then the IERS's rapid values (Bulletin A), daily;
/// - then Bulletin A's predictions, daily for a year;
/// - beyond the last prediction, held at it, as Horizons holds it.
pub fn delta_t(jd: f64) -> f64 {
    let (h0, v0) = HORIZONS.first();
    if jd < h0 {
        let y = 2000.0 + (jd - J2000) / 365.25;
        let ease = ((y - 1700.0) / 100.0).clamp(0.0, 1.0);
        return espenak_meeus(y) + ease * (v0 - espenak_meeus(1800.0));
    }
    if jd < UTC_FROM {
        // Horizons' last value is for 1962-01-01; it holds to the switch.
        return HORIZONS.at(jd).unwrap_or(HORIZONS.last().1);
    }
    if let Some(v) = IERS.at(jd).filter(|_| jd <= IERS.last().0) {
        return v;
    }
    if jd < IERS.first().0 {
        return IERS.first().1;
    }
    // USNO's, then Bulletin A's, each joined to the one before.
    let mut prev = IERS.last();
    for table in [&*USNO, &*BULLETIN_A] {
        let first = table.first();
        if jd < first.0 {
            return prev.1 + (first.1 - prev.1) * (jd - prev.0) / (first.0 - prev.0);
        }
        if let Some(v) = table.at(jd).filter(|_| jd <= table.last().0) {
            return v;
        }
        prev = table.last();
    }
    prev.1
}

/// Espenak and Meeus's ΔT polynomials before 1800, for decimal year `y`.
fn espenak_meeus(y: f64) -> f64 {
    if y >= 1700.0 {
        let t = y - 1700.0;
        8.83 + 0.1603 * t - 0.0059285 * t * t + 0.00013336 * t.powi(3) - t.powi(4) / 1174000.0
    } else if y >= 1600.0 {
        let t = y - 1600.0;
        120.0 - 0.9808 * t - 0.01532 * t * t + t.powi(3) / 7129.0
    } else if y >= 500.0 {
        let u = (y - 1000.0) / 100.0;
        1574.2 - 556.01 * u + 71.23472 * u * u + 0.319781 * u.powi(3)
            - 0.8503463 * u.powi(4)
            - 0.005050998 * u.powi(5)
            + 0.0083572073 * u.powi(6)
    } else if y >= -500.0 {
        let u = y / 100.0;
        10583.6 - 1014.41 * u + 33.78311 * u * u - 5.952053 * u.powi(3) - 0.1798452 * u.powi(4)
            + 0.022174192 * u.powi(5)
            + 0.0090316521 * u.powi(6)
    } else {
        let u = (y - 1820.0) / 100.0;
        -20.0 + 32.0 * u * u
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn julian_dates() {
        assert_eq!(julian_date(2000, 1, 1, 12, 0, 0.0), J2000);
        assert_eq!(julian_date(1962, 1, 19, 12, 0, 0.0), UTC_FROM);
        assert_eq!(julian_date(1800, 1, 1, 0, 0, 0.0), 2378496.5);
        assert_eq!(
            julian_date(1582, 10, 15, 0, 0, 0.0),
            2299160.5,
            "the first Gregorian day"
        );
        for jd in [
            2378496.5,
            2415020.5,
            2451544.5,
            2461329.259722222,
            2524593.5,
        ] {
            let (y, m, d, s) = calendar(jd);
            let back = julian_date(y, m, d, 0, 0, 0.0) + s / DAY;
            assert!(
                (back - jd).abs() < 1e-8,
                "{jd}: {y}-{m}-{d} {s} gives {back}"
            );
        }
        assert_eq!(calendar(2461329.5).0..=calendar(2461329.5).0, 2026..=2026);
    }

    #[test]
    fn tai_utc() {
        assert_eq!(tai_minus_utc(2437000.0), 0.0, "before UTC");
        assert!(
            (tai_minus_utc(2437665.5) - 1.8458580).abs() < 1e-9,
            "1962-01-01"
        );
        assert!(
            (tai_minus_utc(2441317.0) - (4.2131700 + (41316.5 - 39126.0) * 0.002592)).abs() < 1e-9
        );
        assert_eq!(tai_minus_utc(LEAP_FROM), 10.0);
        assert_eq!(tai_minus_utc(julian_date(1999, 6, 1, 0, 0, 0.0)), 32.0);
        assert_eq!(tai_minus_utc(julian_date(2016, 12, 31, 23, 59, 59.0)), 36.0);
        assert_eq!(tai_minus_utc(julian_date(2017, 1, 1, 0, 0, 0.0)), 37.0);
        assert_eq!(
            tai_minus_utc(julian_date(2200, 1, 1, 0, 0, 0.0)),
            37.0,
            "no future leap seconds"
        );
    }

    #[test]
    fn civil_round_trip() {
        for jd in [
            2378496.5,
            2437000.25,
            UTC_FROM - 0.1,
            UTC_FROM + 0.1,
            2451545.0,
            2461329.26,
            2524593.5,
        ] {
            let back = civil_from_tt(tt_from_civil(jd));
            assert!((back - jd).abs() * DAY < 1e-5, "{jd}: {back}");
        }
    }

    #[test]
    fn delta_t_is_continuous_before_1800() {
        let (h0, v0) = HORIZONS.first();
        assert!(
            (delta_t(h0 - 1e-6) - v0).abs() < 0.01,
            "{} against {v0}",
            delta_t(h0 - 1e-6)
        );
        // Before 1700 the polynomials stand unchanged.
        let jd = julian_date(1650, 7, 1, 0, 0, 0.0);
        let y = 2000.0 + (jd - J2000) / 365.25;
        assert_eq!(delta_t(jd), espenak_meeus(y));
    }

    // Through the observed and predicted series: no step where one table hands on to the
    // next, and held after Bulletin A's last prediction.
    #[test]
    fn delta_t_is_continuous_after_1962() {
        let eps = 1e-6;
        for (name, t) in [("USNO", &*USNO), ("Bulletin A", &*BULLETIN_A)] {
            let (jd, v) = t.first();
            assert!((delta_t(jd - eps) - v).abs() < 0.01, "into {name}");
        }
        let (end, v) = BULLETIN_A.last();
        assert!(end - BULLETIN_A.first().0 > 300.0, "a year of predictions");
        assert_eq!(delta_t(end + 1.0), v);
        assert_eq!(delta_t(end + 36525.0), v);
    }
}
