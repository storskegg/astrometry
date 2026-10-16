//! E1's gate (plan 12): time against JPL Horizons and the Swiss Ephemeris.

use astrometry::time::{self, DAY, UTC_FROM};

fn rows(file: &str) -> Vec<Vec<String>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(file);
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

/// Horizons' TDB − UT on the first of each month, 1800–2200, from a clock time: the crate's
/// clock time to TT, plus TDB − TT, must give it.
#[test]
fn clock_time_as_horizons_reads_it() {
    let mut worst = (0.0f64, String::new());
    for r in rows("horizons/tdb-ut.tsv") {
        let (y, m): (i32, u8) = (r[0].parse().unwrap(), r[1].parse().unwrap());
        let want: f64 = r[2].parse().unwrap();
        let jd = time::julian_date(y, m, 1, 0, 0, 0.0);
        let tt = time::tt_from_civil(jd);
        let got = (tt - jd) * DAY + time::tdb_minus_tt(tt);
        let d = (got - want).abs();
        if d > worst.0 {
            worst = (d, format!("{y}-{m:02}: {got:.6} against {want:.6}"));
        }
    }
    // Horizons rounds to the microsecond; f64 Julian dates resolve about 20 µs.
    assert!(worst.0 < 3e-5, "worst: {}", worst.1);
}

/// ΔT against the Swiss Ephemeris', by era: within 0.1 s where both have observations (from
/// Horizons' switch to UTC in 1962 to 2022: the Swiss Ephemeris 2.10.03 extrapolates from
/// there; ours are IERS's monthly values, where the Swiss Ephemeris' coarser table smooths the
/// Earth's seasonal variation, ±30 ms, so the two part by up to 65 ms in May and June), and
/// before 1962 within the difference between Horizons' values (ours) and the Swiss
/// Ephemeris' model (Stephenson, Morrison and Hohenkerk 2016). Beyond the observations they
/// part: the Swiss Ephemeris extrapolates, ours holds, as Horizons does.
#[test]
fn delta_t_against_swiss_ephemeris() {
    let mut worst = [0.0f64; 3];
    for r in rows("swisseph/positions.tsv")
        .iter()
        .filter(|r| r[0] == "deltat")
    {
        let (y, m): (i32, u8) = (r[1].parse().unwrap(), r[2].parse().unwrap());
        let want: f64 = r[3].parse().unwrap();
        let jd = time::julian_date(y, m, 1, 0, 0, 0.0);
        let d = (time::delta_t(jd) - want).abs();
        let era = if jd < UTC_FROM {
            0
        } else if y < 2023 {
            1
        } else {
            2
        };
        worst[era] = worst[era].max(d);
    }
    println!(
        "ΔT against the Swiss Ephemeris: before 1962 {:.3} s, 1962–2022 {:.3} s, after {:.1} s",
        worst[0], worst[1], worst[2]
    );
    assert!(worst[1] < 0.1, "1962–2022: {:.3} s", worst[1]);
    assert!(worst[0] < 1.5, "before 1962: {:.3} s", worst[0]);
}
