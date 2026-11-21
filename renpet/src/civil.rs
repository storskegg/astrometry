//! The civil calendar: twelve months of thirty days and five epagomenal days, 365 days a year,
//! with no leap day, so that it is carried forward unbroken and passes through the seasons
//! once in 1,461 years (a Sothic cycle). Pure arithmetic on Julian Day Numbers, which are
//! integers counted at noon; the Egyptian day is taken to begin at sunrise, on the morning of
//! the Gregorian date whose JDN it is given.
//!
//! Two counts of years are kept. The Era of Nabonassar, whose year 1 begins on 1 Thoth =
//! JDN 1448638 (26 February 747 BCE Julian), the count of Ptolemy's *Almagest* (III.7); and
//! the Sothic cycles, whose cycle 1 begins on 1 Thoth = JDN 705498 (proleptic Gregorian
//! -2781-06-27). Dates before 2781 BCE fall in cycle 0 and the negative cycles; before
//! Nabonassar year 1, the Nabonassar year is 0 or negative.

use jiff::civil::Date;

use crate::Error;

/// JDN of 1 Thoth of Nabonassar year 1.
pub const NABONASSAR_JDN: i64 = 1_448_638;
/// JDN of 1 Thoth of Sothic cycle 1, year 1.
pub const SOTHIC_JDN: i64 = 705_498;
/// Days in a civil year.
const YEAR: i64 = 365;
/// Days in a Sothic cycle: 1,461 civil years.
const CYCLE: i64 = 1461 * YEAR;
/// The JDN of 1970-01-01 (proleptic Gregorian).
const UNIX_JDN: i64 = 2_440_588;

ids_enum!(
    /// The three seasons of four months each.
    Season,
    [
        /// The inundation.
        Akhet => ("season.akhet", "ꜣḫt"),
        /// The emergence, the growing.
        Peret => ("season.peret", "prt"),
        /// The harvest, the low water.
        Shemu => ("season.shemu", "šmw"),
    ]
);

ids_enum!(
    /// The twelve months, in order, four to a season. The transliterations are provisional:
    /// the Ramesside-period forms are to be confirmed. Month XII is wp-rnpt, its name in the
    /// Ramesside period: the Deir el-Medina sources give months I–XI their later names, but XII
    /// still wp-rnpt; mswt-rꜥ is then a feast's name, and the month's only later (J. A.
    /// Belmonte, "Some open questions on the Egyptian calendar", *Trabajos de Egiptología* 2,
    /// 2003, Table 1, p. 27, and p. 36). The id keeps the later name.
    Month,
    [
        /// Provisional: the Ramesside-period forms are to be confirmed.
        Djehuty => ("month.djehuty", "Ḏḥwtj"),
        /// Provisional: the Ramesside-period forms are to be confirmed.
        PaenIpet => ("month.paen_ipet", "pꜣ-n-jpt"),
        /// Provisional: the Ramesside-period forms are to be confirmed.
        Hathor => ("month.hathor", "Ḥwt-Ḥr"),
        /// Provisional: the Ramesside-period forms are to be confirmed.
        KaHerKa => ("month.ka_her_ka", "kꜣ-ḥr-kꜣ"),
        /// Provisional: the Ramesside-period forms are to be confirmed.
        TaAabet => ("month.ta_aabet", "tꜣ-ꜥꜣbt"),
        /// Provisional: the Ramesside-period forms are to be confirmed.
        PaenMekher => ("month.paen_mekher", "pꜣ-n-mḫr"),
        /// Provisional: the Ramesside-period forms are to be confirmed.
        PaenAmenhotep => ("month.paen_amenhotep", "pꜣ-n-jmn-ḥtp"),
        /// Provisional: the Ramesside-period forms are to be confirmed.
        PaenRenenutet => ("month.paen_renenutet", "pꜣ-n-rnnwtt"),
        /// Provisional: the Ramesside-period forms are to be confirmed.
        PaenKhonsu => ("month.paen_khonsu", "pꜣ-n-ḫnsw"),
        /// Provisional: the Ramesside-period forms are to be confirmed.
        PaenInet => ("month.paen_inet", "pꜣ-n-jnt"),
        /// Provisional: the Ramesside-period forms are to be confirmed.
        Ipip => ("month.ipip", "jpjp"),
        /// Provisional: the Ramesside-period forms are to be confirmed.
        MesutRa => ("month.mesut_ra", "wp-rnpt"),
    ]
);

impl Month {
    /// The season of the month: months 1-4 are Akhet, 5-8 Peret, 9-12 Shemu.
    pub fn season(self) -> Season {
        Season::ALL[self as usize / 4]
    }

    /// The month within its season, 1-4.
    pub fn of_season(self) -> u8 {
        (self as u8) % 4 + 1
    }
}

ids_enum!(
    /// The five days added to the year, each named for the god born on it.
    Epagomenal,
    [
        /// Osiris.
        Osiris => ("epagomenal.osiris", "Wsjr"),
        /// Horus.
        Horus => ("epagomenal.horus", "Ḥr"),
        /// Set.
        Set => ("epagomenal.set", "Stẖ"),
        /// Isis.
        Isis => ("epagomenal.isis", "ꜣst"),
        /// Nephthys.
        Nephthys => ("epagomenal.nephthys", "Nbt-ḥwt"),
    ]
);

/// A day of the civil year: of a month (1-30), or one of the five epagomenal days.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CivilDay {
    Month { month: Month, day: u8 },
    Epagomenal(Epagomenal),
}

impl CivilDay {
    /// A day of a month, 1-30.
    pub fn new_month(month: Month, day: u8) -> Result<Self, Error> {
        if (1..=30).contains(&day) {
            Ok(CivilDay::Month { month, day })
        } else {
            Err(Error::InvalidDay(month, day))
        }
    }

    /// The day of the year, 1-365.
    pub fn day_of_year(&self) -> u16 {
        match self {
            CivilDay::Month { month, day } => *month as u16 * 30 + u16::from(*day),
            CivilDay::Epagomenal(e) => 361 + *e as u16,
        }
    }

    /// The day of the year `n`, 1-365.
    pub fn from_day_of_year(n: u16) -> Result<Self, Error> {
        match n {
            1..=360 => {
                let i = usize::from(n - 1);
                Ok(CivilDay::Month {
                    month: Month::ALL[i / 30],
                    day: (i % 30) as u8 + 1,
                })
            }
            361..=365 => Ok(CivilDay::Epagomenal(Epagomenal::ALL[usize::from(n - 361)])),
            _ => Err(Error::InvalidDayOfYear(n)),
        }
    }

    /// The month within its season (1-4), the season and the day, as in "III Peret 9":
    /// `(3, Peret, 9)`. `None` on the epagomenal days.
    pub fn ordinal(&self) -> Option<(u8, Season, u8)> {
        match self {
            CivilDay::Month { month, day } => Some((month.of_season(), month.season(), *day)),
            CivilDay::Epagomenal(_) => None,
        }
    }

    /// The decade, 1-36, of ten days each. `None` on the epagomenal days.
    pub fn decade(&self) -> Option<u8> {
        match self {
            CivilDay::Month { .. } => Some(((self.day_of_year() - 1) / 10) as u8 + 1),
            CivilDay::Epagomenal(_) => None,
        }
    }
}

/// A year of the Sothic cycle: the cycle (cycle 1 begins on JDN 705498, so earlier dates are
/// in cycle 0 and below) and the year in it, 1-1461.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SothicYear {
    pub cycle: i32,
    pub year: u16,
}

/// A day of the civil calendar, in both counts of years.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CivilDate {
    pub day: CivilDay,
    pub year: SothicYear,
    /// The year of the Era of Nabonassar; 0 and below before 747 BCE.
    pub nabonassar: i64,
}

impl CivilDate {
    /// The Julian Day Number of the day's morning.
    pub fn jdn(&self) -> i64 {
        let doy = i64::from(self.day.day_of_year());
        NABONASSAR_JDN
            .saturating_add(self.nabonassar.saturating_sub(1).saturating_mul(YEAR))
            .saturating_add(doy - 1)
    }

    /// The Gregorian date (proleptic, as jiff's) whose morning this Egyptian day begins on.
    /// A date beyond jiff's years -9999 to 9999 (only from fields set by hand) gives the
    /// nearest limit.
    pub fn gregorian(&self) -> Date {
        let jdn = self.jdn();
        date_of_jdn(jdn).unwrap_or(if jdn < jdn_of(Date::MIN) {
            Date::MIN
        } else {
            Date::MAX
        })
    }

    /// The date of a year of the Sothic cycle and a day. Errors if the day isn't a day of the
    /// calendar, the year isn't 1-1461, or the date is beyond the Gregorian years -9999 to 9999.
    pub fn from_parts(year: SothicYear, day: CivilDay) -> Result<Self, Error> {
        if !(1..=1461).contains(&year.year) {
            return Err(Error::InvalidSothicYear(year.year));
        }
        if CivilDay::from_day_of_year(day.day_of_year()).ok() != Some(day) {
            return Err(match day {
                CivilDay::Month { month, day } => Error::InvalidDay(month, day),
                CivilDay::Epagomenal(_) => Error::InvalidDayOfYear(day.day_of_year()),
            });
        }
        let years = (i64::from(year.cycle) - 1) * 1461 + i64::from(year.year) - 1;
        let jdn = SOTHIC_JDN + years * YEAR + i64::from(day.day_of_year()) - 1;
        date_of_jdn(jdn)?;
        Ok(Self::from_jdn(jdn))
    }

    /// The date of a JDN, any integer.
    fn from_jdn(jdn: i64) -> Self {
        let from_anchor = jdn - NABONASSAR_JDN;
        let n = jdn - SOTHIC_JDN;
        CivilDate {
            day: day_of(from_anchor),
            year: SothicYear {
                cycle: (n.div_euclid(CYCLE) + 1) as i32,
                year: (n.rem_euclid(CYCLE) / YEAR) as u16 + 1,
            },
            nabonassar: from_anchor.div_euclid(YEAR) + 1,
        }
    }
}

/// The civil day `from_anchor` days after 1 Thoth of a year.
fn day_of(from_anchor: i64) -> CivilDay {
    let doy = from_anchor.rem_euclid(YEAR) as u16 + 1;
    match CivilDay::from_day_of_year(doy) {
        Ok(d) => d,
        // `doy` is 1-365 by construction.
        Err(_) => CivilDay::Epagomenal(Epagomenal::Nephthys),
    }
}

/// The Julian Day Number of a proleptic Gregorian date.
fn jdn_of(date: Date) -> i64 {
    let (y, m, d) = (
        i64::from(date.year()),
        i64::from(date.month()),
        i64::from(date.day()),
    );
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468 + UNIX_JDN
}

/// The proleptic Gregorian date of a Julian Day Number, if jiff holds it.
fn date_of_jdn(jdn: i64) -> Result<Date, Error> {
    if jdn < jdn_of(Date::MIN) || jdn > jdn_of(Date::MAX) {
        return Err(Error::OutOfRange);
    }
    let z = jdn - UNIX_JDN + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Date::new(y as i16, m as i8, d as i8).map_err(|_| Error::OutOfRange)
}

/// The Egyptian day that begins at sunrise on the morning of a Gregorian date. Pure
/// arithmetic, and never fails.
pub fn civil_date_of(date: Date) -> CivilDate {
    CivilDate::from_jdn(jdn_of(date))
}

/// The first Gregorian date strictly after `after` whose Egyptian day is `d`, and that day.
/// `d` should be a valid day (as [`CivilDay::new_month`] and [`CivilDay::from_day_of_year`]
/// give); the date is at most 365 days on, and at the end of jiff's range it stops at
/// 9999-12-31.
pub fn next(d: CivilDay, after: Date) -> (CivilDate, Date) {
    let target = i64::from(d.day_of_year() - 1).rem_euclid(YEAR);
    let jdn_after = jdn_of(after);
    let current = (jdn_after - NABONASSAR_JDN).rem_euclid(YEAR);
    let step = match (target - current).rem_euclid(YEAR) {
        0 => YEAR,
        s => s,
    };
    let date = CivilDate::from_jdn(jdn_after + step);
    (date, date.gregorian())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn parts(d: &CivilDate) -> (i64, i32, u16, u16) {
        (d.nabonassar, d.year.cycle, d.year.year, d.day.day_of_year())
    }

    #[test]
    fn jdn_conversions() {
        assert_eq!(jdn_of(date(2000, 1, 1)), 2_451_545);
        assert_eq!(jdn_of(date(1970, 1, 1)), 2_440_588);
        assert_eq!(date_of_jdn(2_451_545).unwrap(), date(2000, 1, 1));
        assert!(date_of_jdn(jdn_of(Date::MAX) + 1).is_err());
        assert!(date_of_jdn(jdn_of(Date::MIN) - 1).is_err());
    }

    // Era of Nabonassar: Ptolemy, Almagest III.7. 1 Thoth of year 1 is 26 February 747 BCE
    // Julian, proleptic Gregorian -746-02-18.
    #[test]
    fn nabonassar_anchor() {
        let d = civil_date_of(date(-746, 2, 18));
        assert_eq!(jdn_of(date(-746, 2, 18)), 1_448_638);
        assert_eq!(d.nabonassar, 1);
        assert_eq!(
            d.day,
            CivilDay::Month {
                month: Month::Djehuty,
                day: 1
            }
        );
        assert_eq!(d.gregorian(), date(-746, 2, 18));
        // The day before is the last epagomenal day of year 0.
        let before = civil_date_of(date(-746, 2, 17));
        assert_eq!(before.nabonassar, 0);
        assert_eq!(before.day, CivilDay::Epagomenal(Epagomenal::Nephthys));
    }

    // Censorinus, De die natali 21.10: 1 Thoth fell on 20 July 139 CE Julian (proleptic
    // Gregorian 0139-07-19), JDN 1772028, the first day of a Sothic cycle by his count; year
    // 887 of Nabonassar.
    #[test]
    fn censorinus() {
        let d = civil_date_of(date(139, 7, 19));
        assert_eq!(jdn_of(date(139, 7, 19)), 1_772_028);
        assert_eq!(parts(&d), (887, 3, 1, 1));
    }

    // Canopus Decree, OGIS 56: 17 Ta-aabet (I Peret 17) of the 9th year of Ptolemy III,
    // 7 March 238 BCE Julian, proleptic Gregorian -237-03-03, JDN 1634559.
    #[test]
    fn canopus_decree() {
        let d = civil_date_of(date(-237, 3, 3));
        assert_eq!(jdn_of(date(-237, 3, 3)), 1_634_559);
        assert_eq!(
            d.day,
            CivilDay::Month {
                month: Month::TaAabet,
                day: 17
            }
        );
        assert_eq!(d.day.ordinal(), Some((1, Season::Peret, 17)));
    }

    #[test]
    fn today() {
        let d = civil_date_of(date(2026, 11, 21));
        assert_eq!(d.nabonassar, 2775);
        assert_eq!(
            d.day,
            CivilDay::Month {
                month: Month::PaenRenenutet,
                day: 9
            }
        );
        assert_eq!(d.day.ordinal(), Some((4, Season::Peret, 9)));
        assert_eq!(
            d.year,
            SothicYear {
                cycle: 4,
                year: 428
            }
        );
    }

    #[test]
    fn sothic_cycle_starts() {
        // JDN 2305293, proleptic Gregorian 1599-07-30: cycle 4 begins.
        let d = civil_date_of(date(1599, 7, 30));
        assert_eq!(jdn_of(date(1599, 7, 30)), 2_305_293);
        assert_eq!(d.year, SothicYear { cycle: 4, year: 1 });
        assert_eq!(d.day.day_of_year(), 1);
        // JDN 705498, proleptic Gregorian -2781-06-27: cycle 1 begins.
        let d = civil_date_of(date(-2781, 6, 27));
        assert_eq!(jdn_of(date(-2781, 6, 27)), 705_498);
        assert_eq!(d.year, SothicYear { cycle: 1, year: 1 });
        assert_eq!(d.day.day_of_year(), 1);
        // The day before is the last of cycle 0.
        let d = civil_date_of(date(-2781, 6, 26));
        assert_eq!(
            d.year,
            SothicYear {
                cycle: 0,
                year: 1461
            }
        );
        assert_eq!(d.day, CivilDay::Epagomenal(Epagomenal::Nephthys));
    }

    #[test]
    fn day_of_year_inverts() {
        for n in 1..=365 {
            assert_eq!(CivilDay::from_day_of_year(n).unwrap().day_of_year(), n);
        }
        assert!(CivilDay::from_day_of_year(0).is_err());
        assert!(CivilDay::from_day_of_year(366).is_err());
        assert!(CivilDay::new_month(Month::Ipip, 0).is_err());
        assert!(CivilDay::new_month(Month::Ipip, 31).is_err());
        assert_eq!(
            CivilDay::from_day_of_year(360).unwrap(),
            CivilDay::new_month(Month::MesutRa, 30).unwrap()
        );
        assert_eq!(
            CivilDay::from_day_of_year(361).unwrap(),
            CivilDay::Epagomenal(Epagomenal::Osiris)
        );
    }

    #[test]
    fn decades_and_seasons() {
        let day = |n| CivilDay::from_day_of_year(n).unwrap();
        assert_eq!(day(1).decade(), Some(1));
        assert_eq!(day(10).decade(), Some(1));
        assert_eq!(day(11).decade(), Some(2));
        assert_eq!(day(360).decade(), Some(36));
        assert_eq!(day(361).decade(), None);
        assert_eq!(day(365).ordinal(), None);
        assert_eq!(day(1).ordinal(), Some((1, Season::Akhet, 1)));
        assert_eq!(day(241).ordinal(), Some((1, Season::Shemu, 1)));
        assert_eq!(Month::PaenInet.season(), Season::Shemu);
        assert_eq!(Month::KaHerKa.season(), Season::Akhet);
        assert_eq!(Month::KaHerKa.of_season(), 4);
        assert_eq!(Month::TaAabet.of_season(), 1);
    }

    #[test]
    fn from_parts_inverts() {
        let d = civil_date_of(date(2026, 11, 21));
        assert_eq!(CivilDate::from_parts(d.year, d.day).unwrap(), d);
        let bad = SothicYear { cycle: 4, year: 0 };
        assert!(CivilDate::from_parts(bad, d.day).is_err());
        let bad = SothicYear {
            cycle: 4,
            year: 1462,
        };
        assert!(CivilDate::from_parts(bad, d.day).is_err());
        let day = CivilDay::Month {
            month: Month::Ipip,
            day: 31,
        };
        assert!(CivilDate::from_parts(d.year, day).is_err());
        let far = SothicYear {
            cycle: 1000,
            year: 1,
        };
        assert!(matches!(
            CivilDate::from_parts(far, d.day),
            Err(Error::OutOfRange)
        ));
    }

    #[test]
    fn round_trips() {
        let mut y = -9999;
        while y <= 9999 {
            for (m, dd) in [(1, 1), (2, 28), (3, 1), (7, 4), (12, 31)] {
                let d = date(y, m, dd);
                let c = civil_date_of(d);
                assert_eq!(c.gregorian(), d, "{d}");
                assert_eq!(CivilDate::from_parts(c.year, c.day).unwrap(), c, "{d}");
                assert_eq!(
                    CivilDay::from_day_of_year(c.day.day_of_year()).unwrap(),
                    c.day
                );
                // Each date is the next after the day before.
                let (n, g) = next(c.day, d.yesterday().unwrap_or(d));
                if d != Date::MIN {
                    assert_eq!((n, g), (c, d), "{d}");
                }
            }
            y += 37;
        }
        assert_eq!(civil_date_of(Date::MIN).gregorian(), Date::MIN);
        assert_eq!(civil_date_of(Date::MAX).gregorian(), Date::MAX);
    }

    #[test]
    fn next_is_after_and_within_a_year() {
        let after = date(2026, 11, 21);
        for n in 1..=365 {
            let day = CivilDay::from_day_of_year(n).unwrap();
            let (c, g) = next(day, after);
            assert!(g > after);
            assert!((after.until(g).unwrap().get_days()) <= 365);
            assert_eq!(c.day, day);
            assert_eq!(civil_date_of(g), c);
        }
        // On the day itself, a year on.
        let today = civil_date_of(after).day;
        assert_eq!(next(today, after).1, date(2027, 11, 21));
    }

    #[test]
    fn ids_and_orders() {
        assert_eq!(Month::ALL.len(), 12);
        assert_eq!(Month::ALL[7], Month::PaenRenenutet);
        assert_eq!(Season::Shemu.id(), "season.shemu");
    }
}
