//! Moments as the ephemeris counts them, TT Julian dates, and back.

use astrometry::time::{self, DAY};
use jiff::Timestamp;
use jiff::Zoned;
use jiff::tz::TimeZone;

/// The Julian date of the Unix epoch.
const UNIX_EPOCH: f64 = 2440587.5;

/// The TT Julian date of a moment.
pub fn tt(t: Timestamp) -> f64 {
    let civil = UNIX_EPOCH + (t.as_second() as f64 + f64::from(t.subsec_nanosecond()) * 1e-9) / DAY;
    time::tt_from_civil(civil)
}

/// The moment of a TT Julian date in `tz`, to the second.
pub fn zoned(tt: f64, tz: &TimeZone) -> Zoned {
    let secs = ((time::civil_from_tt(tt) - UNIX_EPOCH) * DAY).round() as i64;
    Timestamp::from_second(secs)
        .expect("in range")
        .to_zoned(tz.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let t = Timestamp::from_second(1_792_000_000).unwrap();
        assert_eq!(zoned(tt(t), &TimeZone::UTC).timestamp(), t);
    }

    // TT runs 69.184 s ahead of UTC since the leap second of 2017.
    #[test]
    fn tt_ahead_of_utc() {
        let t = Timestamp::from_second(1_792_000_000).unwrap();
        let civil = UNIX_EPOCH + t.as_second() as f64 / DAY;
        assert!(((tt(t) - civil) * DAY - 69.184).abs() < 1e-3);
    }
}
