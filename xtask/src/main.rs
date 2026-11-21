//! `cargo xtask`: the repository's tasks.

mod check;
mod de440;
mod fixtures;
mod reference;
mod version;

use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask TASK

tasks:
  fixtures [PART...]
                 fetch the crate's reference data (JPL Horizons, IERS, USNO, IANA, IMCCE,
                 the Swiss Ephemeris' numbers), or the named parts. The part `de440`
                 downloads de440.bsp (120 MB) to local/jpl/, rebuilds the built-in slice and
                 writes the reference fixture from it, runs every test against the full
                 file, and deletes it. The Swiss Ephemeris parts delete target/swisseph when
                 they're done. The part `stars` makes tests/data/swisseph/stars.tsv: the
                 Swiss Ephemeris' Sirius, from data/stars.tsv, apparent positions, risings
                 and settings, and heliacal risings
  de440          cut the crate's built-in slice of DE440 from a de440.bsp you have
                 (ASTROMETRY_DE440, or local/jpl/de440.bsp)
  outer          cut the outer planets' file (Uranus, Neptune, Pluto; feature `outer-planets`)
                 from the same de440.bsp
  check          the CI matrix, locally: fmt, clippy, the tests, wasm32, the MSRV (1.99) and
                 the iOS builds. Builds go through this task: it runs `cargo clean` before and
                 after, so every build starts from nothing. It downloads nothing: the tests
                 use the reference fixture, and the ones that need the full de440.bsp skip
  version [DATE] [--write]
                 print the next calendar version YYYY.WW.PATCH for DATE (YYYY-MM-DD, default
                 today in UTC); --write sets it in the manifests, README and Cargo.lock.
                 It counts the week's local tags: `git fetch --tags` first
";

type Result<T = ()> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["fixtures", parts @ ..] => fixtures::run(parts),
        ["de440"] => de440::run(),
        ["outer"] => de440::outer(),
        ["check", rest @ ..] => check::run(rest),
        ["version", rest @ ..] => version::run(rest),
        ["help" | "--help" | "-h"] => {
            print!("{USAGE}");
            Ok(())
        }
        _ => {
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The repository's root.
fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is inside the repository")
}
