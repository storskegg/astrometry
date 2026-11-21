//! Moments as the ephemeris counts them, TT Julian dates, and back.

use astrometry::time::{self, DAY};
use jiff::Timestamp;

use crate::Error;

/// The Julian date of the Unix epoch.
const UNIX_EPOCH: f64 = 2440587.5;

/// The TT Julian date of a moment.
pub fn tt(t: Timestamp) -> f64 {
    let civil = UNIX_EPOCH + (t.as_second() as f64 + f64::from(t.subsec_nanosecond()) * 1e-9) / DAY;
    time::tt_from_civil(civil)
}

/// The moment of a TT Julian date, to the second.
pub fn timestamp(tt: f64) -> Result<Timestamp, Error> {
    let secs = ((time::civil_from_tt(tt) - UNIX_EPOCH) * DAY).round();
    if !secs.is_finite() {
        return Err(Error::OutOfRange);
    }
    Timestamp::from_second(secs as i64).map_err(|_| Error::OutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let t = Timestamp::from_second(1_792_000_000).unwrap();
        assert_eq!(timestamp(tt(t)).unwrap(), t);
    }
}
