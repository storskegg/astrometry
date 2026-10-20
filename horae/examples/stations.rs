//! The planets' stations in a year: `cargo run --example stations -- 2026`.

use horae::position::degrees_minutes_seconds;
use horae::stations::stations;
use horae::{Builtin, Planet, Sign, time};
use jiff::civil::date;
use jiff::tz::TimeZone;

fn main() {
    let year: i16 = std::env::args()
        .nth(1)
        .map_or(2026, |y| y.parse().expect("a year"));
    let tt = |y| time::tt(date(y, 1, 1).to_zoned(TimeZone::UTC).unwrap().timestamp());
    for p in Planet::ALL {
        for s in stations(&Builtin, p, tt(year), tt(year + 1), &TimeZone::UTC).unwrap() {
            let (d, m, _) = degrees_minutes_seconds(s.longitude);
            println!(
                "{:<8} {:<10} {}  {d}°{m:02}′ {}",
                p.name(),
                s.turns,
                s.at.strftime("%Y-%m-%d %H:%M UT"),
                Sign::of(s.longitude)
            );
        }
    }
}
