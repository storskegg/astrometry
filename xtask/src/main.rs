//! `cargo xtask`: the repository's tasks.

mod de440;
mod fixtures;

use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask TASK

tasks:
  fixtures [PART...]
                 fetch the crate's reference data (JPL Horizons, IERS, USNO, IANA, IMCCE,
                 the Swiss Ephemeris' numbers), or the named parts
  de440          cut the crate's built-in slice of DE440 from de440.bsp (ASTROMETRY_DE440,
                 or target/jpl/de440.bsp)
";

type Result<T = ()> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["fixtures", parts @ ..] => fixtures::run(parts),
        ["de440"] => de440::run(),
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
