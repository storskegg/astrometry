//! Timings of the heavier calls, at a fixed instant.

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use horae::{Builtin, Position, hours, moon, resh, time};
use jiff::civil::date;
use jiff::{SignedDuration, Zoned};

const MADISON: (f64, f64) = (43.07, -89.40);
const TROMSO: (f64, f64) = (69.65, 18.96);

fn instant() -> Zoned {
    date(2026, 11, 2)
        .at(15, 0, 0, 0)
        .in_tz("America/Chicago")
        .unwrap()
}

fn benches(c: &mut Criterion) {
    let t = instant();
    let tt = time::tt(t.timestamp());

    let mut group = c.benchmark_group("hours_day");
    group
        .sample_size(10)
        .measurement_time(Duration::from_secs(5));
    for (name, (lat, lon)) in [("madison", MADISON), ("tromso", TROMSO)] {
        group.bench_function(name, |b| {
            b.iter(|| hours::day(&Builtin, black_box(&t), lat, lon).unwrap())
        });
    }
    group.finish();

    let mut group = c.benchmark_group("resh_times");
    group
        .sample_size(10)
        .measurement_time(Duration::from_secs(5));
    let to = t.checked_add(SignedDuration::from_hours(8 * 24)).unwrap();
    group.bench_function("8_days", |b| {
        b.iter(|| resh::times(&Builtin, black_box(&t), &to, MADISON.0, MADISON.1).unwrap())
    });
    group.finish();

    c.bench_function("resh_next_time", |b| {
        b.iter(|| resh::next_time(&Builtin, black_box(&t), MADISON.0, MADISON.1).unwrap())
    });
    c.bench_function("position_all", |b| {
        b.iter(|| Position::all(&Builtin, black_box(tt)).unwrap())
    });
    c.bench_function("moon_course", |b| {
        b.iter(|| moon::course(&Builtin, black_box(tt), t.time_zone()).unwrap())
    });
}

criterion_group! {
    name = horae_benches;
    config = Criterion::default().sample_size(10).measurement_time(Duration::from_secs(5));
    targets = benches
}
criterion_main!(horae_benches);
