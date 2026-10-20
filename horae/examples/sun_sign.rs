fn main() -> Result<(), Box<dyn std::error::Error>> {
    use astrometry::source::Builtin;
    use horae::{Point, Position, time};

    let now: jiff::Zoned = "2026-10-20T08:00[America/Chicago]".parse()?;
    let sun = Position::at(&Builtin, Point::Sun, time::tt(now.timestamp()))?;
    println!("the Sun in {}", sun.sign());
    Ok(())
}
