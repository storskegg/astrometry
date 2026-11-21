//! What the app built on renpet relies on, pinned: the enums' members, their order and ids
//! (the app takes members by index and keys its strings by id), the seasons' transliterations,
//! the anchors, and with the `serde` feature, the JSON shape of what it caches. A failure here
//! is a change that breaks the app, and needs ASTRO before it's tagged: not a test to update
//! quietly. (The months' transliterations are provisional, and not pinned.)

use renpet::civil::{NABONASSAR_JDN, SOTHIC_JDN};
use renpet::hours::Half;
use renpet::{Epagomenal, Month, Season};

fn ids<T: Copy>(all: &[T], f: impl Fn(T) -> &'static str) -> Vec<&'static str> {
    all.iter().map(|&m| f(m)).collect()
}

#[test]
fn enums_in_order() {
    assert_eq!(
        ids(&Season::ALL, |s| s.id()),
        ["season.akhet", "season.peret", "season.shemu"]
    );
    assert_eq!(
        ids(&Month::ALL, |m| m.id()),
        [
            "month.djehuty",
            "month.paen_ipet",
            "month.hathor",
            "month.ka_her_ka",
            "month.ta_aabet",
            "month.paen_mekher",
            "month.paen_amenhotep",
            "month.paen_renenutet",
            "month.paen_khonsu",
            "month.paen_inet",
            "month.ipip",
            "month.mesut_ra",
        ]
    );
    assert_eq!(
        ids(&Epagomenal::ALL, |e| e.id()),
        [
            "epagomenal.osiris",
            "epagomenal.horus",
            "epagomenal.set",
            "epagomenal.isis",
            "epagomenal.nephthys",
        ]
    );
    assert_eq!(ids(&Half::ALL, |h| h.id()), ["half.day", "half.night"]);
    // The members themselves, in order.
    assert_eq!(Month::ALL[0], Month::Djehuty);
    assert_eq!(Month::ALL[11], Month::MesutRa);
    assert_eq!(Epagomenal::ALL[4], Epagomenal::Nephthys);
}

#[test]
fn season_transliterations() {
    assert_eq!(ids(&Season::ALL, |s| s.translit()), ["ꜣḫt", "prt", "šmw"]);
}

#[test]
fn months_in_their_seasons() {
    for (i, m) in Month::ALL.iter().enumerate() {
        assert_eq!(m.season(), Season::ALL[i / 4]);
        assert_eq!(usize::from(m.of_season()), i % 4 + 1);
    }
}

#[test]
fn anchors() {
    assert_eq!(NABONASSAR_JDN, 1_448_638);
    assert_eq!(SOTHIC_JDN, 705_498);
}

#[cfg(feature = "serde")]
mod json {
    use jiff::Timestamp;
    use jiff::civil::date;
    use renpet::hours::Hour;
    use renpet::{CivilDate, civil_date_of};

    #[test]
    fn civil_date_shape() {
        let d = civil_date_of(date(2026, 11, 21));
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(
            json,
            r#"{"day":{"Month":{"month":"PaenRenenutet","day":9}},"year":{"cycle":4,"year":428},"nabonassar":2775}"#
        );
        assert_eq!(serde_json::from_str::<CivilDate>(&json).unwrap(), d);
        let e = renpet::CivilDay::Epagomenal(renpet::Epagomenal::Osiris);
        assert_eq!(
            serde_json::to_string(&e).unwrap(),
            r#"{"Epagomenal":"Osiris"}"#
        );
    }

    #[test]
    fn hour_shape() {
        let h = Hour {
            half: renpet::hours::Half::Night,
            n: 12,
            start: Timestamp::from_second(1_700_000_000).unwrap(),
            end: Timestamp::from_second(1_700_003_600).unwrap(),
        };
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(
            json,
            r#"{"half":"Night","n":12,"start":"2023-11-14T22:13:20Z","end":"2023-11-14T23:13:20Z"}"#
        );
        assert_eq!(serde_json::from_str::<Hour>(&json).unwrap(), h);
    }
}
