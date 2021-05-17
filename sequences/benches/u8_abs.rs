// Some benchmark functionsd are deprecated but still usable
#![allow(deprecated)]

use criterion::{black_box, criterion_group, criterion_main, Criterion, ParameterizedBenchmark};

#[allow(clippy::cast_lossless)]
fn abs_i8(a: u8, b: u8) -> usize {
    (a as i8 - b as i8).abs() as usize
}

#[allow(clippy::cast_lossless)]
fn abs_i16(a: u8, b: u8) -> usize {
    (a as i16 - b as i16).abs() as usize
}

#[allow(clippy::cast_lossless)]
fn abs_i32(a: u8, b: u8) -> usize {
    (a as i32 - b as i32).abs() as usize
}

#[allow(clippy::cast_lossless)]
fn abs_i64(a: u8, b: u8) -> usize {
    (a as i64 - b as i64).abs() as usize
}

#[allow(clippy::cast_lossless)]
fn abs_isize(a: u8, b: u8) -> usize {
    (a as isize - b as isize).abs() as usize
}

fn abs_min_max(a: u8, b: u8) -> usize {
    (a.max(b) - a.min(b)) as usize
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench(
        "u8_abs",
        ParameterizedBenchmark::new(
            "abs_isize",
            |b, i| b.iter(|| abs_isize(*i, black_box(12))),
            vec![0, 6, 20, 64, 127],
        )
        .with_function("abs_i8", |b, i| b.iter(|| abs_i8(*i, black_box(12))))
        .with_function("abs_i16", |b, i| b.iter(|| abs_i16(*i, black_box(12))))
        .with_function("abs_i32", |b, i| b.iter(|| abs_i32(*i, black_box(12))))
        .with_function("abs_i64", |b, i| b.iter(|| abs_i64(*i, black_box(12))))
        .with_function("abs_min_max", |b, i| {
            b.iter(|| abs_min_max(*i, black_box(12)))
        }),
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
