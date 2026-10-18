//! Reference data for the crate: fetched once from the sources and
//! checked in, so tests run offline and the numbers can't drift.
//!
//! The crate's data:
//!
//! - `data/horizons-tdb-ut1.tsv`: JPL Horizons' TDB − UT1 (its observer
//!   quantity 30), monthly from 1800 to its switch from UT1 to UTC on 1962-01-19 12:00.
//! - `data/iers-ut1-utc.tsv`: IERS's UT1 − UTC (EOP 20 C04), monthly from
//!   1962.
//! - `data/usno-tt-ut1.tsv`: USNO's monthly TT − UT1, after the IERS series
//!   ends.
//! - `data/leap-seconds.list`: the IANA leap second list.
//! - `data/iers-tab5.3a.txt` and `iers-tab5.3b.txt`: the IAU 2000A_R06
//!   nutation series, from the IERS Conventions (2010), chapter 5.
//! - `data/vsop87/VSOP87E.*`: VSOP87E (Bretagnon and Francou, 1988), the
//!   barycentric rectangular series of the Sun and planets, from IMCCE.
//! - `data/elpmpp02/ELP_*.S*`: ELP/MPP02 (Chapront and Francou, 2003), the
//!   authors' series files as distributed by the Paris Observatory, from a mirror (their FTP
//!   server doesn't answer over HTTP), pinned to a commit.
//!
//! Test data:
//!
//! - `tests/data/horizons/tdb-ut.tsv`: Horizons' TDB − UT, monthly from 1800
//!   to 2200: UT1 before the switch, UTC after.
//! - `tests/data/horizons/positions.tsv`: Horizons' apparent positions of
//!   the Sun, Moon and planets at fixed TT instants across 1800–2200.
//! - `tests/data/swisseph/positions.tsv`: the Swiss Ephemeris' positions at
//!   the same instants, in its JPL (DE440) and own modes, and its ΔT. Made in a throwaway
//!   Python environment under `target/`: the Swiss Ephemeris is AGPL-3.0, so only its numbers
//!   come here, never its code.

use std::fmt::Write as _;
use std::fs;
use std::process::Command;

use crate::{Result, root};

const HORIZONS: &str = "https://ssd.jpl.nasa.gov/api/horizons.api";

/// The bodies, by Horizons ID.
const BODIES: [(&str, &str); 10] = [
    ("Sun", "10"),
    ("Moon", "301"),
    ("Mercury", "199"),
    ("Venus", "299"),
    ("Mars", "499"),
    ("Jupiter", "599"),
    ("Saturn", "699"),
    // The planets' system barycentres, which DE440 gives (and the Swiss Ephemeris uses).
    ("MarsBarycenter", "4"),
    ("JupiterBarycenter", "5"),
    ("SaturnBarycenter", "6"),
];

/// The fixtures, by name.
/// A fixture: what makes it.
type Part = fn() -> Result;

const PARTS: [(&str, Part); 9] = [
    ("deltat", deltat),
    ("usno", usno),
    ("leap-seconds", leap_seconds),
    ("nutation", nutation),
    ("theories", theories),
    ("positions", positions),
    ("sidereal", sidereal),
    ("rise-set", rise_set),
    ("swisseph", swisseph),
];

/// `cargo xtask fixtures [PART...]`: fetches the named fixtures, or every one.
pub fn run(names: &[&str]) -> Result {
    for name in names {
        if !PARTS.iter().any(|(n, _)| n == name) {
            let all: Vec<&str> = PARTS.iter().map(|(n, _)| *n).collect();
            return Err(format!("no fixture {name:?}; there are {}", all.join(", ")));
        }
    }
    for (name, part) in PARTS {
        if names.is_empty() || names.contains(&name) {
            part()?;
        }
    }
    Ok(())
}

/// The Julian date of Horizons' switch from UT1 to UTC: 1962-01-19 12:00.
const UTC_FROM: f64 = 2437684.0;

/// The Julian date at 0h of a Gregorian calendar date.
fn jd(y: i32, m: i32, d: i32) -> f64 {
    let (y, m) = if m <= 2 { (y - 1, m + 12) } else { (y, m) };
    let a = y.div_euclid(100);
    let b = 2 - a + a.div_euclid(4);
    (365.25 * f64::from(y + 4716)).floor() + (30.6001 * f64::from(m + 1)).floor() + f64::from(d + b)
        - 1524.5
}

/// The Swiss Ephemeris fixtures, by `fixtures/swisseph_fixtures.py` in a virtual environment
/// under `target/`, with DE440 and the Swiss Ephemeris' own files downloaded there too.
fn swisseph() -> Result {
    let work = root().join("target/swisseph");
    let ephe = work.join("ephe");
    fs::create_dir_all(&ephe).map_err(|e| e.to_string())?;
    let download = |url: &str, to: &std::path::Path| -> Result {
        if to.exists() {
            return Ok(());
        }
        eprintln!("fixtures: downloading {url}");
        let status = Command::new("curl")
            .args(["-sS", "--fail", "-L", "-o"])
            .arg(to)
            .arg(url)
            .status()
            .map_err(|e| format!("curl: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{url}: {status}"))
        }
    };
    // The Swiss Ephemeris looks for the JPL file by name in its data directory.
    let jpl = ephe.join("de440.eph");
    download(
        "https://ssd.jpl.nasa.gov/ftp/eph/planets/Linux/de440/linux_p1550p2650.440",
        &jpl,
    )?;
    for f in ["sepl_18.se1", "semo_18.se1", "seas_18.se1"] {
        let url = format!("https://raw.githubusercontent.com/aloistr/swisseph/master/ephe/{f}");
        download(&url, &ephe.join(f))?;
    }
    let venv = work.join("venv");
    let python = venv.join("bin/python");
    if !python.exists() {
        run_command(Command::new("python3").args(["-m", "venv"]).arg(&venv))?;
        run_command(Command::new(&python).args([
            "-m",
            "pip",
            "install",
            "-q",
            "pyswisseph==2.10.3.2",
        ]))?;
    }
    let instants_file = work.join("instants.txt");
    let list: Vec<String> = instants().iter().map(|jd| format!("{jd:.9}")).collect();
    fs::write(&instants_file, list.join("\n") + "\n").map_err(|e| e.to_string())?;
    let out = root().join("tests/data/swisseph/positions.tsv");
    fs::create_dir_all(out.parent().unwrap()).map_err(|e| e.to_string())?;
    let script = root().join("xtask/src/fixtures/swisseph_fixtures.py");
    run_command(
        Command::new(&python)
            .args(["-I", "-B"])
            .arg(script)
            .arg(&jpl)
            .arg(&ephe)
            .arg(&instants_file)
            .arg(&out),
    )?;
    println!("wrote {}", out.strip_prefix(root()).unwrap().display());
    Ok(())
}

fn run_command(cmd: &mut Command) -> Result {
    let status = cmd.status().map_err(|e| format!("{cmd:?}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{cmd:?}: {status}"))
    }
}

fn fetch(url: &str) -> Result<String> {
    let out = Command::new("curl")
        .args(["-sS", "--fail", "-m", "120", "-L", url])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(format!("{url}: {}", String::from_utf8_lossy(&out.stderr)));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("{url}: {e}"))
}

/// A Horizons observer table for `body` from the geocentre, as CSV rows between `$$SOE` and
/// `$$EOE`.
fn horizons(body: &str, params: &[(&str, &str)]) -> Result<Vec<Vec<String>>> {
    horizons_at(body, "500@399", params)
}

/// A Horizons observer table for `body` from `center`.
fn horizons_at(body: &str, center: &str, params: &[(&str, &str)]) -> Result<Vec<Vec<String>>> {
    let mut url = format!(
        "{HORIZONS}?format=text&COMMAND='{body}'&OBJ_DATA='NO'&MAKE_EPHEM='YES'&EPHEM_TYPE='OBSERVER'\
         &CENTER='{center}'&CSV_FORMAT='YES'&ANG_FORMAT='DEG'&EXTRA_PREC='YES'"
    );
    for (k, v) in params {
        write!(url, "&{k}='{v}'").unwrap();
    }
    let text = fetch(&url.replace(' ', "%20"))?;
    let start = text
        .find("$$SOE")
        .ok_or_else(|| format!("Horizons: no ephemeris:\n{}", &text[..text.len().min(2000)]))?;
    let end = text
        .find("$$EOE")
        .ok_or("Horizons: unterminated ephemeris")?;
    Ok(text[start + 5..end]
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(|f| f.trim().to_string()).collect())
        .collect())
}

/// The Earth's rotation from 1962: IERS's UT1 − UTC (EOP 20 C04, daily at 0h UTC) on the
/// first of each month while it lasts, then USNO's monthly TT − UT1, which runs a little
/// later.
fn usno() -> Result {
    let c04 =
        fetch("https://datacenter.iers.org/data/latestVersion/EOP_20_C04_one_file_1962-now.txt")?;
    let mut iers = String::from(
        "# IERS EOP 20 C04 UT1 - UTC, seconds, on the first of each month at 0h UTC. Fetched by\n\
         # `cargo xtask fixtures`; don't edit.\n# jd_utc\tUT1-UTC\n",
    );
    let mut last = 0.0;
    for line in c04.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 8 || f[2] != "1" {
            continue;
        }
        let (y, m): (i32, i32) = (
            f[0].parse().map_err(|_| "bad year")?,
            f[1].parse().map_err(|_| "bad month")?,
        );
        last = jd(y, m, 1);
        writeln!(iers, "{last:.1}\t{}", f[7]).unwrap();
    }
    write_file("data/iers-ut1-utc.tsv", &iers)?;

    let monthly = fetch("https://maia.usno.navy.mil/ser7/deltat.data")?;
    let mut out = String::from(
        "# USNO's observed TT - UT1, seconds, monthly (deltat.data), after the IERS series ends.\n\
         # Fetched by `cargo xtask fixtures`; don't edit.\n# jd_ut1\tTT-UT1\n",
    );
    for line in monthly.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if let [y, m, d, dt] = f[..] {
            let day = jd(
                y.parse().map_err(|_| "bad year")?,
                m.parse().map_err(|_| "bad month")?,
                d.parse().map_err(|_| "bad day")?,
            );
            if day > last {
                writeln!(out, "{day:.1}\t{dt}").unwrap();
            }
        }
    }
    write_file("data/usno-tt-ut1.tsv", &out)
}

/// Horizons' TDB − UT (quantity 30) on the first of each month from 1800 to 2200: as a
/// fixture, and before its switch to UTC, as the crate's ΔT.
fn deltat() -> Result {
    let mut out = String::from(
        "# JPL Horizons' TDB - UT (observer quantity 30), seconds, on the first of each month at\n\
         # 0h UT: UT1 before 1962-01-19 12:00, UTC after. Fetched by `cargo xtask fixtures`;\n\
         # don't edit.\n# year\tmonth\tTDB-UT\n",
    );
    for year in (1800..2200).step_by(25) {
        let rows = horizons(
            "10",
            &[
                ("START_TIME", &format!("{year}-01-01")),
                ("STOP_TIME", &format!("{}-01-01", year + 25)),
                ("STEP_SIZE", "1 MONTHS"),
                ("QUANTITIES", "30"),
            ],
        )?;
        for r in rows {
            // "1800-Jan-01 00:00", "", "", "18.622435"
            let date = &r[0];
            let (y, rest) = date
                .split_once('-')
                .ok_or_else(|| format!("bad date {date}"))?;
            let mon = &rest[..3];
            let m = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ]
            .iter()
            .position(|x| *x == mon)
            .ok_or_else(|| format!("bad month {mon}"))?
                + 1;
            let dt = r.iter().rev().find(|f| !f.is_empty()).ok_or("no ΔT")?;
            writeln!(out, "{y}\t{m}\t{dt}").unwrap();
        }
    }
    // The last row of each 25-year request repeats as the first of the next.
    let mut seen = std::collections::HashSet::new();
    let rows: Vec<&str> = out
        .lines()
        .filter(|l| l.starts_with('#') || seen.insert(*l))
        .collect();
    write_file("tests/data/horizons/tdb-ut.tsv", &(rows.join("\n") + "\n"))?;

    let mut data = String::from(
        "# JPL Horizons' TDB - UT1 (observer quantity 30), seconds, on the first of each month at\n\
         # 0h UT1, to its switch from UT1 to UTC on 1962-01-19 12:00. Fetched by\n\
         # `cargo xtask fixtures`; don't edit.\n# jd_ut1\tTDB-UT1\n",
    );
    for row in rows.iter().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = row.split('\t').collect();
        let year = f[0].parse().map_err(|_| "bad year")?;
        let day = jd(year, f[1].parse().map_err(|_| "bad month")?, 1);
        if day < UTC_FROM {
            writeln!(data, "{day:.1}\t{}", f[2]).unwrap();
        }
    }
    write_file("data/horizons-tdb-ut1.tsv", &data)
}

/// The mirror of the ELP/MPP02 distribution, and the commit the files are taken from.
const ELP_MIRROR: &str = "https://raw.githubusercontent.com/THRASTRO/ephem.js/2f679a1f698b4f20f9579d94da24af51559458ed/src/elpmpp02/data";

/// The analytical theories' series, as their authors distribute them.
fn theories() -> Result {
    for body in ["sun", "mer", "ven", "ear", "mar", "jup", "sat"] {
        let url = format!("https://ftp.imcce.fr/pub/ephem/planets/vsop87/VSOP87E.{body}");
        let text = retry(&url)?;
        write_file(&format!("data/vsop87/VSOP87E.{body}"), &text)?;
    }
    for f in [
        "ELP_MAIN.S1",
        "ELP_MAIN.S2",
        "ELP_MAIN.S3",
        "ELP_PERT.S1",
        "ELP_PERT.S2",
        "ELP_PERT.S3",
    ] {
        let text = retry(&format!("{ELP_MIRROR}/{f}"))?;
        write_file(&format!("data/elpmpp02/{f}"), &text)?;
    }
    Ok(())
}

/// `fetch`, tried a few times: some of the sources' servers are flaky.
fn retry(url: &str) -> Result<String> {
    let mut last = String::new();
    for _ in 0..6 {
        match fetch(url) {
            Ok(text) => return Ok(text),
            Err(e) => last = e,
        }
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    Err(last)
}

/// The IERS Conventions' nutation tables. Their server is flaky, so each is tried a few times.
fn nutation() -> Result {
    for t in ["5.3a", "5.3b"] {
        let url = format!(
            "https://iers-conventions.obspm.fr/content/chapter5/additional_info/tab{t}.txt"
        );
        let mut last = String::new();
        let text = (0..6).find_map(|_| match fetch(&url) {
            Ok(text) => Some(text),
            Err(e) => {
                last = e;
                std::thread::sleep(std::time::Duration::from_secs(3));
                None
            }
        });
        let text = text.ok_or(last)?;
        write_file(&format!("data/iers-tab{t}.txt"), &text)?;
    }
    Ok(())
}

fn leap_seconds() -> Result {
    let text = fetch("https://data.iana.org/time-zones/tzdb/leap-seconds.list")?;
    write_file("data/leap-seconds.list", &text)
}

/// The fixed instants: 160 TT Julian dates spread over 1800–2200 by a fixed pseudo-random
/// sequence, and the Go version's test moment.
fn instants() -> Vec<f64> {
    const START: f64 = 2378496.5; // 1800-01-01
    const SPAN: f64 = 146097.0; // 400 years
    let mut x: u64 = 0x9E3779B97F4A7C15;
    let mut out = Vec::new();
    for _ in 0..160 {
        // splitmix64
        x = x.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^= z >> 31;
        let u = (z >> 11) as f64 / (1u64 << 53) as f64;
        // Round to the second, so the instant is exact in the file.
        let jd = START + (u * SPAN * 86400.0).round() / 86400.0;
        out.push(jd);
    }
    out.push(2461329.259722222 + 69.18 / 86400.0); // 2026-10-15 18:14 UT, about, in TT
    out
}

fn positions() -> Result {
    let jds = instants();
    let mut out = String::from(
        "# JPL Horizons' apparent geocentric positions at TT instants: ecliptic longitude and\n\
         # latitude of date (quantity 31), right ascension and declination (quantity 2),\n\
         # degrees, and range (quantity 20), au. Fetched by `cargo xtask fixtures`; don't edit.\n\
         # body\tjd_tt\tlon\tlat\tra\tdec\tdelta\n",
    );
    for (name, id) in BODIES {
        for chunk in jds.chunks(40) {
            let list: Vec<String> = chunk.iter().map(|jd| format!("{jd:.9}")).collect();
            let rows = horizons(
                id,
                &[
                    ("TLIST", &list.join("','")),
                    ("TLIST_TYPE", "JD"),
                    ("TIME_TYPE", "TT"),
                    ("TIME_DIGITS", "FRACSEC"),
                    ("QUANTITIES", "2,20,31"),
                    ("CAL_FORMAT", "JD"),
                ],
            )?;
            if rows.len() != chunk.len() {
                return Err(format!(
                    "{name}: {} rows for {} instants",
                    rows.len(),
                    chunk.len()
                ));
            }
            // Horizons returns the rows in time order, not the list's: each row's own Julian
            // date says which instant it is.
            for r in rows {
                // JD, (solar and lunar presence flags), RA, Dec, delta, deldot, lon, lat
                let jd: f64 = r[0]
                    .parse()
                    .map_err(|_| format!("{name}: bad date {:?}", r[0]))?;
                let jd = list
                    .iter()
                    .find(|l| (l.parse::<f64>().unwrap() - jd).abs() < 1e-7)
                    .ok_or_else(|| format!("{name}: row for {jd}, which wasn't asked for"))?;
                let nums: Vec<&String> = r[1..]
                    .iter()
                    .filter(|f| !f.is_empty() && f.parse::<f64>().is_ok())
                    .collect();
                let [ra, dec, delta, _deldot, lon, lat] = nums[..] else {
                    return Err(format!("{name}: unexpected row {r:?}"));
                };
                writeln!(out, "{name}\t{jd}\t{lon}\t{lat}\t{ra}\t{dec}\t{delta}").unwrap();
            }
        }
        eprintln!("fixtures: {name}");
    }
    write_file("tests/data/horizons/positions.tsv", &out)
}

/// Horizons' local apparent sidereal time (quantity 7) on the Greenwich meridian, at the TT
/// instants: the Earth's rotation, UT1 included.
fn sidereal() -> Result {
    let mut out = String::from(
        "# JPL Horizons' local apparent sidereal time (quantity 7) at 0° E, 0° N, hours, at TT\n\
         # instants. Fetched by `cargo xtask fixtures`; don't edit.\n# jd_tt\tLAST\n",
    );
    let jds = instants();
    for chunk in jds.chunks(40) {
        let list: Vec<String> = chunk.iter().map(|jd| format!("{jd:.9}")).collect();
        let rows = horizons_at(
            "10",
            "coord@399",
            &[
                ("COORD_TYPE", "GEODETIC"),
                ("SITE_COORD", "0,0,0"),
                ("TLIST", &list.join("','")),
                ("TLIST_TYPE", "JD"),
                ("TIME_TYPE", "TT"),
                ("QUANTITIES", "7"),
                ("CAL_FORMAT", "JD"),
            ],
        )?;
        for r in rows {
            let jd: f64 = r[0].parse().map_err(|_| format!("bad date {:?}", r[0]))?;
            let at = list
                .iter()
                .find(|l| (l.parse::<f64>().unwrap() - jd).abs() < 1e-7)
                .ok_or("a row that wasn't asked for")?;
            let last = r
                .iter()
                .rev()
                .find(|f| !f.is_empty())
                .ok_or("no sidereal time")?;
            writeln!(out, "{at}\t{last}").unwrap();
        }
    }
    write_file("tests/data/horizons/sidereal.tsv", &out)
}

/// Horizons' sunrises and sunsets on the true visual horizon (R_T_S_ONLY='TVH'), at sites and
/// dates the tests use, polar days and nights included.
fn rise_set() -> Result {
    let sites = [
        ("Madison", "-89.4012,43.0731,0", "2026-10-14", "2026-10-17"),
        (
            "Springfield",
            "-89.64371,39.80172,0",
            "2026-10-14",
            "2026-10-17",
        ),
        (
            "Honolulu",
            "-157.85833,21.30694,0",
            "2026-07-01",
            "2026-07-04",
        ),
        (
            "Utqiagvik",
            "-156.7886,71.2906,0",
            "2026-05-08",
            "2026-05-14",
        ),
        (
            "Utqiagvik",
            "-156.7886,71.2906,0",
            "2026-11-14",
            "2026-11-20",
        ),
        ("Madison", "-89.4012,43.0731,0", "1850-03-19", "1850-03-22"),
        ("Madison", "-89.4012,43.0731,0", "2150-12-20", "2150-12-23"),
    ];
    let mut out = String::from(
        "# JPL Horizons' sunrises and sunsets (true visual horizon, refracted upper limb), UT,\n\
         # one-minute steps. Fetched by `cargo xtask fixtures`; don't edit.\n# site\tlon,lat\tevent\tUT\n",
    );
    for (name, coord, start, stop) in sites {
        let rows = horizons_at(
            "10",
            "coord@399",
            &[
                ("COORD_TYPE", "GEODETIC"),
                ("SITE_COORD", coord),
                ("START_TIME", start),
                ("STOP_TIME", stop),
                ("STEP_SIZE", "1 m"),
                ("R_T_S_ONLY", "TVH"),
                ("QUANTITIES", "4"),
                ("TIME_DIGITS", "SECONDS"),
            ],
        )?;
        for r in rows {
            // "2026-Oct-15 12:12:31", flags ("r" rise, "s" set, "t" transit), ...
            let event = match r.get(2).map(|f| f.trim()) {
                Some("r") => "rise",
                Some("s") => "set",
                _ => continue,
            };
            writeln!(
                out,
                "{name}\t{}\t{event}\t{}",
                coord.rsplit_once(',').unwrap().0,
                r[0]
            )
            .unwrap();
        }
    }
    write_file("tests/data/horizons/rise-set.tsv", &out)
}

fn write_file(rel: &str, text: &str) -> Result {
    let path = root().join(rel);
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| format!("{rel}: {e}"))?;
    fs::write(&path, text).map_err(|e| format!("{rel}: {e}"))?;
    println!("wrote {rel} ({} lines)", text.lines().count());
    Ok(())
}
