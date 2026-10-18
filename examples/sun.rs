//! The Sun's apparent position now-ish: `cargo run --example sun`.

use astrometry::apparent::apparent;
use astrometry::source::{Body, Builtin};
use astrometry::time;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 18 October 2026, 17:00 UTC, as a civil Julian date, then TT.
    let tt = time::tt_from_civil(time::julian_date(2026, 10, 18, 17, 0, 0.0));
    for body in [Body::Sun, Body::Moon, Body::Mars] {
        let p = apparent(&Builtin, body, tt)?;
        println!(
            "{body:?}: longitude {:.5}°, latitude {:.5}°, {:.6} au",
            p.longitude, p.latitude, p.distance
        );
    }
    Ok(())
}
