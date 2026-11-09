//! What the apps built on horae rely on, pinned: the enums' members, their order, names,
//! symbols and abbreviations (the apps take members by index and show the names), and with the
//! `serde` feature, the JSON shape of what they cache. A failure here is a change they must be
//! told of before it's tagged, not a test to update quietly.

use horae::{Dignity, Direction, Element, Kind, Phase, Planet, Point, Resh, Sign, Solar, Tattva};

/// Each member as "name symbol abbreviation", in order, `-` where there's none.
fn rows<T: Copy>(all: &[T], f: impl Fn(T) -> [&'static str; 3]) -> Vec<String> {
    all.iter().map(|&m| f(m).join(" ")).collect()
}

// Older caches hold the label in its former case; parsing ignores case.
#[test]
fn solar_old_label_parses() {
    assert_eq!("Under the beams".parse::<Solar>(), Ok(Solar::UnderTheBeams));
}

#[test]
fn enums_in_order() {
    let tests: [(&str, Vec<String>, &[&str]); 11] = [
        (
            "Planet",
            rows(&Planet::ALL, |p| [p.name(), p.symbol(), p.abbreviation()]),
            &[
                "Sun ☉ Su",
                "Moon ☽ Mo",
                "Mercury ☿ Me",
                "Venus ♀ Ve",
                "Mars ♂ Ma",
                "Jupiter ♃ Ju",
                "Saturn ♄ Sa",
            ],
        ),
        (
            "Point",
            rows(&Point::ALL, |p| [p.name(), p.symbol(), p.abbreviation()]),
            &[
                "Sun ☉ Su",
                "Moon ☽ Mo",
                "Mercury ☿ Me",
                "Venus ♀ Ve",
                "Mars ♂ Ma",
                "Jupiter ♃ Ju",
                "Saturn ♄ Sa",
                "North Node ☊ NN",
                "South Node ☋ SN",
            ],
        ),
        (
            "Sign",
            rows(&Sign::ALL, |s| [s.name(), s.symbol(), s.abbreviation()]),
            &[
                "Aries ♈ Ar",
                "Taurus ♉ Ta",
                "Gemini ♊ Ge",
                "Cancer ♋ Cn",
                "Leo ♌ Le",
                "Virgo ♍ Vi",
                "Libra ♎ Li",
                "Scorpio ♏ Sc",
                "Sagittarius ♐ Sg",
                "Capricorn ♑ Cp",
                "Aquarius ♒ Aq",
                "Pisces ♓ Pi",
            ],
        ),
        (
            "Element",
            rows(&Element::ALL, |e| [e.name(), e.symbol(), "-"]),
            &["Fire 🜂 -", "Air 🜁 -", "Water 🜄 -", "Earth 🜃 -"],
        ),
        (
            "Kind",
            rows(&Kind::ALL, |k| [k.name(), k.symbol(), "-"]),
            &[
                "Conjunction ☌ -",
                "Sextile ⚹ -",
                "Square □ -",
                "Trine △ -",
                "Opposition ☍ -",
            ],
        ),
        (
            "Phase",
            rows(&Phase::ALL, |p| [p.name(), "-", "-"]),
            &[
                "New - -",
                "Crescent - -",
                "First Quarter - -",
                "Gibbous - -",
                "Full - -",
                "Disseminating - -",
                "Last Quarter - -",
                "Balsamic - -",
            ],
        ),
        (
            "Dignity",
            rows(&Dignity::ALL, |d| [d.name(), "-", "-"]),
            &[
                "Domicile - -",
                "Exaltation - -",
                "Detriment - -",
                "Fall - -",
                "Peregrine - -",
            ],
        ),
        (
            // geomancy maps an Option<Solar> to its own enum at index + 1.
            "Solar",
            rows(&Solar::ALL, |s| [s.name(), "-", "-"]),
            &["Cazimi - -", "Combust - -", "Under the Beams - -"],
        ),
        (
            "Direction",
            rows(&Direction::ALL, |d| [d.name(), "-", "-"]),
            &["Direct - -", "Retrograde - -"],
        ),
        (
            "Resh",
            rows(&Resh::ALL, |r| [r.name(), "-", "-"]),
            &["Dawn - -", "Noon - -", "Sunset - -", "Midnight - -"],
        ),
        (
            // The tides' order.
            "Tattva",
            rows(&Tattva::ALL, |t| [t.name(), "-", "-"]),
            &[
                "Spirit - -",
                "Air - -",
                "Fire - -",
                "Water - -",
                "Earth - -",
            ],
        ),
    ];
    for (name, got, want) in tests {
        assert_eq!(got, want, "{name}'s members");
    }
    let latin: Vec<_> = Point::ALL.iter().map(|p| p.latin_name()).collect();
    assert_eq!(latin[7..], ["Caput Draconis", "Cauda Draconis"]);
    // A planet's index is its point's.
    assert!(
        Planet::ALL
            .iter()
            .all(|&p| Point::from(p).index() == p.index())
    );
}

#[test]
fn chart_points_in_order() {
    use horae::chart::Point;
    let got = rows(&Point::ALL, |p| [p.name(), p.symbol(), "-"]);
    assert_eq!(
        got,
        [
            "Sun ☉ -",
            "Moon ☽ -",
            "Mercury ☿ -",
            "Venus ♀ -",
            "Mars ♂ -",
            "Jupiter ♃ -",
            "Saturn ♄ -",
            "Uranus ♅ -",
            "Neptune ♆ -",
            "Pluto ♇ -",
            "North Node ☊ -",
            "South Node ☋ -",
            "Mean North Node ☊ -",
            "Mean South Node ☋ -",
            "Lilith ⚸ -",
            "Ascendant Asc -",
            "Midheaven MC -",
            "Descendant Dsc -",
            "Imum Coeli IC -",
        ]
    );
    // The shared points keep their symbols.
    for p in horae::Point::ALL {
        assert_eq!(Point::from(p).symbol(), p.symbol());
    }
}

#[test]
fn chart_kinds_in_order() {
    use horae::chart::Kind;
    let got: Vec<String> = Kind::ALL
        .iter()
        .map(|k| format!("{} {:.4} {}", k.name(), k.angle(), k.symbol()))
        .collect();
    assert_eq!(
        got,
        [
            "Conjunction 0.0000 ☌",
            "Sextile 60.0000 ⚹",
            "Square 90.0000 □",
            "Trine 120.0000 △",
            "Opposition 180.0000 ☍",
            "Semi-sextile 30.0000 ⚺",
            "Semi-square 45.0000 ∠",
            "Sesquiquadrate 135.0000 ⚼",
            "Quincunx 150.0000 ⚻",
            "Quintile 72.0000 Q",
            "Bi-quintile 144.0000 bQ",
            "Septile 51.4286 S",
            "Novile 40.0000 N",
        ]
    );
    assert_eq!("semi-square".parse::<Kind>(), Ok(Kind::SemiSquare));
    assert_eq!("BI-QUINTILE".parse::<Kind>(), Ok(Kind::BiQuintile));
}

#[cfg(feature = "serde")]
#[test]
fn chart_json() {
    use horae::chart::{Aspect, Kind, Point, Position};
    let position = Position {
        point: Point::ImumCoeli,
        longitude: 12.5,
        speed: -0.25,
    };
    assert_eq!(
        serde_json::to_string(&position).unwrap(),
        r#"{"point":"Imum Coeli","longitude":12.5,"speed":-0.25}"#
    );
    let aspect = Aspect {
        points: [Point::Sun, Point::MeanNorthNode],
        kind: Kind::SemiSquare,
        orb: 0.5,
        angle: 45.5,
        applying: true,
    };
    let json = serde_json::to_string(&aspect).unwrap();
    assert_eq!(
        json,
        r#"{"points":["Sun","Mean North Node"],"kind":"Semi-square","orb":0.5,"angle":45.5,"applying":true}"#
    );
    assert_eq!(serde_json::from_str::<Aspect>(&json).unwrap(), aspect);
}

/// The JSON shape of what astro-clock caches, reduced to its keys and kinds (each array to the
/// union of its elements'), so new data leaves it alone and a changed field doesn't.
#[cfg(feature = "serde")]
mod json {
    use std::collections::BTreeSet;
    use std::fmt::Write as _;

    use horae::{Builtin, Sky, angles, hours, moon, resh, stations};
    use jiff::SignedDuration;
    use jiff::civil::date;
    use jiff::tz::TimeZone;
    use serde_json::Value;

    /// The shape of `v`, one `path: kind` line per leaf, into `out`.
    fn shape(path: &str, v: &Value, out: &mut BTreeSet<String>) {
        match v {
            Value::Object(m) => {
                for (k, v) in m {
                    shape(&format!("{path}.{k}"), v, out);
                }
            }
            Value::Array(a) if a.is_empty() => {
                out.insert(format!("{path}: []"));
            }
            Value::Array(a) => a.iter().for_each(|v| shape(&format!("{path}[]"), v, out)),
            Value::String(_) => drop(out.insert(format!("{path}: string"))),
            Value::Number(_) => drop(out.insert(format!("{path}: number"))),
            Value::Bool(_) => drop(out.insert(format!("{path}: bool"))),
            Value::Null => drop(out.insert(format!("{path}: null"))),
        }
    }

    #[test]
    fn json_shape() {
        let tz = TimeZone::get("America/Chicago").unwrap();
        let t = date(2026, 10, 15)
            .at(13, 14, 0, 0)
            .to_zoned(tz.clone())
            .unwrap();
        let (lat, lon) = (43.0731, -89.4012);
        let tt = horae::time::tt(t.timestamp());
        let sky = Sky::at(&t, lat, lon).unwrap();
        let parts = [
            ("sky", serde_json::to_value(&sky).unwrap()),
            (
                "angles",
                serde_json::to_value(angles(tt, lat, lon)).unwrap(),
            ),
            (
                "course",
                serde_json::to_value(moon::course(&Builtin, tt, &tz).unwrap()).unwrap(),
            ),
            (
                "station",
                serde_json::to_value(
                    stations::next_station(&Builtin, horae::Planet::Venus, tt, &tz).unwrap(),
                )
                .unwrap(),
            ),
        ];
        // A day without a sunset: Tromsø at midsummer.
        let summer = date(2026, 6, 21)
            .at(12, 0, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap();
        let polar = hours::day(&Builtin, &summer, 69.65, 18.96).unwrap();
        assert!(polar.planetary().is_none(), "{polar:?}");
        let parts = [
            parts.as_slice(),
            &[("polar", serde_json::to_value(&polar).unwrap())],
        ]
        .concat();
        // The Resh times of a day, and a tide.
        let day = hours::day(&Builtin, &t, lat, lon).unwrap();
        let times = resh::times(
            &Builtin,
            &t,
            &(&t + SignedDuration::from_hours(24)),
            lat,
            lon,
        )
        .unwrap();
        assert_eq!(times.len(), 4, "{times:?}");
        let tide = resh::tide_at(&day, t.timestamp()).expect("a tide at 13:14");
        let parts = [
            parts.as_slice(),
            &[
                ("resh", serde_json::to_value(&times).unwrap()),
                ("tide", serde_json::to_value(&tide).unwrap()),
            ],
        ]
        .concat();
        let mut got = BTreeSet::new();
        for (name, v) in &parts {
            shape(name, v, &mut got);
        }
        let mut text = String::new();
        for l in &got {
            writeln!(text, "{l}").unwrap();
        }
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/json-shape.txt");
        if std::env::var_os("HORAE_BLESS").is_some() {
            std::fs::write(path, &text).unwrap();
        }
        // By lines: Git may check it out with CRLFs.
        let want = std::fs::read_to_string(path).expect("tests/data/json-shape.txt");
        assert_eq!(
            text.lines().collect::<Vec<_>>(),
            want.lines().collect::<Vec<_>>(),
            "the JSON shape changed: astro-clock's caches depend on it (HORAE_BLESS=1 rewrites \
             the file, once it's agreed)"
        );
    }
}
